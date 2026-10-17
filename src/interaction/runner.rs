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
    command_registry: Arc<SlashCommandRegistry>,
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
        let command_registry = runtime.slash_registry().clone();
        let snapshot_handle = runtime.command_snapshot_handle();
        let parser = InteractionParser::with_snapshot_handle(snapshot_handle);

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

    /// Access the bound production runtime.
    pub fn runtime(&self) -> &Arc<AppRuntime> {
        &self.runtime
    }

    /// Access the authoritative slash command registry Arc.
    pub fn command_registry(&self) -> &Arc<SlashCommandRegistry> {
        &self.command_registry
    }

    /// Access the deterministic interaction parser.
    pub fn parser(&self) -> &InteractionParser {
        &self.parser
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
        self.command_registry = runtime.slash_registry().clone();
        self.parser
            .set_snapshot_handle(runtime.command_snapshot_handle());
        self.runtime = runtime;
        self.workspace_root = self.runtime.workspace_root().to_path_buf();
        self.session_repo = SqliteSessionRepository::new(self.runtime.pool().clone());
        self.invalidate_engine();
    }

    /// Refresh the cached command registry and parser from the runtime snapshot handle.
    pub fn refresh_registry(&mut self) {
        self.command_registry = self.runtime.slash_registry().clone();
        self.parser
            .set_snapshot_handle(self.runtime.command_snapshot_handle());
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
            cfg.active_profile
                .clone()
                .unwrap_or_else(|| "default".to_string()),
        )
    }

    /// Initialize a new durable session or attach to existing.
    ///
    /// Workspace binding rule (fail closed): resuming the SAME workspace
    /// attaches normally; a session from a DIFFERENT workspace is REFUSED —
    /// the caller MUST `rebind_runtime` to a runtime bound to that workspace
    /// first (see `rebind_runtime`). This prevents cross-workspace execution
    /// against the wrong authority graph. Direct `self.runtime` mutation
    /// outside `rebind_runtime` is forbidden.
    pub async fn init_session(
        &mut self,
        resume_id: Option<SessionId>,
    ) -> Result<Session, M31AError> {
        if let Some(id) = resume_id {
            if let Some(sess) = self.session_repo.get_session(id).await? {
                let sess_ws =
                    crate::init::instance::canonicalize_workspace_root(&sess.workspace_root);
                let runtime_ws = crate::init::instance::canonicalize_workspace_root(
                    self.runtime.workspace_root(),
                );
                if sess_ws != runtime_ws {
                    return Err(M31AError::internal(format!(
                        "refusing to resume session '{id}': session workspace '{}' != runtime workspace '{}'; rebind to the session workspace before execution",
                        sess.workspace_root.display(),
                        self.runtime.workspace_root().display()
                    )));
                }
                println!("Resumed session: {}", sess.id);
                self.current_session = Some(sess.clone());
                self.invalidate_engine();
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
                    match self.session_repo.clear_conversation(sess.id).await {
                        Ok(()) => {
                            println!(
                                "Durable conversation history cleared for session {}.",
                                sess.id
                            );
                        }
                        Err(e) => {
                            print_console_error(format!(
                                "Failed to clear durable conversation for session {}: {e}",
                                sess.id
                            ));
                        }
                    }
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
                    command_registry: Some(self.command_registry.as_ref()),
                };

                match self.command_registry.execute_line(&cmd_line, &ctx).await? {
                    CommandOutput::Info(text) => println!("{text}"),
                    CommandOutput::Error(err) => print_console_error(format!("Error: {err}")),
                    CommandOutput::ApplicationAction(act) => {
                        return Box::pin(self.handle_action(act)).await;
                    }
                }
            }

            ApplicationAction::UserCommandRequested { command, args } => {
                // Execute the global user command through the canonical runtime path.
                // This routes through PromptCatalog → PromptCompiler → ModelCaller
                // → PolicyGate → ApprovalCoordinator → ToolPipeline → Verification.
                println!("Executing user command '/{}'...", command);
                if let Err(e) = self
                    .runtime
                    .execute_user_command(&command, args, self.session_id().unwrap())
                    .await
                {
                    print_console_error(format!("User command '/{}' failed: {}", command, e));
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
                    command_registry: Some(self.command_registry.as_ref()),
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
                    // transition active non-terminal pre-execution lifecycle state to cancelled
                    let coordinator = self.runtime.create_pre_execution_coordinator();
                    let sid_str = sess.id.to_string();
                    if let Ok(Some(ls)) = coordinator
                        .lifecycle_repo()
                        .load_lifecycle_state(&sid_str)
                        .await
                    {
                        if !ls.stage.is_terminal() {
                            let _ = coordinator
                                .lifecycle_repo()
                                .save_validated_transition(
                                    &sid_str,
                                    ls.stage,
                                    crate::state_machine::lifecycle::LifecycleEvent::Cancel,
                                )
                                .await;
                        }
                    }

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
                            let sess_ws = crate::init::instance::canonicalize_workspace_root(
                                &sess.workspace_root,
                            );
                            let runtime_ws = crate::init::instance::canonicalize_workspace_root(
                                self.runtime.workspace_root(),
                            );
                            if sess_ws != runtime_ws {
                                eprintln!(
                                    "Refusing to resume session '{session_id}': session workspace '{}' != runtime workspace '{}'; rebind before execution.",
                                    sess.workspace_root.display(),
                                    self.runtime.workspace_root().display()
                                );
                            } else {
                                self.current_session = Some(sess.clone());
                                self.invalidate_engine();
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
                            print_console_error(
                                "Parameter editing is not supported by runtime policy; use approve or reject.",
                            );
                            return Err(M31AError::internal(
                                "Parameter editing of approval requests is unsupported by runtime policy",
                            ));
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

            ApplicationAction::ProviderChangeRequested { provider } => {
                match self.runtime.config().with_session_provider(&provider) {
                    Ok(new_cfg) => {
                        let new_runtime = (*self.runtime).clone().with_config(Arc::new(new_cfg));
                        self.rebind_runtime(Arc::new(new_runtime));
                        println!(
                            "Active provider switched to '{}' for this session.",
                            provider
                        );
                        if let Some(ref sess) = self.current_session
                            && let Ok(seq) = self.session_repo.next_sequence(sess.id).await
                        {
                            let turn = ConversationTurn::AssistantMessage {
                                id: uuid::Uuid::now_v7(),
                                sequence: seq,
                                content: format!(
                                    "Active provider switched to '{}' for this session.",
                                    provider
                                ),
                                created_at: chrono::Utc::now(),
                            };
                            let _ = self.session_repo.append_turn(sess.id, &turn).await;
                        }
                    }
                    Err(e) => {
                        print_console_error(format!("Failed to switch provider: {e}"));
                    }
                }
            }

            ApplicationAction::MissionPauseRequested { mission_id } => {
                if let Ok(mid) = mission_id.parse::<crate::ids::MissionId>() {
                    let _ = self
                        .runtime
                        .pause_mission(mid, "Paused by user request")
                        .await;
                }
            }

            ApplicationAction::MissionResumeRequested { mission_id } => {
                if let Ok(mid) = mission_id.parse::<crate::ids::MissionId>() {
                    let _ = self.runtime.resume_mission(mid).await;
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

            ApplicationAction::ConfigurationReconfigureRequested { config } => {
                let new_runtime = (*self.runtime).clone().with_config(config);
                self.rebind_runtime(Arc::new(new_runtime));
                println!(
                    "Runtime reconfigured with model '{}', provider '{}'.",
                    self.runtime.config().active_model,
                    self.runtime.config().active_provider,
                );
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
                match &governed_stage {
                    // no active lifecycle or terminal stages: start new governed pre-execution lifecycle
                    None
                    | Some(crate::state_machine::lifecycle::LifecycleStage::Completed)
                    | Some(crate::state_machine::lifecycle::LifecycleStage::Failed)
                    | Some(crate::state_machine::lifecycle::LifecycleStage::Cancelled)
                    | Some(crate::state_machine::lifecycle::LifecycleStage::Rejected)
                    | Some(crate::state_machine::lifecycle::LifecycleStage::Blocked) => {
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
                        // Planning-choice gate (single contract shared with TUI):
                        // ask before automatic project planning; persist the
                        // result on the session. Defaults this turn to direct
                        // execution (never silent project-doc generation).
                        {
                            let stored = self
                                .current_session
                                .as_ref()
                                .and_then(crate::interaction::session::SqliteSessionRepository::read_planning_choice);
                            match crate::runtime::PlanningChoice::resolve_for_request(
                                &full_prompt,
                                None,
                                stored,
                                false,
                            ) {
                                Some(crate::runtime::PlanningChoice::ProjectPlanning) => {
                                    if stored
                                        != Some(crate::runtime::PlanningChoice::ProjectPlanning)
                                    {
                                        let _ = self
                                            .session_repo
                                            .set_planning_choice(
                                                session.id,
                                                crate::runtime::PlanningChoice::ProjectPlanning,
                                            )
                                            .await;
                                    }
                                }
                                Some(crate::runtime::PlanningChoice::DirectExecution) => {
                                    if stored
                                        != Some(crate::runtime::PlanningChoice::DirectExecution)
                                    {
                                        let _ = self
                                            .session_repo
                                            .set_planning_choice(
                                                session.id,
                                                crate::runtime::PlanningChoice::DirectExecution,
                                            )
                                            .await;
                                    }
                                }
                                None => {
                                    // Suspend pending request and ask operator
                                    let pending_payload = serde_json::json!({
                                        "full_prompt": full_prompt,
                                        "raw_text": parsed.raw_text,
                                        "normalized_prompt": parsed.normalized_prompt(),
                                        "mentions": parsed.mentions,
                                        "request_id": parsed.request_id,
                                    });
                                    if let Some(sess) = self.current_session.as_mut() {
                                        if let Some(obj) = sess.metadata.as_object_mut() {
                                            obj.insert("planning_choice_pending".to_string(), pending_payload.clone());
                                        }
                                    }
                                    let mut meta = session.metadata.clone();
                                    if let Some(obj) = meta.as_object_mut() {
                                        obj.insert("planning_choice_pending".to_string(), pending_payload);
                                    }
                                    let meta_str = serde_json::to_string(&meta).unwrap_or_default();
                                    let now_str = chrono::Utc::now().to_rfc3339();
                                    let _ = sqlx::query("UPDATE sessions SET metadata = ?, updated_at = ? WHERE id = ?")
                                        .bind(&meta_str)
                                        .bind(&now_str)
                                        .bind(session.id.as_bytes().as_slice())
                                        .execute(self.runtime.pool())
                                        .await;

                                    println!(
                                        "\n[M31A] {}\n       Reply with /plan to generate project-planning documents under .m31a/project/, or /no-plan to work directly without them.\n",
                                        crate::runtime::PlanningChoice::prompt_question()
                                    );
                                    return Ok(false);
                                }
                            }
                        }
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
                    // active lifecycle awaiting operator information (questions answered via text)
                    Some(crate::state_machine::lifecycle::LifecycleStage::AwaitingInformation)
                    | Some(crate::state_machine::lifecycle::LifecycleStage::IntentActive) => {
                        let questions = coordinator
                            .lifecycle_repo()
                            .load_discovery_questions(&sid_str)
                            .await
                            .unwrap_or_default();
                        let pending_questions: Vec<_> = questions
                            .into_iter()
                            .filter(|q| q.answer.is_none())
                            .collect();

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

                        if let Some(q) = pending_questions.first() {
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
                                    return Err(crate::error::M31AError::Internal(anyhow::anyhow!(e)));
                                }
                            }
                        } else {
                            // no pending questions: update existing intent with new clarification text
                            match coordinator
                                .init_intent(&sid_str, &full_prompt, "operator")
                                .await
                            {
                                Ok(resp) => {
                                    self.handle_lifecycle_response(resp).await?;
                                    return Ok(false);
                                }
                                Err(e) => {
                                    print_console_error(format!("[M31A] Error updating intent: {e}"));
                                    return Err(crate::error::M31AError::Internal(anyhow::anyhow!(e)));
                                }
                            }
                        }
                    }
                    Some(crate::state_machine::lifecycle::LifecycleStage::PlanReview) => {
                        let trimmed = parsed.normalized_prompt().trim().to_ascii_lowercase();
                        if trimmed == "1" || trimmed == "accept" || trimmed == "accept plan" {
                            let (rev, hash) = if let Ok(Some(dec)) = self.session_repo.get_pending_decision(session.id).await {
                                if let crate::interaction::decision::DecisionKind::PlanReview { plan_revision, ref content_hash, .. } = dec.kind {
                                    (Some(plan_revision), content_hash.clone())
                                } else {
                                    (None, None)
                                }
                            } else {
                                (None, None)
                            };
                            return Box::pin(self.handle_action(ApplicationAction::PlanAcceptRequested {
                                session_id: Some(sid_str),
                                revision: rev,
                                content_hash: hash,
                            })).await;
                        } else if trimmed == "2" || trimmed.starts_with("revise") {
                            let feedback = if trimmed == "2" {
                                "Operator requested revision".to_string()
                            } else {
                                parsed.normalized_prompt()
                                    .trim()
                                    .strip_prefix("revise")
                                    .unwrap_or(&parsed.normalized_prompt())
                                    .trim()
                                    .to_string()
                            };
                            return Box::pin(self.handle_action(ApplicationAction::PlanRevisionRequested {
                                session_id: Some(sid_str),
                                feedback,
                            })).await;
                        } else if trimmed == "3" || trimmed == "regen" || trimmed == "regenerate" {
                            return Box::pin(self.handle_action(ApplicationAction::PlanRegenerateRequested {
                                session_id: Some(sid_str),
                            })).await;
                        } else if trimmed == "4" || trimmed == "reject" || trimmed.starts_with("reject") {
                            let reason = if trimmed == "4" {
                                "Rejected by operator".to_string()
                            } else {
                                parsed.normalized_prompt()
                                    .trim()
                                    .strip_prefix("reject")
                                    .unwrap_or("Rejected by operator")
                                    .trim()
                                    .to_string()
                            };
                            return Box::pin(self.handle_action(ApplicationAction::PlanRejectRequested {
                                session_id: Some(sid_str),
                                reason,
                            })).await;
                        } else {
                            println!(
                                "\n[M31A] Candidate plan review is pending. Enter 1 (Accept), 2 (Revise), 3 (Regenerate), or 4 (Reject)."
                            );
                            return Ok(false);
                        }
                    }
                    Some(crate::state_machine::lifecycle::LifecycleStage::TasksReview) => {
                        let trimmed = parsed.normalized_prompt().trim().to_ascii_lowercase();
                        if trimmed == "1" || trimmed == "accept" || trimmed == "accept tasks" {
                            let (rev, hash) = if let Ok(Some(dec)) = self.session_repo.get_pending_decision(session.id).await {
                                if let crate::interaction::decision::DecisionKind::TaskReview { task_revision, ref content_hash, .. } = dec.kind {
                                    (Some(task_revision), content_hash.clone())
                                } else {
                                    (None, None)
                                }
                            } else {
                                (None, None)
                            };
                            return Box::pin(self.handle_action(ApplicationAction::TasksAcceptRequested {
                                session_id: Some(sid_str),
                                revision: rev,
                                content_hash: hash,
                            })).await;
                        } else if trimmed == "2" || trimmed == "regen" || trimmed.starts_with("regen") {
                            let feedback = if trimmed == "2" || trimmed == "regen" {
                                None
                            } else {
                                Some(
                                    parsed.normalized_prompt()
                                        .trim()
                                        .strip_prefix("regen")
                                        .unwrap_or("")
                                        .trim()
                                        .to_string(),
                                )
                            };
                            return Box::pin(self.handle_action(ApplicationAction::TaskRegenerateRequested {
                                session_id: Some(sid_str),
                                feedback,
                            })).await;
                        } else if trimmed == "3" || trimmed == "reject" {
                            println!("\nTasks rejected by operator.\n");
                            return Ok(false);
                        } else {
                            println!(
                                "\n[M31A] Candidate task review is pending. Enter 1 (Accept), 2 (Regenerate), or 3 (Reject)."
                            );
                            return Ok(false);
                        }
                    }
                    Some(crate::state_machine::lifecycle::LifecycleStage::ExecutionAwaitingAuthorization) => {
                        let trimmed = parsed.normalized_prompt().trim().to_ascii_lowercase();
                        if trimmed == "1" || trimmed == "yes" || trimmed == "authorize" {
                            return Box::pin(self.handle_action(ApplicationAction::ExecutionAuthorizationSubmitted {
                                session_id: Some(sid_str),
                                decision: true,
                                reason: None,
                            })).await;
                        } else if trimmed == "2" || trimmed == "no" || trimmed == "reject" {
                            return Box::pin(self.handle_action(ApplicationAction::ExecutionAuthorizationSubmitted {
                                session_id: Some(sid_str),
                                decision: false,
                                reason: Some("Rejected by operator".to_string()),
                            })).await;
                        } else {
                            println!(
                                "\n[M31A] Execution authorization is pending. Enter 1 (Authorize) or 2 (Reject)."
                            );
                            return Ok(false);
                        }
                    }
                    Some(crate::state_machine::lifecycle::LifecycleStage::PlanRevision)
                    | Some(crate::state_machine::lifecycle::LifecycleStage::PlanDraft)
                    | Some(crate::state_machine::lifecycle::LifecycleStage::PlanAccepted)
                    | Some(crate::state_machine::lifecycle::LifecycleStage::TasksDraft)
                    | Some(crate::state_machine::lifecycle::LifecycleStage::TasksRevision)
                    | Some(crate::state_machine::lifecycle::LifecycleStage::TasksAccepted)
                    | Some(crate::state_machine::lifecycle::LifecycleStage::ExecutionAuthorized) => {
                        println!(
                            "\n[M31A] Active governed lifecycle is in stage '{:?}'.",
                            governed_stage.as_ref().unwrap()
                        );
                        println!(
                            "       Select an option from the prompt to advance. Free-text input is not accepted in this stage."
                        );
                        return Ok(false);
                    }
                    // execution stage: reject unmanaged free-text during active mission execution
                    Some(crate::state_machine::lifecycle::LifecycleStage::Executing) => {
                        println!(
                            "\n[M31A] Session is currently executing. Use /cancel to abort or wait for completion."
                        );
                        return Ok(false);
                    }
                }
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
                match self.runtime.get_genesis_state(".m31a/project").await {
                    Ok(Some(state)) => {
                        println!("\nCurrent Planning State:");
                        println!("  Lifecycle: {:?}", state.lifecycle_state);
                        println!("  Status: {}", state.approval_status);
                        println!("  Current Phase: {:?}", state.current_phase);
                    }
                    Ok(None) => {
                        println!("No Project Genesis state found in .m31a/project/STATE.md");
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
            | ApplicationAction::PlanningChoiceSubmitted { .. }
            | ApplicationAction::ExecutionAuthorizationSubmitted { .. } => {
                // Planning-choice answers persist on the session and never
                // enter the plan/task/authorization coordinator.
                if let ApplicationAction::PlanningChoiceSubmitted {
                    session_id,
                    generate_project_docs,
                } = &action
                {
                    let choice = if *generate_project_docs {
                        crate::runtime::PlanningChoice::ProjectPlanning
                    } else {
                        crate::runtime::PlanningChoice::DirectExecution
                    };
                    let sid = session_id
                        .clone()
                        .or_else(|| self.current_session.as_ref().map(|s| s.id.to_string()));
                    if let Some(sid_str) = sid {
                        if let Ok(sid_parsed) = sid_str.parse::<crate::ids::SessionId>() {
                            match self
                                .session_repo
                                .set_planning_choice(sid_parsed, choice)
                                .await
                            {
                                Ok(()) => {
                                    let pending = self.current_session.as_mut().and_then(|sess| {
                                        sess.metadata["planning_choice"] =
                                            serde_json::json!(choice);
                                        sess.metadata
                                            .as_object_mut()
                                            .and_then(|obj| obj.remove("planning_choice_pending"))
                                    });
                                    println!(
                                        "\n[M31A] Planning choice recorded: {}.\n",
                                        if *generate_project_docs {
                                            "project-planning documents under .m31a/project/"
                                        } else {
                                            "direct execution without project-level documents"
                                        }
                                    );
                                    if let Some(pending_req) = pending {
                                        let resume_prompt = pending_req
                                            .get("full_prompt")
                                            .and_then(|v| v.as_str())
                                            .or_else(|| {
                                                pending_req
                                                    .get("normalized_prompt")
                                                    .and_then(|v| v.as_str())
                                            })
                                            .unwrap_or_default()
                                            .to_string();
                                        println!(
                                            "[M31A] Resuming suspended request: {resume_prompt}\n"
                                        );
                                        let workspace_root =
                                            self.runtime.workspace_root().to_path_buf();
                                        if *generate_project_docs {
                                            let gen_req =
                                                crate::workflow::genesis::GenesisRequest::new(
                                                    resume_prompt.clone(),
                                                    &workspace_root,
                                                )
                                                .with_mode(
                                                    crate::workflow::genesis::GenesisMode::AutoDetect,
                                                );
                                            match self.runtime.run_genesis(&gen_req).await {
                                                Ok(outcome) => {
                                                    println!(
                                                        "[M31A] Project Genesis planning complete for '{}'. Roadmap lowered to workflow '{}' with {} phases.",
                                                        outcome.charter.project_name,
                                                        outcome.workflow_definition.id,
                                                        outcome.planning.roadmap.phases.len()
                                                    );
                                                }
                                                Err(e) => {
                                                    print_console_error(format!(
                                                        "Genesis error: {e}"
                                                    ));
                                                }
                                            }
                                        } else {
                                            let coordinator =
                                                self.runtime.create_pre_execution_coordinator();
                                            match coordinator
                                                .init_intent(&sid_str, &resume_prompt, "operator")
                                                .await
                                            {
                                                Ok(resp) => {
                                                    self.handle_lifecycle_response(resp).await?;
                                                }
                                                Err(e) => {
                                                    print_console_error(format!(
                                                        "Failed to resume intent: {e}"
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => print_console_error(format!(
                                    "Failed to persist planning choice: {e}"
                                )),
                            }
                        }
                    }
                    return Ok(false);
                }
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
                    ApplicationAction::PlanAcceptRequested {
                        session_id,
                        revision,
                        content_hash,
                    } => {
                        let sid = session_id
                            .or_else(|| self.current_session.as_ref().map(|s| s.id.to_string()));
                        ApplicationAction::PlanAcceptRequested {
                            session_id: sid,
                            revision,
                            content_hash,
                        }
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
                    ApplicationAction::TasksAcceptRequested {
                        session_id,
                        revision,
                        content_hash,
                    } => {
                        let sid = session_id
                            .or_else(|| self.current_session.as_ref().map(|s| s.id.to_string()));
                        ApplicationAction::TasksAcceptRequested {
                            session_id: sid,
                            revision,
                            content_hash,
                        }
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
                    ApplicationAction::PlanningChoiceSubmitted {
                        session_id,
                        generate_project_docs,
                    } => {
                        let sid = session_id
                            .or_else(|| self.current_session.as_ref().map(|s| s.id.to_string()));
                        ApplicationAction::PlanningChoiceSubmitted {
                            session_id: sid,
                            generate_project_docs,
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

            ApplicationAction::SettingsRequested { category } => match category {
                Some(c) => println!(
                    "Settings editor is a TUI surface. Run `m31a tui` and invoke `/settings {c}` to edit the canonical configuration."
                ),
                None => println!(
                    "Settings editor is a TUI surface. Run `m31a tui` and invoke `/settings` to edit the canonical configuration."
                ),
            },

            ApplicationAction::DoctorRequested { category } => {
                let runner = crate::cli::doctor::DoctorRunner::with_default_probes();
                let report = runner.run(category.as_deref()).await;
                println!("{}", report.format_text());
            }

            ApplicationAction::DecisionResponseSubmitted(resp) => {
                let sess_id = self.current_session.as_ref().map(|s| s.id).ok_or_else(|| {
                    crate::error::M31AError::invalid_input(
                        "No active session to process decision response",
                    )
                })?;

                // 1. Check duplicate/replay resistance
                if self
                    .session_repo
                    .is_decision_completed(sess_id, &resp.decision_id)
                    .await
                    .map_err(|e| {
                        crate::error::M31AError::internal(format!(
                            "Failed to check completed decision: {e}"
                        ))
                    })?
                {
                    return Err(crate::error::M31AError::invalid_input(format!(
                        "Decision '{}' has already been completed",
                        resp.decision_id
                    )));
                }

                // 2. Load authoritative pending decision
                let pending = self
                    .session_repo
                    .get_pending_decision(sess_id)
                    .await
                    .map_err(|e| {
                        crate::error::M31AError::internal(format!(
                            "Failed to load pending decision: {e}"
                        ))
                    })?
                    .ok_or_else(|| {
                        crate::error::M31AError::invalid_input(format!(
                            "No pending decision found for ID '{}'",
                            resp.decision_id
                        ))
                    })?;

                // 3. Verify decision ID compatibility (exact match, or stripped legacy prefix)
                let matches_id = pending.id == resp.decision_id
                    || resp.decision_id.strip_prefix("discovery_") == Some(&pending.id)
                    || pending.id.strip_prefix("discovery_") == Some(&resp.decision_id);
                if !matches_id {
                    return Err(crate::error::M31AError::invalid_input(format!(
                        "Decision ID mismatch: active pending decision is '{}', received '{}'",
                        pending.id, resp.decision_id
                    )));
                }

                // 4. Validate response against authoritative decision rules
                pending.validate_response(&resp).map_err(|e| {
                    crate::error::M31AError::invalid_input(format!(
                        "Decision response validation failed: {e}"
                    ))
                })?;

                // 5. Route based strictly on the authoritative pending decision kind!
                return match &pending.kind {
                    crate::interaction::decision::DecisionKind::PlanningChoice => {
                        if resp.cancelled || resp.selected_option_id.as_deref() == Some("cancel") {
                            self.session_repo
                                .finalize_decision(sess_id, &resp.decision_id, true)
                                .await?;
                            println!("\nPlanning request cancelled by operator.\n");
                            return Ok(true);
                        }
                        let generate_project_docs = match resp.selected_option_id.as_deref() {
                            Some("plan") => true,
                            Some("direct") => false,
                            other => {
                                return Err(crate::error::M31AError::invalid_input(format!(
                                    "Invalid option {:?} for planning choice. Expected 'plan', 'direct', or 'cancel'.",
                                    other
                                )));
                            }
                        };
                        let choice = if generate_project_docs {
                            crate::runtime::PlanningChoice::ProjectPlanning
                        } else {
                            crate::runtime::PlanningChoice::DirectExecution
                        };
                        self.session_repo
                            .set_planning_choice(sess_id, choice)
                            .await?;
                        if let Some(ref mut sess) = self.current_session {
                            sess.metadata["planning_choice"] = serde_json::json!(choice);
                        }
                        self.session_repo
                            .finalize_decision(sess_id, &resp.decision_id, false)
                            .await?;
                        Ok(true)
                    }

                    crate::interaction::decision::DecisionKind::PlanReview {
                        plan_revision,
                        content_hash,
                    } => {
                        let coordinator = self.runtime.create_pre_execution_coordinator();
                        if resp.cancelled || resp.selected_option_id.as_deref() == Some("reject") {
                            let reason = resp
                                .custom_answer
                                .clone()
                                .unwrap_or_else(|| "Plan rejected by operator".to_string());
                            let act = ApplicationAction::PlanRejectRequested {
                                session_id: Some(resp.session_id.clone()),
                                reason,
                            };
                            match coordinator.handle_action(act, "operator").await {
                                Ok(lifecycle_resp) => {
                                    self.handle_lifecycle_response(lifecycle_resp).await?;
                                }
                                Err(err) => return Err(crate::error::M31AError::validation(err)),
                            }
                            self.session_repo
                                .finalize_decision(sess_id, &resp.decision_id, false)
                                .await?;
                            return Ok(true);
                        }
                        let act = match resp.selected_option_id.as_deref() {
                            Some("accept") => ApplicationAction::PlanAcceptRequested {
                                session_id: Some(resp.session_id.clone()),
                                revision: Some(*plan_revision),
                                content_hash: content_hash.clone(),
                            },
                            Some("revise") => {
                                let feedback = resp
                                    .custom_answer
                                    .clone()
                                    .unwrap_or_else(|| "Operator requested revision".to_string());
                                ApplicationAction::PlanRevisionRequested {
                                    session_id: Some(resp.session_id.clone()),
                                    feedback,
                                }
                            }
                            Some("regen") | Some("regenerate") => {
                                ApplicationAction::PlanRegenerateRequested {
                                    session_id: Some(resp.session_id.clone()),
                                }
                            }
                            other => {
                                return Err(crate::error::M31AError::invalid_input(format!(
                                    "Invalid option {:?} for plan review. Expected 'accept', 'revise', 'regen', or 'reject'.",
                                    other
                                )));
                            }
                        };
                        match coordinator.handle_action(act, "operator").await {
                            Ok(lifecycle_resp) => {
                                self.handle_lifecycle_response(lifecycle_resp).await?;
                            }
                            Err(err) => return Err(crate::error::M31AError::validation(err)),
                        }
                        self.session_repo
                            .finalize_decision(sess_id, &resp.decision_id, false)
                            .await?;
                        Ok(true)
                    }

                    crate::interaction::decision::DecisionKind::TaskReview {
                        plan_revision: _,
                        task_revision,
                        content_hash,
                    } => {
                        let coordinator = self.runtime.create_pre_execution_coordinator();
                        if resp.cancelled || resp.selected_option_id.as_deref() == Some("reject") {
                            let act = ApplicationAction::TaskRemoveRequested {
                                session_id: Some(resp.session_id.clone()),
                                task_id: crate::ids::TaskId::new().to_string(),
                            };
                            let _ = coordinator.handle_action(act, "operator").await;
                            self.session_repo
                                .finalize_decision(sess_id, &resp.decision_id, false)
                                .await?;
                            println!("\nTasks rejected by operator.\n");
                            return Ok(true);
                        }
                        let act = match resp.selected_option_id.as_deref() {
                            Some("accept") => ApplicationAction::TasksAcceptRequested {
                                session_id: Some(resp.session_id.clone()),
                                revision: Some(*task_revision),
                                content_hash: content_hash.clone(),
                            },
                            Some("regen") => {
                                let feedback = resp.custom_answer.clone();
                                ApplicationAction::TaskRegenerateRequested {
                                    session_id: Some(resp.session_id.clone()),
                                    feedback,
                                }
                            }
                            other => {
                                return Err(crate::error::M31AError::invalid_input(format!(
                                    "Invalid option {:?} for tasks review. Expected 'accept', 'regen', or 'reject'.",
                                    other
                                )));
                            }
                        };
                        match coordinator.handle_action(act, "operator").await {
                            Ok(lifecycle_resp) => {
                                self.handle_lifecycle_response(lifecycle_resp).await?;
                            }
                            Err(err) => return Err(crate::error::M31AError::validation(err)),
                        }
                        self.session_repo
                            .finalize_decision(sess_id, &resp.decision_id, false)
                            .await?;
                        Ok(true)
                    }

                    crate::interaction::decision::DecisionKind::ExecutionAuthorization {
                        ..
                    } => {
                        let coordinator = self.runtime.create_pre_execution_coordinator();
                        let is_auth = !resp.cancelled
                            && resp.selected_option_id.as_deref() == Some("authorize");
                        let reason = if !is_auth {
                            Some(
                                resp.custom_answer
                                    .clone()
                                    .unwrap_or_else(|| "Rejected by operator".to_string()),
                            )
                        } else {
                            None
                        };
                        let act = ApplicationAction::ExecutionAuthorizationSubmitted {
                            session_id: Some(resp.session_id.clone()),
                            decision: is_auth,
                            reason,
                        };
                        match coordinator.handle_action(act, "operator").await {
                            Ok(lifecycle_resp) => {
                                self.handle_lifecycle_response(lifecycle_resp).await?;
                            }
                            Err(err) => return Err(crate::error::M31AError::validation(err)),
                        }
                        self.session_repo
                            .finalize_decision(sess_id, &resp.decision_id, false)
                            .await?;
                        Ok(true)
                    }

                    crate::interaction::decision::DecisionKind::Clarification {
                        question_id,
                        allow_freeform,
                    } => {
                        if resp.cancelled {
                            let has_waiter = crate::interaction::decision::DecisionWaiterRegistry::global()
                                .has_waiter(&resp.decision_id)
                                .await
                                || crate::interaction::decision::DecisionWaiterRegistry::global()
                                    .has_waiter(question_id.as_str())
                                    .await;
                            if has_waiter {
                                crate::interaction::decision::DecisionWaiterRegistry::global()
                                    .resume(
                                        &resp.decision_id,
                                        crate::interaction::decision::DecisionResumeOutcome::Cancelled(
                                            "Cancelled by operator".to_string(),
                                        ),
                                    )
                                    .await;
                                crate::interaction::decision::DecisionWaiterRegistry::global()
                                    .resume(
                                        question_id.as_str(),
                                        crate::interaction::decision::DecisionResumeOutcome::Cancelled(
                                            "Cancelled by operator".to_string(),
                                        ),
                                    )
                                    .await;
                            }
                            self.session_repo
                                .finalize_decision(sess_id, &resp.decision_id, false)
                                .await?;
                            println!("\nClarification cancelled by operator.\n");
                            return Ok(true);
                        }

                        // Resolve canonical answer
                        let answer = if let Some(ref custom) = resp.custom_answer {
                            let trimmed = custom.trim().to_string();
                            if !allow_freeform {
                                let matches = pending.options.iter().any(|o| {
                                    let opt_val = o.value.as_deref().unwrap_or(&o.label);
                                    opt_val.trim().eq_ignore_ascii_case(&trimmed)
                                        || o.label.trim().eq_ignore_ascii_case(&trimmed)
                                });
                                if !matches {
                                    return Err(crate::error::M31AError::invalid_input(format!(
                                        "Custom answer '{}' does not match permitted options for question '{}'",
                                        trimmed, question_id
                                    )));
                                }
                            }
                            trimmed
                        } else if let Some(ref opt_id) = resp.selected_option_id {
                            pending
                                .resolve_option_value(opt_id)
                                .unwrap_or(opt_id)
                                .to_string()
                        } else {
                            return Err(crate::error::M31AError::invalid_input(
                                "Clarification requires either an option selection or a custom answer",
                            ));
                        };

                        let has_waiter =
                            crate::interaction::decision::DecisionWaiterRegistry::global()
                                .has_waiter(&resp.decision_id)
                                .await
                                || crate::interaction::decision::DecisionWaiterRegistry::global()
                                    .has_waiter(question_id.as_str())
                                    .await;

                        if has_waiter {
                            crate::interaction::decision::DecisionWaiterRegistry::global()
                                .resume(
                                    &resp.decision_id,
                                    crate::interaction::decision::DecisionResumeOutcome::Answer(
                                        answer.clone(),
                                    ),
                                )
                                .await;
                            crate::interaction::decision::DecisionWaiterRegistry::global()
                                .resume(
                                    question_id.as_str(),
                                    crate::interaction::decision::DecisionResumeOutcome::Answer(
                                        answer.clone(),
                                    ),
                                )
                                .await;

                            self.session_repo
                                .finalize_decision(sess_id, &resp.decision_id, false)
                                .await?;
                            println!("\nClarification answered: {answer}\n");
                            Ok(true)
                        } else {
                            let coordinator = self.runtime.create_pre_execution_coordinator();
                            let act = ApplicationAction::QuestionAnswerSubmitted {
                                session_id: Some(resp.session_id.clone()),
                                question_id: question_id.clone(),
                                answer,
                            };
                            match coordinator.handle_action(act, "operator").await {
                                Ok(lifecycle_resp) => {
                                    self.handle_lifecycle_response(lifecycle_resp).await?;
                                }
                                Err(err) => return Err(crate::error::M31AError::validation(err)),
                            }
                            self.session_repo
                                .finalize_decision(sess_id, &resp.decision_id, false)
                                .await?;
                            Ok(true)
                        }
                    }

                    crate::interaction::decision::DecisionKind::PermissionApproval { .. } => {
                        let decision = if resp.cancelled
                            || resp.selected_option_id.as_deref() == Some("reject")
                        {
                            crate::tui::approval::ApprovalDecision::Reject
                        } else if resp.selected_option_id.as_deref() == Some("approve_always") {
                            crate::tui::approval::ApprovalDecision::ApproveAlways
                        } else {
                            crate::tui::approval::ApprovalDecision::ApproveOnce
                        };

                        if let Ok(req_uuid) = uuid::Uuid::parse_str(&resp.decision_id) {
                            let action = match decision {
                                crate::tui::approval::ApprovalDecision::ApproveOnce => {
                                    crate::policy::approval::ApprovalAction::AllowOnce
                                }
                                crate::tui::approval::ApprovalDecision::ApproveAlways => {
                                    crate::policy::approval::ApprovalAction::AllowForSession
                                }
                                _ => crate::policy::approval::ApprovalAction::Deny {
                                    reason: "operator rejected".to_string(),
                                },
                            };
                            self.runtime
                                .approval_coordinator()
                                .resolve_request(req_uuid.into(), action, "operator")
                                .await
                                .map_err(|e| {
                                    crate::error::M31AError::internal(format!(
                                        "approval resolution failed: {e}"
                                    ))
                                })?;
                        }
                        self.session_repo
                            .finalize_decision(sess_id, &resp.decision_id, false)
                            .await?;
                        Ok(true)
                    }

                    other => Err(crate::error::M31AError::invalid_input(format!(
                        "Unhandled decision kind '{:?}'",
                        other
                    ))),
                };
            }
            ApplicationAction::CheckpointRestoreRequested { checkpoint_id } => {
                let cid = checkpoint_id
                    .parse::<crate::ids::CheckpointId>()
                    .map_err(|e| {
                        crate::error::M31AError::invalid_input(format!(
                            "Invalid checkpoint ID: {e}"
                        ))
                    })?;
                let res = self
                    .runtime
                    .checkpoint_manager()
                    .restore_checkpoint(cid, &self.workspace_root)
                    .await
                    .map_err(|e| {
                        crate::error::M31AError::internal(format!("Checkpoint restore failed: {e}"))
                    })?;
                println!(
                    "Checkpoint {} restored (mission {})",
                    checkpoint_id, res.mission_id
                );
                return Ok(true);
            }
            ApplicationAction::QuarantineRefreshRequested { .. } => {
                let quarantined = self
                    .runtime
                    .artifact_service()
                    .query_quarantine_records()
                    .await
                    .map_err(|e| {
                        crate::error::M31AError::internal(format!(
                            "Failed to query quarantined artifacts: {e}"
                        ))
                    })?;
                println!("Quarantine status: {} items quarantined", quarantined.len());
                for item in &quarantined {
                    println!("  • {} - {}", item.name, item.reason);
                }
                return Ok(true);
            }
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
                if let Some(first_q) = questions.first() {
                    let dec = crate::interaction::decision::InteractiveDecision::discovery_question(
                        session_id.clone(),
                        first_q,
                    );
                    if let Some(ref sess) = self.current_session {
                        self.session_repo
                            .set_pending_decision(sess.id, &dec)
                            .await
                            .map_err(|e| {
                                M31AError::PersistenceError(format!(
                                    "Failed to persist pending decision: {e}"
                                ))
                            })?;
                    }
                }
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
                let content_hash =
                    crate::planning::PlanRevision::compute_content_hash(&revision.content);
                let dec = crate::interaction::decision::InteractiveDecision::plan_review(
                    revision.session_id.clone(),
                    revision.revision,
                    revision.content.objective.clone(),
                    revision.content.tasks.len(),
                    Some(content_hash),
                );
                if let Some(ref sess) = self.current_session {
                    self.session_repo
                        .set_pending_decision(sess.id, &dec)
                        .await
                        .map_err(|e| {
                            M31AError::PersistenceError(format!(
                                "Failed to persist pending decision: {e}"
                            ))
                        })?;
                }
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
                println!(" Choose an option (enter number or option name):");
                println!("   1. Accept plan and proceed");
                println!("   2. Request revision with feedback (e.g. 2 or revise <notes>)");
                println!("   3. Regenerate candidate plan");
                println!("   4. Reject candidate plan");
                println!("========================================================\n");
                self.prompt_state = SessionPromptState::Idle;
            }
            crate::planning::review::PreExecutionResponse::TasksForReview {
                session_id: _,
                revision,
            } => {
                let content_hash =
                    crate::planning::TaskRevision::compute_tasks_hash(&revision.tasks);
                let dec = crate::interaction::decision::InteractiveDecision::tasks_review(
                    revision.session_id.clone(),
                    revision.plan_revision,
                    revision.revision,
                    revision.tasks.len(),
                    Some(content_hash),
                );
                if let Some(ref sess) = self.current_session {
                    self.session_repo
                        .set_pending_decision(sess.id, &dec)
                        .await
                        .map_err(|e| {
                            M31AError::PersistenceError(format!(
                                "Failed to persist pending decision: {e}"
                            ))
                        })?;
                }
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
                println!(" Choose an option (enter number or option name):");
                println!("   1. Accept tasks and proceed to authorization");
                println!("   2. Regenerate task graph (e.g. 2 or regen [notes])");
                println!("   3. Reject candidate tasks");
                println!("========================================================\n");
                self.prompt_state = SessionPromptState::Idle;
            }
            crate::planning::review::PreExecutionResponse::AuthorizationRequested {
                session_id,
                plan_revision,
                task_revision,
                message,
            } => {
                let dec =
                    crate::interaction::decision::InteractiveDecision::execution_authorization(
                        session_id.clone(),
                        plan_revision,
                        task_revision,
                        message.clone(),
                    );
                if let Some(ref sess) = self.current_session {
                    self.session_repo
                        .set_pending_decision(sess.id, &dec)
                        .await
                        .map_err(|e| {
                            M31AError::PersistenceError(format!(
                                "Failed to persist pending decision: {e}"
                            ))
                        })?;
                }
                println!("\n========================================================");
                println!(" [EXECUTION AUTHORIZATION REQUIRED]");
                println!(" Session: {session_id}");
                println!(" Bound to: Plan Rev {plan_revision}, Task Rev {task_revision}");
                println!(" Message: {message}");
                println!("--------------------------------------------------------");
                println!(
                    " Explicit human authorization is required before any workspace changes occur."
                );
                println!(" Choose an option (enter number or option name):");
                println!("   1. Authorize execution");
                println!("   2. Reject authorization");
                println!("========================================================\n");
                self.prompt_state = SessionPromptState::AwaitingApproval;
            }
            crate::planning::review::PreExecutionResponse::ReadyToExecute {
                session_id: ref sid,
                mut plan,
                tasks,
                ref authorization,
            } => {
                if let Some(ref sess) = self.current_session {
                    let _ = self.session_repo.clear_pending_decision(sess.id).await;
                }
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
                let session_id = match self.current_session.as_ref() {
                    Some(s) => s.id,
                    None => {
                        let sid = uuid::Uuid::parse_str(sid)
                            .unwrap_or_else(|_| *crate::ids::SessionId::new().as_uuid());
                        crate::ids::SessionId::from(sid)
                    }
                };

                println!("Executing authorized tasks via production AutonomyController...");
                match self
                    .runtime
                    .start_authorized_execution(session_id, &plan, authorization.clone())
                    .await
                {
                    Ok(summary) => {
                        println!(
                            "\n[Execution Finished] Execution run '{}' status: {} (tasks completed: {})",
                            summary.mission_id, summary.status, summary.tasks_completed
                        );
                        if let Ok(Some(refreshed)) = self.session_repo.get_session(session_id).await
                        {
                            self.current_session = Some(refreshed);
                        }
                    }
                    Err(e) => {
                        print_console_error(format!("\n[Execution Error] Execution failed: {e}"));
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
                if let Some(ref sess) = self.current_session {
                    let _ = self.session_repo.clear_pending_decision(sess.id).await;
                }
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
                    AgentTurnOutcome::TurnLimitReached { reason }
                    | AgentTurnOutcome::BudgetExhausted { reason } => {
                        println!("\n=== TURN LIMIT REACHED ===");
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
