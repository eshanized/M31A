//! Production WorkerDispatcher implementing the Autonomy Controller consumer seam (AGT-04, AGT-05, D-05).
//!
//! Provides deterministic worker allocation with capability intersection, supervised async job dispatch,
//! and terminal outcome collection.

use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::agent::envelope::calculate_eligible_capabilities;
use crate::agent::model_policy::{ModelCaller, ProviderModelCaller, RoutedModelCaller};
use crate::agent::profile::AgentProfile;
use crate::agent::runner::WorkerRunner;
use crate::agent::supervisor::{AgentOutcome, FailureClass, FailureEvidence, WorkerSupervisor};
use crate::ids::{AgentId, JobId, MissionId, TaskId};
use crate::kernel::plan::{CapabilityAccessMode, CapabilityRequirement};
use crate::kernel::seams::execution::{
    ExecutionError, WorkExecutionHandle, WorkExecutionRequest, WorkExecutionResult,
    WorkerDispatcher,
};
use crate::state_machine::agent::AgentRole;

use crate::capability::registry::CapabilityRegistry;
use crate::kernel::seams::policy::{DefaultPolicyGate, PolicyGate};
use crate::model::provider::ModelProvider;
// (NvidiaProvider referenced via fully-qualified path at construction.)
use crate::model::router::resolver::ModelTier;
use crate::pipeline::dispatcher::ProductionActionDispatcher;
use crate::pipeline::runner::ToolPipelineRunner;
use crate::policy::approval::ApprovalCoordinator;
use crate::state::intake::AutonomyMode;
use crate::tools::definition::{ToolExecutionContext, to_openai_tool};
use crate::tools::registry::ToolRegistry;

type JobOutcomeReceiver =
    Arc<tokio::sync::Mutex<Option<tokio::sync::oneshot::Receiver<AgentOutcome>>>>;

/// Production implementation of the Autonomy Controller WorkerDispatcher seam (D-05, D-12).
///
/// Authority contract: the production path is `from_shared_authorities`,
/// which consumes the runtime-shared capability registry, policy gate,
/// artifact store, model caller, and coordinator WITHOUT constructing
/// competing authorities. The `new*` constructors below are legacy
/// compatibility shims (standalone/test use); they build a self-contained
/// stack and MUST NOT be used on production paths where a
/// `RuntimeAuthorities` set exists.
#[derive(Clone)]
pub struct ProductionWorkerDispatcher {
    active_executions: Arc<RwLock<HashMap<JobId, AgentOutcome>>>,
    execution_usages: Arc<RwLock<HashMap<JobId, crate::model::types::TokenUsage>>>,
    registered_agents: Arc<RwLock<HashMap<AgentId, AgentProfile>>>,
    cancellation_tokens: Arc<RwLock<HashMap<JobId, CancellationToken>>>,
    pending_results: Arc<RwLock<HashMap<JobId, JobOutcomeReceiver>>>,
    pipeline_runner: Arc<ToolPipelineRunner>,
    capability_registry: Arc<CapabilityRegistry>,
    policy_gate: Arc<dyn PolicyGate>,
    autonomy_mode: AutonomyMode,
    model_caller: Arc<dyn ModelCaller>,
    workspace_root: std::path::PathBuf,
    db_pool: Option<sqlx::SqlitePool>,
    approval_coordinator: Option<Arc<ApprovalCoordinator>>,
    /// Shared context compiler (controller authority). When present,
    /// worker runners compile task context through it instead of the
    /// offline leaf default, so worker, controller, engine, and
    /// pre-execution contexts derive from one compiler.
    context_compiler: Option<Arc<dyn crate::kernel::seams::context::ContextCompiler>>,
}

fn is_test_environment() -> bool {
    if std::env::var("M31A_REAL_MODEL_TEST")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return false;
    }
    if std::env::var("M31A_FORCE_MOCK_MODEL")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return true;
    }
    if std::env::var("CARGO_TARGET_TMPDIR").is_ok() {
        return true;
    }
    if let Ok(exe) = std::env::current_exe() {
        let exe_str = exe.to_string_lossy();
        if exe_str.contains("/deps/") || exe_str.contains("\\deps\\") {
            return true;
        }
    }
    false
}

impl ProductionWorkerDispatcher {
    /// Create a new production worker dispatcher wired to real tools, router, and provider.
    pub fn new() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        Self::new_with_workspace(cwd)
    }

    /// Create a production worker dispatcher rooted at the given workspace and storage paths (GAP-03, CTL-01, F-02).
    pub fn new_with_roots(
        workspace_root: impl Into<std::path::PathBuf>,
        storage_root: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self::new_with_roots_and_config(workspace_root, storage_root, None)
    }

    /// Create a production worker dispatcher rooted at given paths with authoritative configuration.
    pub fn new_with_roots_and_config(
        workspace_root: impl Into<std::path::PathBuf>,
        storage_root: impl Into<std::path::PathBuf>,
        config: Option<&crate::config::ResolvedConfiguration>,
    ) -> Self {
        let workspace_root = workspace_root.into();
        let storage_root = storage_root.into();
        let capabilities = Arc::new(CapabilityRegistry::production(&workspace_root, None, None));
        let mut tool_reg = ToolRegistry::new_default(Arc::clone(&capabilities));
        tool_reg.register(crate::tools::definition::CompleteTool);
        tool_reg.register_agentic_tools();
        let tool_registry = Arc::new(tool_reg);
        let artifact_store = Arc::new(crate::persistence::artifacts::FsArtifactStore::new(
            storage_root.join("artifacts"),
        ));
        let pipeline_runner = Arc::new(
            ToolPipelineRunner::new(Arc::clone(&tool_registry)).with_artifact_store(artifact_store),
        );
        let policy_gate: Arc<dyn PolicyGate> = if let Some(cfg) = config {
            Arc::new(
                crate::policy::effective::EffectivePolicy::standard_with_policy_config(
                    &workspace_root,
                    Some(&cfg.app_config.policy),
                ),
            )
        } else {
            Arc::new(DefaultPolicyGate)
        };
        let autonomy_mode = AutonomyMode::Safe;

        let empty_denied: Vec<String> = Vec::new();
        let denied_tools = config
            .map(|c| &c.app_config.policy.denied_tools)
            .unwrap_or(&empty_denied);

        let tool_schemas: Vec<serde_json::Value> = tool_registry
            .list_tools()
            .iter()
            .filter(|t| !denied_tools.contains(&t.id().to_string()))
            .map(|t| to_openai_tool(t.as_ref()))
            .collect();

        // Legacy standalone stack: authoritative configuration wins; ambient
        // environment is NEVER probed for model selection here (Tier-5 env
        // already lives inside `ResolvedConfiguration::build`).
        let active_model = config
            .map(|c| c.active_model.clone())
            .unwrap_or_else(|| "meta/llama-3.2-11b-vision-instruct".to_string());

        let base_url = config
            .and_then(|c| c.app_config.provider.nvidia_nim.as_ref())
            .and_then(|p| p.base_url.clone());

        // Credentials resolve through the single authoritative binding
        // (channel file → environment). The legacy multi-path filesystem
        // cascade is retired: the channel-aware store is the only file source.
        let channel = crate::deployment::DeploymentChannel::current();
        let credentials =
            crate::runtime_authorities::resolve_runtime_credentials(&workspace_root, channel);
        let provider: Option<Arc<dyn ModelProvider>> = if is_test_environment() {
            None
        } else {
            let api_key = credentials.api_key.clone();
            match crate::model::provider::nvidia::NvidiaProvider::new_governed(
                base_url,
                api_key,
                config
                    .map(|c| c.provider_endpoint_source())
                    .unwrap_or(crate::model::provider::EndpointTrustSource::Unknown),
            ) {
                Ok(p) => Some(Arc::new(p)),
                Err(_) => None,
            }
        };

        let active_provider = config
            .map(|c| c.active_provider.clone())
            .unwrap_or_else(|| "nvidia_nim".to_string());

        let provider_status = if let Some(c) = config {
            c.active_provider_status()
        } else {
            let reg = crate::config::provider_registry::ProviderRegistry::new();
            reg.get_status(&active_provider)
        };

        let model_caller: Arc<dyn ModelCaller> = Arc::new(
            RoutedModelCaller::new(provider, ModelTier::Standard, tool_schemas)
                .with_model(active_model)
                .with_provider_status(active_provider, provider_status),
        );

        Self {
            active_executions: Arc::new(RwLock::new(HashMap::new())),
            execution_usages: Arc::new(RwLock::new(HashMap::new())),
            registered_agents: Arc::new(RwLock::new(HashMap::new())),
            cancellation_tokens: Arc::new(RwLock::new(HashMap::new())),
            pending_results: Arc::new(RwLock::new(HashMap::new())),
            pipeline_runner,
            capability_registry: capabilities,
            policy_gate,
            autonomy_mode,
            model_caller,
            workspace_root,
            db_pool: None,
            approval_coordinator: None,
            context_compiler: None,
        }
    }

    /// Create a production worker dispatcher rooted at the given workspace (CTL-01, F-02).
    pub fn new_with_workspace(workspace_root: impl Into<std::path::PathBuf>) -> Self {
        let ws = workspace_root.into();
        let storage = ws.join(".m31a");
        Self::new_with_roots(ws, storage)
    }

    pub fn capability_registry(&self) -> &Arc<CapabilityRegistry> {
        &self.capability_registry
    }

    /// Access the authoritative policy gate.
    pub fn policy_gate(&self) -> &Arc<dyn PolicyGate> {
        &self.policy_gate
    }

    /// Access the bound autonomy mode.
    pub fn dispatcher_autonomy_mode(&self) -> AutonomyMode {
        self.autonomy_mode
    }

    /// Access the bound model caller.
    pub fn model_caller(&self) -> &Arc<dyn ModelCaller> {
        &self.model_caller
    }

    /// Access the workspace root this dispatcher is scoped to.
    pub fn dispatcher_workspace_root(&self) -> &std::path::Path {
        &self.workspace_root
    }

    /// Access the attached database pool, if any.
    pub fn db_pool(&self) -> Option<&sqlx::SqlitePool> {
        self.db_pool.as_ref()
    }

    /// Access the attached approval coordinator, if any.
    pub fn dispatcher_approval_coordinator(&self) -> Option<&Arc<ApprovalCoordinator>> {
        self.approval_coordinator.as_ref()
    }

    /// Canonical production constructor: assemble a dispatcher ENTIRELY from
    /// runtime-shared authorities. Constructs NO registries, NO policy gates,
    /// NO providers, and performs NO environment probing — every authority is
    /// received. `model_caller = None` installs an explicit fail-closed
    /// no-provider caller (typed misconfiguration at call time), never an
    /// ambient env-probed provider.
    #[allow(clippy::too_many_arguments)]
    pub fn from_shared_authorities(
        workspace_root: std::path::PathBuf,
        capability_registry: Arc<CapabilityRegistry>,
        policy_gate: Arc<dyn PolicyGate>,
        artifact_store: Arc<dyn crate::persistence::artifacts::ArtifactStore>,
        model_caller: Option<Arc<dyn ModelCaller>>,
        approval_coordinator: Option<Arc<ApprovalCoordinator>>,
        db_pool: Option<sqlx::SqlitePool>,
        config: Option<&crate::config::ResolvedConfiguration>,
        context_compiler: Option<Arc<dyn crate::kernel::seams::context::ContextCompiler>>,
    ) -> Self {
        let mut tool_reg = ToolRegistry::new_default(Arc::clone(&capability_registry));
        tool_reg.register(crate::tools::definition::CompleteTool);
        tool_reg.register_agentic_tools();
        let tool_registry = Arc::new(tool_reg);

        let denied: &[String] = config
            .map(|c| c.app_config.policy.denied_tools.as_slice())
            .unwrap_or(&[]);
        let autonomy = config
            .map(crate::runtime_authorities::AutonomyPrecedence::from_config)
            .unwrap_or(AutonomyMode::Safe);
        let tool_schemas = crate::runtime_authorities::RuntimeAuthorities::governed_tool_schemas(
            &capability_registry,
            &tool_registry,
            denied,
            autonomy,
        );
        let active_model = config
            .map(|c| c.active_model.clone())
            .unwrap_or_else(|| "meta/llama-3.2-11b-vision-instruct".to_string());
        let active_provider = config
            .map(|c| c.active_provider.clone())
            .unwrap_or_else(|| "nvidia_nim".to_string());

        let caller: Arc<dyn ModelCaller> = match model_caller {
            Some(caller) => caller,
            None => Arc::new(
                RoutedModelCaller::new(None, ModelTier::Standard, tool_schemas)
                    .with_model(active_model)
                    .with_provider_status(
                        active_provider,
                        crate::model::types::ProviderCapabilityStatus::Misconfigured,
                    ),
            ),
        };

        let mut pipeline_runner =
            ToolPipelineRunner::new(Arc::clone(&tool_registry)).with_artifact_store(artifact_store);
        if let Some(ref pool) = db_pool {
            pipeline_runner = pipeline_runner.with_db_pool(pool.clone());
        }
        if let Some(ref coordinator) = approval_coordinator {
            pipeline_runner = pipeline_runner.with_approval_coordinator(coordinator.clone());
        }

        Self {
            active_executions: Arc::new(RwLock::new(HashMap::new())),
            execution_usages: Arc::new(RwLock::new(HashMap::new())),
            registered_agents: Arc::new(RwLock::new(HashMap::new())),
            cancellation_tokens: Arc::new(RwLock::new(HashMap::new())),
            pending_results: Arc::new(RwLock::new(HashMap::new())),
            pipeline_runner: Arc::new(pipeline_runner),
            capability_registry,
            policy_gate,
            autonomy_mode: AutonomyMode::Safe,
            model_caller: caller,
            workspace_root,
            db_pool,
            approval_coordinator,
            context_compiler,
        }
    }

    /// Attach the runtime-shared context compiler so worker runners compile
    /// task context through the canonical context authority.
    pub fn with_context_compiler(
        mut self,
        compiler: Arc<dyn crate::kernel::seams::context::ContextCompiler>,
    ) -> Self {
        self.context_compiler = Some(compiler);
        self
    }

    /// Access the shared context compiler, if attached.
    pub fn context_compiler(
        &self,
    ) -> Option<&Arc<dyn crate::kernel::seams::context::ContextCompiler>> {
        self.context_compiler.as_ref()
    }

    /// Access the tool pipeline runner.
    pub fn pipeline_runner(&self) -> &Arc<ToolPipelineRunner> {
        &self.pipeline_runner
    }

    /// Swap the capability registry and rebuild the derived tool inventory.
    ///
    /// The replacement registry should be the runtime-shared canonical instance.
    /// The artifact store is preserved across the swap so capability rotation
    /// cannot fork artifact authority.
    pub fn with_capabilities(mut self, registry: Arc<CapabilityRegistry>) -> Self {
        self.capability_registry = registry.clone();
        let mut tool_reg = ToolRegistry::new_default(registry);
        tool_reg.register(crate::tools::definition::CompleteTool);
        tool_reg.register_agentic_tools();
        let tool_registry = Arc::new(tool_reg);
        // Preserve the canonical artifact store across capability swaps;
        // fall back to the channel-aware workspace default only when unset.
        let artifact_store = self
            .pipeline_runner
            .artifact_store()
            .cloned()
            .unwrap_or_else(|| {
                Arc::new(crate::persistence::artifacts::FsArtifactStore::new(
                    crate::deployment::DeploymentPaths::project_artifacts_dir(
                        &self.workspace_root,
                        crate::deployment::DeploymentChannel::current(),
                    ),
                )) as Arc<dyn crate::persistence::artifacts::ArtifactStore>
            });
        let mut runner = ToolPipelineRunner::new(tool_registry).with_artifact_store(artifact_store);
        if let Some(ref pool) = self.db_pool {
            runner = runner.with_db_pool(pool.clone());
        }
        if let Some(ref coord) = self.approval_coordinator {
            runner = runner.with_approval_coordinator(coord.clone());
        }
        self.pipeline_runner = Arc::new(runner);
        self
    }

    /// Inject the canonical shared artifact store to preserve unified artifact
    /// authority across execution pipelines.
    pub fn with_artifact_store(
        mut self,
        store: Arc<dyn crate::persistence::artifacts::ArtifactStore>,
    ) -> Self {
        self.pipeline_runner = Arc::new((*self.pipeline_runner).clone().with_artifact_store(store));
        self
    }

    /// Attach an SQLite pool for policy auditing and grant persistence.
    pub fn with_db_pool(mut self, pool: sqlx::SqlitePool) -> Self {
        self.db_pool = Some(pool.clone());
        self.pipeline_runner = Arc::new((*self.pipeline_runner).clone().with_db_pool(pool));
        self
    }

    /// Attach an approval coordinator for interactive operator approvals (P0-A).
    pub fn with_approval_coordinator(mut self, coordinator: Arc<ApprovalCoordinator>) -> Self {
        self.approval_coordinator = Some(coordinator.clone());
        self.pipeline_runner = Arc::new(
            (*self.pipeline_runner)
                .clone()
                .with_approval_coordinator(coordinator),
        );
        self
    }

    pub fn with_workspace(mut self, root: std::path::PathBuf) -> Self {
        self.workspace_root = root;
        self
    }

    /// Configure a custom model caller (e.g. for deterministic unit test doubles).
    pub fn with_model_caller(mut self, caller: Arc<dyn ModelCaller>) -> Self {
        self.model_caller = caller;
        self
    }

    /// Configure a specific model provider and model name.
    pub fn with_provider<P: ModelProvider + 'static>(
        mut self,
        provider: Arc<P>,
        model_name: impl Into<String>,
    ) -> Self {
        let tool_schemas: Vec<serde_json::Value> = self
            .pipeline_runner
            .tool_registry()
            .list_tools()
            .iter()
            .map(|t| to_openai_tool(t.as_ref()))
            .collect();
        self.model_caller = Arc::new(ProviderModelCaller::new(provider, model_name, tool_schemas));
        self
    }

    /// Configure a custom pipeline runner.
    pub fn with_pipeline_runner(mut self, runner: Arc<ToolPipelineRunner>) -> Self {
        self.pipeline_runner = runner;
        self
    }

    /// Configure a custom policy gate.
    pub fn with_policy_gate(mut self, gate: Arc<dyn PolicyGate>) -> Self {
        self.policy_gate = gate;
        self
    }

    /// Configure autonomy mode.
    pub fn with_autonomy_mode(mut self, mode: AutonomyMode) -> Self {
        self.autonomy_mode = mode;
        self
    }

    /// Configure capability registry.
    ///
    /// Total replacement: delegates to `with_capabilities`, rebuilding the
    /// derived tool inventory and pipeline runner so the new registry cannot
    /// leave stale derived state behind. Partial swaps are forbidden.
    pub fn with_capability_registry(self, registry: Arc<CapabilityRegistry>) -> Self {
        self.with_capabilities(registry)
    }

    /// Register an agent profile under an existing agent identifier.
    pub async fn register_agent(&self, agent_id: AgentId, profile: AgentProfile) {
        self.registered_agents
            .write()
            .await
            .insert(agent_id, profile);
    }

    /// Explicitly store or override a job outcome (useful for testing and deterministic mocking).
    pub async fn set_job_outcome(&self, job_id: JobId, outcome: AgentOutcome) {
        self.active_executions.write().await.insert(job_id, outcome);
    }

    /// Request cancellation of an active worker job.
    pub async fn cancel_job(&self, job_id: &JobId) -> bool {
        if let Some(token) = self.cancellation_tokens.read().await.get(job_id) {
            token.cancel();
            true
        } else {
            false
        }
    }
}

impl Default for ProductionWorkerDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WorkerDispatcher for ProductionWorkerDispatcher {
    /// Allocate an agent for a task, selecting a role and validating capability intersection (AGT-04, D-02).
    ///
    /// Role resolution is registry-driven: an explicit `role:` tag resolves
    /// against registered definitions and fails explicitly when unknown;
    /// otherwise the registry's ordered inference rules select a role. No
    /// hardcoded capability-to-role chain lives here.
    async fn allocate_worker(
        &self,
        _task_id: TaskId,
        _mission_id: MissionId,
        capabilities: &[String],
    ) -> Result<AgentId, ExecutionError> {
        use crate::agent::registry::RoleRegistry;
        // 1. Select appropriate role via the registry
        let role = if let Some(role_cap) = capabilities.iter().find(|c| c.starts_with("role:")) {
            let role_str = role_cap[5..].trim();
            let parsed = AgentRole::new(role_str);
            let guard = RoleRegistry::global().read().map_err(|_| {
                ExecutionError::AllocationFailed("role registry lock poisoned".to_string())
            })?;
            if !guard.contains(&parsed) {
                return Err(ExecutionError::AllocationFailed(format!(
                    "unknown agent role '{role_str}': no registered role definition"
                )));
            }
            parsed
        } else {
            let guard = RoleRegistry::global().read().map_err(|_| {
                ExecutionError::AllocationFailed("role registry lock poisoned".to_string())
            })?;
            guard
                .infer_role_for_capabilities(capabilities)
                .unwrap_or_else(AgentRole::planner)
        };

        let profile = {
            let guard = RoleRegistry::global().read().map_err(|_| {
                ExecutionError::AllocationFailed("role registry lock poisoned".to_string())
            })?;
            guard.profile_for(&role).map_err(|e| {
                ExecutionError::AllocationFailed(format!(
                    "cannot materialize profile for role '{}': {}",
                    role.as_str(),
                    e
                ))
            })?
        };

        // 2. Convert string capabilities to typed CapabilityRequirements (filtering out role directive)
        let reqs: Vec<CapabilityRequirement> = capabilities
            .iter()
            .filter(|cap| !cap.starts_with("role:"))
            .map(|cap| {
                let mode = if cap == "fs.write" || cap.ends_with(".write") {
                    CapabilityAccessMode::Write
                } else {
                    CapabilityAccessMode::Read
                };
                CapabilityRequirement::new(cap, mode)
            })
            .collect();

        // 3. Check capability intersection against role envelope (D-02)
        if let Err(err) = calculate_eligible_capabilities(&reqs, &profile.capability_policy) {
            return Err(ExecutionError::AllocationFailed(format!(
                "capability envelope violation: {}",
                err
            )));
        }

        // 4. Register agent and return AgentId
        let agent_id = AgentId::new();
        self.registered_agents
            .write()
            .await
            .insert(agent_id, profile);
        Ok(agent_id)
    }

    /// Dispatch work to a supervised async worker task (D-05).
    async fn dispatch_work(
        &self,
        req: WorkExecutionRequest,
    ) -> Result<WorkExecutionHandle, ExecutionError> {
        let profile = {
            let agents = self.registered_agents.read().await;
            agents
                .get(&req.agent_id)
                .cloned()
                .ok_or_else(|| ExecutionError::DispatchFailed("agent not registered".to_string()))?
        };

        let job_id = JobId::new();
        let token = CancellationToken::new();
        self.cancellation_tokens
            .write()
            .await
            .insert(job_id, token.clone());

        let supervisor = WorkerSupervisor::new(
            req.agent_id,
            req.task_id,
            token.clone(),
            Duration::from_secs(profile.termination_policy.step_stall_timeout_secs),
            Duration::from_secs(profile.termination_policy.task_wall_clock_timeout_secs),
            Duration::from_millis(100),
        );

        let active_executions = Arc::clone(&self.active_executions);
        let execution_usages = Arc::clone(&self.execution_usages);
        let activity_tracker = supervisor.activity_tracker();
        let pipeline_runner = Arc::clone(&self.pipeline_runner);
        let capability_registry = Arc::clone(&self.capability_registry);
        let policy_gate = Arc::clone(&self.policy_gate);
        let autonomy_mode = self.autonomy_mode;
        let model_caller = Arc::clone(&self.model_caller);
        let context_compiler = self.context_compiler.clone();

        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pending_results
            .write()
            .await
            .insert(job_id, Arc::new(tokio::sync::Mutex::new(Some(rx))));

        let workspace_root = req
            .workspace_root
            .clone()
            .unwrap_or_else(|| self.workspace_root.clone());

        tokio::spawn(async move {
            let outcome = supervisor
                .run_supervised(async move {
                    // Canonical context authority: the worker MUST compile
                    // through the dispatcher-bound (runtime-shared in
                    // production) compiler. A missing binding fails the
                    // dispatch closed — silently constructing a divergent
                    // worker-owned catalog/compiler would fork prompt
                    // authority (wiring remediation v0.1.1).
                    let Some(compiler) = context_compiler else {
                        return AgentOutcome::Failed(FailureEvidence {
                            failure_class: FailureClass::ModelError,
                            message: "dispatch failed: no context compiler bound to dispatcher"
                                .to_string(),
                            step_number: 0,
                            occurred_at: Utc::now(),
                            is_panic: false,
                            diagnostics: std::collections::HashMap::new(),
                        });
                    };
                    let mut runner = WorkerRunner::new(
                        req.mission_id,
                        req.agent_id,
                        req.task_id,
                        profile.clone(),
                        compiler,
                    )
                    .with_workspace_root(workspace_root.clone())
                    // Typed prompt execution binding: the work request's
                    // workflow/task reference flows into the worker and
                    // takes precedence over the profile (role default)
                    // prompt. It is NEVER downgraded into description text.
                    .with_prompt_ref_opt(req.prompt_ref.clone());
                    if let Some(ref obj) = req.mission_objective {
                        runner = runner.with_mission_objective(obj.clone());
                    }
                    if let Some(ref obj) = req.task_objective {
                        runner = runner.with_task_objective(obj.clone());
                    }
                    if let Some(ref desc) = req.task_description {
                        runner = runner.with_task_description(desc.clone());
                    }
                    if !req.task_criteria.is_empty() {
                        runner = runner.with_task_criteria(req.task_criteria.clone());
                    }
                    if !req.requirement_keys.is_empty() {
                        runner = runner.with_requirement_keys(req.requirement_keys.clone());
                    }
                    if !req.task_assumptions.is_empty() {
                        runner = runner.with_task_assumptions(req.task_assumptions.clone());
                    }
                    if let Some(ref charter) = req.upstream_charter {
                        runner = runner.with_upstream_charter(charter.clone());
                    }
                    if let Some(ref arch) = req.upstream_architecture {
                        runner = runner.with_upstream_architecture(arch.clone());
                    }
                    if !req.upstream_requirements.is_empty() {
                        runner =
                            runner.with_upstream_requirements(req.upstream_requirements.clone());
                    }
                    if !req.upstream_assumptions.is_empty() {
                        runner = runner.with_upstream_assumptions(req.upstream_assumptions.clone());
                    }
                    if !req.upstream_decisions.is_empty() {
                        runner = runner.with_upstream_decisions(req.upstream_decisions.clone());
                    }
                    if let Some(ref summary) = req.upstream_research_summary {
                        runner = runner.with_upstream_research_summary(summary.clone());
                    }
                    if let Some(ref verification) = req.verification {
                        runner = runner.with_verification(verification.clone());
                    }
                    // Authoritative capability environment: the dispatcher's
                    // runtime-shared registry — never a per-dispatch fork.
                    // Cross-workspace dispatch is a separate runtime scope and
                    // requires a separately constructed dispatcher; silently
                    // forking a registry here would split tool authority.
                    let context = ToolExecutionContext::new(
                        capability_registry,
                        workspace_root,
                        token.clone(),
                    )
                    .with_role_envelope(profile.capability_policy.clone())
                    .with_agent_role(profile.role.clone())
                    .with_autonomy_mode(autonomy_mode)
                    .with_policy_hash(
                        policy_gate
                            .policy_hash()
                            .unwrap_or_else(|| "uncompiled-policy".to_string()),
                    )
                    .with_mission_id(req.mission_id)
                    .with_task_id(req.task_id)
                    .with_agent_id(req.agent_id);

                    let dispatcher = ProductionActionDispatcher::new(
                        pipeline_runner,
                        context,
                        policy_gate,
                        autonomy_mode,
                    );
                    runner
                        .run_step_loop(model_caller.as_ref(), &dispatcher, &token, activity_tracker)
                        .await
                })
                .await;

            let usage = supervisor.activity_tracker().total_token_usage().await;
            execution_usages.write().await.insert(job_id, usage);
            active_executions
                .write()
                .await
                .insert(job_id, outcome.clone());
            let _ = tx.send(outcome);
        });

        Ok(WorkExecutionHandle {
            job_id,
            task_id: req.task_id,
            agent_id: req.agent_id,
        })
    }

    /// Collect the execution outcome for a previously dispatched work item.
    async fn collect_result(
        &self,
        handle: &WorkExecutionHandle,
    ) -> Result<WorkExecutionResult, ExecutionError> {
        let usage = self
            .execution_usages
            .read()
            .await
            .get(&handle.job_id)
            .cloned();
        // 1. Check if outcome is already recorded in active_executions
        {
            let executions = self.active_executions.read().await;
            if let Some(outcome) = executions.get(&handle.job_id) {
                return outcome_to_result(handle.task_id, outcome, usage);
            }
        }

        // 2. Wait for pending execution if still running
        let rx_mutex_opt = {
            let pending = self.pending_results.read().await;
            pending.get(&handle.job_id).cloned()
        };

        if let Some(mutex_rx) = rx_mutex_opt {
            let mut guard = mutex_rx.lock().await;
            if let Some(rx) = guard.take() {
                match rx.await {
                    Ok(outcome) => {
                        let final_usage = self
                            .execution_usages
                            .read()
                            .await
                            .get(&handle.job_id)
                            .cloned();
                        return outcome_to_result(handle.task_id, &outcome, final_usage);
                    }
                    Err(_) => {
                        let executions = self.active_executions.read().await;
                        if let Some(outcome) = executions.get(&handle.job_id) {
                            let final_usage = self
                                .execution_usages
                                .read()
                                .await
                                .get(&handle.job_id)
                                .cloned();
                            return outcome_to_result(handle.task_id, outcome, final_usage);
                        }
                        return Err(ExecutionError::ExecutionFailed(
                            "worker task dropped before completion".to_string(),
                        ));
                    }
                }
            }
        }

        let executions = self.active_executions.read().await;
        let outcome = executions.get(&handle.job_id).ok_or_else(|| {
            ExecutionError::ExecutionFailed("work execution still pending".to_string())
        })?;

        outcome_to_result(handle.task_id, outcome, usage)
    }

    async fn cancel_job(&self, job_id: &JobId) -> bool {
        ProductionWorkerDispatcher::cancel_job(self, job_id).await
    }
}

fn outcome_to_result(
    task_id: TaskId,
    outcome: &AgentOutcome,
    usage: Option<crate::model::types::TokenUsage>,
) -> Result<WorkExecutionResult, ExecutionError> {
    match outcome {
        AgentOutcome::Succeeded { output, .. } => Ok(WorkExecutionResult {
            task_id,
            success: true,
            output: output.clone(),
            error_detail: None,
            token_usage: usage,
        }),
        AgentOutcome::Cancelled { reason, .. } => Ok(WorkExecutionResult {
            task_id,
            success: false,
            output: String::new(),
            error_detail: Some(format!("cancelled: {}", reason)),
            token_usage: usage,
        }),
        AgentOutcome::TimedOut { deadline_secs, .. } => Ok(WorkExecutionResult {
            task_id,
            success: false,
            output: String::new(),
            error_detail: Some(format!("timed out after {}s", deadline_secs)),
            token_usage: usage,
        }),
        AgentOutcome::Stalled {
            last_progress_secs_ago,
            ..
        } => Ok(WorkExecutionResult {
            task_id,
            success: false,
            output: String::new(),
            error_detail: Some(format!(
                "stalled: no progress for {}s",
                last_progress_secs_ago
            )),
            token_usage: usage,
        }),
        AgentOutcome::StepLimitExceeded { limit, consumed } => Ok(WorkExecutionResult {
            task_id,
            success: false,
            output: String::new(),
            error_detail: Some(format!("step limit exceeded: {}/{}", consumed, limit)),
            token_usage: usage,
        }),
        AgentOutcome::Failed(evidence) => Ok(WorkExecutionResult {
            task_id,
            success: false,
            output: String::new(),
            error_detail: Some(format!(
                "failure ({:?}): {}",
                evidence.failure_class, evidence.message
            )),
            token_usage: usage,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::model_policy::TestModelCaller;

    #[tokio::test]
    async fn test_allocate_worker_success() {
        let dispatcher = ProductionWorkerDispatcher::new();
        let agent_id = dispatcher
            .allocate_worker(
                TaskId::new(),
                MissionId::new(),
                &["fs.write".to_string(), "fs.read".to_string()],
            )
            .await
            .unwrap();

        let agents = dispatcher.registered_agents.read().await;
        let profile = agents.get(&agent_id).unwrap();
        assert_eq!(profile.role, AgentRole::implementer());
    }

    #[tokio::test]
    async fn test_allocate_worker_rejects_out_of_envelope() {
        let dispatcher = ProductionWorkerDispatcher::new();
        // Request forbidden capability on Planner
        let err = dispatcher
            .allocate_worker(
                TaskId::new(),
                MissionId::new(),
                &["plan.propose".to_string(), "admin.escalate".to_string()],
            )
            .await
            .unwrap_err();

        assert!(matches!(err, ExecutionError::AllocationFailed(_)));
    }

    #[tokio::test]
    async fn test_dispatch_and_collect_result() {
        // Isolated test compiler: explicitly constructed test infrastructure
        // (production binds the runtime-shared compiler instead).
        let test_compiler: Arc<dyn crate::kernel::seams::context::ContextCompiler> =
            Arc::new(crate::context::compiler::ProductionContextCompiler::new());
        let dispatcher = ProductionWorkerDispatcher::new()
            .with_context_compiler(test_compiler)
            .with_model_caller(Arc::new(TestModelCaller::new(
                "task executed under worker supervision",
            )));
        let agent_id = dispatcher
            .allocate_worker(TaskId::new(), MissionId::new(), &["fs.read".to_string()])
            .await
            .unwrap();

        let req = WorkExecutionRequest::new(MissionId::new(), TaskId::new(), agent_id, "ctx-1");

        let handle = dispatcher.dispatch_work(req).await.unwrap();

        // Poll for outcome with short timeout
        let mut result = None;
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(20)).await;
            if let Ok(res) = dispatcher.collect_result(&handle).await {
                result = Some(res);
                break;
            }
        }

        let res = result.expect("job failed to complete within polling window");
        assert!(res.success);
        assert!(
            res.output
                .contains("task executed under worker supervision")
        );
        assert!(res.error_detail.is_none());
    }

    #[tokio::test]
    async fn test_dispatch_fails_closed_without_provider() {
        if std::env::var("NVIDIA_API_KEY").is_err() {
            // Isolated test compiler so this test exercises the
            // no-provider path (not the no-compiler path).
            let test_compiler: Arc<dyn crate::kernel::seams::context::ContextCompiler> =
                Arc::new(crate::context::compiler::ProductionContextCompiler::new());
            let dispatcher = ProductionWorkerDispatcher::new().with_context_compiler(test_compiler);
            let agent_id = dispatcher
                .allocate_worker(
                    TaskId::new(),
                    MissionId::new(),
                    &["fs.write".to_string(), "fs.read".to_string()],
                )
                .await
                .unwrap();

            let req = WorkExecutionRequest::new(
                MissionId::new(),
                TaskId::new(),
                agent_id,
                "ctx-fail-closed",
            );

            let handle = dispatcher.dispatch_work(req).await.unwrap();

            let mut result = None;
            for _ in 0..50 {
                tokio::time::sleep(Duration::from_millis(20)).await;
                if let Ok(res) = dispatcher.collect_result(&handle).await {
                    result = Some(res);
                    break;
                }
            }

            let res = result.expect("job failed to complete within polling window");
            assert!(
                !res.success,
                "dispatcher must not fake success without provider"
            );
            assert!(
                res.error_detail.is_some(),
                "error detail should be present when failing closed"
            );
        }
    }
}
