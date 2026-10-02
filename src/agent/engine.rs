//! Canonical Continuous Interactive Coding-Agent Engine.
//!
//! "The model proposes. The runtime decides."
//!
//! Provides the primary continuous interactive coding loop:
//! ```text
//! User Intent (minimal)
//!   ↓
//! AgentEngine (run_continuous)
//!   ↓
//! IntentState (inject into system prompt)
//!   ↓
//! Model Turn (understand intent via tools)
//!   ↓
//! Tool Calls / AskUser / Complete
//!   ↓
//! Runtime Authorization (PolicyGate & Approval)
//!   ↓
//! Tool Execution (ToolPipelineRunner)
//!   ↓
//! Tool Results (Structured Evidence)
//!   ↓
//! IntentState update (persist enrichment)
//!   ↓
//! Next Agent Turn (Fresh Context with IntentState)
//!   ↓
//! Model ...
//! ```
//!
//! `IntentState` is integrated as a first-class field on `AgentEngine`.
//! It is not a separate engine. The model proposes intent enrichment via tools;
//! the runtime validates, persists, and injects the state into each turn's context.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::agent::adaptive::{
    AdaptiveBudget, AdaptiveReplanOutcome, AdaptiveReplanRequest, AssumptionInvalidationReport,
    DiagnosticEvidence, ExecutionFailureCategory, ExecutionObservation, StallDetector,
    StallEvaluation, SubagentFailureEvidence,
};
use crate::agent::intent::{
    AssumptionInvalidation, FormedTask, FormedTaskStatus, IntentError, IntentState, IntentUnknown,
    SteeringConstraint, TaskShape, UnknownResolution,
};
use crate::agent::intent_repository::SqliteIntentRepository;
use crate::agent::model_policy::ModelCaller;
use crate::agent::runner::ActionRequest;
use crate::error::M31AError;
use crate::events::bus::EventBus;
use crate::ids::{MissionId, SessionId, TaskId};
use crate::interaction::session::{ConversationTurn, SqliteSessionRepository};
use crate::kernel::seams::context::{CompiledContext, ContextCompiler};
use crate::kernel::seams::policy::{PolicyDecision, PolicyEvaluationRequest, PolicyGate};
use crate::kernel::seams::recovery::FailureClassification;
use crate::model::types::{ChatMessage, ModelProposal, ModelToolCall, UserOption};
use crate::pipeline::runner::ToolPipelineRunner;
use crate::policy::approval::ApprovalCoordinator;
use crate::prompt::catalog::PromptCatalog;
use crate::state::intake::AutonomyMode;
use crate::tools::definition::ToolExecutionContext;
use crate::tools::registry::ToolRegistry;
use crate::verification::gate::EvidenceCompletionGate;

/// Structured canonical outcome of a single tool execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructuredToolResult {
    pub call_id: String,
    pub tool_name: String,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub policy_decision: Option<String>,
    pub approval_id: Option<String>,
    pub artifacts_created: Vec<String>,
    /// Structured diagnostic evidence produced by tool execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<DiagnosticEvidence>,
}

/// Durable, distinct lifecycle states of the continuous AgentEngine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AgentEngineState {
    /// Agent is idle, awaiting initial prompt or next task.
    Idle,
    /// Agent is actively reasoning or executing tools.
    Running,
    /// Agent needs semantic user input/decision before proceeding (first-class pause).
    WaitingForUser {
        question: String,
        options: Vec<UserOption>,
    },
    /// Agent proposed an action that requires explicit operator policy approval.
    WaitingForApproval {
        request_id: String,
        tool_name: String,
        parameters: serde_json::Value,
    },
    /// Task has completed with verified runtime evidence.
    Completed { final_summary: String },
    /// Task encountered an unrecoverable failure.
    Failed { error: String },
    /// Execution was cancelled by operator or token.
    Cancelled { reason: String },
    /// Turn budget or time ceiling was reached.
    BudgetExhausted { reason: String },
}

/// Granular outcome of a single step in the continuous agent loop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AgentTurnOutcome {
    /// Model emitted conversational deliberation/narration during active autonomous execution (loop continues).
    AssistantCommentary { content: String },
    /// Model emitted conversational narrative yielding control to the operator (state becomes Idle, not Completed).
    AssistantText { content: String },
    /// Model proposed and executed one or more tool calls, returning structured evidence.
    ToolResults { results: Vec<StructuredToolResult> },
    /// Model requires semantic clarification from the operator.
    WaitingForUser {
        question: String,
        options: Vec<UserOption>,
    },
    /// Policy gate requires operator authorization for an action.
    WaitingForApproval {
        request_id: String,
        tool_name: String,
        parameters: serde_json::Value,
    },
    /// EvidenceCompletionGate verified task completion.
    Completed { summary: String },
    /// Bounded failure occurred.
    Failed { error: String },
    /// Cancelled explicitly.
    Cancelled { reason: String },
    /// Budget limit reached.
    BudgetExhausted { reason: String },
}

/// The unified canonical continuous coding agent runtime engine.
pub struct AgentEngine {
    session_id: SessionId,
    workspace_root: PathBuf,
    session_repo: SqliteSessionRepository,
    model_caller: Arc<dyn ModelCaller>,
    tool_registry: Arc<ToolRegistry>,
    pipeline_runner: Arc<ToolPipelineRunner>,
    policy_gate: Arc<dyn PolicyGate>,
    approval_coordinator: Arc<ApprovalCoordinator>,
    completion_gate: Arc<EvidenceCompletionGate>,
    context_compiler: Arc<dyn ContextCompiler>,
    event_bus: Option<Arc<dyn EventBus>>,
    autonomy_mode: AutonomyMode,
    state: AgentEngineState,
    turn_number: u32,
    max_turns: u32,
    delegation_depth: u32,
    cancel_token: CancellationToken,
    active_mission_id: Option<MissionId>,
    consecutive_text_turns: u32,
    recent_action_fingerprints: Vec<(String, bool)>,
    /// First-class intent state tracking.
    /// Populated on first user message, enriched by model tool calls,
    /// injected into every model turn's system prompt context.
    intent_state: Option<IntentState>,
    /// Intent state persistent repository.
    intent_repo: Option<SqliteIntentRepository>,
    /// Canonical evidence-based stall and loop detector.
    stall_detector: StallDetector,
    /// Finite runtime-owned execution budgets.
    adaptive_budget: AdaptiveBudget,
    /// Recent diagnostic evidence records.
    recent_diagnostics: Vec<DiagnosticEvidence>,
    /// Canonical prompt catalog for stable behavioral directive loading.
    /// When set, `compile_turn_messages` sources stable instructions from the v2 prompt
    /// contract instead of hardcoded Rust strings. If None, a minimal fallback is used.
    prompt_catalog: Option<Arc<dyn PromptCatalog>>,
}

impl AgentEngine {
    /// Construct a new AgentEngine instance.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_id: SessionId,
        workspace_root: PathBuf,
        session_repo: SqliteSessionRepository,
        model_caller: Arc<dyn ModelCaller>,
        tool_registry: Arc<ToolRegistry>,
        pipeline_runner: Arc<ToolPipelineRunner>,
        policy_gate: Arc<dyn PolicyGate>,
        approval_coordinator: Arc<ApprovalCoordinator>,
        completion_gate: Arc<EvidenceCompletionGate>,
        context_compiler: Arc<dyn ContextCompiler>,
        event_bus: Option<Arc<dyn EventBus>>,
    ) -> Self {
        Self {
            session_id,
            workspace_root,
            session_repo,
            model_caller,
            tool_registry,
            pipeline_runner,
            policy_gate,
            approval_coordinator,
            completion_gate,
            context_compiler,
            event_bus,
            autonomy_mode: AutonomyMode::Autonomous,
            state: AgentEngineState::Idle,
            turn_number: 0,
            max_turns: 50,
            delegation_depth: 0,
            cancel_token: CancellationToken::new(),
            active_mission_id: None,
            consecutive_text_turns: 0,
            recent_action_fingerprints: Vec::new(),
            intent_state: None,
            intent_repo: None,
            stall_detector: StallDetector::new(),
            adaptive_budget: AdaptiveBudget::default(),
            recent_diagnostics: Vec::new(),
            prompt_catalog: None,
        }
    }

    /// Wire the canonical prompt catalog for stable behavioral directive loading.
    ///
    /// When set, `compile_turn_messages` sources the stable model-facing instructions from the
    /// v2 implementer prompt contract rather than a minimal inline fallback.
    /// This must be set for production interactive sessions; tests without a full catalog
    /// still compile and execute correctly via the fallback.
    pub fn with_prompt_catalog(mut self, catalog: Arc<dyn PromptCatalog>) -> Self {
        self.prompt_catalog = Some(catalog);
        self
    }

    /// Access active session ID.
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Access current engine state.
    pub fn state(&self) -> &AgentEngineState {
        &self.state
    }

    /// Access turns consumed so far.
    pub fn turn_number(&self) -> u32 {
        self.turn_number
    }

    /// Access the current intent state.
    pub fn intent_state(&self) -> Option<&IntentState> {
        self.intent_state.as_ref()
    }

    /// Access mutable intent state.
    pub fn intent_state_mut(&mut self) -> Option<&mut IntentState> {
        self.intent_state.as_mut()
    }

    /// Attach persistent SQLite intent repository.
    pub fn with_intent_repo(mut self, repo: SqliteIntentRepository) -> Self {
        self.intent_repo = Some(repo);
        self
    }

    /// Configure adaptive execution budget.
    pub fn with_adaptive_budget(mut self, budget: AdaptiveBudget) -> Self {
        self.adaptive_budget = budget;
        self
    }

    /// Access adaptive execution budget.
    pub fn adaptive_budget(&self) -> &AdaptiveBudget {
        &self.adaptive_budget
    }

    /// Access mutable adaptive execution budget.
    pub fn adaptive_budget_mut(&mut self) -> &mut AdaptiveBudget {
        &mut self.adaptive_budget
    }

    /// Access canonical stall and loop detector.
    pub fn stall_detector(&self) -> &StallDetector {
        &self.stall_detector
    }

    /// Access mutable canonical stall and loop detector.
    pub fn stall_detector_mut(&mut self) -> &mut StallDetector {
        &mut self.stall_detector
    }

    /// Access recent diagnostic evidence history.
    pub fn recent_diagnostics(&self) -> &[DiagnosticEvidence] {
        &self.recent_diagnostics
    }

    /// Compute a deterministic hash of the current workspace mutation state.
    ///
    /// Routes git queries through the canonical [`GitService`] capability boundary.
    /// Direct `std::process::Command` is forbidden in orchestration layers; all git interactions
    /// must pass through capability security policy.
    pub async fn compute_workspace_fingerprint(&self) -> String {
        use sha2::{Digest, Sha256};

        // Retrieve git status and HEAD commit via canonical GitService capability.
        // Falls back to empty strings if the git capability is unavailable,
        // preserving previous behavior (fingerprint based on workspace root path alone).
        let git_service = self.tool_registry.capabilities().and_then(|c| c.git());

        let (status, head) = if let Some(git) = git_service {
            let status = git.status_porcelain().await.unwrap_or_default();
            // Use git log(1) to obtain the current HEAD commit hash.
            let head = git
                .log(1)
                .await
                .ok()
                .and_then(|commits| commits.into_iter().next().map(|c| c.commit_hash))
                .unwrap_or_default();
            (status, head)
        } else {
            (String::new(), String::new())
        };

        let mut hasher = Sha256::new();
        hasher.update(status.trim().as_bytes());
        hasher.update(b":");
        hasher.update(head.trim().as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Persist current intent state if repository is attached.
    pub async fn persist_intent_state(&self) {
        if let (Some(repo), Some(intent)) = (&self.intent_repo, &self.intent_state) {
            let _ = repo.save(intent).await;
        }
    }

    /// Transition the active execution strategy/shape.
    pub async fn transition_strategy(
        &mut self,
        new_shape: TaskShape,
        reason: impl Into<String>,
    ) -> Result<(), IntentError> {
        let reason_str = reason.into();
        if let Some(ref mut intent) = self.intent_state {
            intent.transition_strategy(new_shape, reason_str)?;
        }
        self.persist_intent_state().await;
        Ok(())
    }

    /// Replan formed tasks with task completion preservation.
    pub async fn replan(
        &mut self,
        request: AdaptiveReplanRequest,
    ) -> Result<AdaptiveReplanOutcome, String> {
        self.adaptive_budget.record_replan()?;
        let outcome = if let Some(ref mut intent) = self.intent_state {
            intent.replan(request)
        } else {
            return Err("No active intent state to replan".to_string());
        };
        self.persist_intent_state().await;
        Ok(outcome)
    }

    /// Invalidate an assumption and propagate to dependent work.
    pub async fn invalidate_assumption(
        &mut self,
        assumption_id: &str,
        contradiction: AssumptionInvalidation,
    ) -> Result<AssumptionInvalidationReport, IntentError> {
        let report = if let Some(ref mut intent) = self.intent_state {
            intent.invalidate_assumption(assumption_id, contradiction)?
        } else {
            return Err(IntentError::AssumptionNotFound {
                id: assumption_id.to_string(),
            });
        };
        self.persist_intent_state().await;
        Ok(report)
    }

    /// Add a newly discovered unknown during execution.
    pub async fn add_unknown(&mut self, unknown: IntentUnknown) -> Result<(), IntentError> {
        if let Some(ref mut intent) = self.intent_state {
            intent.add_unknown(unknown)?;
        }
        self.persist_intent_state().await;
        Ok(())
    }

    /// Resolve an unknown with evidence.
    pub async fn resolve_unknown(
        &mut self,
        unknown_id: &str,
        resolution: UnknownResolution,
    ) -> bool {
        let resolved = if let Some(ref mut intent) = self.intent_state {
            intent.resolve_unknown(unknown_id, resolution)
        } else {
            false
        };
        if resolved {
            self.persist_intent_state().await;
        }
        resolved
    }

    /// Initialize intent state from a user prompt.
    ///
    /// Called when the engine receives the first user message. Does not
    /// fabricate information — sets only the raw_prompt and timestamps.
    pub fn initialize_intent_from_prompt(&mut self, prompt: &str) {
        if self.intent_state.is_none() {
            self.intent_state = Some(IntentState::initial_from_prompt(
                self.session_id.to_string(),
                prompt,
            ));
        }
    }

    /// Apply user steering to the current intent state.
    ///
    /// Steering invalidates conflicting assumptions and formed tasks.
    /// The constraint is persisted in intent_state and injected into the
    /// next model turn's system prompt.
    pub fn apply_steering_constraint(&mut self, constraint: SteeringConstraint) {
        if let Some(intent) = self.intent_state.as_mut() {
            intent.apply_steering(constraint);
        }
    }

    /// Set cancellation token.
    pub fn with_cancellation_token(mut self, token: CancellationToken) -> Self {
        self.cancel_token = token;
        self
    }

    /// Set max turns budget.
    pub fn with_max_turns(mut self, max_turns: u32) -> Self {
        self.max_turns = max_turns;
        self
    }

    /// Set delegation depth.
    pub fn with_delegation_depth(mut self, depth: u32) -> Self {
        self.delegation_depth = depth;
        self
    }

    /// Set active mission ID.
    pub fn with_active_mission(mut self, mid: Option<MissionId>) -> Self {
        self.active_mission_id = mid;
        self
    }

    /// Resume or initialize from durable session.
    pub async fn load_session_state(&mut self) -> Result<(), M31AError> {
        if let Some(session) = self.session_repo.get_session(self.session_id).await? {
            self.active_mission_id = session.active_mission_id;
        }
        let turns = self.session_repo.get_conversation(self.session_id).await?;
        self.turn_number = (turns.len() / 2) as u32;

        // Restore persistent intent state across process restarts
        if let Some(ref repo) = self.intent_repo {
            if let Ok(Some(loaded)) = repo.load(self.session_id).await {
                self.intent_state = Some(loaded);
            }
        }

        // Restore pending pause states across process restarts
        if let Some(last_turn) = turns.last() {
            match last_turn {
                ConversationTurn::AskUserMessage {
                    question,
                    options,
                    answer,
                    ..
                } if answer.is_none() => {
                    self.state = AgentEngineState::WaitingForUser {
                        question: question.clone(),
                        options: options.clone(),
                    };
                }
                ConversationTurn::ApprovalMessage {
                    request_id,
                    decision,
                    prompt,
                    ..
                } if decision.is_none() => {
                    self.state = AgentEngineState::WaitingForApproval {
                        request_id: request_id.clone(),
                        tool_name: prompt.clone(),
                        parameters: serde_json::Value::Null,
                    };
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Provide operator response to a pending `WaitingForUser` state.
    pub async fn provide_user_response(&mut self, answer: &str) -> Result<(), M31AError> {
        if !matches!(self.state, AgentEngineState::WaitingForUser { .. }) {
            return Err(M31AError::Internal(anyhow::anyhow!(
                "AgentEngine is not waiting for user response"
            )));
        }

        let seq = self.session_repo.next_sequence(self.session_id).await?;
        let turn = ConversationTurn::UserMessage {
            id: Uuid::now_v7(),
            sequence: seq,
            content: answer.to_string(),
            raw_text: answer.to_string(),
            mentions: Vec::new(),
            created_at: Utc::now(),
        };
        self.session_repo
            .append_turn(self.session_id, &turn)
            .await?;
        self.state = AgentEngineState::Running;
        Ok(())
    }

    /// Provide operator approval decision for a pending action.
    pub async fn provide_approval(
        &mut self,
        request_id: &str,
        approved: bool,
    ) -> Result<(), M31AError> {
        if !matches!(self.state, AgentEngineState::WaitingForApproval { .. }) {
            return Err(M31AError::Internal(anyhow::anyhow!(
                "AgentEngine is not waiting for approval"
            )));
        }

        let action = if approved {
            crate::policy::approval::ApprovalAction::AllowOnce
        } else {
            crate::policy::approval::ApprovalAction::Deny {
                reason: "operator rejected action".to_string(),
            }
        };

        if let Ok(req_uuid) = Uuid::parse_str(request_id) {
            let _ = self
                .approval_coordinator
                .resolve_request(req_uuid.into(), action, "operator")
                .await;
        }

        let seq = self.session_repo.next_sequence(self.session_id).await?;
        let turn = ConversationTurn::ApprovalMessage {
            id: Uuid::now_v7(),
            sequence: seq,
            request_id: request_id.to_string(),
            prompt: "Operator approval decision".to_string(),
            decision: Some(if approved { "approved" } else { "rejected" }.to_string()),
            created_at: Utc::now(),
        };
        self.session_repo
            .append_turn(self.session_id, &turn)
            .await?;
        self.state = AgentEngineState::Running;
        Ok(())
    }

    /// Compile fresh prompt messages from the durable session history and repository context.
    ///
    /// ## Prompt Architecture
    ///
    /// The system message has two distinct layers:
    ///
    /// 1. **Stable behavioral layer** — sourced from the canonical `agent.implementer` v2 prompt
    ///    contract in [`InMemoryPromptCatalog`]. This ensures behavioral guidelines are versioned,
    ///    auditable, and not duplicated in Rust source.
    ///
    /// 2. **Dynamic runtime context** — workspace root, git state (via [`GitService`] capability
    ///    boundary), available tools, intent state, and recovery context. These are legitimate
    ///    runtime values that cannot be expressed as static prompt assets.
    ///
    /// Git queries route through [`GitService`] (canonical capability boundary).
    /// Direct `std::process::Command` is forbidden in orchestration layers; all git interactions
    /// must pass through capability security policy.
    pub async fn compile_turn_messages(&self) -> Result<Vec<ChatMessage>, M31AError> {
        let turns = self.session_repo.get_conversation(self.session_id).await?;
        let mut messages = Vec::new();

        // 1. Dynamic Git Workspace State — via canonical GitService capability boundary.
        let git_section = {
            let git_service = self.tool_registry.capabilities().and_then(|c| c.git());

            if let Some(git) = git_service {
                match git.status().await {
                    Ok(status) => {
                        if status.is_clean {
                            format!(
                                "Git Workspace: clean working directory on branch '{}'",
                                status.branch
                            )
                        } else {
                            let dirty: Vec<String> = status
                                .staged
                                .iter()
                                .map(|f| format!("M  {f}"))
                                .chain(status.unstaged.iter().map(|f| format!(" M {f}")))
                                .chain(status.untracked.iter().map(|f| format!("?? {f}")))
                                .take(15)
                                .collect();
                            format!(
                                "Git Workspace: dirty on branch '{}'. Modified/untracked files:\n{}",
                                status.branch,
                                dirty.join("\n")
                            )
                        }
                    }
                    Err(_) => format!(
                        "Git Workspace: status unavailable (workspace: {})",
                        self.workspace_root.display()
                    ),
                }
            } else {
                format!(
                    "Git Workspace: git capability not configured (workspace: {})",
                    self.workspace_root.display()
                )
            }
        };

        let tools_list: Vec<String> = self
            .tool_registry
            .list_tools()
            .iter()
            .map(|t| format!("- {}: {}", t.id(), t.description()))
            .collect();
        let tools_section = format!("Available Tools:\n{}", tools_list.join("\n"));

        // System Prompt with strict runtime boundaries and authoritative workspace state.
        // IntentState context fragment appended when available.
        let intent_fragment = self
            .intent_state
            .as_ref()
            .map(|is| format!("\n\n{}", is.render_context_fragment()))
            .unwrap_or_default();

        // Structured Recovery Context appended during recovery or after diagnostic failures.
        let recovery_fragment = if let Some(ref intent) = self.intent_state {
            if intent.current_strategy == TaskShape::Recover || !self.recent_diagnostics.is_empty()
            {
                format!(
                    "\n\n{}",
                    crate::agent::adaptive::render_recovery_context(
                        intent,
                        &self.adaptive_budget,
                        &self.recent_diagnostics,
                    )
                )
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // Stable behavioral instructions sourced from canonical prompt catalog.
        // Stable agent behavioral directives are authored in `prompts/agents/implementer.v2.toml`
        // and loaded via PromptCatalog. For interactive sessions, we use the prompt body template
        // directly (the PromptCompiler 7-layer pipeline is used by WorkerRunner for DAG tasks
        // which have full task context).
        let stable_instructions: String = self
            .prompt_catalog
            .as_deref()
            .and_then(|cat| cat.get("agent.implementer", 2).ok().map(|c| c.template_body.clone()))
            .unwrap_or_else(|| {
                // Canonical minimal fallback — used when prompt catalog is not loaded
                // (e.g., bare test fixtures). The stable behavioral content lives in
                // `prompts/agents/implementer.v2.toml`; this is NOT a duplicate of it.
                "# ROLE: M31A Autonomous Software Engineering Agent\n\
                 You operate under the governing invariant: **The model proposes. The runtime decides.**\n\
                 Explore the repository, inspect code and test failures carefully, verify every change\n\
                 by running tests before proposing completion.\n\
                 Never declare task completion without genuine test evidence."
                    .to_string()
            });

        let system_prompt = format!(
            "Workspace Root: {}\n{}\n\n{}\n\n{}{}{}",
            self.workspace_root.display(),
            git_section,
            tools_section,
            stable_instructions,
            intent_fragment,
            recovery_fragment
        );
        messages.push(ChatMessage::System {
            content: system_prompt,
        });

        // 2. Multi-turn conversation history
        for turn in turns {
            match turn {
                ConversationTurn::UserMessage { content, .. } => {
                    messages.push(ChatMessage::User { content });
                }
                ConversationTurn::AssistantMessage { content, .. } => {
                    messages.push(ChatMessage::Assistant {
                        content: Some(content),
                        tool_calls: Vec::new(),
                    });
                }
                ConversationTurn::ToolCallMessage {
                    call_id,
                    tool_name,
                    arguments,
                    ..
                } => {
                    messages.push(ChatMessage::Assistant {
                        content: None,
                        tool_calls: vec![ModelToolCall {
                            id: call_id,
                            name: tool_name,
                            arguments,
                        }],
                    });
                }
                ConversationTurn::ToolResultMessage {
                    call_id,
                    output,
                    success,
                    ..
                } => {
                    let text = if success {
                        output
                    } else {
                        format!("Error: {output}")
                    };
                    messages.push(ChatMessage::Tool {
                        tool_call_id: call_id,
                        content: text,
                    });
                }
                ConversationTurn::SystemMessage { content, .. } => {
                    messages.push(ChatMessage::System { content });
                }
                ConversationTurn::ApprovalMessage {
                    request_id,
                    decision,
                    ..
                } => {
                    messages.push(ChatMessage::System {
                        content: format!(
                            "Approval request '{}' outcome: {}",
                            request_id,
                            decision.as_deref().unwrap_or("pending")
                        ),
                    });
                }
                ConversationTurn::VerificationMessage {
                    passed, summary, ..
                } => {
                    messages.push(ChatMessage::System {
                        content: format!(
                            "Runtime Verification Outcome: {} - {}",
                            if passed { "PASSED" } else { "FAILED" },
                            summary
                        ),
                    });
                }
                ConversationTurn::AskUserMessage {
                    question,
                    options,
                    answer,
                    ..
                } => {
                    let opt_str = options
                        .iter()
                        .map(|o| format!("- [{}]: {}", o.id, o.label))
                        .collect::<Vec<_>>()
                        .join("\n");
                    messages.push(ChatMessage::Assistant {
                        content: Some(format!("Question: {}\nOptions:\n{}", question, opt_str)),
                        tool_calls: Vec::new(),
                    });
                    if let Some(ans) = answer {
                        messages.push(ChatMessage::User { content: ans });
                    }
                }
            }
        }

        Ok(messages)
    }

    /// Execute a single step in the continuous interactive coding agent loop.
    pub fn step<'a>(
        &'a mut self,
        steering_input: Option<&'a str>,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<AgentTurnOutcome, M31AError>> + Send + 'a>,
    > {
        Box::pin(async move { self.step_internal(steering_input).await })
    }

    async fn step_internal(
        &mut self,
        steering_input: Option<&str>,
    ) -> Result<AgentTurnOutcome, M31AError> {
        // 1. Cooperative cancellation check
        if self.cancel_token.is_cancelled() {
            self.state = AgentEngineState::Cancelled {
                reason: "Operation cancelled at step boundary".to_string(),
            };
            return Ok(AgentTurnOutcome::Cancelled {
                reason: "Operation cancelled at step boundary".to_string(),
            });
        }

        // 2. Budget check
        if self.turn_number >= self.max_turns {
            self.state = AgentEngineState::BudgetExhausted {
                reason: format!("Turn budget limit of {} turns reached", self.max_turns),
            };
            return Ok(AgentTurnOutcome::BudgetExhausted {
                reason: format!("Turn budget limit of {} turns reached", self.max_turns),
            });
        }

        // 3. User steering incorporation
        if let Some(steer) = steering_input {
            let trimmed = steer.trim();
            if trimmed.eq_ignore_ascii_case("stop")
                || trimmed.eq_ignore_ascii_case("/cancel")
                || trimmed.eq_ignore_ascii_case("cancel")
            {
                self.cancel_token.cancel();
                self.state = AgentEngineState::Cancelled {
                    reason: "Cancelled by user steering command".to_string(),
                };
                let seq = self.session_repo.next_sequence(self.session_id).await?;
                let _ = self
                    .session_repo
                    .append_turn(
                        self.session_id,
                        &ConversationTurn::UserMessage {
                            id: Uuid::now_v7(),
                            sequence: seq,
                            content: trimmed.to_string(),
                            raw_text: trimmed.to_string(),
                            mentions: Vec::new(),
                            created_at: Utc::now(),
                        },
                    )
                    .await;
                return Ok(AgentTurnOutcome::Cancelled {
                    reason: "Cancelled by user steering command".to_string(),
                });
            }

            // Initialize or update intent state from the steering input.
            // This ensures even the first user message anchors intent_state.raw_prompt.
            if self.intent_state.is_none() {
                self.initialize_intent_from_prompt(trimmed);
            } else {
                // Mid-session steering: apply as a canonical constraint.
                // This invalidates conflicting assumptions and pending tasks.
                self.apply_steering_constraint(SteeringConstraint::new(trimmed, trimmed));
            }

            let seq = self.session_repo.next_sequence(self.session_id).await?;
            let turn = ConversationTurn::UserMessage {
                id: Uuid::now_v7(),
                sequence: seq,
                content: trimmed.to_string(),
                raw_text: trimmed.to_string(),
                mentions: Vec::new(),
                created_at: Utc::now(),
            };
            self.session_repo
                .append_turn(self.session_id, &turn)
                .await?;
            self.state = AgentEngineState::Running;
        }

        // 4. Pause state validations
        if let AgentEngineState::WaitingForUser { question, options } = &self.state {
            return Ok(AgentTurnOutcome::WaitingForUser {
                question: question.clone(),
                options: options.clone(),
            });
        }
        if let AgentEngineState::WaitingForApproval {
            request_id,
            tool_name,
            parameters,
        } = &self.state
        {
            return Ok(AgentTurnOutcome::WaitingForApproval {
                request_id: request_id.clone(),
                tool_name: tool_name.clone(),
                parameters: parameters.clone(),
            });
        }

        self.state = AgentEngineState::Running;

        // 5. Compile fresh context from durable session
        let messages = self.compile_turn_messages().await?;
        let compiled_context = CompiledContext {
            system_prompt: messages
                .first()
                .and_then(|m| match m {
                    ChatMessage::System { content } => Some(content.clone()),
                    _ => None,
                })
                .unwrap_or_default(),
            messages: messages.clone(),
            context_id: format!("ctx-{}-turn-{}", self.session_id, self.turn_number),
            token_count: 1024,
            manifest: None,
        };

        // 6. Invoke model through canonical ModelCaller
        let (proposal, usage) = match self
            .model_caller
            .call_model_with_context_and_usage(&compiled_context, &self.cancel_token)
            .await
        {
            Ok(res) => res,
            Err(e) => {
                let err_msg = format!("Model invocation failed: {e}");
                self.state = AgentEngineState::Failed {
                    error: err_msg.clone(),
                };
                let seq = self.session_repo.next_sequence(self.session_id).await?;
                let _ = self
                    .session_repo
                    .append_turn(
                        self.session_id,
                        &ConversationTurn::SystemMessage {
                            id: Uuid::now_v7(),
                            sequence: seq,
                            content: err_msg.clone(),
                            created_at: Utc::now(),
                        },
                    )
                    .await;
                return Ok(AgentTurnOutcome::Failed { error: err_msg });
            }
        };

        if let Some(ref bus) = self.event_bus {
            let inv_id = uuid::Uuid::now_v7();
            let envelope = crate::events::envelope::EventEnvelope::new(
                0,
                self.active_mission_id,
                None,
                "agent_engine".to_string(),
                crate::events::types::EventType::ModelUsageUpdated {
                    invocation_id: Some(inv_id),
                    mission_id: self.active_mission_id,
                    task_id: None,
                    provider: "model".to_string(),
                    model: "model".to_string(),
                    usage,
                    cumulative_usage: None,
                },
            );
            let _ = bus.publish(envelope).await;
        }

        self.turn_number += 1;

        // 7. Route model proposal by first-class variant
        match proposal {
            ModelProposal::AssistantText { content } => {
                let seq = self.session_repo.next_sequence(self.session_id).await?;
                let turn = ConversationTurn::AssistantMessage {
                    id: Uuid::now_v7(),
                    sequence: seq,
                    content: content.clone(),
                    created_at: Utc::now(),
                };
                self.session_repo
                    .append_turn(self.session_id, &turn)
                    .await?;

                let turns = self.session_repo.get_conversation(self.session_id).await?;
                let has_recent_tool = turns.iter().rev().take(4).any(|t| {
                    matches!(
                        t,
                        ConversationTurn::ToolResultMessage { .. }
                            | ConversationTurn::ToolCallMessage { .. }
                    )
                });

                if (self.active_mission_id.is_some() || has_recent_tool)
                    && self.consecutive_text_turns == 0
                {
                    self.consecutive_text_turns += 1;
                    self.state = AgentEngineState::Running;
                    Ok(AgentTurnOutcome::AssistantCommentary { content })
                } else {
                    self.consecutive_text_turns = 0;
                    self.state = AgentEngineState::Idle;
                    Ok(AgentTurnOutcome::AssistantText { content })
                }
            }

            ModelProposal::AskUser { question, options } => {
                let seq = self.session_repo.next_sequence(self.session_id).await?;
                let turn = ConversationTurn::AskUserMessage {
                    id: Uuid::now_v7(),
                    sequence: seq,
                    question: question.clone(),
                    options: options.clone(),
                    answer: None,
                    created_at: Utc::now(),
                };
                self.session_repo
                    .append_turn(self.session_id, &turn)
                    .await?;
                self.state = AgentEngineState::WaitingForUser {
                    question: question.clone(),
                    options: options.clone(),
                };
                Ok(AgentTurnOutcome::WaitingForUser { question, options })
            }

            ModelProposal::Handoff {
                target_role,
                reason,
            } => {
                // Operational subagent delegation: enforce bounded delegation depth.
                if self.delegation_depth >= 2 {
                    let err_msg = format!(
                        "Delegation rejected: maximum subagent depth of 2 exceeded for role '{}'",
                        target_role
                    );
                    let seq = self.session_repo.next_sequence(self.session_id).await?;
                    let turn = ConversationTurn::SystemMessage {
                        id: Uuid::now_v7(),
                        sequence: seq,
                        content: err_msg.clone(),
                        created_at: Utc::now(),
                    };
                    self.session_repo
                        .append_turn(self.session_id, &turn)
                        .await?;
                    return Ok(AgentTurnOutcome::ToolResults {
                        results: vec![StructuredToolResult {
                            call_id: format!("handoff-{}", Uuid::now_v7()),
                            tool_name: "handoff".to_string(),
                            success: false,
                            output: String::new(),
                            error: Some(err_msg),
                            duration_ms: 0,
                            policy_decision: Some("depth_exceeded".to_string()),
                            approval_id: None,
                            artifacts_created: Vec::new(),
                            diagnostic: None,
                        }],
                    });
                }

                let sub_session = self
                    .session_repo
                    .create_session(&self.workspace_root)
                    .await?;
                let mut subagent = AgentEngine::new(
                    sub_session.id,
                    self.workspace_root.clone(),
                    self.session_repo.clone(),
                    self.model_caller.clone(),
                    self.tool_registry.clone(),
                    self.pipeline_runner.clone(),
                    self.policy_gate.clone(),
                    self.approval_coordinator.clone(),
                    self.completion_gate.clone(),
                    self.context_compiler.clone(),
                    self.event_bus.clone(),
                )
                .with_delegation_depth(self.delegation_depth + 1)
                .with_max_turns(10)
                .with_cancellation_token(self.cancel_token.clone());

                let sub_prompt = format!(
                    "Subagent Delegation [Role: {}]: {}\nContext: Proceed with isolated objective and verify your work.",
                    target_role, reason
                );
                let _ = subagent.step(Some(&sub_prompt)).await?;
                let sub_final_state = subagent.run_until_terminal().await?;

                let (sub_success, sub_output, subagent_diag) = match sub_final_state {
                    AgentEngineState::Completed { final_summary } => (true, final_summary, None),
                    AgentEngineState::Failed { error } => {
                        let diag = DiagnosticEvidence::new(
                            format!("subagent_{}", target_role),
                            FailureClassification::Model,
                            error.clone(),
                        )
                        .with_detailed_output(error.clone())
                        .with_category(ExecutionFailureCategory::ModelFailure)
                        .with_subagent_failure(SubagentFailureEvidence {
                            subagent_role: target_role.clone(),
                            error_message: error.clone(),
                            completed_work_summary: None,
                            unresolved_unknowns: Vec::new(),
                            invalidated_assumptions: Vec::new(),
                        });
                        (false, format!("Subagent failed: {error}"), Some(diag))
                    }
                    AgentEngineState::Cancelled {
                        reason: cancel_reason,
                    } => {
                        let diag = DiagnosticEvidence::new(
                            format!("subagent_{}", target_role),
                            FailureClassification::Unknown,
                            cancel_reason.clone(),
                        )
                        .with_detailed_output(cancel_reason.clone())
                        .with_category(ExecutionFailureCategory::UnrecoverableFailure)
                        .with_subagent_failure(SubagentFailureEvidence {
                            subagent_role: target_role.clone(),
                            error_message: cancel_reason.clone(),
                            completed_work_summary: None,
                            unresolved_unknowns: Vec::new(),
                            invalidated_assumptions: Vec::new(),
                        });
                        (
                            false,
                            format!("Subagent cancelled: {cancel_reason}"),
                            Some(diag),
                        )
                    }
                    _ => (
                        true,
                        format!("Subagent delegation to '{}' concluded.", target_role),
                        None,
                    ),
                };

                if let Some(ref diag) = subagent_diag {
                    self.recent_diagnostics.push(diag.clone());
                    if self.recent_diagnostics.len() > 10 {
                        self.recent_diagnostics.remove(0);
                    }
                }

                let parent_seq = self.session_repo.next_sequence(self.session_id).await?;
                let turn = ConversationTurn::AssistantMessage {
                    id: Uuid::now_v7(),
                    sequence: parent_seq,
                    content: format!("Delegated subagent outcome: {}", sub_output),
                    created_at: Utc::now(),
                };
                self.session_repo
                    .append_turn(self.session_id, &turn)
                    .await?;

                Ok(AgentTurnOutcome::ToolResults {
                    results: vec![StructuredToolResult {
                        call_id: format!("handoff-{}", sub_session.id),
                        tool_name: format!("subagent_{}", target_role),
                        success: sub_success,
                        output: sub_output,
                        error: None,
                        duration_ms: 0,
                        policy_decision: Some("allowed".to_string()),
                        approval_id: None,
                        artifacts_created: Vec::new(),
                        diagnostic: subagent_diag,
                    }],
                })
            }

            ModelProposal::Complete { summary, .. } => {
                // EvidenceCompletionGate authoritative verification check: completion requires evidence.
                let turns = self.session_repo.get_conversation(self.session_id).await?;
                let gate_decision = self
                    .completion_gate
                    .evaluate_session_completion_with_intent(
                        self.session_id,
                        &turns,
                        self.intent_state.as_ref(),
                    )
                    .await
                    .map_err(|e| M31AError::Internal(anyhow::anyhow!(e)))?;

                if !gate_decision.is_satisfied {
                    let rejection = format!(
                        "Premature completion rejected (AGENTS.md Rule 6: Completion requires evidence): {}",
                        gate_decision.violations.join("; ")
                    );
                    let seq = self.session_repo.next_sequence(self.session_id).await?;
                    let turn = ConversationTurn::AssistantMessage {
                        id: Uuid::now_v7(),
                        sequence: seq,
                        content: rejection.clone(),
                        created_at: Utc::now(),
                    };
                    self.session_repo
                        .append_turn(self.session_id, &turn)
                        .await?;
                    return Ok(AgentTurnOutcome::ToolResults {
                        results: vec![StructuredToolResult {
                            call_id: format!("gate-{}", Uuid::now_v7()),
                            tool_name: "completion_gate".to_string(),
                            success: false,
                            output: String::new(),
                            error: Some(rejection),
                            duration_ms: 0,
                            policy_decision: Some("denied".to_string()),
                            approval_id: None,
                            artifacts_created: Vec::new(),
                            diagnostic: None,
                        }],
                    });
                }

                if let Some(ref mut intent) = self.intent_state {
                    for task in &mut intent.formed_tasks {
                        if matches!(
                            task.status,
                            FormedTaskStatus::Pending | FormedTaskStatus::InProgress
                        ) {
                            task.status = FormedTaskStatus::Completed {
                                summary: summary.clone(),
                            };
                        }
                    }
                }
                self.persist_intent_state().await;

                let seq = self.session_repo.next_sequence(self.session_id).await?;
                let turn = ConversationTurn::AssistantMessage {
                    id: Uuid::now_v7(),
                    sequence: seq,
                    content: summary.clone(),
                    created_at: Utc::now(),
                };
                self.session_repo
                    .append_turn(self.session_id, &turn)
                    .await?;

                let v_seq = self.session_repo.next_sequence(self.session_id).await?;
                let v_turn = ConversationTurn::VerificationMessage {
                    id: Uuid::now_v7(),
                    sequence: v_seq,
                    passed: true,
                    summary: summary.clone(),
                    created_at: Utc::now(),
                };
                self.session_repo
                    .append_turn(self.session_id, &v_turn)
                    .await?;

                self.state = AgentEngineState::Completed {
                    final_summary: summary.clone(),
                };
                Ok(AgentTurnOutcome::Completed { summary })
            }

            ModelProposal::ToolCalls { calls } => {
                self.consecutive_text_turns = 0;
                let results = self.execute_tool_calls(calls).await?;
                if let AgentEngineState::WaitingForApproval {
                    request_id,
                    tool_name,
                    parameters,
                } = &self.state
                {
                    Ok(AgentTurnOutcome::WaitingForApproval {
                        request_id: request_id.clone(),
                        tool_name: tool_name.clone(),
                        parameters: parameters.clone(),
                    })
                } else if let AgentEngineState::Failed { error } = &self.state {
                    Ok(AgentTurnOutcome::Failed {
                        error: error.clone(),
                    })
                } else {
                    Ok(AgentTurnOutcome::ToolResults { results })
                }
            }
        }
    }

    /// Execute a vector of native tool calls through the full 11-stage policy pipeline.
    async fn execute_tool_calls(
        &mut self,
        calls: Vec<ModelToolCall>,
    ) -> Result<Vec<StructuredToolResult>, M31AError> {
        let mut results = Vec::new();
        let capabilities = Arc::new(crate::capability::registry::CapabilityRegistry::production(
            &self.workspace_root,
            self.event_bus.clone(),
            None,
        ));
        let ctx = ToolExecutionContext::new(
            capabilities,
            self.workspace_root.clone(),
            self.cancel_token.clone(),
        );

        for call in calls {
            let start = std::time::Instant::now();
            let action_req = ActionRequest {
                id: call.id.clone(),
                tool_name: call.name.clone(),
                parameters: call.arguments.clone(),
            };

            let action_fingerprint =
                crate::agent::runner::compute_action_fingerprint(&call.name, &call.arguments);

            // 1. Record ToolCallMessage in session
            let seq = self.session_repo.next_sequence(self.session_id).await?;
            let call_turn = ConversationTurn::ToolCallMessage {
                id: Uuid::now_v7(),
                sequence: seq,
                call_id: call.id.clone(),
                tool_name: call.name.clone(),
                arguments: call.arguments.clone(),
                created_at: Utc::now(),
            };
            self.session_repo
                .append_turn(self.session_id, &call_turn)
                .await?;

            let current_ws = self.compute_workspace_fingerprint().await;
            let stall_eval =
                self.stall_detector
                    .evaluate_proposal(&call.name, &action_fingerprint, &current_ws);

            match stall_eval {
                StallEvaluation::StagnationLoopDetected {
                    tool_name, error, ..
                } => {
                    tracing::error!(tool = %tool_name, "Non-progress loop detected: identical failing action repeated with identical workspace state");
                    let r_seq = self.session_repo.next_sequence(self.session_id).await?;
                    let _ = self
                        .session_repo
                        .append_turn(
                            self.session_id,
                            &ConversationTurn::ToolResultMessage {
                                id: Uuid::now_v7(),
                                sequence: r_seq,
                                call_id: call.id.clone(),
                                tool_name: call.name.clone(),
                                output: String::new(),
                                success: false,
                                created_at: Utc::now(),
                            },
                        )
                        .await;

                    self.state = AgentEngineState::Failed {
                        error: error.clone(),
                    };
                    results.push(StructuredToolResult {
                        call_id: call.id,
                        tool_name: call.name,
                        success: false,
                        output: String::new(),
                        error: Some(error),
                        duration_ms: 0,
                        policy_decision: Some("loop_detected_abort".to_string()),
                        approval_id: None,
                        artifacts_created: Vec::new(),
                        diagnostic: None,
                    });
                    return Ok(results);
                }
                StallEvaluation::RepeatedActionRejected {
                    tool_name, advice, ..
                } => {
                    tracing::warn!(tool = %tool_name, "Rejected repeated failing action");
                    let r_seq = self.session_repo.next_sequence(self.session_id).await?;
                    let _ = self
                        .session_repo
                        .append_turn(
                            self.session_id,
                            &ConversationTurn::ToolResultMessage {
                                id: Uuid::now_v7(),
                                sequence: r_seq,
                                call_id: call.id.clone(),
                                tool_name: call.name.clone(),
                                output: String::new(),
                                success: false,
                                created_at: Utc::now(),
                            },
                        )
                        .await;

                    self.stall_detector
                        .record_observation(ExecutionObservation {
                            tool_name: call.name.clone(),
                            input_fingerprint: action_fingerprint.clone(),
                            workspace_state_fingerprint: current_ws.clone(),
                            success: false,
                            failure_signature: Some("repeated_action_rejected".to_string()),
                            timestamp: Utc::now(),
                        });

                    results.push(StructuredToolResult {
                        call_id: call.id,
                        tool_name: call.name,
                        success: false,
                        output: String::new(),
                        error: Some(advice),
                        duration_ms: 0,
                        policy_decision: Some("rejected_loop".to_string()),
                        approval_id: None,
                        artifacts_created: Vec::new(),
                        diagnostic: None,
                    });
                    continue;
                }
                StallEvaluation::Progressing => {}
            }

            // 2. PolicyGate pre-check for approval requirement
            let policy_req = PolicyEvaluationRequest {
                mission_id: self.active_mission_id.unwrap_or_default(),
                task_id: TaskId::new(),
                tool_or_action: call.name.clone(),
                context_digest: format!(
                    "ws={};args={}",
                    self.workspace_root.display(),
                    call.arguments
                ),
            };

            let policy_decision = self.policy_gate.evaluate(policy_req).await;
            if let Ok(PolicyDecision::Ask) = policy_decision {
                let req_id = Uuid::now_v7().to_string();
                self.state = AgentEngineState::WaitingForApproval {
                    request_id: req_id.clone(),
                    tool_name: call.name.clone(),
                    parameters: call.arguments.clone(),
                };
                let a_seq = self.session_repo.next_sequence(self.session_id).await?;
                let _ = self
                    .session_repo
                    .append_turn(
                        self.session_id,
                        &ConversationTurn::ApprovalMessage {
                            id: Uuid::now_v7(),
                            sequence: a_seq,
                            request_id: req_id,
                            prompt: format!("Approval required for {}: operator authorization required by policy", call.name),
                            decision: None,
                            created_at: Utc::now(),
                        },
                    )
                    .await;
                return Ok(results);
            }

            // 3. Execute via ToolPipelineRunner through all 11 stages
            let action_res = self
                .pipeline_runner
                .execute_action(
                    &action_req,
                    &ctx,
                    self.policy_gate.as_ref(),
                    self.autonomy_mode,
                )
                .await;

            let duration_ms = start.elapsed().as_millis() as u64;

            // Strategy adaptation execution if model invoked adapt_strategy
            if call.name == "adapt_strategy" && action_res.success {
                if let Ok(input) = serde_json::from_value::<
                    crate::tools::definition::AdaptStrategyInput,
                >(call.arguments.clone())
                {
                    let target_shape = match input.strategy.to_lowercase().as_str() {
                        "investigate_then_act" | "investigate" => {
                            Some(TaskShape::InvestigateThenAct)
                        }
                        "research_then_act" | "research" => Some(TaskShape::ResearchThenAct),
                        "ask_user_then_act" | "ask_user" => Some(TaskShape::AskUserThenAct),
                        "plan_then_execute" | "plan" => Some(TaskShape::PlanThenExecute),
                        "recover" | "recovery" => Some(TaskShape::Recover),
                        "direct_tool_execution" | "direct" => Some(TaskShape::DirectToolExecution),
                        s if s.starts_with("delegate:") => {
                            let role = s.trim_start_matches("delegate:").trim();
                            Some(TaskShape::Delegate {
                                target_role: role.to_string(),
                            })
                        }
                        _ => None,
                    };

                    if let Some(shape) = target_shape {
                        let _ = self.transition_strategy(shape, &input.reason).await;
                    }

                    if !input.tasks_to_supersede.is_empty() || !input.new_tasks.is_empty() {
                        let tasks_to_supersede = input
                            .tasks_to_supersede
                            .iter()
                            .map(|id| (id.clone(), input.reason.clone()))
                            .collect();
                        let new_tasks = input
                            .new_tasks
                            .iter()
                            .map(|title| FormedTask {
                                id: format!("task-rev-{}", Uuid::now_v7()),
                                title: title.clone(),
                                description: title.clone(),
                                shape: TaskShape::InvestigateThenAct,
                                status: FormedTaskStatus::Pending,
                                assumption_ids: Vec::new(),
                                blocking_unknown_ids: Vec::new(),
                                replacement_for_task_id: None,
                                created_at: Utc::now(),
                            })
                            .collect();

                        let replan_req = AdaptiveReplanRequest {
                            reason: input.reason.clone(),
                            tasks_to_supersede,
                            new_tasks,
                            invalidated_assumption_ids: Vec::new(),
                        };
                        let _ = self.replan(replan_req).await;
                    }
                }
            }

            // Structured Diagnostic Evidence capture and loop observation
            let is_policy_denied = matches!(policy_decision, Ok(PolicyDecision::Deny))
                || action_res
                    .error
                    .as_deref()
                    .unwrap_or_default()
                    .starts_with("Policy denied");

            let (failure_class, category) = if is_policy_denied {
                (
                    FailureClassification::Policy,
                    ExecutionFailureCategory::PolicyDenied,
                )
            } else if call.name == "run_tests"
                || action_res
                    .error
                    .as_deref()
                    .unwrap_or_default()
                    .contains("test failed")
            {
                (
                    FailureClassification::Test,
                    ExecutionFailureCategory::VerificationFailure,
                )
            } else {
                (
                    FailureClassification::ToolContract,
                    ExecutionFailureCategory::ExecutionFailed,
                )
            };

            let diagnostic = if !action_res.success {
                let err_str = action_res
                    .error
                    .clone()
                    .unwrap_or_else(|| action_res.output.clone());
                let diag = DiagnosticEvidence::new(call.name.clone(), failure_class, err_str)
                    .with_detailed_output(action_res.output.clone())
                    .with_policy_denial(is_policy_denied)
                    .with_category(category);
                self.recent_diagnostics.push(diag.clone());
                if self.recent_diagnostics.len() > 10 {
                    self.recent_diagnostics.remove(0);
                }
                Some(diag)
            } else {
                None
            };

            let current_ws_after = self.compute_workspace_fingerprint().await;
            self.stall_detector
                .record_observation(ExecutionObservation {
                    tool_name: call.name.clone(),
                    input_fingerprint: action_fingerprint.clone(),
                    workspace_state_fingerprint: current_ws_after,
                    success: action_res.success,
                    failure_signature: if action_res.success {
                        None
                    } else {
                        Some(
                            action_res
                                .error
                                .clone()
                                .unwrap_or_else(|| action_res.output.clone()),
                        )
                    },
                    timestamp: Utc::now(),
                });

            self.recent_action_fingerprints
                .push((action_fingerprint, action_res.success));
            if self.recent_action_fingerprints.len() > 30 {
                self.recent_action_fingerprints.remove(0);
            }

            // 4. Record ToolResultMessage in session
            let r_seq = self.session_repo.next_sequence(self.session_id).await?;
            let res_turn = ConversationTurn::ToolResultMessage {
                id: Uuid::now_v7(),
                sequence: r_seq,
                call_id: call.id.clone(),
                tool_name: call.name.clone(),
                output: if action_res.success {
                    action_res.output.clone()
                } else {
                    action_res
                        .error
                        .clone()
                        .unwrap_or_else(|| action_res.output.clone())
                },
                success: action_res.success,
                created_at: Utc::now(),
            };
            self.session_repo
                .append_turn(self.session_id, &res_turn)
                .await?;

            results.push(StructuredToolResult {
                call_id: call.id,
                tool_name: call.name,
                success: action_res.success,
                output: action_res.output,
                error: action_res.error,
                duration_ms,
                policy_decision: Some(format!(
                    "{:?}",
                    policy_decision.unwrap_or(PolicyDecision::Allow)
                )),
                approval_id: None,
                artifacts_created: Vec::new(),
                diagnostic,
            });
        }

        Ok(results)
    }

    /// Run the continuous autonomous loop until a waiting or terminal state is reached.
    ///
    /// Observes each turn outcome via the provided callback.
    pub async fn run_continuous<F>(&mut self, mut on_turn: F) -> Result<AgentEngineState, M31AError>
    where
        F: FnMut(&AgentTurnOutcome) -> Result<(), M31AError>,
    {
        while matches!(
            self.state,
            AgentEngineState::Running | AgentEngineState::Idle
        ) {
            let outcome = self.step(None).await?;
            on_turn(&outcome)?;

            match outcome {
                AgentTurnOutcome::AssistantCommentary { .. } => {
                    // Commentary produced during active execution; continuous autonomous execution continues
                }
                AgentTurnOutcome::ToolResults { .. } => {
                    // Tool results collected; continuous autonomous execution continues with fresh context
                }
                AgentTurnOutcome::AssistantText { .. } => {
                    // Model intentionally yielded control; pause for operator input
                    self.state = AgentEngineState::Idle;
                    break;
                }
                AgentTurnOutcome::WaitingForUser { .. }
                | AgentTurnOutcome::WaitingForApproval { .. }
                | AgentTurnOutcome::Completed { .. }
                | AgentTurnOutcome::Failed { .. }
                | AgentTurnOutcome::Cancelled { .. }
                | AgentTurnOutcome::BudgetExhausted { .. } => {
                    break;
                }
            }
        }

        Ok(self.state.clone())
    }

    /// Run the continuous interactive agent loop until a paused or terminal state is reached.
    pub async fn run_until_terminal(&mut self) -> Result<AgentEngineState, M31AError> {
        self.run_continuous(|_| Ok(())).await
    }
}
