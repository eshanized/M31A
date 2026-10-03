//! Interactive CLI Session Runner and Terminal REPL Loop (PRD §01, CLI-01, CLI-04).
//!
//! Provides the primary interactive developer experience:
//! - Natural language task input
//! - Structured multiline input (line continuation with `\`, block quotes with `"""`, paste)
//! - Centralized slash command dispatch (/help, /status, /diff, /commit, etc.)
//! - Real @file and @directory mention resolution and context injection
//! - Follow-up conversation turns maintaining durable session history
//! - Interactive approval prompt when policy triggers ASK
//! - Graceful Ctrl-C cancellation preserving session integrity
//! - Deterministic session persistence and resumption

use std::io::{IsTerminal, Write, stdout};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_util::sync::CancellationToken;

use crate::agent::{AgentEngine, AgentEngineState, AgentTurnOutcome};
use crate::error::M31AError;
use crate::ids::SessionId;
use crate::interaction::action::ApplicationAction;
use crate::interaction::commands::{CommandContext, CommandOutput, SlashCommandRegistry};
use crate::interaction::mentions::MentionParser;
use crate::interaction::parser::InteractionParser;
use crate::interaction::session::{
    ConversationTurn, Session, SessionState, SqliteSessionRepository,
};
use crate::interaction::state::SessionPromptState;
use crate::runtime::AppRuntime;

/// Interactive CLI session runner.
pub struct InteractiveSessionRunner {
    runtime: Arc<AppRuntime>,
    session_repo: SqliteSessionRepository,
    command_registry: SlashCommandRegistry,
    parser: InteractionParser,
    workspace_root: PathBuf,
    current_session: Option<Session>,
    prompt_state: SessionPromptState,
    cancel_token: CancellationToken,
    active_engine: Option<AgentEngine>,
}

/// Helper to sanitize console error output before presenting to operator (Finding B).
fn print_console_error(msg: impl std::fmt::Display) {
    let sanitized = crate::telemetry::SecretRedactor::new().sanitize_error(&msg.to_string());
    eprintln!("{sanitized}");
}

impl InteractiveSessionRunner {
    /// Construct a new session runner attached to the production runtime.
    pub fn new(runtime: Arc<AppRuntime>) -> Self {
        let ws = runtime.workspace_root().to_path_buf();
        let pool = runtime.pool().clone();
        let session_repo = SqliteSessionRepository::new(pool);
        let command_registry = SlashCommandRegistry::new_standard();
        let parser = InteractionParser::new(SlashCommandRegistry::new_standard());

        Self {
            runtime,
            session_repo,
            command_registry,
            parser,
            workspace_root: ws,
            current_session: None,
            prompt_state: SessionPromptState::Idle,
            cancel_token: CancellationToken::new(),
            active_engine: None,
        }
    }

    /// Access the active session if initialized.
    pub fn current_session(&self) -> Option<&Session> {
        self.current_session.as_ref()
    }

    /// Access the session repository.
    pub fn session_repo(&self) -> &SqliteSessionRepository {
        &self.session_repo
    }

    /// Access active session ID if any.
    pub fn session_id(&self) -> Option<SessionId> {
        self.current_session.as_ref().map(|s| s.id)
    }

    /// Access workspace root directory.
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Whether a derived `AgentEngine` is currently cached.
    pub fn has_active_engine(&self) -> bool {
        self.active_engine.is_some()
    }

    /// Invalidate the cached derived engine.
    ///
    /// REQUIRED after every runtime authority change (model / profile /
    /// configuration swap): the cached engine holds the previous generation's
    /// model caller, policy gate, registries, approval coordinator, context
    /// compiler, and autonomy. Retaining it would execute subsequent turns
    /// under stale authorities. The next turn rebuilds from the current
    /// runtime via `create_agent_engine`.
    pub fn invalidate_engine(&mut self) {
        self.active_engine = None;
    }

    /// Replace the bound runtime, invalidating the derived engine atomically.
    ///
    /// All runtime-swap paths MUST funnel through here so the engine
    /// invalidation cannot be forgotten (Invariant 5).
    pub fn rebind_runtime(&mut self, runtime: Arc<AppRuntime>) {
        self.runtime = runtime;
        self.workspace_root = self.runtime.workspace_root().to_path_buf();
        self.session_repo = SqliteSessionRepository::new(self.runtime.pool().clone());
        self.invalidate_engine();
    }

    /// Runtime-truth model/provider/profile triple for display contexts.
    ///
    /// Reads the ACTIVE runtime configuration — never ambient environment
    /// probing. The provider is reported only when its authoritative status
    /// is `Available`; otherwise `none` (fail-closed display).
    fn runtime_model_status(&self) -> (String, String, String) {
        let cfg = self.runtime.config();
        let provider = if self.runtime.active_provider_status()
            == crate::model::types::ProviderCapabilityStatus::Available
        {
            cfg.active_provider.clone()
        } else {
            "none".to_string()
        };
        (
            cfg.active_model.clone(),
            provider,
            cfg.active_profile.clone().unwrap_or_else(|| "default".to_string()),
        )
    }

    /// Initialize a new durable session or attach to existing.
    pub async fn init_session(
        &mut self,
        resume_id: Option<SessionId>,
    ) -> Result<Session, M31AError> {
        if let Some(id) = resume_id {
            if let Some(sess) = self.session_repo.get_session(id).await? {
                println!("Resumed session: {}", sess.id);
                self.current_session = Some(sess.clone());
                return Ok(sess);
            } else {
                eprintln!("Session '{id}' not found. Starting a new session.");
            }
        }

        let sess = self
            .session_repo
            .create_session(&self.workspace_root)
            .await?;
        self.current_session = Some(sess.clone());
        Ok(sess)
    }

    /// Display initial banner.
    ///
    /// Reports the ACTIVE runtime truth (configured model / provider /
    /// profile), never ambient environment probing.
    pub fn print_banner(&self) {
        let (model, provider, profile) = self.runtime_model_status();
        println!("\nM31A {}", env!("CARGO_PKG_VERSION"));
        println!("workspace: {}", self.workspace_root.display());
        println!("model:     {model} (provider: {provider}, profile: {profile})");
        println!("Type /help for available commands or enter a task to begin.\n");
    }

    /// Process a single input string as an interactive session turn.
    /// Returns Ok(true) if session termination was requested, Ok(false) otherwise.
    pub async fn handle_input(&mut self, input_text: &str) -> Result<bool, M31AError> {
        let trimmed = input_text.trim();
        if trimmed.is_empty() {
            return Ok(false);
        }

        if self.current_session.is_none() {
            self.init_session(None).await?;
        }

        let has_active_mission = self
            .current_session
            .as_ref()
            .and_then(|s| s.active_mission_id)
            .is_some();

        let action = match self.parser.parse(
            trimmed,
            &self.workspace_root,
            self.prompt_state,
            None,
            has_active_mission,
        ) {
            Some(act) => act,
            None => return Ok(false),
        };

        self.handle_action(action).await
    }

    /// Execute the interactive REPL loop reading from stdin until exit or EOF.
    pub async fn run_loop(&mut self) -> Result<(), M31AError> {
        if self.current_session.is_none() {
            self.init_session(None).await?;
        }

        self.print_banner();

        let _is_interactive = std::io::stdin().is_terminal();
        let mut stdin_reader = BufReader::new(tokio::io::stdin()).lines();

        loop {
            // Prompt display
            let prompt = match self.prompt_state {
                SessionPromptState::Idle => "m31a> ",
                SessionPromptState::Editing => "... ",
                SessionPromptState::AwaitingApproval => "approve? [y/N/cancel] > ",
                SessionPromptState::WaitingForUser => "answer> ",
                _ => "> ",
            };

            print!("{prompt}");
            let _ = stdout().flush();

            // Read line or block
            let mut accumulated = String::new();
            let mut in_multiline_block = false;

            loop {
                let line = match stdin_reader.next_line().await {
                    Ok(Some(l)) => l,
                    Ok(None) => {
                        // EOF reached
                        println!("\nExiting session.");
                        if let Some(ref sess) = self.current_session {
                            let _ = self
                                .session_repo
                                .update_status(sess.id, SessionState::Closed)
                                .await;
                        }
                        return Ok(());
                    }
                    Err(e) => {
                        print_console_error(format!("Error reading input: {e}"));
                        return Err(M31AError::Internal(anyhow::anyhow!(e)));
                    }
                };

                let trimmed = line.trim();

                // Multiline triple-quote block handling
                if trimmed.starts_with("\"\"\"") {
                    if in_multiline_block {
                        // End of block
                        break;
                    } else {
                        // Start of block
                        in_multiline_block = true;
                        let after_quotes = trimmed.trim_start_matches("\"\"\"");
                        if !after_quotes.is_empty() {
                            accumulated.push_str(after_quotes);
                            accumulated.push('\n');
                        }
                        continue;
                    }
                }

                if in_multiline_block {
                    accumulated.push_str(&line);
                    accumulated.push('\n');
                    continue;
                }

                // Explicit trailing backslash continuation
                if line.ends_with('\\') {
                    accumulated.push_str(&line[..line.len() - 1]);
                    accumulated.push(' ');
                    print!("... ");
                    let _ = stdout().flush();
                    continue;
                }

                accumulated.push_str(&line);
                break;
            }

            let input_text = accumulated.trim();
            if input_text.is_empty() {
                continue;
            }

            // Dispatch typed action
            let should_exit = self.handle_input(input_text).await?;
            if should_exit {
                break;
            }
        }

        Ok(())
    }

    /// Dispatch and execute a typed application action.
    pub async fn handle_action(&mut self, action: ApplicationAction) -> Result<bool, M31AError> {
        match action {
            ApplicationAction::ExitRequested => {
                println!("Exiting M31A interactive session. State persisted.");
                if let Some(ref sess) = self.current_session {
                    let _ = self
                        .session_repo
                        .update_status(sess.id, SessionState::Closed)
                        .await;
                }
                return Ok(true);
            }

            ApplicationAction::ClearRequested => {
                // Clear console screen safely
                print!("\x1B[2J\x1B[1;1H");
                let _ = stdout().flush();
                self.print_banner();
            }

            ApplicationAction::ClearSessionRequested => {
                if let Some(ref sess) = self.current_session {
                    let _ = self.session_repo.clear_conversation(sess.id).await;
                    println!(
                        "Durable conversation history cleared for session {}.",
                        sess.id
                    );
                }
            }

            ApplicationAction::SlashCommandSubmitted { command, args } => {
                let cmd_line = format!("/{} {}", command, args.join(" "));
                // Runtime truth: slash-command status reflects the active
                // runtime configuration, never ambient environment probing.
                let (configured_model, configured_provider, active_profile) =
                    self.runtime_model_status();

                let ctx = CommandContext {
                    workspace_root: &self.workspace_root,
                    session_id: self.current_session.as_ref().map(|s| s.id),
                    active_mission_id: self
                        .current_session
                        .as_ref()
                        .and_then(|s| s.active_mission_id),
                    pool: self.runtime.pool(),
                    event_bus: self.runtime.event_bus(),
                    tool_registry: Some(self.runtime.tool_registry().clone()),
                    configured_model,
                    configured_provider,
                    active_profile,
                };

                match self.command_registry.execute_line(&cmd_line, &ctx).await? {
                    CommandOutput::Info(text) => println!("{text}"),
                    CommandOutput::Error(err) => print_console_error(format!("Error: {err}")),
                    CommandOutput::ApplicationAction(act) => {
                        return Box::pin(self.handle_action(act)).await;
                    }
                }
            }

            ApplicationAction::DiffRequested => {
                self.prompt_state = SessionPromptState::ShowingDiff;
                match self.runtime.get_git_diff().await {
                    Ok(diff) => {
                        println!("\n{diff}\n");
                        if let Some(ref sess) = self.current_session {
                            if let Ok(seq) = self.session_repo.next_sequence(sess.id).await {
                                let turn = ConversationTurn::UserMessage {
                                    id: uuid::Uuid::now_v7(),
                                    sequence: seq,
                                    content: "Show me the diff.".to_string(),
                                    raw_text: "Show me the diff.".to_string(),
                                    mentions: Vec::new(),
                                    created_at: chrono::Utc::now(),
                                };
                                let _ = self.session_repo.append_turn(sess.id, &turn).await;
                            }
                            if let Ok(seq) = self.session_repo.next_sequence(sess.id).await {
                                let turn = ConversationTurn::AssistantMessage {
                                    id: uuid::Uuid::now_v7(),
                                    sequence: seq,
                                    content: diff,
                                    created_at: chrono::Utc::now(),
                                };
                                let _ = self.session_repo.append_turn(sess.id, &turn).await;
                            }
                        }
                    }
                    Err(e) => print_console_error(format!("Error inspecting diff: {e}")),
                }
                self.prompt_state = SessionPromptState::Idle;
            }

            ApplicationAction::CommitRequested { message } => {
                let active_mid = self
                    .current_session
                    .as_ref()
                    .and_then(|s| s.active_mission_id);
                let raw_msg = message.clone().unwrap_or_else(|| "Commit it.".to_string());
                match self
                    .runtime
                    .commit_changes(active_mid, message.as_deref())
                    .await
                {
                    Ok(summary) => {
                        println!("\n{summary}\n");
                        if let Some(ref sess) = self.current_session {
                            if let Ok(seq) = self.session_repo.next_sequence(sess.id).await {
                                let turn = ConversationTurn::UserMessage {
                                    id: uuid::Uuid::now_v7(),
                                    sequence: seq,
                                    content: raw_msg.clone(),
                                    raw_text: raw_msg,
                                    mentions: Vec::new(),
                                    created_at: chrono::Utc::now(),
                                };
                                let _ = self.session_repo.append_turn(sess.id, &turn).await;
                            }
                            if let Ok(seq) = self.session_repo.next_sequence(sess.id).await {
                                let turn = ConversationTurn::AssistantMessage {
                                    id: uuid::Uuid::now_v7(),
                                    sequence: seq,
                                    content: summary,
                                    created_at: chrono::Utc::now(),
                                };
                                let _ = self.session_repo.append_turn(sess.id, &turn).await;
                            }
                        }
                    }
                    Err(e) => print_console_error(format!("Commit rejected: {e}")),
                }
            }

            ApplicationAction::StatusRequested => {
                let (configured_model, configured_provider, active_profile) =
                    self.runtime_model_status();
                let ctx = CommandContext {
                    workspace_root: &self.workspace_root,
                    session_id: self.current_session.as_ref().map(|s| s.id),
                    active_mission_id: self
                        .current_session
                        .as_ref()
                        .and_then(|s| s.active_mission_id),
                    pool: self.runtime.pool(),
                    event_bus: self.runtime.event_bus(),
                    tool_registry: Some(self.runtime.tool_registry().clone()),
                    configured_model,
                    configured_provider,
                    active_profile,
                };
                if let Ok(CommandOutput::Info(info)) =
                    self.command_registry.execute_line("/status", &ctx).await
                {
                    println!("\n{info}\n");
                }
            }

            ApplicationAction::CancelRequested => {
                println!("Cancel requested. Terminating current operation...");
                self.cancel_token.cancel();
                self.cancel_token = CancellationToken::new();
                self.active_engine = None;
                if let Some(mid) = self
                    .current_session
                    .as_ref()
                    .and_then(|s| s.active_mission_id)
                {
                    let _ = self
                        .runtime
                        .cancel_mission(mid, "Operator cancelled via interactive session")
                        .await;
                }

                if let Some(ref sess) = self.current_session {
                    let seq = self.session_repo.next_sequence(sess.id).await.unwrap_or(1);
                    let turn = ConversationTurn::SystemMessage {
                        id: uuid::Uuid::now_v7(),
                        sequence: seq,
                        content: "Operation cancelled.".to_string(),
                        created_at: chrono::Utc::now(),
                    };
                    let _ = self.session_repo.append_turn(sess.id, &turn).await;
                }
                self.prompt_state = SessionPromptState::Idle;
            }

            ApplicationAction::SessionResumeRequested { session_id } => {
                if let Ok(uuid) = session_id.parse::<uuid::Uuid>() {
                    let sid = SessionId::from(uuid);
                    match self.session_repo.get_session(sid).await? {
                        Some(sess) => {
                            self.current_session = Some(sess.clone());
                            println!("Resumed session: {}", sess.id);
                            let turns = self.session_repo.get_conversation(sess.id).await?;
                            println!("Loaded {} prior conversation turn(s):", turns.len());
                            for t in turns {
                                println!(
                                    "  [{}] {}: {}",
                                    t.sequence(),
                                    t.kind_str(),
                                    t.text_content()
                                );
                            }
                        }
                        None => eprintln!("Session '{session_id}' not found."),
                    }
                } else {
                    eprintln!("Invalid session ID: '{session_id}'");
                }
            }

            ApplicationAction::ApprovalDecision {
                request_id,
                decision,
            } => {
                let is_approved = matches!(
                    decision,
                    crate::tui::approval::ApprovalDecision::ApproveOnce
                        | crate::tui::approval::ApprovalDecision::ApproveAlways
                );

                if let Ok(req_uuid) = uuid::Uuid::parse_str(&request_id) {
                    let action = match decision {
                        crate::tui::approval::ApprovalDecision::ApproveOnce => {
                            crate::policy::approval::ApprovalAction::AllowOnce
                        }
                        crate::tui::approval::ApprovalDecision::ApproveAlways => {
                            crate::policy::approval::ApprovalAction::AllowForSession
                        }
                        crate::tui::approval::ApprovalDecision::Reject => {
                            crate::policy::approval::ApprovalAction::Deny {
                                reason: "operator rejected".to_string(),
                            }
                        }
                        crate::tui::approval::ApprovalDecision::Dismiss => {
                            crate::policy::approval::ApprovalAction::Deny {
                                reason: "operator dismissed".to_string(),
                            }
                        }
                        crate::tui::approval::ApprovalDecision::Edit => {
                            crate::policy::approval::ApprovalAction::Deny {
                                reason: "operator requested edit".to_string(),
                            }
                        }
                    };

                    // Approval resolution is a real coordinator operation: a
                    // failure to resolve is an execution error and propagates
                    // (never silently ignored).
                    self.runtime
                        .approval_coordinator()
                        .resolve_request(req_uuid.into(), action, "operator")
                        .await
                        .map_err(|e| {
                            M31AError::internal(format!(
                                "approval resolution failed for '{request_id}': {e}"
                            ))
                        })?;

                    if let Some(ref mut engine) = self.active_engine {
                        engine.provide_approval(&request_id, is_approved).await?;
                    }

                    if let Some(ref sess) = self.current_session {
                        let seq = self.session_repo.next_sequence(sess.id).await.unwrap_or(1);
                        let turn = ConversationTurn::ApprovalMessage {
                            id: uuid::Uuid::now_v7(),
                            sequence: seq,
                            request_id,
                            prompt: "Approval resolved by operator".to_string(),
                            decision: Some(format!("{decision:?}")),
                            created_at: chrono::Utc::now(),
                        };
                        let _ = self.session_repo.append_turn(sess.id, &turn).await;
                    }
                } else if let Some(ref sess) = self.current_session {
                    let coordinator = self.runtime.create_pre_execution_coordinator();
                    let sid_str = sess.id.to_string();
                    if let Ok(Some(state)) = coordinator
                        .lifecycle_repo()
                        .load_lifecycle_state(&sid_str)
                        .await
                    {
                        if state.stage == crate::state_machine::lifecycle::LifecycleStage::ExecutionAwaitingAuthorization {
                            let act = ApplicationAction::ExecutionAuthorizationSubmitted {
                                session_id: Some(sid_str),
                                decision: is_approved,
                                reason: None,
                            };
                            return Box::pin(self.handle_action(act)).await;
                        }
                    }
                }

                if is_approved {
                    if let Some(mut engine) = self.active_engine.take() {
                        self.prompt_state = SessionPromptState::Executing;
                        self.run_agent_turn_loop(&mut engine).await?;
                        self.active_engine = Some(engine);
                    } else {
                        self.prompt_state = SessionPromptState::Idle;
                    }
                } else {
                    self.prompt_state = SessionPromptState::Idle;
                }
            }

            ApplicationAction::ModelChangeRequested { model } => {
                match self.runtime.config().with_session_model(&model) {
                    Ok(new_cfg) => {
                        let new_runtime = (*self.runtime).clone().with_config(Arc::new(new_cfg));
                        // Authority change invalidates the derived engine.
                        self.rebind_runtime(Arc::new(new_runtime));
                        println!("Active model switched to '{}' for this session.", model);
                        if let Some(ref sess) = self.current_session
                            && let Ok(seq) = self.session_repo.next_sequence(sess.id).await
                        {
                            let turn = ConversationTurn::AssistantMessage {
                                id: uuid::Uuid::now_v7(),
                                sequence: seq,
                                content: format!(
                                    "Active model switched to '{}' for this session.",
                                    model
                                ),
                                created_at: chrono::Utc::now(),
                            };
                            let _ = self.session_repo.append_turn(sess.id, &turn).await;
                        }
                    }
                    Err(e) => {
                        print_console_error(format!("Failed to switch model: {e}"));
                    }
                }
            }

            ApplicationAction::ProfileChangeRequested { profile } => {
                match self.runtime.config().with_session_profile(&profile) {
                    Ok(new_cfg) => {
                        let new_runtime = (*self.runtime).clone().with_config(Arc::new(new_cfg));
                        // Authority change invalidates the derived engine.
                        self.rebind_runtime(Arc::new(new_runtime));
                        println!("Autonomy profile set to '{}' for this session.", profile);
                        if let Some(ref sess) = self.current_session
                            && let Ok(seq) = self.session_repo.next_sequence(sess.id).await
                        {
                            let turn = ConversationTurn::AssistantMessage {
                                id: uuid::Uuid::now_v7(),
                                sequence: seq,
                                content: format!(
                                    "Autonomy profile set to '{}' for this session.",
                                    profile
                                ),
                                created_at: chrono::Utc::now(),
                            };
                            let _ = self.session_repo.append_turn(sess.id, &turn).await;
                        }
                    }
                    Err(e) => {
                        print_console_error(format!("Failed to switch profile: {e}"));
                    }
                }
            }

            ApplicationAction::ConfigOverrideRequested { key, value } => {
                let parsed_val = serde_json::from_str::<serde_json::Value>(&value)
                    .unwrap_or_else(|_| serde_json::Value::String(value.clone()));
                match self
                    .runtime
                    .config()
                    .with_session_override(&key, parsed_val)
                {
                    Ok(new_cfg) => {
                        let new_runtime = (*self.runtime).clone().with_config(Arc::new(new_cfg));
                        // Authority change invalidates the derived engine.
                        self.rebind_runtime(Arc::new(new_runtime));
                        println!("Session override applied: {} = {}", key, value);
                        if let Some(ref sess) = self.current_session
                            && let Ok(seq) = self.session_repo.next_sequence(sess.id).await
                        {
                            let turn = ConversationTurn::AssistantMessage {
                                id: uuid::Uuid::now_v7(),
                                sequence: seq,
                                content: format!("Session override applied: {} = {}", key, value),
                                created_at: chrono::Utc::now(),
                            };
                            let _ = self.session_repo.append_turn(sess.id, &turn).await;
                        }
                    }
                    Err(e) => {
                        print_console_error(format!("Failed to apply configuration override: {e}"));
                    }
                }
            }

            ApplicationAction::UserTextSubmitted(parsed) => {
                let session = match self.current_session.as_ref() {
                    Some(s) => s.clone(),
                    None => self.init_session(None).await?,
                };

                // Enrich prompt with resolved @mentions
                let mention_ctx =
                    MentionParser::inject_mention_context(&self.workspace_root, &parsed.mentions);
                let full_prompt = format!("{}{}", parsed.raw_text, mention_ctx);

                // INVARIANT F — REAL FRONT DOOR:
                // Check for an active governed lifecycle in durable state BEFORE
                // routing to AgentEngine. A new governed implementation request MUST
                // enter the PreExecution lifecycle (intent → discovery → plan → tasks
                // → authorization → execution). Only already-executing agent sessions
                // or explicit conversational continuations go to run_agent_turn_loop.
                let coordinator = self.runtime.create_pre_execution_coordinator();
                let sid_str = session.id.to_string();
                let lifecycle_state = coordinator
                    .lifecycle_repo()
                    .load_lifecycle_state(&sid_str)
                    .await
                    .unwrap_or(None);

                // Determine routing by durable lifecycle stage
                let governed_stage = lifecycle_state.as_ref().map(|ls| ls.stage);
                let route_to_lifecycle = match &governed_stage {
                    // No active lifecycle → start a new governed PreExecution lifecycle
                    None => true,
                    // Active lifecycle awaiting operator information (questions answered via text)
                    Some(crate::state_machine::lifecycle::LifecycleStage::AwaitingInformation)
                    | Some(crate::state_machine::lifecycle::LifecycleStage::IntentActive) => {
                        // Delegate to existing WaitingForUser logic below (pending Q&A)
                        false // handled in WaitingForUser branch
                    }
                    // Active lifecycle in review/authorization — these are also lifecycle events
                    Some(crate::state_machine::lifecycle::LifecycleStage::PlanReview)
                    | Some(crate::state_machine::lifecycle::LifecycleStage::PlanRevision)
                    | Some(crate::state_machine::lifecycle::LifecycleStage::PlanDraft)
                    | Some(crate::state_machine::lifecycle::LifecycleStage::PlanAccepted)
                    | Some(crate::state_machine::lifecycle::LifecycleStage::TasksDraft)
                    | Some(crate::state_machine::lifecycle::LifecycleStage::TasksReview)
                    | Some(crate::state_machine::lifecycle::LifecycleStage::TasksRevision)
                    | Some(crate::state_machine::lifecycle::LifecycleStage::TasksAccepted)
                    | Some(crate::state_machine::lifecycle::LifecycleStage::ExecutionAwaitingAuthorization)
                    | Some(crate::state_machine::lifecycle::LifecycleStage::ExecutionAuthorized) => {
                        // Lifecycle is active but not in a text-answer stage;
                        // free-text is not the right input here
                        println!(
                            "\n[M31A] Active governed lifecycle is in stage '{:?}'.",
                            governed_stage.as_ref().unwrap()
                        );
                        println!(
                            "       Use lifecycle commands (/plan-accept, /task-accept, /authorize, etc.) \
                             to advance. Free-text input is not accepted in this stage."
                        );
                        return Ok(false);
                    }
                    // Terminal or execution stages → fall through to AgentEngine continuation
                    Some(_) => false,
                };

                if route_to_lifecycle {
                    // No active lifecycle: start the governed PreExecution lifecycle
                    let seq = self.session_repo.next_sequence(session.id).await?;
                    let user_turn = ConversationTurn::UserMessage {
                        id: uuid::Uuid::now_v7(),
                        sequence: seq,
                        content: parsed.normalized_prompt(),
                        raw_text: full_prompt.clone(),
                        mentions: parsed.mentions.clone(),
                        created_at: chrono::Utc::now(),
                    };
                    self.session_repo
                        .append_turn(session.id, &user_turn)
                        .await?;

                    println!(
                        "\n[M31A] Starting governed PreExecution lifecycle for new mission request...\n"
                    );
                    match coordinator
                        .init_intent(&sid_str, &full_prompt, "operator")
                        .await
                    {
                        Ok(resp) => {
                            self.handle_lifecycle_response(resp).await?;
                        }
                        Err(e) => {
                            print_console_error(format!("[M31A] Error starting lifecycle: {e}"));
                            return Err(crate::error::M31AError::Internal(anyhow::anyhow!(e)));
                        }
                    }
                    return Ok(false);
                }

                let mut engine = match self.active_engine.take() {
                    Some(eng) => eng,
                    None => {
                        let mut eng = self.runtime.create_agent_engine(session.id);
                        let _ = eng.load_session_state().await;
                        eng
                    }
                };

                if self.prompt_state == SessionPromptState::WaitingForUser {
                    let questions = coordinator
                        .lifecycle_repo()
                        .load_discovery_questions(&sid_str)
                        .await
                        .unwrap_or_default();
                    let pending_questions: Vec<_> = questions
                        .into_iter()
                        .filter(|q| q.answer.is_none())
                        .collect();
                    if let Some(q) = pending_questions.first() {
                        let seq = self.session_repo.next_sequence(session.id).await?;
                        let user_turn = ConversationTurn::UserMessage {
                            id: uuid::Uuid::now_v7(),
                            sequence: seq,
                            content: parsed.normalized_prompt(),
                            raw_text: full_prompt.clone(),
                            mentions: parsed.mentions.clone(),
                            created_at: chrono::Utc::now(),
                        };
                        self.session_repo
                            .append_turn(session.id, &user_turn)
                            .await?;

                        match coordinator
                            .submit_answer(&sid_str, &q.question_id, &full_prompt, "operator")
                            .await
                        {
                            Ok(resp) => {
                                self.handle_lifecycle_response(resp).await?;
                                return Ok(false);
                            }
                            Err(e) => {
                                print_console_error(format!("[M31A] Error submitting answer: {e}"));
                            }
                        }
                    } else if let Err(e) = engine.provide_user_response(&full_prompt).await {
                        print_console_error(format!("[M31A] Error submitting user response: {e}"));
                    }
                } else {
                    // Record user turn in SQLite
                    let seq = self.session_repo.next_sequence(session.id).await?;
                    let user_turn = ConversationTurn::UserMessage {
                        id: uuid::Uuid::now_v7(),
                        sequence: seq,
                        content: parsed.normalized_prompt(),
                        raw_text: full_prompt.clone(),
                        mentions: parsed.mentions.clone(),
                        created_at: chrono::Utc::now(),
                    };
                    self.session_repo
                        .append_turn(session.id, &user_turn)
                        .await?;
                }

                // Existing agent turn — run continuous interactive agent loop
                self.prompt_state = SessionPromptState::Executing;
                println!("\n[M31A] Agent reasoning and executing...");

                self.run_agent_turn_loop(&mut engine).await?;
                self.active_engine = Some(engine);
            }

            ApplicationAction::GenesisRequested { prompt, mode } => {
                let session = match self.current_session.as_ref() {
                    Some(s) => s.clone(),
                    None => self.init_session(None).await?,
                };

                println!("\nInitiating Project Genesis intake and planning...");
                println!("Objective: {prompt}\n");

                let gen_mode = match mode.as_deref() {
                    Some("greenfield") => crate::workflow::genesis::GenesisMode::Greenfield,
                    Some("brownfield") => crate::workflow::genesis::GenesisMode::Brownfield,
                    _ => crate::workflow::genesis::GenesisMode::AutoDetect,
                };

                let req = crate::workflow::genesis::GenesisRequest::new(
                    prompt.clone(),
                    &self.workspace_root,
                )
                .with_mode(gen_mode);

                match self.runtime.run_genesis(&req).await {
                    Ok(outcome) => {
                        println!("Project Genesis planning complete!");
                        println!("Charter: {}", outcome.charter.project_name);
                        println!(
                            "Requirements synthesized: {}",
                            outcome.planning.requirements.requirements.len()
                        );
                        println!(
                            "Architecture components: {}",
                            outcome.planning.architecture.components.len()
                        );
                        println!("Roadmap phases: {}", outcome.planning.roadmap.phases.len());
                        println!(
                            "Registered artifacts: {}",
                            outcome.registered_artifacts.len()
                        );

                        if let Ok(seq) = self.session_repo.next_sequence(session.id).await {
                            let turn = ConversationTurn::AssistantMessage {
                                id: uuid::Uuid::now_v7(),
                                sequence: seq,
                                content: format!(
                                    "Project Genesis planning complete for '{}'. Roadmap lowered to workflow '{}' with {} phases.",
                                    outcome.charter.project_name,
                                    outcome.workflow_definition.id,
                                    outcome.planning.roadmap.phases.len()
                                ),
                                created_at: chrono::Utc::now(),
                            };
                            let _ = self.session_repo.append_turn(session.id, &turn).await;
                        }
                    }
                    Err(err) => {
                        print_console_error(format!("\nGenesis error: {err}"));
                        if let Ok(seq) = self.session_repo.next_sequence(session.id).await {
                            let err_turn = ConversationTurn::SystemMessage {
                                id: uuid::Uuid::now_v7(),
                                sequence: seq,
                                content: format!("Genesis error: {err}"),
                                created_at: chrono::Utc::now(),
                            };
                            let _ = self.session_repo.append_turn(session.id, &err_turn).await;
                        }
                    }
                }
            }

            ApplicationAction::GenesisStateRequested => {
                match self.runtime.get_genesis_state(".planning").await {
                    Ok(Some(state)) => {
                        println!("\nCurrent Planning State:");
                        println!("  Lifecycle: {:?}", state.lifecycle_state);
                        println!("  Status: {}", state.approval_status);
                        println!("  Current Phase: {:?}", state.current_phase);
                    }
                    Ok(None) => {
                        println!("No Project Genesis state found in .planning/STATE.md");
                    }
                    Err(e) => {
                        print_console_error(format!("Error inspecting Genesis state: {e}"));
                    }
                }
            }

            ApplicationAction::WorkflowResumeRequested { run_id } => {
                // Canonical owner: AppRuntime::handle_workflow_resume (one
                // mutation path shared with the TUI bridge). The runtime
                // decides; failures are surfaced, never printed as success.
                match self.runtime.handle_workflow_resume(&run_id).await {
                    Ok(report) => println!("{report}"),
                    Err(e) => {
                        print_console_error(format!("Workflow resume failed for run {run_id}: {e}"))
                    }
                }
            }

            ApplicationAction::WorkflowApprovalSubmitted {
                run_id,
                step_key,
                approved,
                reason,
            } => {
                // Canonical owner: AppRuntime::handle_workflow_approval.
                match self
                    .runtime
                    .handle_workflow_approval(&run_id, &step_key, approved, reason.clone())
                    .await
                {
                    Ok(report) => println!("{report}"),
                    Err(e) => print_console_error(format!(
                        "Workflow approval failed for run {run_id}, step {step_key}: {e}"
                    )),
                }
            }

            ApplicationAction::WorkflowPauseRequested { run_id, reason } => {
                // Canonical owner: AppRuntime::pause_workflow_run.
                match self.runtime.pause_workflow_run(&run_id, &reason).await {
                    Ok(report) => println!("{report}"),
                    Err(e) => {
                        print_console_error(format!("Workflow pause failed for run {run_id}: {e}"))
                    }
                }
            }

            ApplicationAction::WorkflowCancelRequested { run_id, reason } => {
                // Canonical owner: AppRuntime::cancel_workflow_run.
                match self.runtime.cancel_workflow_run(&run_id, &reason).await {
                    Ok(report) => println!("{report}"),
                    Err(e) => {
                        print_console_error(format!("Workflow cancel failed for run {run_id}: {e}"))
                    }
                }
            }

            ApplicationAction::WorkflowInspectRequested { run_id } => {
                // Canonical owner: AppRuntime::inspect_workflow_run (read-only).
                match self.runtime.inspect_workflow_run(&run_id).await {
                    Ok(report) => println!("{report}"),
                    Err(e) => print_console_error(format!(
                        "Workflow inspect failed for run {run_id}: {e}"
                    )),
                }
            }

            ApplicationAction::IntentInitRequested { .. }
            | ApplicationAction::QuestionAnswerSubmitted { .. }
            | ApplicationAction::PlanEditRequested { .. }
            | ApplicationAction::PlanRevisionRequested { .. }
            | ApplicationAction::PlanRegenerateRequested { .. }
            | ApplicationAction::PlanAcceptRequested { .. }
            | ApplicationAction::PlanRejectRequested { .. }
            | ApplicationAction::TaskEditRequested { .. }
            | ApplicationAction::TaskAddRequested { .. }
            | ApplicationAction::TaskRemoveRequested { .. }
            | ApplicationAction::TaskRegenerateRequested { .. }
            | ApplicationAction::TasksAcceptRequested { .. }
            | ApplicationAction::ExecutionAuthorizationSubmitted { .. } => {
                let coordinator = self.runtime.create_pre_execution_coordinator();
                let effective_action = match action {
                    ApplicationAction::IntentInitRequested {
                        session_id,
                        raw_prompt,
                    } => {
                        let sid = session_id
                            .or_else(|| self.current_session.as_ref().map(|s| s.id.to_string()));
                        ApplicationAction::IntentInitRequested {
                            session_id: sid,
                            raw_prompt,
                        }
                    }
                    ApplicationAction::QuestionAnswerSubmitted {
                        session_id,
                        question_id,
                        answer,
                    } => {
                        let sid = session_id
                            .or_else(|| self.current_session.as_ref().map(|s| s.id.to_string()));
                        ApplicationAction::QuestionAnswerSubmitted {
                            session_id: sid,
                            question_id,
                            answer,
                        }
                    }
                    ApplicationAction::PlanEditRequested {
                        session_id,
                        plan_json,
                    } => {
                        let sid = session_id
                            .or_else(|| self.current_session.as_ref().map(|s| s.id.to_string()));
                        ApplicationAction::PlanEditRequested {
                            session_id: sid,
                            plan_json,
                        }
                    }
                    ApplicationAction::PlanRevisionRequested {
                        session_id,
                        feedback,
                    } => {
                        let sid = session_id
                            .or_else(|| self.current_session.as_ref().map(|s| s.id.to_string()));
                        ApplicationAction::PlanRevisionRequested {
                            session_id: sid,
                            feedback,
                        }
                    }
                    ApplicationAction::PlanRegenerateRequested { session_id } => {
                        let sid = session_id
                            .or_else(|| self.current_session.as_ref().map(|s| s.id.to_string()));
                        ApplicationAction::PlanRegenerateRequested { session_id: sid }
                    }
                    ApplicationAction::PlanAcceptRequested { session_id } => {
                        let sid = session_id
                            .or_else(|| self.current_session.as_ref().map(|s| s.id.to_string()));
                        ApplicationAction::PlanAcceptRequested { session_id: sid }
                    }
                    ApplicationAction::PlanRejectRequested { session_id, reason } => {
                        let sid = session_id
                            .or_else(|| self.current_session.as_ref().map(|s| s.id.to_string()));
                        ApplicationAction::PlanRejectRequested {
                            session_id: sid,
                            reason,
                        }
                    }
                    ApplicationAction::TaskEditRequested {
                        session_id,
                        task_json,
                    } => {
                        let sid = session_id
                            .or_else(|| self.current_session.as_ref().map(|s| s.id.to_string()));
                        ApplicationAction::TaskEditRequested {
                            session_id: sid,
                            task_json,
                        }
                    }
                    ApplicationAction::TaskAddRequested {
                        session_id,
                        task_json,
                    } => {
                        let sid = session_id
                            .or_else(|| self.current_session.as_ref().map(|s| s.id.to_string()));
                        ApplicationAction::TaskAddRequested {
                            session_id: sid,
                            task_json,
                        }
                    }
                    ApplicationAction::TaskRemoveRequested {
                        session_id,
                        task_id,
                    } => {
                        let sid = session_id
                            .or_else(|| self.current_session.as_ref().map(|s| s.id.to_string()));
                        ApplicationAction::TaskRemoveRequested {
                            session_id: sid,
                            task_id,
                        }
                    }
                    ApplicationAction::TaskRegenerateRequested {
                        session_id,
                        feedback,
                    } => {
                        let sid = session_id
                            .or_else(|| self.current_session.as_ref().map(|s| s.id.to_string()));
                        ApplicationAction::TaskRegenerateRequested {
                            session_id: sid,
                            feedback,
                        }
                    }
                    ApplicationAction::TasksAcceptRequested { session_id } => {
                        let sid = session_id
                            .or_else(|| self.current_session.as_ref().map(|s| s.id.to_string()));
                        ApplicationAction::TasksAcceptRequested { session_id: sid }
                    }
                    ApplicationAction::ExecutionAuthorizationSubmitted {
                        session_id,
                        decision,
                        reason,
                    } => {
                        let sid = session_id
                            .or_else(|| self.current_session.as_ref().map(|s| s.id.to_string()));
                        ApplicationAction::ExecutionAuthorizationSubmitted {
                            session_id: sid,
                            decision,
                            reason,
                        }
                    }
                    _ => unreachable!(),
                };

                match coordinator
                    .handle_action(effective_action, "operator")
                    .await
                {
                    Ok(resp) => {
                        self.handle_lifecycle_response(resp).await?;
                    }
                    Err(err) => {
                        eprintln!("\n[Lifecycle Error] {err}\n");
                        if let Some(ref sess) = self.current_session {
                            if let Ok(seq) = self.session_repo.next_sequence(sess.id).await {
                                let turn = ConversationTurn::SystemMessage {
                                    id: uuid::Uuid::now_v7(),
                                    sequence: seq,
                                    content: format!("Lifecycle error: {err}"),
                                    created_at: chrono::Utc::now(),
                                };
                                let _ = self.session_repo.append_turn(sess.id, &turn).await;
                            }
                        }
                        return Err(M31AError::validation(err));
                    }
                }
            }

            ApplicationAction::MentionResolved(_) => {}
        }

        Ok(false)
    }

    /// Process PreExecutionResponse lifecycle outputs, update prompt state, and display instructions.
    async fn handle_lifecycle_response(
        &mut self,
        resp: crate::planning::review::PreExecutionResponse,
    ) -> Result<bool, M31AError> {
        match resp {
            crate::planning::review::PreExecutionResponse::QuestionsRequired {
                session_id,
                questions,
            } => {
                println!(
                    "\n[M31A Discovery] Additional information needed before planning (Session: {session_id}):"
                );
                for (i, q) in questions.iter().enumerate() {
                    println!("\n{}. [{}] {}", i + 1, q.question_id, q.text);
                    if !q.options.is_empty() {
                        println!("   Options:");
                        for opt in &q.options {
                            println!("     - {}", opt);
                        }
                    }
                }
                println!("\nProvide answers to continue planning.\n");
                self.prompt_state = SessionPromptState::WaitingForUser;
            }
            crate::planning::review::PreExecutionResponse::PlanForReview {
                session_id: _,
                revision,
            } => {
                println!("\n========================================================");
                println!(
                    " [PLAN REVIEW] Revision {} (Plan ID: {})",
                    revision.revision, revision.plan_id
                );
                println!(
                    " Author: {} ({:?})",
                    revision.created_by, revision.author_type
                );
                println!(" Objective: {}", revision.content.objective);
                println!(" Tasks: {}", revision.content.tasks.len());
                println!(" Assumptions: {}", revision.content.assumptions.len());
                println!("========================================================");
                println!(" Commands available:");
                println!("   /plan accept          - Approve this proposed plan");
                println!("   /plan edit <json>     - Supply manual JSON edit");
                println!("   /plan revise <prompt> - Request model revision");
                println!("   /plan regen           - Regenerate candidate plan");
                println!("   /plan reject <reason> - Reject and stop");
                println!("========================================================\n");
                self.prompt_state = SessionPromptState::Idle;
            }
            crate::planning::review::PreExecutionResponse::TasksForReview {
                session_id: _,
                revision,
            } => {
                println!("\n========================================================");
                println!(
                    " [TASK REVIEW] Revision {} (for Plan Revision {})",
                    revision.revision, revision.plan_revision
                );
                println!(" Total Tasks: {}", revision.tasks.len());
                println!("--------------------------------------------------------");
                for t in &revision.tasks {
                    let deps: Vec<String> = t.depends_on.iter().map(|d| d.to_string()).collect();
                    println!(
                        "  • [{}] {} (Role: {}, Deps: {:?})",
                        t.id, t.objective, t.role, deps
                    );
                }
                println!("========================================================");
                println!(" Commands available:");
                println!("   /tasks accept          - Approve this task set");
                println!("   /task edit <json>      - Edit task specification");
                println!("   /task add <json>       - Add a candidate task");
                println!("   /task remove <id>      - Remove a candidate task");
                println!("   /tasks regen [notes]   - Regenerate task graph");
                println!("========================================================\n");
                self.prompt_state = SessionPromptState::Idle;
            }
            crate::planning::review::PreExecutionResponse::AuthorizationRequested {
                session_id,
                plan_revision,
                task_revision,
                message,
            } => {
                println!("\n========================================================");
                println!(" [EXECUTION AUTHORIZATION REQUIRED]");
                println!(" Session: {session_id}");
                println!(" Bound to: Plan Rev {plan_revision}, Task Rev {task_revision}");
                println!(" Message: {message}");
                println!("--------------------------------------------------------");
                println!(
                    " Explicit human authorization is required before any workspace changes occur."
                );
                println!(" Commands available:");
                println!("   /authorize yes  - Explicitly authorize execution");
                println!("   /authorize no   - Reject authorization");
                println!("========================================================\n");
                self.prompt_state = SessionPromptState::AwaitingApproval;
            }
            crate::planning::review::PreExecutionResponse::ReadyToExecute {
                session_id: ref sid,
                mut plan,
                tasks,
                ref authorization,
            } => {
                println!("\n========================================================");
                println!(" [EXECUTION AUTHORIZED]");
                println!(" Authorization ID: {}", authorization.id);
                println!(" Authorized by: {}", authorization.authorized_by);
                println!(
                    " Bound to: Plan Rev {}, Task Rev {}",
                    authorization.plan_revision, authorization.task_revision
                );
                println!(" Executing {} tasks via DAG Runtime...", tasks.len());
                println!("========================================================\n");

                plan.tasks = tasks;

                // Materialize into authoritative TaskGraph in SQLite with revision binding
                let mission_id = self
                    .current_session
                    .as_ref()
                    .and_then(|s| s.active_mission_id)
                    .unwrap_or_default();
                let materializer = crate::dag::materializer::TaskGraphMaterializer::new(
                    self.runtime.pool().clone(),
                );
                match materializer
                    .materialize_authorized(
                        mission_id,
                        &plan,
                        authorization.plan_revision,
                        authorization.task_revision,
                        authorization,
                    )
                    .await
                {
                    Ok(graph) => {
                        println!(
                            "TaskGraph materialized: {} with {} tasks.",
                            graph.id,
                            graph.tasks.len()
                        );

                        // INVARIANT D — StartExecution: transition durable lifecycle to
                        // Executing NOW, after materialization is committed but before
                        // handing off to the AutonomyController. This is the verifiable
                        // execution boundary. A crash after this point recovers as Executing.
                        // Pure law first, persistence second (D2): durable write goes
                        // through the validated transition seam.
                        let coordinator = self.runtime.create_pre_execution_coordinator();
                        let lifecycle_repo = coordinator.lifecycle_repo();
                        if let Ok(Some(ls)) = lifecycle_repo.load_lifecycle_state(sid).await {
                            if ls.stage
                                == crate::state_machine::lifecycle::LifecycleStage::ExecutionAuthorized
                            {
                                if let Err(e) = lifecycle_repo
                                    .save_validated_transition(
                                        sid,
                                        crate::state_machine::lifecycle::LifecycleStage::ExecutionAuthorized,
                                        crate::state_machine::lifecycle::LifecycleEvent::StartExecution,
                                    )
                                    .await
                                {
                                    print_console_error(format!(
                                        "[M31A] Warning: failed to persist Executing lifecycle state: {e}"
                                    ));
                                }
                            }
                        }

                        println!("Executing authorized tasks via production AutonomyController...");
                        match self
                            .runtime
                            .run_authorized_mission(mission_id, &plan.objective)
                            .await
                        {
                            Ok(summary) => {
                                println!(
                                    "\n[Execution Finished] Mission '{}' status: {} (tasks completed: {})",
                                    summary.mission_id, summary.status, summary.tasks_completed
                                );
                            }
                            Err(e) => {
                                print_console_error(format!(
                                    "\n[Execution Error] Mission execution failed: {e}"
                                ));
                                return Err(e);
                            }
                        }
                    }
                    Err(e) => {
                        print_console_error(format!("Error: failed to materialize TaskGraph: {e}"));
                        return Err(e);
                    }
                }

                self.prompt_state = SessionPromptState::Idle;
            }
            crate::planning::review::PreExecutionResponse::Terminated {
                session_id,
                stage,
                reason,
            } => {
                println!(
                    "\n[Lifecycle Terminated] Session {session_id} is in terminal state '{stage}': {reason}\n"
                );
                self.prompt_state = SessionPromptState::Idle;
            }
        }
        Ok(false)
    }

    /// Run the continuous interactive agent loop until it reaches a user wait, approval, or completion.
    async fn run_agent_turn_loop(&mut self, engine: &mut AgentEngine) -> Result<(), M31AError> {
        let final_state = engine
            .run_continuous(|outcome| {
                match outcome {
                    AgentTurnOutcome::AssistantCommentary { content } => {
                        println!("\n[Assistant Commentary]\n{content}\n");
                    }
                    AgentTurnOutcome::AssistantText { content } => {
                        println!("\n[Assistant]\n{content}\n");
                    }
                    AgentTurnOutcome::ToolResults { results } => {
                        println!("\n[Tools Executed: {}]", results.len());
                        for r in results {
                            if r.success {
                                println!("  ✓ {} ({}ms)", r.tool_name, r.duration_ms);
                            } else {
                                println!(
                                    "  ✗ {} ({}ms): {}",
                                    r.tool_name,
                                    r.duration_ms,
                                    r.error.as_deref().unwrap_or(&r.output)
                                );
                            }
                        }
                    }
                    AgentTurnOutcome::WaitingForUser { question, options } => {
                        println!("\n[Agent Question]\n{}", question);
                        if !options.is_empty() {
                            println!("Options:");
                            for opt in options {
                                println!("  [{}] {}", opt.id, opt.label);
                            }
                        }
                        println!();
                    }
                    AgentTurnOutcome::WaitingForApproval {
                        request_id,
                        tool_name,
                        parameters,
                    } => {
                        println!("\n[Action Requires Approval]");
                        println!("Request ID: {}", request_id);
                        println!("Tool:       {}", tool_name);
                        println!(
                            "Parameters: {}",
                            serde_json::to_string_pretty(parameters).unwrap_or_default()
                        );
                        println!();
                    }
                    AgentTurnOutcome::Completed { summary } => {
                        // §39: an assistant turn completing is not mission
                        // completion; mission truth comes from the
                        // verification-gated completion path only.
                        println!("\n=== ASSISTANT TURN COMPLETED (not mission completion) ===");
                        println!("{summary}\n");
                    }
                    AgentTurnOutcome::Failed { error } => {
                        print_console_error(format!("\n=== TASK FAILED ===\n{error}\n"));
                    }
                    AgentTurnOutcome::Cancelled { reason } => {
                        println!("\n=== TASK CANCELLED ===");
                        println!("{reason}\n");
                    }
                    AgentTurnOutcome::BudgetExhausted { reason } => {
                        println!("\n=== TURN BUDGET EXHAUSTED ===");
                        println!("{reason}\n");
                    }
                }
                Ok(())
            })
            .await?;

        // Update prompt state according to authoritative engine state
        match final_state {
            AgentEngineState::WaitingForUser { .. } => {
                self.prompt_state = SessionPromptState::WaitingForUser;
            }
            AgentEngineState::WaitingForApproval { .. } => {
                self.prompt_state = SessionPromptState::AwaitingApproval;
            }
            AgentEngineState::Failed { .. } => {
                self.prompt_state = SessionPromptState::Error;
            }
            _ => {
                self.prompt_state = SessionPromptState::Idle;
            }
        }

        Ok(())
    }
}
