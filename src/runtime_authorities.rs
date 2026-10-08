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
    /// Global user-defined slash command execution (`/<command>`).
    ///
    /// A declarative orchestration contract authored by the user in
    /// `<global_config_dir>/prompts/commands/*.toml`. Invocations of this
    /// kind are tool-capable (like [`Self::Implementation`]): the command
    /// achieves its effects through governed tools, never through direct
    /// side effects. Capability requests declared in the TOML are advisory
    /// only — the canonical capability registry, policy gate, approval
    /// coordinator, and tool pipeline remain authoritative.
    UserCommand,
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

/// Typed model invocation binding prompt authority to a model call.
///
/// The production model-invocation API takes an [`EffectivePrompt`](crate::prompt::EffectivePrompt)
/// here — never a raw `system_prompt: &str` — so callers cannot bypass
/// PromptOS. The prompt reference, invocation kind, and autonomy mode are
/// structured authority state, never inferred from prompt text via
/// `contains(...)` heuristics.
#[derive(Debug, Clone)]
pub struct ModelInvocation {
    /// Effective prompt compiled by the canonical PromptCompiler.
    pub prompt: crate::prompt::EffectivePrompt,
    /// Explicit prompt reference that produced `prompt`.
    pub prompt_ref: crate::prompt::PromptReference,
    /// Executing role.
    pub role: AgentRole,
    /// Typed invocation purpose (planning vs. implementation vs. review …).
    pub invocation_kind: ModelInvocationKind,
    /// Effective execution latitude (task > session > runtime default).
    pub autonomy_mode: AutonomyMode,
    pub mission_id: Option<MissionId>,
    pub task_id: Option<TaskId>,
    pub agent_id: Option<AgentId>,
}

impl ModelInvocation {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        prompt: crate::prompt::EffectivePrompt,
        prompt_ref: crate::prompt::PromptReference,
        role: AgentRole,
        invocation_kind: ModelInvocationKind,
        autonomy_mode: AutonomyMode,
    ) -> Self {
        Self {
            prompt,
            prompt_ref,
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

    /// Build the typed invocation context for this model call.
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

    /// Fail-closed provenance gate: the compiled prompt MUST carry
    /// invocation provenance describing the exact contract compiled.
    /// A prompt without provenance MUST NOT be sent to a model.
    pub fn require_provenance(
        &self,
    ) -> Result<&crate::prompt::provenance::PromptInvocationProvenance, String> {
        self.prompt.provenance.as_ref().map(|p| &p.invocation).ok_or_else(|| {
            format!(
                "refusing model invocation for '{}' (v{}): effective prompt carries no provenance",
                self.prompt_ref.id, self.prompt_ref.version
            )
        })
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
    /// Canonical profiles own their mode via `ProfileResolver`; legacy
    /// autonomy-mode names (`plan`, `assisted`, `unattended`, `safe`) remain
    /// accepted as direct modes. Unknown or absent ids fail closed to `Safe`
    /// (intentionally unbound), never silently escalate.
    pub fn from_profile_name(profile: Option<&str>) -> AutonomyMode {
        let Some(raw) = profile else {
            return AutonomyMode::Safe;
        };
        let normalized = raw.trim().to_lowercase().replace('-', "_");
        // direct autonomy-mode names stay valid (session/CLI overrides)
        if let Ok(mode) = normalized.parse::<AutonomyMode>() {
            return mode;
        }
        match normalized.as_str() {
            // canonical profiles (resolver is authoritative; mapping mirrors
            // `ProfileResolver::canonical_autonomy_for_profile` for callers
            // without config access)
            "autonomous" => AutonomyMode::Autonomous,
            "balanced" | "coding" | "release" => AutonomyMode::Assisted,
            "ci" => AutonomyMode::Unattended,
            "safe" | "conservative" | "research" | "code_reviewer" | "security_review" => {
                AutonomyMode::Safe
            }
            // legacy aliases that predate the canonical profile universe
            "full" => AutonomyMode::Autonomous,
            "guided" => AutonomyMode::Assisted,
            _ => AutonomyMode::Safe,
        }
    }

    /// Derive the session-level autonomy from authoritative configuration.
    /// Precedence: explicit CLI `--autonomy` (Tier6 `autonomy_mode`) >
    /// canonical resolved profile's `autonomy_mode` > legacy name mapping.
    /// The profile's own metadata is authoritative; no second mapping table
    /// is consulted after resolution.
    pub fn from_config(config: &ResolvedConfiguration) -> AutonomyMode {
        if let Some(explicit) = config.provenance.resolve("autonomy_mode") {
            if let Some(s) = explicit.value.as_str()
                && let Ok(mode) = s.parse::<AutonomyMode>()
            {
                return mode;
            }
        }
        if let Some(ref prof) = config.active_profile {
            let normalized = prof.trim().to_lowercase().replace('-', "_");
            // prefer the resolver-owned metadata when the profile is known
            if let Ok(mode_str) =
                crate::config::profile::ProfileResolver::canonical_autonomy_for_profile(&normalized)
                && let Ok(mode) = mode_str.parse::<AutonomyMode>()
            {
                return mode;
            }
        }
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
    /// Canonical global user credential store (never cross-channel).
    GlobalFile(PathBuf),
    /// Legacy channel-aware workspace credential store (migration source).
    ChannelFile(PathBuf),
    /// Explicit process environment variable.
    Environment(&'static str),
    /// No credential material found.
    Absent,
}

/// Supported precedence, highest first:
/// global credential file (`nvidia_nim` key) → legacy workspace channel
/// file → `NVIDIA_API_KEY` → `API_KEY_NVIDIA`.
/// File material wins over environment (preserves pre-migration semantics);
/// the global store wins over the legacy workspace file (canonical source).
/// The development channel NEVER reads production credentials (global dirs
/// and legacy filenames are both channel-isolated).
#[derive(Debug, Clone)]
pub struct CredentialResolution {
    pub api_key: Option<String>,
    pub source: CredentialSource,
}

fn read_nvidia_key(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|data| serde_json::from_str::<HashMap<String, String>>(&data).ok())
        .and_then(|map| map.get("nvidia_nim").cloned())
        .filter(|k| !k.trim().is_empty())
}

/// Single authoritative credential resolver for all runtime provider bindings.
///
/// Used by `AppRuntime`, the worker dispatcher, provider creation, CLI/TUI
/// status, and startup validation. Environment probing (`SafeEnvironmentStatus`)
/// remains available for DIAGNOSTICS ONLY and must never override this binding.
///
/// Precedence: global user store → legacy workspace file (migration
/// fallback) → environment. File material wins over environment (legacy
/// semantics preserved); the global store wins over legacy (canonical).
/// The workspace file is never written by new code.
pub fn resolve_runtime_credentials(
    workspace_root: &Path,
    channel: DeploymentChannel,
) -> CredentialResolution {
    // 1. Canonical global user store (platform config, test-isolated).
    let global =
        crate::storage::StorageLayout::new(workspace_root, channel).global_credentials_file();
    if let Some(key) = read_nvidia_key(&global) {
        return CredentialResolution {
            api_key: Some(key),
            source: CredentialSource::GlobalFile(global),
        };
    }
    // 2. Legacy workspace file (migration fallback; never written anew).
    let legacy = DeploymentPaths::project_credentials_file(workspace_root, channel);
    if legacy != global
        && let Some(key) = read_nvidia_key(&legacy)
    {
        return CredentialResolution {
            api_key: Some(key),
            source: CredentialSource::ChannelFile(legacy),
        };
    }
    // 3. Explicit environment.
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
/// Single strongly-typed production execution authority bundle.
///
/// `RuntimeAuthorities` IS the execution authority bundle: one instance per
/// runtime scope owns every identity-bearing authority. Derived components
/// (controller, worker dispatcher, engines) MUST consume `Arc` clones from
/// this set and MUST NOT reconstruct competing authorities.
///
/// Scope separation is architectural:
/// - immutable/shared: policy, prompt catalog/compiler, event bus, channel
/// - shared mutable: budget counters, catalog contents, approval waiters
/// - workspace-scoped: capability registry root, context workspace binding
/// - mission-scoped: derived controller/dispatcher bound per mission/worktree
/// - ephemeral: per-dispatch cancellation tokens, outcomes (never in bundle)
pub type ExecutionAuthorities = RuntimeAuthorities;

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
    prompt_compiler: Arc<dyn crate::prompt::PromptCompiler>,
    tool_pipeline: Arc<crate::pipeline::runner::ToolPipelineRunner>,
    artifact_store: Arc<crate::persistence::artifacts::FsArtifactStore>,
    artifact_service: Arc<crate::persistence::artifacts::ArtifactService>,
    event_bus: Arc<BroadcastEventBus>,
    git_service: Arc<dyn crate::capability::traits::git::GitService>,
    /// Long-horizon engineering memory authority shared by canonical and
    /// worktree context compilers. Same `Arc` (same SQLite pool) so worktree
    /// missions observe identical memory semantics.
    memory_store: Arc<dyn crate::memory::EngineeringMemoryStore>,
    /// Durable pooled job supervisor shared by runtime and every scoped
    /// capability registry. Same `Arc` so submissions and restart
    /// reconciliation share ONE job lifecycle.
    job_supervisor: Arc<crate::process::job::JobSupervisor>,
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
        prompt_compiler: Arc<dyn crate::prompt::PromptCompiler>,
        tool_pipeline: Arc<crate::pipeline::runner::ToolPipelineRunner>,
        artifact_store: Arc<crate::persistence::artifacts::FsArtifactStore>,
        artifact_service: Arc<crate::persistence::artifacts::ArtifactService>,
        event_bus: Arc<BroadcastEventBus>,
        git_service: Arc<dyn crate::capability::traits::git::GitService>,
        memory_store: Arc<dyn crate::memory::EngineeringMemoryStore>,
        job_supervisor: Arc<crate::process::job::JobSupervisor>,
        workspace_root: PathBuf,
        storage_root: PathBuf,
        channel: DeploymentChannel,
    ) -> Self {
        // Fail closed on trust-root fork: the explicit auth authority (when
        // provided separately) must be the registry's own instance. Here the
        // authority is derived from the registry itself, so identity holds by
        // construction; scoped registries MUST be built via
        // `production_with_auth` with the canonical trust root.
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
            prompt_compiler,
            tool_pipeline,
            artifact_store,
            artifact_service,
            event_bus,
            git_service,
            memory_store,
            job_supervisor,
            workspace_root,
            storage_root,
            channel,
        }
    }

    /// Canonical fail-closed model caller for dispatcher execution.
    ///
    /// Returns the runtime-shared caller when bound, otherwise builds the
    /// typed misconfigured caller ONCE from the shared registries (same
    /// schemas the live caller would observe). Production dispatchers MUST
    /// use this instead of constructing their own `RoutedModelCaller`.
    pub fn canonical_model_caller_for_dispatch(
        &self,
        explicit: Option<Arc<dyn crate::agent::model_policy::ModelCaller>>,
    ) -> Arc<dyn crate::agent::model_policy::ModelCaller> {
        if let Some(caller) = explicit {
            return caller;
        }
        if let Some(caller) = self.model_caller.clone() {
            return caller;
        }
        let tool_schemas = Self::governed_tool_schemas(
            &self.capability_registry,
            &self.tool_registry,
            &self.config.app_config.policy.denied_tools,
            AutonomyPrecedence::from_config(&self.config),
        );
        Arc::new(
            crate::agent::model_policy::RoutedModelCaller::new(
                self.model_provider.clone(),
                crate::model::router::resolver::ModelTier::Standard,
                tool_schemas,
            )
            .with_model(self.config.active_model.clone())
            .with_provider_status(
                self.config.active_provider.clone(),
                crate::model::types::ProviderCapabilityStatus::Misconfigured,
            ),
        )
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
    /// Single canonical governed tool-schema derivation for a specific role:
    /// the SAME capability + tool authorities used for execution, filtered by
    /// the role envelope, policy denials, and autonomy mode (Issue 3).
    pub fn governed_tool_schemas_for_role(
        role: &crate::state_machine::agent::AgentRole,
        capability_registry: &Arc<CapabilityRegistry>,
        tool_registry: &Arc<crate::tools::registry::ToolRegistry>,
        denied_tools: &[String],
        autonomy_mode: AutonomyMode,
    ) -> Vec<serde_json::Value> {
        let profile = AgentProfile::built_in(role.clone());
        let criteria = crate::tools::filter::FilterCriteria::new(capability_registry.clone())
            .with_role_envelope(&profile.capability_policy)
            .with_denied_tools(denied_tools.iter().cloned())
            .with_autonomy_mode(autonomy_mode);
        crate::tools::filter::ToolFilter::new(tool_registry.clone())
            .filter_to_wire_format(&criteria)
    }

    /// Single canonical governed tool-schema derivation: the SAME capability +
    /// tool authorities used for execution, filtered by the role envelope,
    /// policy denials, and autonomy mode.
    pub fn governed_tool_schemas(
        capability_registry: &Arc<CapabilityRegistry>,
        tool_registry: &Arc<crate::tools::registry::ToolRegistry>,
        denied_tools: &[String],
        autonomy_mode: AutonomyMode,
    ) -> Vec<serde_json::Value> {
        Self::governed_tool_schemas_for_role(
            &crate::state_machine::agent::AgentRole::implementer(),
            capability_registry,
            tool_registry,
            denied_tools,
            autonomy_mode,
        )
    }

    /// Schemas from THIS authority set for a specific role envelope (Issue 3).
    pub fn model_tool_schemas_for_role(
        &self,
        role: &crate::state_machine::agent::AgentRole,
    ) -> Vec<serde_json::Value> {
        Self::governed_tool_schemas_for_role(
            role,
            &self.capability_registry,
            &self.tool_registry,
            &self.config.app_config.policy.denied_tools,
            AutonomyPrecedence::from_config(&self.config),
        )
    }

    /// authoritative scoped tool authority for a role (p0 tool authority convergence).
    pub fn tool_authority_scope_for_role(
        &self,
        role: &crate::state_machine::agent::AgentRole,
    ) -> crate::tools::filter::ToolAuthorityScope {
        crate::tools::filter::ToolAuthorityScope::new(
            role.clone(),
            self.capability_registry.clone(),
            self.tool_registry.clone(),
            AutonomyPrecedence::from_config(&self.config),
        )
        .with_denied_tools(self.config.app_config.policy.denied_tools.clone())
    }

    /// Schemas from THIS authority set for the implementer envelope.
    pub fn model_tool_schemas(&self) -> Vec<serde_json::Value> {
        self.model_tool_schemas_for_role(&crate::state_machine::agent::AgentRole::implementer())
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
    /// Canonical prompt compiler (RUNTIME_SHARED immutable).
    ///
    /// The single compilation authority for all production context
    /// generation: worker context compilation, interactive engine stable
    /// layers, review/diagnosis prompts, and planning prompts all compile
    /// through this instance. Reconfiguration carries it over untouched so
    /// derived engines can never observe a stale compiler.
    pub fn prompt_compiler(&self) -> &Arc<dyn crate::prompt::PromptCompiler> {
        &self.prompt_compiler
    }
    /// Canonical tool execution pipeline runner (Phase B single authority).
    pub fn tool_pipeline(&self) -> &Arc<crate::pipeline::runner::ToolPipelineRunner> {
        &self.tool_pipeline
    }
    /// Canonical prompt catalog as the engine-facing trait object.
    pub fn prompt_catalog_arc(&self) -> Arc<dyn crate::prompt::PromptCatalog> {
        self.prompt_catalog.clone() as Arc<dyn crate::prompt::PromptCatalog>
    }
    /// Canonical artifact authority (PROCESS_SHARED stateless file authority).
    pub fn artifact_store(&self) -> &Arc<crate::persistence::artifacts::FsArtifactStore> {
        &self.artifact_store
    }
    /// Canonical artifact lifecycle service (store + SQLite ledger).
    /// Capability-level artifact operations MUST go through this service
    /// (via `FsArtifactStoreProvider::from_service`) so tool writes are
    /// visible through the canonical ledger.
    pub fn artifact_service(&self) -> &Arc<crate::persistence::artifacts::ArtifactService> {
        &self.artifact_service
    }
    /// Long-horizon engineering memory authority (RUNTIME_SHARED).
    pub fn memory_store(&self) -> &Arc<dyn crate::memory::EngineeringMemoryStore> {
        &self.memory_store
    }
    /// Durable pooled job supervisor (RUNTIME_SHARED). Scoped capability
    /// registries MUST reuse this `Arc`, never `JobSupervisor::new(...)`.
    pub fn job_supervisor(&self) -> &Arc<crate::process::job::JobSupervisor> {
        &self.job_supervisor
    }
    /// Identity-bearing authorization trust root, derived from the canonical
    /// capability registry. `Arc::ptr_eq` with `runtime.auth_authority`,
    /// tool execution auth, and Git authorization must hold.
    pub fn auth_authority(&self) -> Arc<crate::git::AuthorizationAuthority> {
        self.capability_registry.authorization_authority().clone()
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
        .with_role_envelope(envelope)
        .with_agent_role(self.role.clone())
        .with_autonomy_mode(self.autonomy_mode)
        .with_policy_hash(self.authorities.policy().active_policy_hash().to_string());
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
