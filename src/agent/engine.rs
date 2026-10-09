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
    AssumptionInvalidation, FormedTaskStatus, IntentError, IntentState, IntentUnknown,
    SteeringConstraint, TaskShape, UnknownResolution,
};
use crate::agent::intent_repository::SqliteIntentRepository;
use crate::agent::model_policy::ModelCaller;
use crate::agent::profile::AgentProfile;
use crate::agent::runner::ActionRequest;
use crate::capability::registry::CapabilityRegistry;
use crate::error::M31AError;
use crate::events::bus::EventBus;
use crate::ids::{AgentId, MissionId, SessionId, TaskId};
use crate::interaction::session::{ConversationTurn, SqliteSessionRepository};
use crate::kernel::seams::context::{ContextCompilationRequest, ContextCompiler};
use crate::kernel::seams::policy::PolicyGate;
use crate::kernel::seams::recovery::FailureClassification;
use crate::model::types::{ChatMessage, ModelProposal, ModelToolCall, UserOption};
use crate::pipeline::runner::ToolPipelineRunner;
use crate::policy::approval::ApprovalCoordinator;
use crate::prompt::catalog::PromptCatalog;
use crate::runtime_authorities::ModelInvocationKind;
use crate::state::intake::AutonomyMode;
use crate::state_machine::agent::AgentRole;
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
///
/// Authority binding contract (wiring remediation v0.1.1):
/// - `capability_registry` is the RUNTIME-SHARED instance, received at
///   construction — never built inside execution paths.
/// - `active_role` is typed execution authority (`AgentRole`); the role
///   envelope is propagated into every `ToolExecutionContext`.
/// - `autonomy_mode` is bound by the runtime/session/task, never hardcoded.
/// - Policy governance is owned SOLELY by the pipeline (`ToolPipelineRunner`
///   stages 7-8). This engine performs no duplicate policy precheck.
/// - Approval flows through the authoritative `ApprovalCoordinator` inside the
///   pipeline; user-visible approval IDs are always real coordinator requests.
pub struct AgentEngine {
    session_id: SessionId,
    workspace_root: PathBuf,
    session_repo: SqliteSessionRepository,
    model_caller: Arc<dyn ModelCaller>,
    tool_registry: Arc<ToolRegistry>,
    capability_registry: Arc<CapabilityRegistry>,
    pipeline_runner: Arc<ToolPipelineRunner>,
    policy_gate: Arc<dyn PolicyGate>,
    approval_coordinator: Arc<ApprovalCoordinator>,
    completion_gate: Arc<EvidenceCompletionGate>,
    context_compiler: Arc<dyn ContextCompiler>,
    event_bus: Option<Arc<dyn EventBus>>,
    autonomy_mode: AutonomyMode,
    denied_tools: Vec<String>,
    /// Typed role authority for this engine instance (implementer by default;
    /// delegation rebinds it — never prompt text alone).
    active_role: AgentRole,
    state: AgentEngineState,
    turn_number: u32,
    max_turns: u32,
    delegation_depth: u32,
    cancel_token: CancellationToken,
    active_mission_id: Option<MissionId>,
    active_task_id: Option<TaskId>,
    active_agent_id: Option<AgentId>,
    /// Durable mission scope authority. When tool execution begins without a
    /// bound mission, a REAL mission row is created through this repository
    /// (never a zero-UUID placeholder): policy evaluation, approval
    /// persistence (FK-guarded), audit, and telemetry all require durable
    /// identities. `None` only in unit fixtures that never execute tools.
    mission_repo: Option<crate::persistence::sqlite::repositories::SqliteMissionRepository>,
    /// Durable task scope authority (same contract as `mission_repo`).
    task_repo: Option<crate::persistence::sqlite::repositories::SqliteTaskRepository>,
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
    /// When set, `compile_turn_messages` sources stable instructions from the
    /// role's versioned prompt contract instead of hardcoded Rust strings.
    /// Production MUST bind the runtime-shared catalog; compilation fails
    /// closed when it is absent (never a silent hardcoded fallback).
    prompt_catalog: Option<Arc<dyn PromptCatalog>>,
    /// Canonical prompt compiler for stable behavioral directive compilation.
    /// Production MUST bind the runtime-shared compiler so interactive and
    /// worker execution share one compilation authority.
    prompt_compiler: Option<Arc<dyn crate::prompt::PromptCompiler>>,
    /// Invocation provenance of the EffectivePrompt compiled for the most
    /// recent turn. Attached to the turn's model invocation context so every
    /// interactive model call carries auditable prompt provenance.
    last_prompt_provenance: Option<crate::prompt::provenance::PromptInvocationProvenance>,
    /// Optional streaming chunk sender for live UI token/fragment exposure.
    stream_chunk_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::model::types::StreamChunk>>,
}

impl AgentEngine {
    /// Construct a new AgentEngine bound to the runtime's authoritative dependencies.
    ///
    /// `capability_registry` MUST be the runtime-shared instance (the same `Arc`
    /// the runtime's tool registry and model schemas observe). `autonomy_mode`
    /// is set explicitly by the composition root via `with_autonomy_mode`;
    /// the constructor default is `Safe` (fail-closed) so an unbound engine
    /// can never execute with maximum latitude.
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
        capability_registry: Arc<CapabilityRegistry>,
    ) -> Self {
        Self {
            session_id,
            workspace_root,
            session_repo,
            model_caller,
            tool_registry,
            capability_registry,
            pipeline_runner,
            policy_gate,
            approval_coordinator,
            completion_gate,
            context_compiler,
            event_bus,
            autonomy_mode: AutonomyMode::Safe,
            denied_tools: Vec::new(),
            active_role: AgentRole::implementer(),
            state: AgentEngineState::Idle,
            turn_number: 0,
            max_turns: 50,
            delegation_depth: 0,
            cancel_token: CancellationToken::new(),
            active_mission_id: None,
            active_task_id: None,
            active_agent_id: None,
            mission_repo: None,
            task_repo: None,
            consecutive_text_turns: 0,
            recent_action_fingerprints: Vec::new(),
            intent_state: None,
            intent_repo: None,
            stall_detector: StallDetector::new(),
            adaptive_budget: AdaptiveBudget::default(),
            recent_diagnostics: Vec::new(),
            prompt_catalog: None,
            prompt_compiler: None,
            last_prompt_provenance: None,
            stream_chunk_tx: None,
        }
    }

    /// bind the effective execution autonomy (runtime/session/task precedence
    /// resolved by the caller via `AutonomyPrecedence::resolve`).
    pub fn with_autonomy_mode(mut self, mode: AutonomyMode) -> Self {
        self.autonomy_mode = mode;
        self.rebind_scoped_role_tools()
    }

    /// bind configured denied tools policy from runtime or session envelope.
    pub fn with_denied_tools(mut self, denied: Vec<String>) -> Self {
        self.denied_tools = denied;
        self.rebind_scoped_role_tools()
    }

    /// access the authoritative tool authority scope for the active role and envelope.
    pub fn tool_authority_scope(&self) -> crate::tools::filter::ToolAuthorityScope {
        let mut scope = crate::tools::filter::ToolAuthorityScope::new(
            self.active_role.clone(),
            self.capability_registry.clone(),
            self.tool_registry.clone(),
            self.autonomy_mode,
        )
        .with_denied_tools(self.denied_tools.clone());
        if let Some(agent_id) = self.active_agent_id {
            scope = scope.with_agent_id(agent_id);
        }
        scope
    }

    fn rebind_scoped_role_tools(mut self) -> Self {
        let scope = self.tool_authority_scope();
        let role_schemas = scope.model_tool_schemas();
        if let Some(rebound) = self
            .model_caller
            .bind_role_tools(&self.active_role, role_schemas)
        {
            self.model_caller = rebound;
        }
        self
    }

    /// bind typed role authority. changing the role changes the capability
    /// envelope and rebinds model-visible tool schemas via ToolAuthorityScope.
    pub fn with_role_authority(mut self, role: AgentRole) -> Self {
        self.active_role = role;
        self.rebind_scoped_role_tools()
    }

    /// Bind the agent identity for this engine instance.
    pub fn with_agent_identity(mut self, agent_id: AgentId) -> Self {
        self.active_agent_id = Some(agent_id);
        self
    }

    /// Bind the task identity for this engine instance.
    pub fn with_active_task(mut self, task_id: TaskId) -> Self {
        self.active_task_id = Some(task_id);
        self
    }

    /// Atomically bind the full execution identity (mission + task + agent).
    ///
    /// Policy evaluation, approval, audit events, telemetry, and completion
    /// verification observe these same identities because the execution
    /// context is derived from them (see `build_execution_context`).
    pub fn bind_execution_identity(
        mut self,
        mission_id: Option<MissionId>,
        task_id: Option<TaskId>,
        agent_id: Option<AgentId>,
    ) -> Self {
        self.active_mission_id = mission_id;
        self.active_task_id = task_id;
        self.active_agent_id = agent_id;
        self
    }

    /// bind model caller authority (e.g. for injected test model caller or live provider override).
    pub fn with_model_caller(mut self, caller: Arc<dyn ModelCaller>) -> Self {
        self.model_caller = caller;
        self.rebind_scoped_role_tools()
    }

    /// Access the runtime-shared capability registry (Invariant 1 guard).
    pub fn capability_registry(&self) -> &Arc<CapabilityRegistry> {
        &self.capability_registry
    }

    /// Attach durable mission/task scope repositories.
    ///
    /// Production engines receive these from the composition root
    /// (`AppRuntime::create_agent_engine`). Without them the engine cannot
    /// guarantee durable execution scope and tool execution fails closed
    /// instead of running under fictional identities.
    pub fn with_scope_repos(
        mut self,
        mission_repo: crate::persistence::sqlite::repositories::SqliteMissionRepository,
        task_repo: crate::persistence::sqlite::repositories::SqliteTaskRepository,
    ) -> Self {
        self.mission_repo = Some(mission_repo);
        self.task_repo = Some(task_repo);
        self
    }

    /// Access the bound pipeline runner Arc.
    pub fn pipeline_runner(&self) -> &Arc<ToolPipelineRunner> {
        &self.pipeline_runner
    }

    /// Ensure durable execution scope before side effects.
    ///
    /// Binds a REAL mission row (creating one from the session intent when
    /// unbound, and linking it back to the session) and a REAL task row under
    /// it. Approval persistence is FK-guarded on these rows: fictional
    /// zero-UUID identities would fail closed at the database, so scope is
    /// established BEFORE the pipeline runs. Agent identity binds only to
    /// real registered-agent rows (dispatcher flows); interactive engines
    /// execute as the operator with `None` agent scope.
    async fn ensure_execution_scope(&mut self) -> Result<(), M31AError> {
        if self.active_mission_id.is_none() {
            let repo = self.mission_repo.clone().ok_or_else(|| {
                M31AError::validation(
                    "tool execution requires durable mission scope: no mission bound and no mission repository attached",
                )
            })?;
            let objective = self
                .intent_state
                .as_ref()
                .map(|intent| {
                    let prompt = intent.raw_prompt.trim();
                    if prompt.len() > 200 {
                        format!("{}…", &prompt[..200])
                    } else if prompt.is_empty() {
                        format!("Interactive agent session {}", self.session_id)
                    } else {
                        prompt.to_string()
                    }
                })
                .unwrap_or_else(|| format!("Interactive agent session {}", self.session_id));
            let mission_id = MissionId::new();
            let mission = crate::state::Mission::new(mission_id, objective);
            repo.insert(&mission).await?;
            self.active_mission_id = Some(mission_id);
            // Link scope back to the durable session so restarts restore it
            // via `load_session_state` instead of forking a second mission.
            let _ = self
                .session_repo
                .set_active_mission(self.session_id, mission_id)
                .await;
        }
        if self.active_task_id.is_none()
            && let (Some(mission_id), Some(task_repo)) =
                (self.active_mission_id, self.task_repo.clone())
        {
            let existing_tasks = task_repo
                .list_by_mission(mission_id)
                .await
                .unwrap_or_default();
            if let Some(active) = existing_tasks.into_iter().find(|t| {
                matches!(
                    t.status,
                    crate::state_machine::TaskState::Running
                        | crate::state_machine::TaskState::Ready
                        | crate::state_machine::TaskState::Pending
                )
            }) {
                self.active_task_id = Some(active.id);
            } else {
                let task_id = TaskId::new();
                let title = format!("Agent turn scope {}", self.turn_number);
                let task = crate::state::Task::new(task_id, mission_id, title);
                // A task-row write failure degrades to unbound task scope (the
                // mission link still holds for approval persistence); it never
                // blocks execution with a fictional identity.
                if task_repo.insert(&task).await.is_ok() {
                    self.active_task_id = Some(task_id);
                }
            }
        }
        Ok(())
    }

    /// Access the runtime-shared tool registry (Invariant 2 guard).
    pub fn tool_registry(&self) -> &Arc<ToolRegistry> {
        &self.tool_registry
    }

    /// Access the bound typed role authority.
    pub fn active_role(&self) -> &AgentRole {
        &self.active_role
    }

    /// Access the bound execution autonomy mode.
    pub fn autonomy_mode(&self) -> AutonomyMode {
        self.autonomy_mode
    }

    /// Access the bound mission identity, if any.
    pub fn active_mission_id(&self) -> Option<MissionId> {
        self.active_mission_id
    }

    /// Access the bound task identity, if any.
    pub fn active_task_id(&self) -> Option<TaskId> {
        self.active_task_id
    }

    /// Access the bound agent identity, if any.
    pub fn active_agent_id(&self) -> Option<AgentId> {
        self.active_agent_id
    }

    /// Build the fully-bound tool execution context for this engine's scope.
    ///
    /// Carries the SAME authoritative role (envelope), mission, task, and
    /// agent identity that policy evaluation, approval, audit, telemetry, and
    /// completion verification observe. The capability registry is the
    /// runtime-shared instance — never a per-batch fork.
    pub fn policy_hash(&self) -> String {
        self.policy_gate
            .policy_hash()
            .unwrap_or_else(|| "uncompiled-policy".to_string())
    }

    pub fn build_execution_context(&self) -> ToolExecutionContext {
        let envelope = AgentProfile::built_in(self.active_role.clone()).capability_policy;
        let mut ctx = ToolExecutionContext::new(
            self.capability_registry.clone(),
            self.workspace_root.clone(),
            self.cancel_token.clone(),
        )
        .with_role_envelope(envelope)
        .with_agent_role(self.active_role.clone())
        .with_autonomy_mode(self.autonomy_mode)
        .with_policy_hash(self.policy_hash());
        if let Some(mission_id) = self.active_mission_id {
            ctx = ctx.with_mission_id(mission_id);
        }
        if let Some(task_id) = self.active_task_id {
            ctx = ctx.with_task_id(task_id);
        }
        if let Some(agent_id) = self.active_agent_id {
            ctx = ctx.with_agent_id(agent_id);
        }
        if let Some(ref repo) = self.task_repo {
            ctx = ctx.with_task_repo(repo.clone());
        }
        ctx
    }

    /// Typed model-invocation context for implementation turns.
    pub fn invocation_context(&self) -> crate::runtime_authorities::ModelInvocationContext {
        let mut ctx = crate::runtime_authorities::ModelInvocationContext::new(
            self.active_role.clone(),
            ModelInvocationKind::Implementation,
            self.autonomy_mode,
        );
        if let Some(mission_id) = self.active_mission_id {
            ctx = ctx.with_mission_id(mission_id);
        }
        if let Some(task_id) = self.active_task_id {
            ctx = ctx.with_task_id(task_id);
        }
        if let Some(agent_id) = self.active_agent_id {
            ctx = ctx.with_agent_id(agent_id);
        }
        ctx
    }

    /// Attach a streaming chunk channel for live UI token/tool delta observation.
    pub fn with_stream_sender(
        mut self,
        tx: tokio::sync::mpsc::UnboundedSender<crate::model::types::StreamChunk>,
    ) -> Self {
        self.stream_chunk_tx = Some(tx);
        self
    }

    /// Wire the canonical prompt catalog for stable behavioral directive loading.
    ///
    /// Production MUST bind the runtime-shared catalog. Compilation fails
    /// closed when it is absent — hardcoded fallback prompts are forbidden.
    pub fn with_prompt_catalog(mut self, catalog: Arc<dyn PromptCatalog>) -> Self {
        self.prompt_catalog = Some(catalog);
        self
    }

    /// Wire the canonical prompt compiler for stable behavioral directive
    /// compilation.
    ///
    /// Production MUST bind the runtime-shared compiler so interactive and
    /// worker execution share one compilation authority.
    pub fn with_prompt_compiler(
        mut self,
        compiler: Arc<dyn crate::prompt::PromptCompiler>,
    ) -> Self {
        self.prompt_compiler = Some(compiler);
        self
    }

    /// Compile the stable behavioral layer through the canonical PromptOS
    /// chain (catalog → compiler → EffectivePrompt).
    ///
    /// The ACTIVE role selects its registry-bound prompt contract
    /// ([`PromptReference::for_role`](crate::prompt::PromptReference::for_role)):
    /// changing the role changes the compiled prompt — the implementer
    /// contract is never hardcoded. The contract canonicalizes through the
    /// catalog (v1 upgrades to the canonical v2 generation) and compiles
    /// through the runtime-shared [`PromptCompiler`](crate::prompt::PromptCompiler),
    /// the same authority worker execution uses. Interactive and worker
    /// execution may carry different dynamic context, but their static
    /// prompt authority is identical.
    ///
    /// Fails closed when the catalog/compiler is unbound, the contract is
    /// missing, parameters are missing, or compilation fails: the model
    /// invocation MUST NOT happen behind a substitute prompt. There is no
    /// hardcoded fallback body.
    fn compile_stable_layer(&mut self) -> Result<String, M31AError> {
        use crate::prompt::{CompilationOptions, PromptContext, PromptReference};

        let catalog = self.prompt_catalog.clone().ok_or_else(|| {
            M31AError::internal(
                "interactive prompt compilation requires the runtime-shared prompt catalog: \
                 no catalog bound to AgentEngine; refusing to substitute a hardcoded prompt",
            )
        })?;
        let compiler = self.prompt_compiler.clone().ok_or_else(|| {
            M31AError::internal(
                "interactive prompt compilation requires the runtime-shared prompt compiler: \
                 no compiler bound to AgentEngine; refusing to substitute a hardcoded prompt",
            )
        })?;

        // Authoritative role → prompt binding: the active role's registry
        // contract, canonicalized (v1 → canonical v2 where one exists).
        let prompt_ref = PromptReference::for_role(self.active_role.clone());
        let contract = catalog
            .resolve_canonical(&prompt_ref.id, prompt_ref.version)
            .map_err(|e| {
                M31AError::internal(format!(
                    "failed to resolve role prompt '{}' (v{}): {e}; \
                     refusing to substitute a hardcoded prompt",
                    prompt_ref.id, prompt_ref.version
                ))
            })?
            .clone();

        let stage = contract
            .stage
            .or_else(|| {
                crate::agent::registry::RoleRegistry::global()
                    .read()
                    .ok()
                    .and_then(|guard| guard.stage_for(&self.active_role))
            })
            .ok_or_else(|| {
                M31AError::internal(format!(
                    "unknown agent role '{}': no registered role definition; \
                 refusing to substitute a hardcoded prompt",
                    self.active_role.as_str()
                ))
            })?;

        let mission_label = self
            .active_mission_id
            .map(|m| m.to_string())
            .unwrap_or_else(|| format!("session:{}", self.session_id));
        let task_label = self
            .active_task_id
            .map(|t| t.to_string())
            .unwrap_or_else(|| format!("session:{}:turn:{}", self.session_id, self.turn_number));
        let task_objective = self
            .intent_state
            .as_ref()
            .map(|intent| intent.raw_prompt.clone())
            .filter(|prompt| !prompt.trim().is_empty())
            .unwrap_or_else(|| "Execute assigned engineering work autonomously.".to_string());

        let prompt_ctx = PromptContext::new(
            format!("ctx-{}-turn-{}", self.session_id, self.turn_number),
            mission_label,
            task_label,
            self.active_role.clone(),
            stage,
            task_objective,
        );

        let effective = compiler
            .compile_with_guidance(
                catalog.as_ref(),
                &contract,
                &prompt_ctx,
                &CompilationOptions::default(),
            )
            .map_err(|e| {
                M31AError::internal(format!(
                    "interactive prompt compilation failed for '{}' (v{}): {e}; \
                     model invocation must not proceed behind a substitute prompt",
                    contract.id, contract.version
                ))
            })?;

        // Provenance describes the prompt ACTUALLY compiled (resolved
        // contract identity, never a requested alias).
        if let Some(mut invocation) = effective.invocation_provenance() {
            if let Some(mission_id) = self.active_mission_id {
                invocation = invocation.with_mission_id(mission_id.to_string());
            }
            if let Some(task_id) = self.active_task_id {
                invocation = invocation.with_task_id(task_id.to_string());
            }
            if let Some(agent_id) = self.active_agent_id {
                invocation = invocation.with_agent_id(agent_id.to_string());
            }
            self.last_prompt_provenance = Some(invocation);
        }

        Ok(effective.system_prompt)
    }

    /// Provenance of the EffectivePrompt compiled for the most recent turn.
    pub fn last_prompt_provenance(
        &self,
    ) -> Option<&crate::prompt::provenance::PromptInvocationProvenance> {
        self.last_prompt_provenance.as_ref()
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
    ///
    /// The request ID MUST be a real coordinator request (created by the
    /// pipeline's approval stage). Resolution failures are hard execution
    /// errors and propagate — never silently ignored.
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

        let req_uuid = Uuid::parse_str(request_id).map_err(|e| {
            M31AError::validation(format!("invalid approval request id '{request_id}': {e}"))
        })?;
        self.approval_coordinator
            .resolve_request(req_uuid.into(), action, "operator")
            .await
            .map_err(|e| {
                M31AError::internal(format!(
                    "approval resolution failed for request '{request_id}': {e}"
                ))
            })?;

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
    pub async fn compile_turn_messages(&mut self) -> Result<Vec<ChatMessage>, M31AError> {
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

        let profile = AgentProfile::built_in(self.active_role.clone());
        let criteria = crate::tools::filter::FilterCriteria::new(self.capability_registry.clone())
            .with_role_envelope(&profile.capability_policy)
            .with_autonomy_mode(self.autonomy_mode);
        let allowed_tools = crate::tools::filter::ToolFilter::new(self.tool_registry.clone())
            .filter_tools(&criteria);
        let tools_list: Vec<String> = allowed_tools
            .iter()
            .map(|t| format!("- {}: {}", t.id(), t.description()))
            .collect();
        let tools_section = format!(
            "Available Tools (Role: {}):\n{}",
            self.active_role.as_str(),
            tools_list.join("\n")
        );

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

        // Stable behavioral instructions compiled through the canonical
        // PromptOS chain (catalog → compiler → EffectivePrompt) for the
        // ACTIVE role. Fails closed when prompt authority is unbound or
        // compilation fails: no hardcoded substitute prompt is ever used.
        // (PromptCompiler 7-layer pipeline; WorkerRunner DAG tasks compile
        // through the same catalog/compiler authorities with full task
        // context, while interactive turns carry session dynamic context.)
        let stable_instructions: String = self.compile_stable_layer()?;

        let scope_fragment = match (self.active_mission_id, self.active_task_id) {
            (Some(m), Some(t)) => format!("\nActive Mission: {}\nActive Task: {}", m, t),
            (Some(m), None) => format!("\nActive Mission: {}", m),
            _ => String::new(),
        };

        let system_prompt = format!(
            "Workspace Root: {}\n{}{}\n\n{}\n\n{}{}{}",
            self.workspace_root.display(),
            git_section,
            scope_fragment,
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
                    // P0-03: model context receives ONLY the model-visible
                    // (scrubbed) projection. Raw execution evidence must never
                    // enter ChatMessage::Tool content.
                    let scrubbed =
                        crate::telemetry::redactor::SecretRedactor::new().redact_text(&output);
                    let text = if success {
                        scrubbed
                    } else {
                        format!("Error: {scrubbed}")
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

        // 5. Compile fresh context from durable session using canonical ContextCompiler (Issue 1, Issue 6)
        let messages = self.compile_turn_messages().await?;
        let mission_id = self.active_mission_id.unwrap_or_default();
        let task_id = self.active_task_id.unwrap_or_default();
        let objective = self
            .intent_state
            .as_ref()
            .map(|is| is.raw_prompt.clone())
            .unwrap_or_else(|| "Execute assigned engineering work autonomously.".to_string());

        let mut req = ContextCompilationRequest::new(mission_id, task_id, 16384)
            .with_mission_objective(&objective)
            .with_task_objective(&objective)
            .with_role(self.active_role.clone())
            .with_workspace_root(self.workspace_root.clone())
            .with_interactive_messages(messages);

        if let Some(agent_id) = self.active_agent_id {
            req = req.with_agent_id(agent_id);
        }

        let compiled_context = self
            .context_compiler
            .compile_context(req)
            .await
            .map_err(|e| {
                M31AError::internal(format!("Canonical context compilation failed: {e}"))
            })?;

        self.last_prompt_provenance = compiled_context.prompt_provenance.clone();

        // 6. invoke model through canonical model caller
        let invocation_id = uuid::Uuid::now_v7();
        if let Some(ref tx) = self.stream_chunk_tx {
            let _ = tx.send(crate::model::types::StreamChunk::InvocationStarted { invocation_id });
        }

        let (proposal, usage) = match self
            .model_caller
            .call_model_with_context_and_usage_streaming(
                &compiled_context,
                &self.cancel_token,
                self.stream_chunk_tx.clone(),
            )
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

        let provider = self.model_caller.provider_name();
        let model = self.model_caller.model_name();
        let (cost_usd, cost_provenance) =
            crate::model::pricing::calculate_cost_from_usage(&provider, &model, &usage);

        // record model invocation in persistent repository if mission and task are bound
        if let (Some(mission_id), Some(task_id)) = (self.active_mission_id, self.active_task_id) {
            let agent_id = self.active_agent_id.unwrap_or_else(|| {
                let id = AgentId::new();
                self.active_agent_id = Some(id);
                id
            });
            let agent_repo = crate::persistence::sqlite::repositories::SqliteAgentRepository::new(
                self.session_repo.pool().clone(),
            );
            let agent =
                crate::state::Agent::new(agent_id, mission_id, self.active_role.to_string())
                    .with_runtime_details(
                        Some(task_id),
                        String::new(),
                        self.max_turns,
                        model.clone(),
                    );
            let _ = agent_repo.insert(&agent).await;

            let mut record = crate::model::persistence::invocation::ModelInvocationRecord::new(
                mission_id,
                task_id,
                agent_id,
                self.turn_number,
                &provider,
                &model,
                1,
                "success",
                &usage,
                "agent_engine_step",
            )
            .with_cost(cost_usd, cost_provenance);
            if let Some(prov) = self.last_prompt_provenance() {
                if let Ok(json_str) = serde_json::to_string(prov) {
                    record = record.with_prompt_provenance(json_str);
                }
            }
            let repo = crate::model::persistence::invocation::SqliteModelInvocationRepository::new(
                self.session_repo.pool().clone(),
            );
            let _ = repo.insert_invocation(&record).await;
        }

        if let Some(ref bus) = self.event_bus {
            let envelope = crate::events::envelope::EventEnvelope::new(
                0,
                self.active_mission_id,
                None,
                "agent_engine".to_string(),
                crate::events::types::EventType::ModelUsageUpdated {
                    invocation_id: Some(invocation_id),
                    mission_id: self.active_mission_id,
                    task_id: self.active_task_id,
                    provider,
                    model,
                    usage,
                    cumulative_usage: None,
                    cost_usd,
                    cost_provenance,
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
                // Typed delegation: the target role becomes the child's runtime
                // role authority (capability envelope, model role, invocation
                // context) — not merely prompt text. The child inherits the
                // parent mission; its task scope is created durably by
                // `ensure_execution_scope` (never fictional row-less IDs, which
                // would violate approval FK integrity). Agent identity stays
                // unbound (operator-driven delegation, no synthetic agent row).
                let delegated_role = AgentRole::new(target_role.clone());
                let child_agent_id = AgentId::new();
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
                    self.capability_registry.clone(),
                )
                .with_delegation_depth(self.delegation_depth + 1)
                .with_max_turns(10)
                .with_cancellation_token(self.cancel_token.clone())
                .with_denied_tools(self.denied_tools.clone())
                .with_autonomy_mode(self.autonomy_mode)
                .with_role_authority(delegated_role)
                .with_agent_identity(child_agent_id)
                .bind_execution_identity(
                    self.active_mission_id,
                    None,
                    Some(child_agent_id),
                );
                // Prompt authority inheritance: the child compiles through
                // the SAME catalog/compiler authorities as its parent — it
                // must never construct divergent prompt state. A parent
                // without bound authorities yields a child that fails
                // closed at prompt compilation (never a silent fallback).
                if let Some(ref catalog) = self.prompt_catalog {
                    subagent = subagent.with_prompt_catalog(catalog.clone());
                }
                if let Some(ref compiler) = self.prompt_compiler {
                    subagent = subagent.with_prompt_compiler(compiler.clone());
                }
                if let (Some(mission_repo), Some(task_repo)) =
                    (self.mission_repo.clone(), self.task_repo.clone())
                {
                    subagent = subagent.with_scope_repos(mission_repo.clone(), task_repo.clone());
                }

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
    ///
    /// Governance contract: the pipeline owns the SOLE policy evaluation
    /// (stage 7) and approval resolution (stage 8) for each action, operating
    /// on the fully-bound execution context (role envelope + mission + task +
    /// agent identities). This engine performs NO separate policy precheck —
    /// duplicate governance decisions for the same action are forbidden.
    /// Approval, when required, flows through the authoritative
    /// `ApprovalCoordinator` inside the pipeline, so every approval ID is a
    /// real coordinator request.
    async fn execute_tool_calls(
        &mut self,
        calls: Vec<ModelToolCall>,
    ) -> Result<Vec<StructuredToolResult>, M31AError> {
        let mut results = Vec::new();
        // Durable scope FIRST: no tool runs under fictional identities.
        self.ensure_execution_scope().await?;
        // Runtime-shared capability authority + bound identity. Never a
        // per-batch fork: model-visible tool schemas and execution observe
        // the same registry.
        let ctx = self.build_execution_context();

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

            // scope validation: model-visible tools and executable tools derive from the same ToolAuthorityScope
            let scope = self.tool_authority_scope();
            if !scope.executable_tools().contains(&call.name) {
                let err_msg = format!(
                    "tool '{}' is not permitted for role '{}' under active authority scope",
                    call.name, self.active_role
                );
                tracing::warn!(tool = %call.name, role = %self.active_role, "tool rejected by role authority scope");
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
                results.push(StructuredToolResult {
                    call_id: call.id,
                    tool_name: call.name,
                    success: false,
                    output: String::new(),
                    error: Some(err_msg),
                    duration_ms: start.elapsed().as_millis() as u64,
                    policy_decision: Some("scope_denied".to_string()),
                    approval_id: None,
                    artifacts_created: Vec::new(),
                    diagnostic: None,
                });
                continue;
            }

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

            // 2. Execute via ToolPipelineRunner through all 11 stages.
            // The pipeline performs the single authoritative policy
            // evaluation and approval resolution for this action. Blocking on
            // operator approval happens inside the coordinator (with timeout);
            // the operator-facing approval ID is the real coordinator request.
            let action_res = self
                .pipeline_runner
                .execute_action(
                    &action_req,
                    &ctx,
                    self.policy_gate.as_ref(),
                    self.autonomy_mode,
                )
                .await;

            // The pipeline decision is the single governance record for this
            // action: success implies authorization; denial is classified from
            // the authoritative pipeline error (never from a second evaluation).
            let action_error = action_res.error.clone().unwrap_or_default();
            let pipeline_denied = action_error.contains("POLICY_DENIED")
                || action_error.contains("denied by policy")
                || action_error.contains("denied execution")
                || action_error.starts_with("Policy denied");
            let recorded_policy_decision: Option<String> = if action_res.success {
                Some("allow".to_string())
            } else if pipeline_denied {
                Some("deny".to_string())
            } else {
                None
            };

            let duration_ms = start.elapsed().as_millis() as u64;

            // adapt_strategy is proposal-only: persistent replanning is owned strictly
            // by canonical replan_authority via agent_action::replan, never by inline tool execution.

            // Structured Diagnostic Evidence capture and loop observation
            let is_policy_denied = pipeline_denied;

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
                // Single-authority governance record: the pipeline's decision
                // for this action (allow / deny / None for non-governance
                // failures). Approval IDs are coordinator-owned; see
                // `pending_request_ids` for the authoritative request set.
                policy_decision: recorded_policy_decision,
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
    pub async fn run_continuous<F>(&mut self, on_turn: F) -> Result<AgentEngineState, M31AError>
    where
        F: FnMut(&AgentTurnOutcome) -> Result<(), M31AError>,
    {
        self.run_continuous_with_steering(None, on_turn).await
    }

    /// Run the continuous autonomous loop until a waiting or terminal state is reached,
    /// applying an optional initial steering input on the first turn.
    pub async fn run_continuous_with_steering<F>(
        &mut self,
        initial_steering: Option<&str>,
        mut on_turn: F,
    ) -> Result<AgentEngineState, M31AError>
    where
        F: FnMut(&AgentTurnOutcome) -> Result<(), M31AError>,
    {
        let mut first_turn = true;
        while matches!(
            self.state,
            AgentEngineState::Running | AgentEngineState::Idle
        ) {
            let steering = if first_turn {
                first_turn = false;
                initial_steering
            } else {
                None
            };
            let outcome = self.step(steering).await?;
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
