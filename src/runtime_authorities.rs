//! Single canonical runtime authority set (wiring remediation v0.1.1).
//!
//! "The model proposes. The runtime decides."
//!
//! This module is the answer to the split-brain audit (`docs/audits/WIRING-AUDIT-v0.1.1.md`):
//! exactly ONE authoritative instance of each runtime-critical authority per runtime scope,
//! composed ONCE at the production composition root
//! (`AppRuntime::from_pool_workspace_and_config`) and consumed downstream by reference.
//!
//! ```text
//! configuration
//!     ↓
//! RuntimeAuthorities (this module: ONE per runtime scope)
//!     ↓
//! derived runtime services (controller, dispatcher, engines, coordinators)
//!     ↓
//! execution engines/controllers
//!     ↓
//! tool/model/policy/approval execution (single governance decision per action)
//! ```
//!
//! Mutation rule: authorities are immutable as a set. Reconfiguration builds a
//! COMPLETE new set (`reconfigured`) and swaps it atomically; partial `with_*`
//! mutation of related authorities is forbidden because it forks derived state
//! (stale model schemas, stale catalog locks, stale policy bindings).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::agent::profile::AgentProfile;
use crate::budget::enforcer::BudgetEnforcer;
use crate::capability::registry::CapabilityRegistry;
use crate::config::ResolvedConfiguration;
use crate::deployment::{DeploymentChannel, DeploymentPaths};
use crate::error::M31AError;
use crate::events::bus::BroadcastEventBus;
use crate::ids::{AgentId, ApprovalRequestId, MissionId, SessionId, TaskId};
use crate::policy::approval::ApprovalCoordinator;
use crate::policy::effective::EffectivePolicy;
use crate::state::intake::AutonomyMode;
use crate::state_machine::agent::AgentRole;
use crate::tools::definition::ToolExecutionContext;

// ── Model invocation kind ────────────────────────────────────────────────────

/// Typed model-invocation purpose. Replaces prompt-text sniffing
/// (`prompt.contains("ROLE: ...")`) as the authority for tool visibility.
///
/// `Unspecified` is a documented compatibility fallback for call paths that
/// have not yet been migrated to pass an explicit kind; new code MUST pass an
/// explicit kind. The legacy heuristic lives in exactly one place
/// (`is_legacy_structured_prompt`) and is only consulted for `Unspecified`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ModelInvocationKind {
    Planning,
    Discovery,
    Implementation,
    Review,
    Verification,
    Recovery,
    Other,
    /// Compatibility only: apply the single legacy prompt heuristic.
    Unspecified,
}

impl ModelInvocationKind {
    /// Whether invocations of this kind are tool-free by construction.
    ///
    /// Planning, discovery, review, and verification reason over assembled
    /// context and MUST NOT receive executable tool schemas. Implementation,
    /// recovery, and explicitly-typed other invocations receive the governed
    /// tool set. `Unspecified` defers to the legacy heuristic (compat only).
    pub fn is_tool_free_by_default(&self) -> bool {
        matches!(
            self,
            Self::Planning | Self::Discovery | Self::Review | Self::Verification
        )
    }
}

/// Typed invocation context carried alongside every model call that influences
/// execution authority. Structured state — never inferred from prompt text.
#[derive(Debug, Clone)]
pub struct ModelInvocationContext {
    pub role: AgentRole,
    pub invocation_kind: ModelInvocationKind,
    pub autonomy_mode: AutonomyMode,
    pub mission_id: Option<MissionId>,
    pub task_id: Option<TaskId>,
    pub agent_id: Option<AgentId>,
}

impl ModelInvocationContext {
    pub fn new(
        role: AgentRole,
        invocation_kind: ModelInvocationKind,
        autonomy_mode: AutonomyMode,
    ) -> Self {
        Self {
            role,
            invocation_kind,
            autonomy_mode,
            mission_id: None,
            task_id: None,
            agent_id: None,
        }
    }

    pub fn with_mission_id(mut self, mission_id: MissionId) -> Self {
        self.mission_id = Some(mission_id);
        self
    }

    pub fn with_task_id(mut self, task_id: TaskId) -> Self {
        self.task_id = Some(task_id);
        self
    }

    pub fn with_agent_id(mut self, agent_id: AgentId) -> Self {
        self.agent_id = Some(agent_id);
        self
    }

    /// Whether this invocation must be served WITHOUT executable tool schemas.
    pub fn requires_tool_free(&self) -> bool {
        self.invocation_kind.is_tool_free_by_default()
    }
}

/// The single legacy prompt-text heuristic, kept for `Unspecified` invocations
/// only. Any behavior change here is intentional and versioned; do NOT scatter
/// additional `contains(...)` rules at call sites.
pub fn is_legacy_structured_prompt(context: &str) -> bool {
    context.contains("ROLE: Discovery Analyst")
        || context.contains("ROLE: Lead Planner")
        || context.contains("ROLE: Task Decomposition Planner")
        || context.contains("ROLE: Plan Revision Architect")
        || context.contains("Decompose the following mission objective")
        || context.contains("Decompose the goal into an acyclic")
        || context.contains("genesis.dynamic_questions")
        || context.contains("planning.decompose")
        || context.contains("planning.revision")
        || context.contains("planning.task_revision")
        || context.contains("planning/plan_dag")
        || context.contains("planning/candidate_plan")
        || context.contains("planning/candidate_tasks")
        || context.contains("genesis/dynamic_questions")
}

// ── Autonomy precedence ──────────────────────────────────────────────────────

/// Documented autonomy precedence: task binding > session binding > runtime default.
///
/// Roles NEVER override autonomy: a role grants tool visibility (capability
/// envelope), never execution latitude. Profile names map to modes in exactly
/// one place (`from_profile_name`); `run_mission` and engine construction share it.
pub struct AutonomyPrecedence;

impl AutonomyPrecedence {
    /// Map a profile/session name to its autonomy mode. Single definition site.
    pub fn from_profile_name(profile: Option<&str>) -> AutonomyMode {
        match profile {
            Some("autonomous") | Some("full") => AutonomyMode::Autonomous,
            Some("guided") | Some("assisted") => AutonomyMode::Assisted,
            Some("unattended") => AutonomyMode::Unattended,
            Some("plan") => AutonomyMode::Plan,
            _ => AutonomyMode::Safe,
        }
    }

    /// Derive the session-level autonomy from authoritative configuration.
    pub fn from_config(config: &ResolvedConfiguration) -> AutonomyMode {
        Self::from_profile_name(config.active_profile.as_deref())
    }

    /// Resolve the effective execution mode: task > session > runtime default.
    pub fn resolve(
        task: Option<AutonomyMode>,
        session: Option<AutonomyMode>,
        runtime_default: AutonomyMode,
    ) -> AutonomyMode {
        task.or(session).unwrap_or(runtime_default)
    }
}

// ── Unified credential resolution ────────────────────────────────────────────

/// Where runtime credentials came from. Exactly one precedence, one binding path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialSource {
    /// Channel-aware workspace credential store (never cross-channel).
    ChannelFile(PathBuf),
    /// Explicit process environment variable.
    Environment(&'static str),
    /// No credential material found.
    Absent,
}

/// Supported precedence, highest first:
/// channel credential file (`nvidia_nim` key) → `NVIDIA_API_KEY` → `API_KEY_NVIDIA`.
/// The development channel NEVER reads the production credential file.
#[derive(Debug, Clone)]
pub struct CredentialResolution {
    pub api_key: Option<String>,
    pub source: CredentialSource,
}

/// Single authoritative credential resolver for all runtime provider bindings.
///
/// Used by `AppRuntime`, the worker dispatcher, provider creation, CLI/TUI
/// status, and startup validation. Environment probing (`SafeEnvironmentStatus`)
/// remains available for DIAGNOSTICS ONLY and must never override this binding.
pub fn resolve_runtime_credentials(
    workspace_root: &Path,
    channel: DeploymentChannel,
) -> CredentialResolution {
    let path = DeploymentPaths::project_credentials_file(workspace_root, channel);
    if let Ok(data) = std::fs::read_to_string(&path)
        && let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&data)
        && let Some(key) = map.get("nvidia_nim").cloned()
        && !key.trim().is_empty()
    {
        return CredentialResolution {
            api_key: Some(key),
            source: CredentialSource::ChannelFile(path),
        };
    }
    for env_name in ["NVIDIA_API_KEY", "API_KEY_NVIDIA"] {
        if let Ok(value) = std::env::var(env_name)
            && !value.trim().is_empty()
        {
            return CredentialResolution {
                api_key: Some(value),
                source: CredentialSource::Environment(env_name),
            };
        }
    }
    CredentialResolution {
        api_key: None,
        source: CredentialSource::Absent,
    }
}

// ── RuntimeAuthorities ───────────────────────────────────────────────────────

/// The single authoritative runtime dependency set for one runtime scope.
///
/// Owns every runtime-critical authority required by production execution.
/// Constructed ONCE at the production composition root; every downstream
/// consumer (controller, dispatcher, engines, coordinators, CLI/TUI handlers)
/// receives `Arc` clones from this set and MUST NOT construct competing
/// authorities.
///
/// Scope contract per member is documented on the accessor.
#[derive(Clone)]
pub struct RuntimeAuthorities {
    config: Arc<ResolvedConfiguration>,
    policy: Arc<EffectivePolicy>,
    capability_registry: Arc<CapabilityRegistry>,
    tool_registry: Arc<crate::tools::registry::ToolRegistry>,
    budget: Arc<BudgetEnforcer>,
    approval_coordinator: Arc<ApprovalCoordinator>,
    model_catalog: Arc<tokio::sync::RwLock<crate::model::catalog::ModelCatalog>>,
    model_provider: Option<Arc<dyn crate::model::provider::ModelProvider>>,
    model_caller: Option<Arc<dyn crate::agent::model_policy::ModelCaller>>,
    context_compiler: Arc<dyn crate::kernel::seams::ContextCompiler>,
    prompt_catalog: Arc<crate::prompt::InMemoryPromptCatalog>,
    artifact_store: Arc<crate::persistence::artifacts::FsArtifactStore>,
    event_bus: Arc<BroadcastEventBus>,
    git_service: Arc<dyn crate::capability::traits::git::GitService>,
    workspace_root: PathBuf,
    storage_root: PathBuf,
    channel: DeploymentChannel,
}

impl RuntimeAuthorities {
    /// Pack an already-composed authority set. The composition root is the ONLY
    /// production caller; every member must be the runtime-shared instance.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: Arc<ResolvedConfiguration>,
        policy: Arc<EffectivePolicy>,
        capability_registry: Arc<CapabilityRegistry>,
        tool_registry: Arc<crate::tools::registry::ToolRegistry>,
        budget: Arc<BudgetEnforcer>,
        approval_coordinator: Arc<ApprovalCoordinator>,
        model_catalog: Arc<tokio::sync::RwLock<crate::model::catalog::ModelCatalog>>,
        model_provider: Option<Arc<dyn crate::model::provider::ModelProvider>>,
        model_caller: Option<Arc<dyn crate::agent::model_policy::ModelCaller>>,
        context_compiler: Arc<dyn crate::kernel::seams::ContextCompiler>,
        prompt_catalog: Arc<crate::prompt::InMemoryPromptCatalog>,
        artifact_store: Arc<crate::persistence::artifacts::FsArtifactStore>,
        event_bus: Arc<BroadcastEventBus>,
        git_service: Arc<dyn crate::capability::traits::git::GitService>,
        workspace_root: PathBuf,
        storage_root: PathBuf,
        channel: DeploymentChannel,
    ) -> Self {
        Self {
            config,
            policy,
            capability_registry,
            tool_registry,
            budget,
            approval_coordinator,
            model_catalog,
            model_provider,
            model_caller,
            context_compiler,
            prompt_catalog,
            artifact_store,
            event_bus,
            git_service,
            workspace_root,
            storage_root,
            channel,
        }
    }

    /// Atomically reconfigure for a new resolved configuration (preferred
    /// `with_config` semantics): rebuilds EVERY config-derived authority
    /// (policy, model caller) from the new config while preserving shared
    /// mutable state (budget counters, catalog contents, approval waiters).
    ///
    /// Members that do not derive from configuration (registries, catalog
    /// lock, coordinator, compiler, stores, bus) are carried over by `Arc`
    /// clone so no second instance can fork.
    pub fn reconfigured(&self, config: Arc<ResolvedConfiguration>) -> Self {
        let policy = Arc::new(EffectivePolicy::standard_with_policy_config(
            &self.workspace_root,
            Some(&config.app_config.policy),
        ));
        let model_caller = match self.model_provider.clone() {
            Some(provider) => {
                let tool_schemas = Self::governed_tool_schemas(
                    &self.capability_registry,
                    &self.tool_registry,
                    &config.app_config.policy.denied_tools,
                    AutonomyPrecedence::from_config(&config),
                );
                Some(Arc::new(
                    crate::agent::model_policy::RoutedModelCaller::new(
                        Some(provider),
                        crate::model::router::resolver::ModelTier::Standard,
                        tool_schemas,
                    )
                    .with_catalog_lock(self.model_catalog.clone())
                    .with_model(config.active_model.clone())
                    .with_provider_status(
                        config.active_provider.clone(),
                        crate::model::types::ProviderCapabilityStatus::Available,
                    ),
                )
                    as Arc<dyn crate::agent::model_policy::ModelCaller>)
            }
            // Fail closed: no provider means no caller. A stale caller holding
            // previous provider/model wiring must never survive reconfiguration.
            None => None,
        };
        Self {
            config,
            policy,
            model_caller,
            ..self.clone()
        }
    }

    /// Rebind the model catalog lock AND rebuild the dependent model caller
    /// atomically, so the caller can never observe a stale catalog.
    pub fn with_catalog_rebound(&self, catalog: crate::model::catalog::ModelCatalog) -> Self {
        let model_catalog = Arc::new(tokio::sync::RwLock::new(catalog));
        let config = self.config.clone();
        let model_caller = match self.model_provider.clone() {
            Some(provider) => {
                let tool_schemas = Self::governed_tool_schemas(
                    &self.capability_registry,
                    &self.tool_registry,
                    &config.app_config.policy.denied_tools,
                    AutonomyPrecedence::from_config(&config),
                );
                Some(Arc::new(
                    crate::agent::model_policy::RoutedModelCaller::new(
                        Some(provider),
                        crate::model::router::resolver::ModelTier::Standard,
                        tool_schemas,
                    )
                    .with_catalog_lock(model_catalog.clone())
                    .with_model(config.active_model.clone())
                    .with_provider_status(
                        config.active_provider.clone(),
                        crate::model::types::ProviderCapabilityStatus::Available,
                    ),
                )
                    as Arc<dyn crate::agent::model_policy::ModelCaller>)
            }
            None => None,
        };
        Self {
            model_catalog,
            model_caller,
            ..self.clone()
        }
    }

    /// Single canonical governed tool-schema derivation: the SAME capability +
    /// tool authorities used for execution, filtered by the role envelope,
    /// policy denials, and autonomy mode. Model-visible tools and execution
    /// tools can never diverge into separate registries through this path.
    pub fn governed_tool_schemas(
        capability_registry: &Arc<CapabilityRegistry>,
        tool_registry: &Arc<crate::tools::registry::ToolRegistry>,
        denied_tools: &[String],
        autonomy_mode: AutonomyMode,
    ) -> Vec<serde_json::Value> {
        let profile = AgentProfile::built_in(crate::state_machine::agent::AgentRole::implementer());
        let criteria = crate::tools::filter::FilterCriteria::new(capability_registry.clone())
            .with_role_envelope(&profile.capability_policy)
            .with_denied_tools(denied_tools.iter().cloned())
            .with_autonomy_mode(autonomy_mode);
        crate::tools::filter::ToolFilter::new(tool_registry.clone())
            .filter_to_wire_format(&criteria)
    }

    /// Schemas from THIS authority set for the implementer envelope.
    pub fn model_tool_schemas(&self) -> Vec<serde_json::Value> {
        Self::governed_tool_schemas(
            &self.capability_registry,
            &self.tool_registry,
            &self.config.app_config.policy.denied_tools,
            AutonomyPrecedence::from_config(&self.config),
        )
    }

    /// Authoritative resolved configuration (RUNTIME_SHARED immutable).
    pub fn config(&self) -> &Arc<ResolvedConfiguration> {
        &self.config
    }
    /// Effective security policy (RUNTIME_SHARED immutable; replaced on reconfigure).
    pub fn policy(&self) -> &Arc<EffectivePolicy> {
        &self.policy
    }
    /// Canonical capability environment (RUNTIME_SHARED).
    pub fn capability_registry(&self) -> &Arc<CapabilityRegistry> {
        &self.capability_registry
    }
    /// Canonical tool inventory over the shared capabilities (RUNTIME_SHARED).
    pub fn tool_registry(&self) -> &Arc<crate::tools::registry::ToolRegistry> {
        &self.tool_registry
    }
    /// Two-phase budget authority (RUNTIME_SHARED mutable via `update_limits`).
    pub fn budget(&self) -> &Arc<BudgetEnforcer> {
        &self.budget
    }
    /// Human authorization authority (RUNTIME_SHARED; waiters survive reconfigure).
    pub fn approval_coordinator(&self) -> &Arc<ApprovalCoordinator> {
        &self.approval_coordinator
    }
    /// Pending approval request IDs currently owned by the coordinator.
    /// Every user-visible approval ID MUST be a member of this set (Invariant 4).
    pub async fn pending_approval_ids(&self) -> Vec<ApprovalRequestId> {
        self.approval_coordinator.pending_request_ids().await
    }
    /// Shared dynamic model catalog lock (single binding; rebound atomically).
    pub fn model_catalog(&self) -> &Arc<tokio::sync::RwLock<crate::model::catalog::ModelCatalog>> {
        &self.model_catalog
    }
    /// Bound model provider, if credentials/configuration resolved one.
    pub fn model_provider(&self) -> Option<Arc<dyn crate::model::provider::ModelProvider>> {
        self.model_provider.clone()
    }
    /// Authoritative model caller (derived from provider + shared registries).
    pub fn model_caller(&self) -> Option<Arc<dyn crate::agent::model_policy::ModelCaller>> {
        self.model_caller.clone()
    }
    /// Canonical context compiler (workspace + prompt catalog + memory + role-stage).
    pub fn context_compiler(&self) -> &Arc<dyn crate::kernel::seams::ContextCompiler> {
        &self.context_compiler
    }
    /// Canonical prompt catalog (RUNTIME_SHARED immutable).
    pub fn prompt_catalog(&self) -> &Arc<crate::prompt::InMemoryPromptCatalog> {
        &self.prompt_catalog
    }
    /// Canonical prompt catalog as the engine-facing trait object.
    pub fn prompt_catalog_arc(&self) -> Arc<dyn crate::prompt::PromptCatalog> {
        self.prompt_catalog.clone() as Arc<dyn crate::prompt::PromptCatalog>
    }
    /// Canonical artifact authority (PROCESS_SHARED stateless file authority).
    pub fn artifact_store(&self) -> &Arc<crate::persistence::artifacts::FsArtifactStore> {
        &self.artifact_store
    }
    /// Central broadcast event bus (RUNTIME_SHARED).
    pub fn event_bus(&self) -> &Arc<BroadcastEventBus> {
        &self.event_bus
    }
    /// Centralized git service (capability boundary).
    pub fn git_service(&self) -> Arc<dyn crate::capability::traits::git::GitService> {
        self.git_service.clone()
    }
    /// Canonical workspace root.
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }
    /// Channel-aware storage root.
    pub fn storage_root(&self) -> &Path {
        &self.storage_root
    }
    /// Deployment channel this authority set was composed for.
    pub fn channel(&self) -> DeploymentChannel {
        self.channel
    }
}

// ── RuntimeBinding ───────────────────────────────────────────────────────────

/// Typed binding of one execution scope to the authority set.
///
/// An execution context can never accidentally omit identity/authority that
/// already exists upstream: every construction path funnels through this
/// binding, and `execution_context()` always emits a FULLY BOUND context
/// (role envelope + mission + task + agent whenever known).
#[derive(Clone)]
pub struct RuntimeBinding {
    pub authorities: Arc<RuntimeAuthorities>,
    pub session_id: SessionId,
    pub mission_id: Option<MissionId>,
    pub task_id: Option<TaskId>,
    pub agent_id: Option<AgentId>,
    pub role: AgentRole,
    pub autonomy_mode: AutonomyMode,
    pub invocation_kind: ModelInvocationKind,
}

impl RuntimeBinding {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        authorities: Arc<RuntimeAuthorities>,
        session_id: SessionId,
        mission_id: Option<MissionId>,
        task_id: Option<TaskId>,
        agent_id: Option<AgentId>,
        role: AgentRole,
        autonomy_mode: AutonomyMode,
    ) -> Self {
        Self {
            authorities,
            session_id,
            mission_id,
            task_id,
            agent_id,
            role,
            autonomy_mode,
            invocation_kind: ModelInvocationKind::Implementation,
        }
    }

    pub fn with_invocation_kind(mut self, kind: ModelInvocationKind) -> Self {
        self.invocation_kind = kind;
        self
    }

    /// Build the fully-bound tool execution context for this scope.
    pub fn execution_context(&self, cancellation: CancellationToken) -> ToolExecutionContext {
        let envelope = AgentProfile::built_in(self.role.clone()).capability_policy;
        let mut ctx = ToolExecutionContext::new(
            self.authorities.capability_registry().clone(),
            self.authorities.workspace_root().to_path_buf(),
            cancellation,
        )
        .with_role_envelope(envelope);
        if let Some(mission_id) = self.mission_id {
            ctx = ctx.with_mission_id(mission_id);
        }
        if let Some(task_id) = self.task_id {
            ctx = ctx.with_task_id(task_id);
        }
        if let Some(agent_id) = self.agent_id {
            ctx = ctx.with_agent_id(agent_id);
        }
        ctx
    }

    /// Build the typed model-invocation context for this scope.
    pub fn invocation_context(&self) -> ModelInvocationContext {
        let mut ctx = ModelInvocationContext::new(
            self.role.clone(),
            self.invocation_kind,
            self.autonomy_mode,
        );
        if let Some(mission_id) = self.mission_id {
            ctx = ctx.with_mission_id(mission_id);
        }
        if let Some(task_id) = self.task_id {
            ctx = ctx.with_task_id(task_id);
        }
        if let Some(agent_id) = self.agent_id {
            ctx = ctx.with_agent_id(agent_id);
        }
        ctx
    }
}

/// Fail-closed runtime-assembly error: execution callers MUST propagate this,
/// never convert it into a synthetic success (Invariant 7).
pub fn runtime_unavailable(reason: impl Into<String>) -> M31AError {
    M31AError::internal(format!("runtime unavailable: {}", reason.into()))
}
