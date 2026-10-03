//! Production Composition Root & Application Runtime (PRD §01, SYS-01, BLK-01, F-01).
//!
//! "The model proposes. The runtime decides."
//!
//! Central runtime assembling all real production subsystems:
//! - SQLite ACID storage & migrations
//! - Bounded BroadcastEventBus
//! - Telemetry streaming forwarder (.m31a/telemetry)
//! - FsArtifactStore (.m31a/artifacts)
//! - EffectivePolicy & approval escalation
//! - Two-phase BudgetEnforcer
//! - WorktreeManager with Git isolation
//! - Deterministic ReportGenerator (.m31a/reports)
//! - StartupCrashRecoveryScanner
//! - AutonomyController 12-stage closed loop

use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use crate::budget::enforcer::BudgetEnforcer;
use crate::capability::traits::git::GitService;
use crate::checkpoint::crash_recovery::StartupCrashRecoveryScanner;
use crate::controller::dependencies::ControllerDependencies;
use crate::controller::{AutonomyController, ControllerHaltReason};
use crate::error::M31AError;
use crate::events::bus::{BroadcastEventBus, EventBus};
use crate::events::envelope::EventEnvelope;
use crate::events::types::EventType;
use crate::git::trailers::CommitTrailers;
use crate::git::worktree::{WorktreeConfig, WorktreeManager};
use crate::ids::{MissionId, WorkflowRunId};
use crate::memory::repository::{EngineeringMemoryStore, SqliteEngineeringMemoryRepository};
use crate::persistence::artifacts::{ArtifactRecord, ArtifactService, FsArtifactStore};
use crate::persistence::paths::project_local_dir;
use crate::persistence::sqlite::repositories::SqliteTelemetryRepository;
use crate::persistence::sqlite::repositories::report::SqliteReportRepository;
use crate::persistence::sqlite::schema::initialize_database;
use crate::policy::approval::ApprovalCoordinator;
use crate::policy::effective::EffectivePolicy;
use crate::report::ReportGenerator;
use crate::state::Mission;
use crate::state::budget::ResourceBudget;
use crate::state::intake::AutonomyMode;
use crate::telemetry::collector::TelemetryCollector;
use crate::telemetry::context::CorrelationContext;
use crate::telemetry::redactor::SecretRedactor;
use crate::telemetry::stream::NdjsonStreamWriter;
use crate::telemetry::types::SpanKind;
use crate::workflow::definition::WorkflowDefinition;
use crate::workflow::genesis::{
    BrownfieldMap, DiscoverySession, GenesisController, GenesisMode, GenesisRequest,
    ProjectCharter, ResearchDecision, ResearchSummary, WorkspaceEnvironment,
};
use crate::workflow::planning::PlanningPipelineOutcome;

/// Summary of an executed autonomous mission.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MissionExecutionSummary {
    pub mission_id: MissionId,
    pub objective: String,
    pub status: String,
    pub halt_reason: String,
    pub tasks_completed: usize,
    pub worktree_path: Option<PathBuf>,
}

/// Comprehensive outcome of executing Project Genesis intake, discovery, planning, and lowering to workflow.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GenesisExecutionOutcome {
    pub environment: WorkspaceEnvironment,
    pub brownfield_map: Option<BrownfieldMap>,
    pub charter: ProjectCharter,
    pub charter_artifact: ArtifactRecord,
    pub research_decision: ResearchDecision,
    pub research_summary: Option<ResearchSummary>,
    pub planning: PlanningPipelineOutcome,
    pub registered_artifacts: Vec<ArtifactRecord>,
    pub workflow_definition: WorkflowDefinition,
}

/// The unified M31A production application runtime.
///
/// Authority scope contract:
/// - `capability_registry` / `tool_registry`: RUNTIME_SHARED. every
///   production consumer (agent engine, dispatcher, CLI/TUI inventory, model
///   tool schemas) observes the same capability environment.
/// - `prompt_catalog`: RUNTIME_SHARED immutable (workspace overrides loaded
///   once at construction).
/// - `policy`: RUNTIME_SHARED immutable; replaced atomically by `with_config`.
/// - `budget_enforcer`: RUNTIME_SHARED mutable via `update_limits`; consumption
///   counters survive reconfiguration.
/// - `artifact_store` / `artifact_service`: PROCESS_SHARED stateless file
///   authority (same base dir); the service ledger is the SQLite pool.
/// - `dependencies`: mirrors the same policy/budget/artifact instances via
///   `production_with_shared_authorities` (no shadow authorities).
#[derive(Clone)]
pub struct AppRuntime {
    pool: SqlitePool,
    workspace_root: PathBuf,
    storage_root: PathBuf,
    event_bus: Arc<BroadcastEventBus>,
    artifact_store: Arc<FsArtifactStore>,
    artifact_service: Arc<ArtifactService>,
    policy: Arc<EffectivePolicy>,
    budget_enforcer: Arc<BudgetEnforcer>,
    telemetry_collector: Arc<TelemetryCollector>,
    worktree_manager: Arc<WorktreeManager>,
    report_generator: Arc<ReportGenerator>,
    dependencies: ControllerDependencies,
    model_caller: Option<Arc<dyn crate::agent::model_policy::ModelCaller>>,
    model_provider: Option<Arc<dyn crate::model::provider::ModelProvider>>,
    config: Arc<crate::config::ResolvedConfiguration>,
    model_catalog: Arc<tokio::sync::RwLock<crate::model::catalog::ModelCatalog>>,
    approval_coordinator: Arc<ApprovalCoordinator>,
    git_service: Arc<dyn crate::capability::traits::git::GitService>,
    capability_registry: Arc<crate::capability::registry::CapabilityRegistry>,
    tool_registry: Arc<crate::tools::registry::ToolRegistry>,
    prompt_catalog: Arc<crate::prompt::InMemoryPromptCatalog>,
    /// Single canonical authority set. Every field above that is also a
    /// member of `RuntimeAuthorities` mirrors the SAME `Arc` instance held
    /// here; `sync_authorities` re-packs the set atomically after any
    /// mutation so derived consumers can never observe a fork.
    authorities: Arc<crate::runtime_authorities::RuntimeAuthorities>,
}

impl AppRuntime {
    /// Construct a complete production runtime for the specified workspace root.
    pub async fn new(workspace_root: impl Into<PathBuf>) -> Result<Self, M31AError> {
        let root = workspace_root.into();
        let storage_root = project_local_dir(&root);
        tokio::fs::create_dir_all(&storage_root)
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e)))?;

        // Channel-aware database path: production keeps the legacy
        // `.m31a/m31a.db`; development uses the isolated `.m31a/m31a-dev.db`.
        // A hardcoded prod path here would corrupt production state from dev builds.
        let channel = crate::deployment::DeploymentChannel::current();
        let db_path = crate::deployment::DeploymentPaths::project_db_path(&root, channel);
        let pool = initialize_database(&db_path).await?;
        let event_bus = Arc::new(BroadcastEventBus::new(2048));

        Self::from_pool_and_workspace(pool, root, event_bus).await
    }

    /// Construct a runtime from an existing database pool, workspace root, and event bus.
    ///
    /// Configuration is loaded STRICTLY: a present-but-invalid workspace
    /// configuration surfaces as an error (never silently collapses into
    /// defaults). Only intentional absence (no config file) uses documented
    /// safe defaults.
    pub async fn from_pool_and_workspace(
        pool: SqlitePool,
        workspace_root: PathBuf,
        event_bus: Arc<BroadcastEventBus>,
    ) -> Result<Self, M31AError> {
        let (config, fallback_error) =
            crate::config::ResolvedConfigBuilder::new(&workspace_root).build_with_report();
        if let Some(error) = fallback_error {
            if crate::config::ResolvedConfiguration::workspace_config_exists(&workspace_root) {
                return Err(M31AError::internal(format!(
                    "invalid workspace configuration ({}): refusing to start on fallback defaults",
                    error
                )));
            }
        }
        let config = Arc::new(config);
        Self::from_pool_workspace_and_config(pool, workspace_root, event_bus, config).await
    }

    /// Construct a runtime with an explicit authoritative configuration (CFG-01..04).
    pub async fn from_pool_workspace_and_config(
        pool: SqlitePool,
        workspace_root: PathBuf,
        event_bus: Arc<BroadcastEventBus>,
        config: Arc<crate::config::ResolvedConfiguration>,
    ) -> Result<Self, M31AError> {
        crate::config::load_dotenv_from_workspace(&workspace_root);
        let storage_root = project_local_dir(&workspace_root);
        tokio::fs::create_dir_all(&storage_root)
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e)))?;

        // Deployment-channel isolation: runtime state directories resolve
        // through the channel-aware authority. Production keeps legacy
        // sibling paths (backward compatible); development is isolated.
        let channel = crate::deployment::DeploymentChannel::current();
        let artifacts_dir =
            crate::deployment::DeploymentPaths::project_artifacts_dir(&workspace_root, channel);
        tokio::fs::create_dir_all(&artifacts_dir)
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e)))?;
        let artifact_store = Arc::new(FsArtifactStore::new(artifacts_dir));
        let artifact_service = Arc::new(ArtifactService::new(artifact_store.clone(), pool.clone()));

        let redactor = Arc::new(SecretRedactor::new());
        let telemetry_dir =
            crate::deployment::DeploymentPaths::project_telemetry_dir(&workspace_root, channel);
        tokio::fs::create_dir_all(&telemetry_dir)
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e)))?;
        let stream_writer = NdjsonStreamWriter::new(&telemetry_dir);
        let telemetry_repo = SqliteTelemetryRepository::new(pool.clone());
        let telemetry_collector = Arc::new(TelemetryCollector::new(
            redactor.clone(),
            telemetry_repo,
            stream_writer,
        ));

        // Spawn event bus forwarder to telemetry stream (F-12)
        let forwarder_bus = event_bus.clone();
        let forwarder_collector = telemetry_collector.clone();
        tokio::spawn(async move {
            use futures::StreamExt;
            let mut rx = forwarder_bus
                .subscribe(crate::events::bus::EventFilter::all())
                .await;
            while let Some(Ok(envelope)) = rx.next().await {
                let context = CorrelationContext::new_root(
                    envelope.mission_id.unwrap_or_else(MissionId::new),
                );
                let evt_name = match &envelope.event_type {
                    EventType::MissionStarted { .. } => "mission.started",
                    EventType::MissionCompleted { .. } => "mission.completed",
                    EventType::MissionFailed { .. } => "mission.failed",
                    EventType::TaskStarted { .. } => "task.started",
                    EventType::TaskCompleted { .. } => "task.completed",
                    EventType::TaskFailed { .. } => "task.failed",
                    EventType::ControllerCycleStarted { .. } => "controller.cycle_started",
                    EventType::ControllerHalted { .. } => "controller.halted",
                    _ => "runtime.event",
                };
                let _ = forwarder_collector
                    .start_span(&context, evt_name, SpanKind::Job)
                    .await;
            }
        });

        let policy = Arc::new(EffectivePolicy::standard_with_policy_config(
            &workspace_root,
            Some(&config.app_config.policy),
        ));

        let retention = match config
            .app_config
            .git
            .retention_policy
            .to_lowercase()
            .as_str()
        {
            "always_remove" | "remove" => {
                crate::git::worktree::WorktreeRetentionPolicy::AlwaysRemove
            }
            "always_keep" | "keep" => crate::git::worktree::WorktreeRetentionPolicy::AlwaysKeep,
            _ => crate::git::worktree::WorktreeRetentionPolicy::KeepOnFailure,
        };
        let mut wt_cfg = WorktreeConfig::new(&workspace_root).with_retention(retention);
        if let Some(ref dir) = config.app_config.git.worktree_dir {
            wt_cfg = wt_cfg.with_worktrees_dir(dir.clone());
        }
        let worktree_manager = Arc::new(WorktreeManager::new(wt_cfg));

        let report_repo = Arc::new(SqliteReportRepository::new(pool.clone()));
        let report_generator = Arc::new(ReportGenerator::new(
            pool.clone(),
            artifact_store.clone(),
            report_repo,
            redactor.clone(),
        ));

        let mut budget = ResourceBudget::default();
        budget.max_agent_steps = config.app_config.budget.max_agent_steps;
        budget.max_tokens = config.app_config.budget.max_tokens;
        budget.max_wall_clock_seconds = config
            .app_config
            .budget
            .max_wall_clock_seconds
            .or(Some(config.app_config.runtime.timeout_secs));
        budget.max_cost_usd = config.app_config.budget.max_cost_usd;
        budget.max_retries = config.app_config.budget.max_retries;
        budget.max_concurrent_agents = Some(config.app_config.runtime.concurrency_limit);
        let budget_enforcer = Arc::new(BudgetEnforcer::new(budget));

        // Run startup crash recovery scanner (F-14)
        let scanner =
            StartupCrashRecoveryScanner::new(pool.clone(), artifact_store.clone(), &workspace_root);
        let _ = scanner.scan_all_in_flight().await;

        let approval_coordinator = Arc::new(
            ApprovalCoordinator::new(Some(pool.clone()), None)
                .with_event_bus(event_bus.clone() as Arc<dyn EventBus>),
        );

        let git_service: Arc<dyn crate::capability::traits::git::GitService> = Arc::new(
            crate::capability::providers::CliGitProvider::new(&workspace_root),
        );

        let cache_path =
            crate::model::catalog::ModelCatalog::cache_path_for_channel(&workspace_root, channel);
        let catalog = crate::model::catalog::ModelCatalog::load_from_cache_file(&cache_path)
            .ok()
            .filter(|c| c.schema_version >= crate::model::catalog::CURRENT_CATALOG_SCHEMA_VERSION)
            .unwrap_or_else(|| crate::model::catalog::ModelCatalog::new(&config.active_provider));
        let model_catalog = Arc::new(tokio::sync::RwLock::new(catalog));

        // Canonical shared capability and tool authorities: built once here and
        // cloned into every production consumer so all components observe the same
        // capability environment.
        let capability_registry =
            Arc::new(crate::capability::registry::CapabilityRegistry::production(
                &workspace_root,
                Some(event_bus.clone()),
                None,
            ));
        let mut tool_reg =
            crate::tools::registry::ToolRegistry::new_default(capability_registry.clone());
        tool_reg.register(crate::tools::definition::CompleteTool);
        tool_reg.register_agentic_tools();
        let tool_registry = Arc::new(tool_reg);

        // Canonical shared prompt catalog: workspace overrides load once at
        // construction; prompt templates are never duplicated per call site.
        let prompt_catalog = Arc::new(
            crate::prompt::InMemoryPromptCatalog::with_builtins_and_workspace(&workspace_root),
        );

        // Auto-detect and wire canonical ModelProvider and ModelCaller through
        // the single composition root. Credentials resolve through the ONE
        // authoritative binding (`resolve_runtime_credentials`: channel file →
        // environment); the resolved key is injected explicitly so provider
        // state reflects the runtime binding — never ambient probing alone.
        // Configuration names the provider; a resolvable binding (key material
        // present or explicit provider selection) attempts provider creation
        // and fails closed inside `new_governed` when unusable.
        let mut model_provider: Option<Arc<dyn crate::model::provider::ModelProvider>> = None;
        let mut model_caller: Option<Arc<dyn crate::agent::model_policy::ModelCaller>> = None;

        let credentials =
            crate::runtime_authorities::resolve_runtime_credentials(&workspace_root, channel);
        if credentials.api_key.is_some() || config.active_provider == "nvidia" {
            let base_url = config
                .app_config
                .provider
                .nvidia_nim
                .as_ref()
                .and_then(|p| p.base_url.clone());
            if let Ok(p) = crate::model::provider::nvidia::NvidiaProvider::new_governed(
                base_url,
                credentials.api_key.clone(),
                config.provider_endpoint_source(),
            ) {
                let provider_arc: Arc<dyn crate::model::provider::ModelProvider> = Arc::new(p);
                model_provider = Some(provider_arc.clone());

                let tool_schemas =
                    crate::runtime_authorities::RuntimeAuthorities::governed_tool_schemas(
                        &capability_registry,
                        &tool_registry,
                        &config.app_config.policy.denied_tools,
                        crate::runtime_authorities::AutonomyPrecedence::from_config(&config),
                    );

                let caller = Arc::new(
                    crate::agent::model_policy::RoutedModelCaller::new(
                        Some(provider_arc),
                        crate::model::router::resolver::ModelTier::Standard,
                        tool_schemas,
                    )
                    .with_catalog_lock(model_catalog.clone())
                    // Seed static fallback candidates for the configured model
                    // (pure local data, no network) so resolution never depends on
                    // hidden network discovery; the dynamic catalog still takes
                    // precedence once explicitly refreshed.
                    .with_model(config.active_model.clone()),
                );
                model_caller = Some(caller);
            }
        }

        // Canonical shared context compiler: workspace + shared prompt
        // catalog + memory store + role-stage fallback. This ONE instance is
        // the context authority for engines, the pre-execution coordinator,
        // and (via `production_with_shared_authorities`) the controller.
        let memory_repo = Arc::new(SqliteEngineeringMemoryRepository::new(pool.clone()));
        let context_compiler: Arc<dyn crate::kernel::seams::ContextCompiler> =
            Arc::new(
                crate::context::compiler::ProductionContextCompiler::new()
                    .with_workspace_root(workspace_root.clone())
                    .with_prompt_catalog(
                        prompt_catalog.clone() as Arc<dyn crate::prompt::PromptCatalog>
                    )
                    .with_memory_store(memory_repo)
                    .with_role_stage_fallback(Arc::new(|role| {
                        crate::agent::registry::RoleRegistry::global()
                            .read()
                            .ok()
                            .and_then(|guard| guard.stage_for(role))
                    })),
            );

        let mut dependencies = ControllerDependencies::production_with_shared_authorities(
            pool.clone(),
            workspace_root.clone(),
            storage_root.clone(),
            Some(event_bus.clone()),
            model_caller.clone(),
            Some(&config),
            Some(approval_coordinator.clone()),
            policy.clone(),
            artifact_store.clone(),
            budget_enforcer.clone(),
            Some(capability_registry.clone()),
            Some(context_compiler.clone()),
        );
        dependencies = dependencies.with_git_service(git_service.clone());

        let authorities = Arc::new(crate::runtime_authorities::RuntimeAuthorities::new(
            config.clone(),
            policy.clone(),
            capability_registry.clone(),
            tool_registry.clone(),
            budget_enforcer.clone(),
            approval_coordinator.clone(),
            model_catalog.clone(),
            model_provider.clone(),
            model_caller.clone(),
            context_compiler,
            prompt_catalog.clone(),
            artifact_store.clone(),
            event_bus.clone(),
            git_service.clone(),
            workspace_root.clone(),
            storage_root.clone(),
            channel,
        ));

        Ok(Self {
            pool,
            workspace_root,
            storage_root,
            event_bus,
            artifact_store,
            artifact_service,
            policy,
            budget_enforcer,
            telemetry_collector,
            worktree_manager,
            report_generator,
            dependencies,
            model_caller,
            model_provider,
            config,
            model_catalog,
            approval_coordinator,
            git_service,
            capability_registry,
            tool_registry,
            prompt_catalog,
            authorities,
        })
    }

    /// Access the SQLite database pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Access the central broadcast event bus.
    pub fn event_bus(&self) -> &Arc<BroadcastEventBus> {
        &self.event_bus
    }

    /// Access the workspace root path.
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Access the centralized git service.
    pub fn git_service(&self) -> Arc<dyn crate::capability::traits::git::GitService> {
        self.git_service.clone()
    }

    /// Access the local storage directory (.m31a).
    pub fn storage_root(&self) -> &Path {
        &self.storage_root
    }

    /// Access the production controller dependencies bundle.
    pub fn dependencies(&self) -> &ControllerDependencies {
        &self.dependencies
    }

    /// Access the report generator.
    pub fn report_generator(&self) -> &Arc<ReportGenerator> {
        &self.report_generator
    }

    /// Access the budget enforcer.
    pub fn budget_enforcer(&self) -> &Arc<BudgetEnforcer> {
        &self.budget_enforcer
    }

    /// Access the persistent artifact store.
    pub fn artifact_store(&self) -> &Arc<FsArtifactStore> {
        &self.artifact_store
    }

    /// Access the canonical artifact service.
    pub fn artifact_service(&self) -> &Arc<ArtifactService> {
        &self.artifact_service
    }

    /// Access or construct the canonical evidence completion gate.
    pub fn completion_gate(&self) -> Arc<crate::verification::gate::EvidenceCompletionGate> {
        let hierarchy = Arc::new(
            crate::verification::hierarchy::VerificationHierarchyEngine::for_workspace_with_config(
                &self.workspace_root,
                &self.config,
            ),
        );
        Arc::new(
            crate::verification::gate::EvidenceCompletionGate::new(
                self.pool.clone(),
                self.artifact_store.clone(),
                &self.workspace_root,
            )
            .with_hierarchy_engine(hierarchy),
        )
    }

    /// Access the canonical shared capability registry (RUNTIME_SHARED).
    pub fn capability_registry(&self) -> &Arc<crate::capability::registry::CapabilityRegistry> {
        &self.capability_registry
    }

    /// Access the canonical shared tool registry (RUNTIME_SHARED).
    pub fn tool_registry(&self) -> &Arc<crate::tools::registry::ToolRegistry> {
        &self.tool_registry
    }

    /// Access the canonical shared prompt catalog (RUNTIME_SHARED).
    pub fn prompt_catalog(&self) -> &Arc<crate::prompt::InMemoryPromptCatalog> {
        &self.prompt_catalog
    }

    /// Access the canonical prompt catalog as a trait object for engine wiring.
    pub fn prompt_catalog_arc(&self) -> Arc<dyn crate::prompt::PromptCatalog> {
        self.prompt_catalog.clone() as Arc<dyn crate::prompt::PromptCatalog>
    }

    /// Access the checkpoint manager.
    pub fn checkpoint_manager(&self) -> Arc<crate::checkpoint::manager::CheckpointManager> {
        self.dependencies
            .checkpoint_manager()
            .cloned()
            .unwrap_or_else(|| {
                Arc::new(crate::checkpoint::manager::CheckpointManager::new(
                    self.pool.clone(),
                    self.artifact_store.clone(),
                    crate::deployment::DeploymentPaths::project_staging_dir(
                        &self.workspace_root,
                        self.authorities.channel(),
                    ),
                ))
            })
    }

    /// Access the effective security policy.
    pub fn policy(&self) -> &Arc<EffectivePolicy> {
        &self.policy
    }

    /// Access the telemetry collector.
    pub fn telemetry_collector(&self) -> &Arc<TelemetryCollector> {
        &self.telemetry_collector
    }

    /// Access the authoritative resolved configuration.
    pub fn config(&self) -> &Arc<crate::config::ResolvedConfiguration> {
        &self.config
    }

    /// Access the active provider identifier configured in the runtime.
    pub fn active_provider(&self) -> &str {
        &self.config.active_provider
    }

    /// Retrieve the truthful capability status of the runtime's active provider (WS-I §1, §10).
    pub fn active_provider_status(&self) -> crate::model::types::ProviderCapabilityStatus {
        self.config.active_provider_status()
    }

    /// Access the current dynamic model catalog snapshot.
    pub async fn model_catalog(&self) -> crate::model::catalog::ModelCatalog {
        self.model_catalog.read().await.clone()
    }

    /// Access configured model caller if any.
    pub fn model_caller(&self) -> Option<Arc<dyn crate::agent::model_policy::ModelCaller>> {
        self.model_caller.clone()
    }

    /// Access configured model provider if any.
    pub fn model_provider(&self) -> Option<Arc<dyn crate::model::provider::ModelProvider>> {
        self.model_provider.clone()
    }

    /// Report whether the installed model provider is a deterministic test
    /// double rather than the live production provider.
    ///
    /// Returns `false` when no provider is installed (fail-closed: no model
    /// path at all) and when the installed provider is a real production
    /// provider. Returns `true` only when an explicit test double
    /// (`MockProvider`) was injected through `with_model_provider`. The TUI
    /// model selector and executable audits use this to distinguish
    /// structural/offline verification from live-provider verification.
    pub fn model_provider_is_test_double(&self) -> bool {
        self.model_provider
            .as_ref()
            .is_some_and(|p| p.is_test_double())
    }

    /// Construct a continuous interactive AgentEngine attached to this production runtime.
    ///
    /// The engine is BOUND to the runtime-shared authorities: capability and
    /// tool registries, policy, approval coordinator, context compiler, and
    /// model caller are the same `Arc` instances the dispatcher, model tool
    /// schemas, and controller observe. Autonomy derives from the
    /// authoritative configuration (never hardcoded); role defaults to the
    /// implementer and identity binds per session/mission via the engine's
    /// `bind_*` APIs. The returned engine is valid only for the current
    /// authority generation — any `with_config` reconfiguration invalidates
    /// previously created engines (see `InteractiveSessionRunner::invalidate_engine`).
    pub fn create_agent_engine(
        &self,
        session_id: crate::ids::SessionId,
    ) -> crate::agent::engine::AgentEngine {
        let tool_registry = self.tool_registry.clone();
        let pipeline_runner = Arc::new(
            crate::pipeline::runner::ToolPipelineRunner::new(tool_registry.clone())
                .with_artifact_store(self.artifact_store.clone())
                .with_db_pool(self.pool.clone())
                .with_approval_coordinator(self.approval_coordinator.clone()),
        );
        let caller = self.model_caller.clone().unwrap_or_else(|| {
            // Fail-closed fallback (no provider bound): serves the governed
            // shared-registry schemas so tool visibility still derives from
            // the canonical authorities, and fails with a typed
            // misconfigured error at call time (no provider to route to).
            Arc::new(crate::agent::model_policy::RoutedModelCaller::new(
                self.model_provider.clone(),
                crate::model::router::resolver::ModelTier::Standard,
                self.authorities.model_tool_schemas(),
            ))
        });
        // One verification hierarchy: the engine gate shares the same
        // workspace+config hierarchy semantics as the controller verifier, so
        // task completion and mission completion cannot diverge.
        let hierarchy = Arc::new(
            crate::verification::hierarchy::VerificationHierarchyEngine::for_workspace_with_config(
                &self.workspace_root,
                &self.config,
            ),
        );
        let completion_gate = Arc::new(
            crate::verification::gate::EvidenceCompletionGate::new(
                self.pool.clone(),
                self.artifact_store.clone(),
                &self.workspace_root,
            )
            .with_hierarchy_engine(hierarchy),
        );
        // Event-attached session repository: session lifecycle transitions
        // emit `Session*` events like every other session path (no silent
        // sessions).
        let session_repo =
            crate::interaction::session::SqliteSessionRepository::new(self.pool.clone())
                .with_event_bus(self.event_bus.clone() as Arc<dyn crate::events::bus::EventBus>);

        crate::agent::engine::AgentEngine::new(
            session_id,
            self.workspace_root.clone(),
            session_repo,
            caller,
            tool_registry,
            pipeline_runner,
            self.policy.clone(),
            self.approval_coordinator.clone(),
            completion_gate,
            self.authorities.context_compiler().clone(),
            Some(self.event_bus.clone() as Arc<dyn crate::events::bus::EventBus>),
            self.capability_registry.clone(),
        )
        .with_prompt_catalog(self.prompt_catalog_arc())
        .with_autonomy_mode(crate::runtime_authorities::AutonomyPrecedence::from_config(
            &self.config,
        ))
        .with_scope_repos(
            crate::persistence::sqlite::repositories::SqliteMissionRepository::new(
                self.pool.clone(),
            ),
            crate::persistence::sqlite::repositories::SqliteTaskRepository::new(self.pool.clone()),
        )
        .with_intent_repo(
            crate::agent::intent_repository::SqliteIntentRepository::new(self.pool.clone()),
        )
    }

    /// Access the approval coordinator for human authorization (P0-A).
    pub fn approval_coordinator(&self) -> &Arc<ApprovalCoordinator> {
        &self.approval_coordinator
    }

    /// Configure a custom approval coordinator on the runtime.
    ///
    /// Re-packs the authority set and reassembles dependents so the new
    /// coordinator is observed consistently (no split approval authority).
    pub fn with_approval_coordinator(mut self, coordinator: Arc<ApprovalCoordinator>) -> Self {
        self.approval_coordinator = coordinator;
        self.rebuild_dependencies();
        self.sync_authorities();
        self
    }

    /// Configure a custom approval channel on the runtime's approval coordinator.
    pub fn with_approval_channel(
        self,
        channel: Arc<dyn crate::policy::approval::ApprovalChannel>,
    ) -> Self {
        self.approval_coordinator.set_channel(channel);
        self
    }

    /// Access the shared thread-safe model catalog reference.
    pub fn model_catalog_ref(
        &self,
    ) -> &Arc<tokio::sync::RwLock<crate::model::catalog::ModelCatalog>> {
        &self.model_catalog
    }

    /// Explicitly override or initialize the model catalog.
    ///
    /// Rebinds the catalog lock AND rebuilds the dependent model caller
    /// atomically (`with_catalog_rebound`), so the caller can never observe a
    /// stale catalog after replacement (Invariant 6).
    pub fn with_model_catalog(mut self, catalog: crate::model::catalog::ModelCatalog) -> Self {
        self.authorities = Arc::new(self.authorities.with_catalog_rebound(catalog));
        self.model_catalog = self.authorities.model_catalog().clone();
        self.model_caller = self.authorities.model_caller();
        self.rebuild_dependencies();
        self
    }

    /// Refresh the dynamic model catalog by querying the configured model provider.
    pub async fn refresh_model_catalog(
        &self,
    ) -> Result<crate::model::catalog::ModelCatalog, crate::model::types::ModelError> {
        if let Some(ref provider) = self.model_provider {
            match provider.discover_models().await {
                Ok(models) => {
                    let mut cat = self.model_catalog.write().await;
                    cat.update_from_provider(&self.config.active_provider, models);
                    let cache_path = crate::model::catalog::ModelCatalog::cache_path_for_channel(
                        &self.workspace_root,
                        self.authorities.channel(),
                    );
                    let _ = cat.save_to_cache_file(&cache_path);
                    Ok(cat.clone())
                }
                Err(e) => {
                    let mut cat = self.model_catalog.write().await;
                    if !cat.models.is_empty() {
                        cat.refresh_state =
                            crate::model::catalog::CatalogRefreshState::DiscoveryFailedWithCache;
                    } else {
                        cat.refresh_state =
                            crate::model::catalog::CatalogRefreshState::DiscoveryFailedNoCache;
                    }
                    Err(e)
                }
            }
        } else {
            Err(crate::model::types::ModelError::EndpointUnavailable(
                "No model provider configured on runtime".to_string(),
            ))
        }
    }

    /// Construct a complete WorkflowEngine attached to this runtime's execution spine.
    pub fn create_workflow_engine(
        &self,
        prompt_catalog: Arc<dyn crate::prompt::PromptCatalog>,
    ) -> crate::workflow::engine::WorkflowEngine {
        let repo = Arc::new(crate::workflow::repository::SqliteWorkflowRepository::new(
            self.pool.clone(),
        ));
        crate::workflow::engine::WorkflowEngine::new(
            repo,
            prompt_catalog,
            Some(self.event_bus.clone() as Arc<dyn EventBus>),
        )
        .with_dependencies(self.dependencies.clone())
        .with_artifact_service(self.artifact_service.clone())
    }

    /// Create a PreExecutionCoordinator attached to this runtime's authoritative
    /// database pool, event bus, model caller, prompt catalog, and context compiler.
    ///
    /// Consumes the runtime-shared authorities — never a second planner/model/
    /// context stack. The context compiler is the canonical shared instance
    /// (workspace + prompt catalog + memory + role-stage), not a fresh
    /// `ProductionContextCompiler::new()` with divergent inputs.
    pub fn create_pre_execution_coordinator(
        &self,
    ) -> crate::planning::review::PreExecutionCoordinator {
        let mut coord = crate::planning::review::PreExecutionCoordinator::new(
            self.pool.clone(),
            Some(self.event_bus.clone()),
        )
        .with_workspace_root(self.workspace_root.clone());
        if let Some(ref caller) = self.model_caller {
            coord = coord.with_model_caller(caller.clone());
        }
        // Reuse the runtime-shared prompt catalog (one prompt authority).
        coord = coord.with_prompt_catalog(self.prompt_catalog_arc());
        coord = coord.with_context_compiler(self.authorities.context_compiler().clone());
        coord
    }

    /// Schemas for the runtime-shared tool registry, filtered to the wire
    /// format for the implementer role envelope. Used whenever a model
    /// caller is (re)built so the caller always observes the canonical
    /// registry — never a stale or forked copy.
    fn shared_governed_tool_schemas(&self) -> Vec<serde_json::Value> {
        self.authorities.model_tool_schemas()
    }

    /// Access the single canonical authority set for this runtime scope.
    ///
    /// Derived consumers (controllers, dispatchers, engines, coordinators)
    /// MUST observe these instances. The set is re-packed atomically after
    /// every mutation (`sync_authorities`), so holders of a previous `Arc`
    /// generation can detect staleness by pointer comparison.
    pub fn authorities(&self) -> &Arc<crate::runtime_authorities::RuntimeAuthorities> {
        &self.authorities
    }

    /// Re-pack the canonical authority set from the current field instances.
    ///
    /// Called at the end of EVERY mutation path so `authorities` always
    /// mirrors the same `Arc`s the field accessors expose. Partial mutation
    /// without re-packing is forbidden: it would fork the authority graph.
    fn sync_authorities(&mut self) {
        self.authorities = Arc::new(crate::runtime_authorities::RuntimeAuthorities::new(
            self.config.clone(),
            self.policy.clone(),
            self.capability_registry.clone(),
            self.tool_registry.clone(),
            self.budget_enforcer.clone(),
            self.approval_coordinator.clone(),
            self.model_catalog.clone(),
            self.model_provider.clone(),
            self.model_caller.clone(),
            self.authorities.context_compiler().clone(),
            self.prompt_catalog.clone(),
            self.artifact_store.clone(),
            self.event_bus.clone(),
            self.git_service.clone(),
            self.workspace_root.clone(),
            self.storage_root.clone(),
            self.authorities.channel(),
        ));
    }

    /// Reconfigure runtime with a mutated or customized configuration.
    ///
    /// Atomic set reconstruction: the canonical authority set is rebuilt via
    /// `RuntimeAuthorities::reconfigured` (policy + model caller re-derived
    /// from the new config; shared mutable state — budget counters, catalog
    /// contents, approval waiters — preserved), field mirrors are synced, and
    /// controller dependencies are reassembled from the SAME instances. No
    /// stale consumer of the previous generation survives inside this
    /// runtime. Externally cached derived objects (notably a previously
    /// created `AgentEngine`) are INVALIDATED by reconfiguration and must be
    /// recreated via `create_agent_engine`.
    pub fn with_config(mut self, config: Arc<crate::config::ResolvedConfiguration>) -> Self {
        // 1. Atomic authority-set reconstruction (policy + caller re-derived).
        self.authorities = Arc::new(self.authorities.reconfigured(config.clone()));
        // 2. Sync field mirrors from the new authoritative generation.
        self.config = config.clone();
        self.policy = self.authorities.policy().clone();
        self.model_caller = self.authorities.model_caller();

        let mut budget = ResourceBudget::default();
        budget.max_agent_steps = config.app_config.budget.max_agent_steps;
        budget.max_tokens = config.app_config.budget.max_tokens;
        budget.max_wall_clock_seconds = config
            .app_config
            .budget
            .max_wall_clock_seconds
            .or(Some(config.app_config.runtime.timeout_secs));
        budget.max_cost_usd = config.app_config.budget.max_cost_usd;
        budget.max_retries = config.app_config.budget.max_retries;
        budget.max_concurrent_agents = Some(config.app_config.runtime.concurrency_limit);
        // Update in place: all holders of the shared enforcer observe the
        // new limits without losing consumption counters.
        self.budget_enforcer.update_limits(budget);

        let retention = match config
            .app_config
            .git
            .retention_policy
            .to_lowercase()
            .as_str()
        {
            "always_remove" | "remove" => {
                crate::git::worktree::WorktreeRetentionPolicy::AlwaysRemove
            }
            "always_keep" | "keep" => crate::git::worktree::WorktreeRetentionPolicy::AlwaysKeep,
            _ => crate::git::worktree::WorktreeRetentionPolicy::KeepOnFailure,
        };
        let mut wt_cfg = WorktreeConfig::new(&self.workspace_root).with_retention(retention);
        if let Some(ref dir) = config.app_config.git.worktree_dir {
            wt_cfg = wt_cfg.with_worktrees_dir(dir.clone());
        }
        self.worktree_manager = Arc::new(WorktreeManager::new(wt_cfg));

        // Model caller already re-derived atomically by `reconfigured` above
        // (fail-closed to `None` when no provider is bound); the mirror sync
        // at the top of this method installed it. Reassemble dependents.
        self.rebuild_dependencies();
        self
    }

    /// Reassemble controller dependencies from the current authoritative field
    /// instances. Every dependency-mutating path funnels through here so the
    /// controller bundle can never diverge from runtime authorities.
    fn rebuild_dependencies(&mut self) {
        self.dependencies = ControllerDependencies::production_with_shared_authorities(
            self.pool.clone(),
            self.workspace_root.clone(),
            self.storage_root.clone(),
            Some(self.event_bus.clone()),
            self.model_caller.clone(),
            Some(&self.config),
            Some(self.approval_coordinator.clone()),
            self.policy.clone(),
            self.artifact_store.clone(),
            self.budget_enforcer.clone(),
            Some(self.capability_registry.clone()),
            Some(self.authorities.context_compiler().clone()),
        );
        self.dependencies = self
            .dependencies
            .clone()
            .with_git_service(self.git_service.clone());
    }

    /// Configure a custom model caller (for autonomous execution with real/mock models).
    ///
    /// Rebuilds controller dependencies against the same shared
    /// policy/budget/artifact authorities so the new caller cannot leave a
    /// stale consumer behind.
    pub fn with_model_caller(
        mut self,
        caller: Arc<dyn crate::agent::model_policy::ModelCaller>,
    ) -> Self {
        self.model_caller = Some(caller);
        self.rebuild_dependencies();
        self.sync_authorities();
        self
    }

    /// Clear configured model caller and provider (for deterministic tests testing no-model behavior).
    pub fn without_model_caller(mut self) -> Self {
        self.model_caller = None;
        self.model_provider = None;
        self.rebuild_dependencies();
        self.sync_authorities();
        self
    }

    /// Configure a real model provider on the production runtime, preserving full routing and tools.
    ///
    /// Tool schemas derive from the runtime-shared registries so the new
    /// caller observes the canonical capability environment.
    pub fn with_model_provider(
        mut self,
        provider: Arc<dyn crate::model::provider::ModelProvider>,
    ) -> Self {
        self.model_provider = Some(provider.clone());
        let tool_schemas = self.shared_governed_tool_schemas();
        let caller = Arc::new(
            crate::agent::model_policy::RoutedModelCaller::new(
                Some(provider),
                crate::model::router::resolver::ModelTier::Standard,
                tool_schemas,
            )
            .with_catalog_lock(self.model_catalog.clone())
            // Atomic wiring: include static candidates for the active model plus
            // the active provider identity.
            .with_model(self.config.active_model.clone())
            .with_provider_status(
                self.config.active_provider.clone(),
                crate::model::types::ProviderCapabilityStatus::Available,
            ),
        );
        self.with_model_caller(caller)
    }

    /// Execute a mission to completion via the AutonomyController closed loop (F-01).
    pub async fn run_mission(
        &self,
        prompt: &str,
        profile_name: Option<&str>,
        wait_for_approval: bool,
    ) -> Result<MissionExecutionSummary, M31AError> {
        self.run_mission_with_context(prompt, profile_name, wait_for_approval, None)
            .await
    }

    /// Execute a mission with optional pre-synthesized upstream planning context.
    /// If upstream context is not provided, autonomous upstream discovery and intent expansion
    /// evaluates the prompt tier and synthesizes target domain, architecture, and requirements
    /// for greenfield and consequential workflows.
    pub async fn run_mission_with_context(
        &self,
        prompt: &str,
        profile_name: Option<&str>,
        _wait_for_approval: bool,
        mut upstream_context: Option<crate::kernel::seams::planner::UpstreamPlanContext>,
    ) -> Result<MissionExecutionSummary, M31AError> {
        let mission_id = MissionId::new();
        // Single definition site for profile → autonomy mapping
        // (`AutonomyPrecedence::from_profile_name`); interactive engine
        // construction shares it so mission and interactive execution agree.
        let mode = crate::runtime_authorities::AutonomyPrecedence::from_profile_name(profile_name);

        let full_prompt = if !prompt.contains("<explicit_developer_mentions>") {
            let parsed = crate::interaction::mentions::MentionParser::parse_implicit_or_explicit(
                prompt,
                &self.workspace_root,
            );
            if !parsed.mentions.is_empty() {
                let mention_ctx =
                    crate::interaction::mentions::MentionParser::inject_mention_context(
                        &self.workspace_root,
                        &parsed.mentions,
                    );
                format!("{}{}", prompt, mention_ctx)
            } else {
                prompt.to_string()
            }
        } else {
            prompt.to_string()
        };

        // Fail closed immediately if worktree isolation is required by policy but git is not initialized (Findings F & G)
        let isolation_required = self.config.app_config.git.execution_isolation == "required";
        if isolation_required && !self.workspace_root.join(".git").exists() {
            return Err(M31AError::Internal(anyhow::anyhow!(
                "Execution blocked: git worktree isolation required by policy, but workspace \
                 '{}' is not a git repository (.git missing). Initialize a git repository \
                 or set git.execution_isolation = \"best_effort\" to allow unisolated execution.",
                self.workspace_root.display()
            )));
        }

        // Upstream discovery and intent expansion if not explicitly provided
        if upstream_context.is_none() {
            let env = crate::workflow::genesis::GenesisController::probe(&self.workspace_root)
                .unwrap_or_else(|_| crate::workflow::genesis::WorkspaceEnvironment {
                    workspace_root: self.workspace_root.clone(),
                    facts: Vec::new(),
                    detected_mode: crate::workflow::genesis::GenesisMode::AutoDetect,
                    detected_stack: None,
                    file_count: 0,
                    has_git: false,
                });
            let tier =
                crate::workflow::genesis::discovery::classify_workflow_tier(&full_prompt, &env);

            if (tier == crate::workflow::genesis::WorkflowTier::Greenfield
                || tier == crate::workflow::genesis::WorkflowTier::Consequential)
                && self.model_caller.is_some()
            {
                // Research follows operator configuration (`workflow.research`, default on).
                // The evidence-driven research decision (tier skip, fully-specified skip,
                // localized-fix skip, targeted brownfield subset) keeps depth proportional
                // once enabled.
                let options = crate::workflow::genesis::GenesisOptions::from_config(&self.config);
                let gen_req = crate::workflow::genesis::GenesisRequest::new(
                    &full_prompt,
                    &self.workspace_root,
                )
                .with_mission_id(mission_id)
                .with_options(options)
                .with_mode(
                    if env.detected_mode == crate::workflow::genesis::GenesisMode::Brownfield {
                        crate::workflow::genesis::GenesisMode::Brownfield
                    } else {
                        crate::workflow::genesis::GenesisMode::Greenfield
                    },
                );

                let outcome = self.run_genesis(&gen_req).await.map_err(|e| {
                    tracing::error!(
                        mission = %mission_id,
                        error = %e,
                        "Genesis lifecycle failed for Greenfield/Consequential mission; \
                         cannot proceed without architectural context"
                    );
                    e
                })?;
                let charter_md = outcome.charter.to_markdown();
                let arch_md = outcome.planning.architecture.to_markdown();
                let req_list = outcome
                    .planning
                    .requirements
                    .requirements
                    .iter()
                    .map(|r| {
                        format!(
                            "{}: {}",
                            r.key,
                            r.title.as_deref().unwrap_or(&r.description)
                        )
                    })
                    .collect();

                // 1. Genuine engineering assumptions extracted from domain defaults
                let mut assumptions_list = Vec::new();
                for d in &outcome.charter.domain_model.inferred_defaults {
                    assumptions_list.push(format!("Inferred default: {}", d));
                }

                // 2. Unresolved epistemic unknowns (both discovery and research coexist)
                let mut unknowns_list = Vec::new();
                for u in &outcome.charter.ambiguity_assessment.unresolved_areas {
                    unknowns_list.push(format!("Discovery unknown: {}", u));
                }
                for u in &outcome.planning.risks.unknowns {
                    unknowns_list.push(format!("Research unknown {}: {}", u.id, u.description));
                }

                // 3. Consequential user decisions requiring operator direction
                let mut user_decisions_list = Vec::new();
                let discovery_unknowns = crate::workflow::genesis::discovery::extract_unknowns(
                    &gen_req.prompt,
                    outcome.charter.workflow_tier,
                );
                for u in discovery_unknowns {
                    if u.fate == crate::planning::risks::UnknownFate::UserDecisionRequired
                        || u.fate == crate::planning::risks::UnknownFate::Blocking
                    {
                        user_decisions_list.push(format!(
                            "{}: {} (Required Decision: {})",
                            u.id,
                            u.description,
                            u.resolution
                                .as_deref()
                                .unwrap_or("Operator decision required")
                        ));
                    }
                }

                // 4. Definitively resolved invariants
                let resolved_list = outcome
                    .charter
                    .ambiguity_assessment
                    .resolved_invariants
                    .clone();

                let decisions_list = outcome
                    .planning
                    .adrs
                    .adrs
                    .values()
                    .map(|a| format!("{}: {}", a.id, a.title))
                    .collect();
                let research_md = outcome.research_summary.map(|s| s.to_markdown());

                upstream_context = Some(crate::kernel::seams::planner::UpstreamPlanContext {
                    project_name: outcome.charter.project_name.clone(),
                    charter: charter_md,
                    architecture: arch_md,
                    requirements: req_list,
                    assumptions: assumptions_list,
                    decisions: decisions_list,
                    research_summary: research_md,
                    workflow_tier: format!("{:?}", outcome.charter.workflow_tier),
                    unknowns: unknowns_list,
                    user_decisions: user_decisions_list,
                    resolved_invariants: resolved_list,
                });
            } else if (tier == crate::workflow::genesis::WorkflowTier::Greenfield
                || tier == crate::workflow::genesis::WorkflowTier::Consequential)
                && self.model_caller.is_none()
            {
                tracing::warn!(
                    mission = %mission_id,
                    "No model caller configured for Greenfield/Consequential mission; \
                     skipping Genesis lifecycle and delegating missing-model enforcement to planner"
                );
            }
        }

        self.execute_mission_loop(mission_id, prompt, full_prompt, mode, upstream_context)
            .await
    }

    /// Execute an authorized mission whose TaskGraph has already been materialized.
    pub async fn run_authorized_mission(
        &self,
        mission_id: MissionId,
        objective: &str,
    ) -> Result<MissionExecutionSummary, M31AError> {
        self.execute_mission_loop(
            mission_id,
            objective,
            objective.to_string(),
            AutonomyMode::Safe,
            None,
        )
        .await
    }

    async fn execute_mission_loop(
        &self,
        mission_id: MissionId,
        prompt: &str,
        full_prompt: String,
        mode: AutonomyMode,
        upstream_context: Option<crate::kernel::seams::planner::UpstreamPlanContext>,
    ) -> Result<MissionExecutionSummary, M31AError> {
        // 1. Insert mission into SQLite via canonical repository
        let mission = Mission::new(mission_id, full_prompt.clone());
        if let Some(repo) = self.dependencies.mission_repo() {
            match repo.insert(&mission).await {
                Ok(_) => {}
                Err(e) => {
                    // Check for UNIQUE constraint (mission already exists with this ID: idempotent)
                    let err_str = e.to_string().to_lowercase();
                    if !err_str.contains("unique") && !err_str.contains("already exists") {
                        return Err(M31AError::Internal(anyhow::anyhow!(
                            "Mission persistence failed for mission {}: {}",
                            mission_id,
                            e
                        )));
                    }
                    // Mission already exists with this ID — idempotent, continue
                    tracing::debug!(
                        mission = %mission_id,
                        "Mission already exists in repository; continuing with existing record"
                    );
                }
            }
        } else {
            let repo = crate::persistence::sqlite::repositories::SqliteMissionRepository::new(
                self.pool.clone(),
            );
            match repo.insert(&mission).await {
                Ok(_) => {}
                Err(e) => {
                    let err_str = e.to_string().to_lowercase();
                    if !err_str.contains("unique") && !err_str.contains("already exists") {
                        return Err(M31AError::Internal(anyhow::anyhow!(
                            "Mission persistence failed for mission {}: {}",
                            mission_id,
                            e
                        )));
                    }
                    tracing::debug!(
                        mission = %mission_id,
                        "Mission already exists in repository; continuing with existing record"
                    );
                }
            }
        }

        // 2. Publish MissionStarted event
        let start_env = EventEnvelope::new(
            0,
            Some(mission_id),
            None,
            "runtime".to_string(),
            EventType::MissionStarted {
                mission_id,
                objective: full_prompt.clone(),
            },
        );
        let _ = self.event_bus.publish(start_env).await;

        // 3. Setup worktree isolation if git repository exists (F-13, Findings F & G).
        // Isolation policy controls whether a failed worktree blocks execution:
        //   "required"    — worktree failure returns an explicit error; no silent downgrade. (Default)
        //   "best_effort" — worktree failure is logged prominently but execution continues
        //                   in the primary workspace. Explicit opt-in only.
        let isolation_required = self.config.app_config.git.execution_isolation == "required";
        let worktree_opt = if self.workspace_root.join(".git").exists() {
            match self
                .worktree_manager
                .create_worktree(&mission_id, None)
                .await
            {
                Ok(wt) => Some(wt),
                Err(ref e) => {
                    if isolation_required {
                        return Err(M31AError::Internal(anyhow::anyhow!(
                            "Execution blocked: git worktree isolation required by policy but \
                             worktree creation failed for mission {}: {}. Set \
                             git.execution_isolation = \"best_effort\" to allow primary-workspace \
                             fallback, or fix the isolation failure.",
                            mission_id,
                            e
                        )));
                    }
                    // best_effort: log explicitly (never silently) and fall through to
                    // primary workspace execution with the default dependencies.
                    tracing::warn!(
                        mission = %mission_id,
                        error = %e,
                        isolation_policy = "best_effort",
                        "ISOLATION DOWNGRADE: Failed to create isolated worktree; \
                         mission will proceed in primary workspace. \
                         Set git.execution_isolation = \"required\" to block this."
                    );
                    None
                }
            }
        } else if isolation_required {
            return Err(M31AError::Internal(anyhow::anyhow!(
                "Execution blocked: git worktree isolation required by policy, but workspace \
                 '{}' is not a git repository (.git missing). Initialize a git repository \
                 or set git.execution_isolation = \"best_effort\" to allow unisolated execution.",
                self.workspace_root.display()
            )));
        } else {
            tracing::warn!(
                mission = %mission_id,
                isolation_policy = "best_effort",
                "ISOLATION DOWNGRADE: Workspace is not a git repository; \
                 mission will proceed in primary workspace. \
                 Set git.execution_isolation = \"required\" to block this."
            );
            None
        };

        if let Some(ref wt) = worktree_opt {
            let ws_branch = self
                .git_service()
                .status()
                .await
                .map(|s| s.branch)
                .unwrap_or_else(|_| "N/A".to_string());
            let envelope = EventEnvelope::new(
                0,
                Some(mission_id),
                None,
                "runtime".to_string(),
                EventType::GitStateChanged {
                    workspace_branch: ws_branch,
                    execution_branch: Some(wt.branch.clone()),
                    is_clean: true,
                },
            );
            let _ = self.event_bus.publish(envelope).await;
        }

        // 4. Instantiate AutonomyController with production dependencies.
        // Mission-scoped worktree runs share the runtime's canonical
        // policy/budget/artifact authorities; capabilities AND the context
        // compiler re-derive for the worktree root (MISSION_SCOPED) since the
        // execution root differs. This is an explicit scope rule, not a fork:
        // same policy/budget/artifacts/approval/model authorities, with
        // root-bound environment (capabilities, compiler workspace) rebuilt
        // deterministically from the worktree root.
        let active_deps = if let Some(ref wt) = worktree_opt {
            ControllerDependencies::production_with_shared_authorities(
                self.pool.clone(),
                wt.path.clone(),
                self.storage_root.clone(),
                Some(self.event_bus.clone()),
                self.model_caller.clone(),
                Some(&self.config),
                Some(self.approval_coordinator.clone()),
                self.policy.clone(),
                self.artifact_store.clone(),
                self.budget_enforcer.clone(),
                None,
                None,
            )
            .with_git_service(self.git_service.clone())
        } else {
            self.dependencies.clone()
        };

        let cancel_token = CancellationToken::new();
        let mut controller = AutonomyController::with_budget(
            mission_id,
            mode,
            active_deps,
            self.budget_enforcer.budget().clone(),
            None,
            self.event_bus.clone() as Arc<dyn crate::events::EventBus>,
            cancel_token,
        )
        .with_mission_objective(&full_prompt)
        .with_workspace_root(
            worktree_opt
                .as_ref()
                .map(|w| w.path.clone())
                .unwrap_or_else(|| self.workspace_root.clone()),
        );

        if let Some(ctx) = upstream_context {
            controller = controller.with_upstream_context(ctx);
        }

        // 5. Run controller loop to terminal state (F-01)
        let run_res = controller.run().await;
        let halt_reason = match run_res {
            Ok(reason) => reason,
            Err(e) => {
                if let Some(ref wt) = worktree_opt
                    && self.worktree_manager.config().retention_policy
                        == crate::git::worktree::WorktreeRetentionPolicy::AlwaysRemove
                {
                    let _ = self
                        .worktree_manager
                        .remove_worktree(wt, true, &crate::git::GitGate::authorized())
                        .await;
                }
                return Err(M31AError::Internal(anyhow::anyhow!(e.to_string())));
            }
        };

        // 6. Seal report if satisfied or generate completion candidate (F-09)
        let (mut final_status, mut is_success) = match &halt_reason {
            ControllerHaltReason::MissionCompleted => ("Completed", true),
            ControllerHaltReason::Cancelled => ("Cancelled", false),
            ControllerHaltReason::Paused => ("Paused", false),
            _ => ("Failed", false),
        };
        let mut worktree_cleaned = false;

        if is_success && self.config.app_config.git.auto_commit {
            if let Some(ref wt) = worktree_opt {
                let trailers = CommitTrailers::new(
                    mission_id,
                    crate::ids::TaskId::new(),
                    crate::state_machine::agent::AgentRole::implementer(),
                    &self.config.active_model,
                );
                let msg = CommitTrailers::embed_trailers(
                    &format!(
                        "fix: {}\n\nAutonomous mission completed successfully.",
                        prompt
                    ),
                    &trailers,
                )
                .unwrap_or_else(|_| format!("fix: {}\n\nM31A-Mission: {}", prompt, mission_id));

                let _ = crate::git::worktree::WorktreeManager::ensure_git_excludes(&wt.path).await;
                let _ = crate::git::worktree::WorktreeManager::ensure_git_excludes(
                    &self.workspace_root,
                )
                .await;

                let wt_git = crate::capability::providers::CliGitProvider::new(&wt.path);
                let _ = wt_git.add_all().await;
                let _ = wt_git
                    .reset(&[".m31a", "target", "node_modules", "__pycache__"])
                    .await;

                let commit_out = wt_git.commit(&msg).await;

                let mut merge_succeeded = false;
                if commit_out.is_ok() {
                    let _ = crate::git::worktree::WorktreeManager::ensure_git_excludes(
                        &self.workspace_root,
                    )
                    .await;
                    let merge_out = self.git_service.merge(&wt.branch, true).await;

                    match merge_out {
                        Ok(_) => {
                            merge_succeeded = true;
                        }
                        Err(err) => {
                            tracing::warn!(
                                mission_id = %mission_id,
                                error = %err,
                                "Git merge into workspace root failed; rolling back safely without modifying primary workspace files"
                            );

                            // Safely abort/rollback the attempted merge if Git entered MERGING state.
                            // git merge --abort only restores index/HEAD and NEVER deletes untracked files.
                            let _ = self.git_service.merge_abort().await;

                            is_success = false;
                            final_status = "Failed";
                        }
                    }

                    // Handle ephemeral worktree cleanup according to retention policy
                    if merge_succeeded {
                        if self.worktree_manager.config().retention_policy
                            != crate::git::worktree::WorktreeRetentionPolicy::AlwaysKeep
                        {
                            let _ = self
                                .worktree_manager
                                .remove_worktree(wt, false, &crate::git::GitGate::authorized())
                                .await;
                            worktree_cleaned = true;
                        }
                    } else if self.worktree_manager.config().retention_policy
                        == crate::git::worktree::WorktreeRetentionPolicy::AlwaysRemove
                    {
                        let _ = self
                            .worktree_manager
                            .remove_worktree(wt, true, &crate::git::GitGate::authorized())
                            .await;
                        worktree_cleaned = true;
                    }
                } else {
                    is_success = false;
                    final_status = "Failed";
                    if self.worktree_manager.config().retention_policy
                        == crate::git::worktree::WorktreeRetentionPolicy::AlwaysRemove
                    {
                        let _ = self
                            .worktree_manager
                            .remove_worktree(wt, true, &crate::git::GitGate::authorized())
                            .await;
                        worktree_cleaned = true;
                    }
                }
            } else if self.workspace_root.join(".git").exists() {
                let trailers = CommitTrailers::new(
                    mission_id,
                    crate::ids::TaskId::new(),
                    crate::state_machine::agent::AgentRole::implementer(),
                    &self.config.active_model,
                );
                let msg = CommitTrailers::embed_trailers(
                    &format!(
                        "fix: {}\n\nAutonomous mission completed successfully.",
                        prompt
                    ),
                    &trailers,
                )
                .unwrap_or_else(|_| format!("fix: {}\n\nM31A-Mission: {}", prompt, mission_id));

                let _ = crate::git::worktree::WorktreeManager::ensure_git_excludes(
                    &self.workspace_root,
                )
                .await;

                let _ = self.git_service.add_all().await;

                let _ = self
                    .git_service
                    .reset(&[".m31a", "target", "node_modules", "__pycache__"])
                    .await;

                let _ = self.git_service.commit(&msg).await;
            }
        }

        // Guaranteed cleanup for worktrees if not already cleaned up
        if !worktree_cleaned && let Some(ref wt) = worktree_opt {
            let policy = self.worktree_manager.config().retention_policy;
            if policy == crate::git::worktree::WorktreeRetentionPolicy::AlwaysRemove {
                let _ = self
                    .worktree_manager
                    .remove_worktree(wt, true, &crate::git::GitGate::authorized())
                    .await;
            } else if is_success
                && policy != crate::git::worktree::WorktreeRetentionPolicy::AlwaysKeep
            {
                let _ = self
                    .worktree_manager
                    .remove_worktree(wt, false, &crate::git::GitGate::authorized())
                    .await;
            }
        }

        if worktree_opt.is_some() {
            let ws_branch = self
                .git_service()
                .status()
                .await
                .map(|s| s.branch)
                .unwrap_or_else(|_| "N/A".to_string());
            let envelope = EventEnvelope::new(
                0,
                Some(mission_id),
                None,
                "runtime".to_string(),
                EventType::GitStateChanged {
                    workspace_branch: ws_branch,
                    execution_branch: None,
                    is_clean: true,
                },
            );
            let _ = self.event_bus.publish(envelope).await;
        }

        if let Ok(candidate) = self.report_generator.build_candidate(&mission_id).await {
            let gate_res = if is_success { Ok(()) } else { Err(vec![]) };
            if let Ok(report) = self
                .report_generator
                .seal_report(candidate, &gate_res)
                .await
            {
                let reports_dir = self
                    .storage_root
                    .join("reports")
                    .join(mission_id.to_string());
                let _ = self
                    .report_generator
                    .write_report_files(&report, &reports_dir)
                    .await;
            }
        }

        // 7. Update mission final status through MissionRepository.
        // Persistence failure propagates: a completed mission whose terminal
        // status was not recorded must not report success.
        let mission_repo =
            crate::persistence::sqlite::repositories::mission::SqliteMissionRepository::new(
                self.pool.clone(),
            );
        let mission_state = if is_success {
            crate::state_machine::MissionState::Completed
        } else if final_status == "Cancelled" {
            crate::state_machine::MissionState::Cancelled
        } else {
            crate::state_machine::MissionState::Failed
        };
        crate::persistence::sqlite::repositories::MissionRepository::update_status(
            &mission_repo,
            mission_id,
            mission_state,
        )
        .await
        .map_err(|e| {
            M31AError::internal(format!("mission terminal status persistence failed: {e}"))
        })?;

        let halt_summary = if !is_success && halt_reason == ControllerHaltReason::MissionCompleted {
            "MergeFailed".to_string()
        } else {
            format!("{:?}", halt_reason)
        };

        // 8. Publish terminal event
        let terminal_env = EventEnvelope::new(
            0,
            Some(mission_id),
            None,
            "runtime".to_string(),
            if is_success {
                EventType::MissionCompleted { mission_id }
            } else {
                EventType::MissionFailed {
                    mission_id,
                    reason: halt_summary.clone(),
                }
            },
        );
        let _ = self.event_bus.publish(terminal_env).await;

        // 9. Count completed tasks via canonical task repository
        let task_repo =
            crate::persistence::sqlite::repositories::SqliteTaskRepository::new(self.pool.clone());
        let completed_tasks = task_repo
            .count_completed_by_mission(mission_id)
            .await
            .unwrap_or(0);

        Ok(MissionExecutionSummary {
            mission_id,
            objective: prompt.to_string(),
            status: final_status.to_string(),
            halt_reason: halt_summary,
            tasks_completed: completed_tasks,
            worktree_path: worktree_opt.map(|w| w.path),
        })
    }

    /// Cancel an active mission, updating persistent state through MissionRepository
    /// and broadcasting a canonical `MissionCancelled` event (PRD §01, AD-008).
    pub async fn cancel_mission(
        &self,
        mission_id: MissionId,
        reason: &str,
    ) -> Result<(), M31AError> {
        let mission_repo =
            crate::persistence::sqlite::repositories::mission::SqliteMissionRepository::new(
                self.pool.clone(),
            );
        crate::persistence::sqlite::repositories::MissionRepository::update_status(
            &mission_repo,
            mission_id,
            crate::state_machine::MissionState::Cancelled,
        )
        .await?;

        let env = EventEnvelope::new(
            0,
            Some(mission_id),
            None,
            "runtime".to_string(),
            EventType::MissionCancelled {
                mission_id,
                reason: reason.to_string(),
            },
        );
        let _ = self.event_bus.publish(env).await;
        Ok(())
    }

    /// Access the SQLite session repository.
    pub fn session_repo(&self) -> crate::interaction::session::SqliteSessionRepository {
        crate::interaction::session::SqliteSessionRepository::new(self.pool.clone())
            .with_event_bus(self.event_bus.clone() as Arc<dyn crate::events::EventBus>)
    }

    /// Create and persist a new interactive developer session.
    pub async fn create_session(&self) -> Result<crate::interaction::session::Session, M31AError> {
        self.session_repo()
            .create_session(&self.workspace_root)
            .await
    }

    /// Retrieve an existing session by ID.
    pub async fn get_session(
        &self,
        id: crate::ids::SessionId,
    ) -> Result<Option<crate::interaction::session::Session>, M31AError> {
        self.session_repo().get_session(id).await
    }

    /// List all known developer sessions.
    pub async fn list_sessions(
        &self,
    ) -> Result<Vec<crate::interaction::session::Session>, M31AError> {
        self.session_repo().list_sessions().await
    }

    /// Resume an existing session and reconstruct its runtime conversation state.
    pub async fn resume_session(
        &self,
        id: crate::ids::SessionId,
    ) -> Result<crate::interaction::session::Session, M31AError> {
        let repo = self.session_repo();
        let session = repo
            .get_session(id)
            .await?
            .ok_or_else(|| M31AError::NotFound(format!("Session '{id}' not found")))?;

        // Ensure session status is Active upon resume
        if session.status != crate::interaction::session::SessionState::Active {
            repo.update_status(id, crate::interaction::session::SessionState::Active)
                .await?;
        }

        // Publish SessionResumed event
        let env = EventEnvelope::new(
            0,
            session.active_mission_id,
            None,
            "runtime".to_string(),
            EventType::SessionStarted {
                session_id: session.id,
                mission_id: session.active_mission_id.unwrap_or_else(MissionId::new),
            },
        );
        let _ = self.event_bus.publish(env).await;

        Ok(session)
    }

    /// Inspect Git and worktree diff safely without altering repository state.
    pub async fn get_git_diff(&self) -> Result<String, M31AError> {
        if !self.workspace_root.join(".git").exists() {
            return Ok("Not a git repository.".to_string());
        }

        // Check for active worktrees
        let wt_dir = self.storage_root.join("worktrees");
        let mut target_dir = self.workspace_root.clone();

        if wt_dir.exists()
            && let Ok(mut entries) = tokio::fs::read_dir(&wt_dir).await
        {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if entry.path().is_dir() {
                    let entry_git = crate::capability::providers::CliGitProvider::new(entry.path());
                    if let Ok(out) = entry_git.status_porcelain().await
                        && !out.trim().is_empty()
                    {
                        target_dir = entry.path();
                        break;
                    }
                }
            }
        }

        let target_git = crate::capability::providers::CliGitProvider::new(&target_dir);
        let status_str = target_git
            .status_porcelain()
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!("git status failed: {e}")))?;

        let diff_out = target_git
            .diff_range("HEAD")
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!("git diff failed: {e}")))?;

        let mut diff_str = diff_out;
        if diff_str.trim().is_empty()
            && let Ok(c) = target_git.diff_range("--cached").await
        {
            diff_str = c;
        }

        let mut showing_latest_commit = false;
        if diff_str.trim().is_empty()
            && let Ok(h) = target_git.diff_range("HEAD~1..HEAD").await
        {
            let s = h.trim().to_string();
            if !s.is_empty() {
                diff_str = s;
                showing_latest_commit = true;
            }
        }

        let mut result = format!(
            "Target: {}\n\nFiles changed:\n{}\n",
            target_dir.display(),
            if showing_latest_commit {
                "(working tree clean, showing latest commit HEAD~1..HEAD)"
            } else if status_str.trim().is_empty() {
                "(working tree clean)"
            } else {
                status_str.trim()
            }
        );

        if !diff_str.trim().is_empty() {
            let lines: Vec<&str> = diff_str.lines().collect();
            let bounded = if lines.len() > 300 {
                format!(
                    "{}\n... [Diff output bounded: showing first 300 of {} total lines] ...",
                    lines[..300].join("\n"),
                    lines.len()
                )
            } else {
                diff_str
            };
            result.push_str(&format!("\nDiff:\n{bounded}"));
        }

        Ok(result)
    }

    /// Commit verified changes with RFC 2822-compliant M31A trailers.
    pub async fn commit_changes(
        &self,
        mission_id: Option<MissionId>,
        message: Option<&str>,
    ) -> Result<String, M31AError> {
        if !self.workspace_root.join(".git").exists() {
            return Err(M31AError::Internal(anyhow::anyhow!("Not a git repository")));
        }

        // Verify independent verification checks before committing via canonical capability provider
        let verifier =
            crate::capability::providers::LocalVerificationProvider::new(&self.workspace_root);
        use crate::capability::traits::verification::{
            VerificationKind, VerificationService, VerificationTarget,
        };
        let target = VerificationTarget {
            name: "pre_commit_tests".to_string(),
            kind: VerificationKind::Test,
            args: vec![],
        };
        if let Ok(report) = verifier.run_verification(&target).await
            && !report.passed
        {
            return Err(M31AError::validation(
                "Commit rejected: repository tests are currently failing. Tests must pass before committing.".to_string(),
            ));
        }

        let status = self
            .git_service
            .status_porcelain()
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!("git status failed: {e}")))?;

        if status.trim().is_empty() {
            return Ok("Working tree is clean. Nothing to commit.".to_string());
        }

        let mid = mission_id.unwrap_or_default();
        let commit_msg = message.unwrap_or("Autonomous changes verified by M31A");
        let trailers = CommitTrailers::new(
            mid,
            crate::ids::TaskId::new(),
            crate::state_machine::agent::AgentRole::implementer(),
            "m31a-agent",
        );
        let full_msg = CommitTrailers::embed_trailers(
            &format!("fix: {commit_msg}\n\nCommitted via M31A interactive session."),
            &trailers,
        )
        .unwrap_or_else(|_| format!("fix: {commit_msg}\n\nM31A-Mission: {mid}"));

        let _ = self.git_service.add_all().await;

        let _commit_hash = self
            .git_service
            .commit(&full_msg)
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!("git commit failed: {e}")))?;

        Ok(format!("Committed changes successfully:\n{full_msg}"))
    }

    /// Execute a conversational or task turn within a durable session.
    pub async fn run_session_turn(
        &self,
        session_id: crate::ids::SessionId,
        user_prompt: &str,
        profile_name: Option<&str>,
        _cancel_token: CancellationToken,
    ) -> Result<MissionExecutionSummary, M31AError> {
        let session_repo = self.session_repo();
        let _session = session_repo
            .get_session(session_id)
            .await?
            .ok_or_else(|| M31AError::NotFound(format!("Session '{session_id}' not found")))?;

        // Parse mentions
        let parsed =
            crate::interaction::mentions::MentionParser::parse(user_prompt, &self.workspace_root);

        // Record UserMessage turn
        let user_seq = session_repo.next_sequence(session_id).await?;
        let user_turn = crate::interaction::session::ConversationTurn::UserMessage {
            id: uuid::Uuid::now_v7(),
            sequence: user_seq,
            content: parsed.normalized_prompt(),
            raw_text: user_prompt.to_string(),
            mentions: parsed.mentions.clone(),
            created_at: chrono::Utc::now(),
        };
        session_repo.append_turn(session_id, &user_turn).await?;

        // Enrich prompt with mention context
        let mention_ctx = crate::interaction::mentions::MentionParser::inject_mention_context(
            &self.workspace_root,
            &parsed.mentions,
        );
        let full_prompt = format!("{}{}", user_prompt, mention_ctx);

        // Execute mission
        let summary = self.run_mission(&full_prompt, profile_name, false).await?;

        // Update active mission on session
        session_repo
            .set_active_mission(session_id, summary.mission_id)
            .await?;

        // Record Assistant turn
        let asst_seq = session_repo.next_sequence(session_id).await?;
        let asst_turn = crate::interaction::session::ConversationTurn::AssistantMessage {
            id: uuid::Uuid::now_v7(),
            sequence: asst_seq,
            content: format!(
                "Mission {} concluded with status {}: {}",
                summary.mission_id, summary.status, summary.objective
            ),
            created_at: chrono::Utc::now(),
        };
        session_repo.append_turn(session_id, &asst_turn).await?;

        // Record Verification turn
        let verif_seq = session_repo.next_sequence(session_id).await?;
        let verif_turn = crate::interaction::session::ConversationTurn::VerificationMessage {
            id: uuid::Uuid::now_v7(),
            sequence: verif_seq,
            passed: summary.status == "Completed",
            summary: format!(
                "Tasks completed: {}, halt reason: {}",
                summary.tasks_completed, summary.halt_reason
            ),
            created_at: chrono::Utc::now(),
        };
        session_repo.append_turn(session_id, &verif_turn).await?;

        Ok(summary)
    }

    /// Execute the complete Project Genesis lifecycle:
    /// 1. Deterministic workspace environment probing (greenfield vs brownfield)
    /// 2. Codebase topology mapping if brownfield
    /// 3. Socratic discovery session & charter synthesis
    /// 4. ProjectCharter validation & save to projection directory
    /// 5. Register charter artifact in ArtifactService
    /// 6. Evaluate research decision (FullGreenfield vs TargetedDelta vs SkipResearch)
    /// 7. Execute bounded parallel research via WorkflowEngine if triggered
    /// 8. Execute comprehensive planning (Requirements, Architecture, ADRs, Risks, Roadmap, State)
    /// 9. Register all generated planning artifacts in ArtifactService (content hash + provenance)
    /// 10. Lower dependency-ordered Roadmap into executable WorkflowDefinition
    pub async fn run_genesis(
        &self,
        request: &GenesisRequest,
    ) -> Result<GenesisExecutionOutcome, M31AError> {
        // 0. Emit the canonical genesis lifecycle event so it is persisted
        // via the bus forwarder and visible to diagnostic observability.
        let genesis_env = EventEnvelope::new(
            0,
            request.mission_id,
            None,
            "runtime".to_string(),
            EventType::GenesisStarted {
                request_id: request.request_id.clone(),
                mode: format!("{:?}", request.mode),
            },
        );
        let _ = self.event_bus.publish(genesis_env).await;

        // 1. Probe environment deterministically
        let mut env = GenesisController::probe(&self.workspace_root)
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))?;

        if request.mode != GenesisMode::AutoDetect {
            env.detected_mode = request.mode;
        }

        // 2. Brownfield mapping if brownfield mode detected
        let brownfield_map = if env.detected_mode == GenesisMode::Brownfield {
            Some(
                GenesisController::map_brownfield(
                    &self.workspace_root,
                    &request.options.projection_dir,
                )
                .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))?,
            )
        } else {
            None
        };

        // 3. Socratic discovery session & charter synthesis
        let mut session = DiscoverySession::new(request.clone(), env.clone());
        session
            .submit_response(&request.prompt)
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))?;
        let charter = GenesisController::run_discovery(
            &mut session,
            &self.workspace_root,
            &request.options.projection_dir,
        )
        .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))?;

        // 4. Register charter in ArtifactService
        let charter_path = self
            .workspace_root
            .join(&request.options.projection_dir)
            .join("PROJECT.md");
        let charter_rec = self
            .artifact_service
            .register_genesis_artifact("PROJECT.md", &charter_path, "genesis.discovery", None)
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))?;
        let charter_id = charter_rec.id;

        // 5. Evaluate research decision
        let decision = GenesisController::decide_research(&charter, &env, &request.options);

        // 6. Execute bounded parallel research via WorkflowEngine if triggered
        // (shared prompt catalog: one prompt authority).
        let (summary, research_findings) = if decision.execute_research {
            let engine = self.create_workflow_engine(self.prompt_catalog_arc());
            GenesisController::execute_research(
                &engine,
                &charter,
                &decision,
                &request.options,
                &self.workspace_root,
            )
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))?
        } else {
            (None, Vec::new())
        };

        let mut registered_artifacts = vec![charter_rec.clone()];
        let summary_path = self
            .workspace_root
            .join(&request.options.projection_dir)
            .join("research")
            .join("SUMMARY.md");
        if summary_path.exists() {
            let sum_rec = self
                .artifact_service
                .register_genesis_artifact(
                    "SUMMARY.md",
                    &summary_path,
                    "genesis.research_synthesis",
                    Some(charter_id),
                )
                .await
                .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))?;
            registered_artifacts.push(sum_rec);
        }

        // 7. Comprehensive planning synthesis (Requirements, Architecture, ADRs, Risks, Roadmap, State)
        let planning = GenesisController::run_planning(
            &charter,
            summary.as_ref(),
            &research_findings,
            brownfield_map.as_ref(),
            Some(&env),
            &request.options,
            &self.workspace_root,
        )
        .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))?;

        // 7b. Persist architectural decisions and assumptions to engineering memory (D-02, KRN-25)
        let memory_store = SqliteEngineeringMemoryRepository::new(self.pool.clone());
        for adr in planning.adrs.adrs.values() {
            let decision = crate::memory::EngineeringDecision::new(
                adr.id.as_str(),
                crate::memory::MemoryScope::Project,
                &adr.title,
                &adr.context,
                &adr.decision,
                &adr.rationale,
                "genesis.planning",
            )
            .with_status(crate::memory::DecisionStatus::Accepted)
            .with_alternatives(adr.alternatives_considered.clone())
            .with_consequences(adr.consequences.clone());
            let _ = memory_store.save_decision(&decision).await;
        }
        for unk in &planning.risks.unknowns {
            // Use the linked mission ID for provenance; fall back to a new ID
            // only when genesis is invoked without a parent mission context.
            let assumption_mission_id = request.mission_id.unwrap_or_default();
            let assumption = crate::memory::EngineeringAssumption::new(
                assumption_mission_id,
                &unk.description,
                crate::memory::MemoryScope::Project,
            );
            if let Err(e) = memory_store.save_assumption(&assumption).await {
                tracing::warn!(
                    request_id = %request.request_id,
                    unknown_id = %unk.id,
                    error = %e,
                    "Failed to persist engineering assumption to memory store"
                );
            }
        }

        // 8. Register planning artifacts into ArtifactService
        let planning_recs = planning
            .register_artifacts(&self.artifact_service, Some(charter_id))
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))?;
        registered_artifacts.extend(planning_recs);

        // 9. Lower dependency-ordered Roadmap into executable WorkflowDefinition
        let workflow_definition = planning
            .roadmap
            .lower_to_workflow_definition(&request.options.projection_dir)
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))?;

        Ok(GenesisExecutionOutcome {
            environment: env,
            brownfield_map,
            charter,
            charter_artifact: charter_rec,
            research_decision: decision,
            research_summary: summary,
            planning,
            registered_artifacts,
            workflow_definition,
        })
    }

    /// Start executing a workflow definition via the canonical WorkflowEngine.
    pub async fn start_genesis_workflow(
        &self,
        workflow_def: &WorkflowDefinition,
    ) -> Result<crate::workflow::engine::WorkflowRunHandle, M31AError> {
        let engine = self.create_workflow_engine(self.prompt_catalog_arc());
        let compiled =
            crate::workflow::compiler::CompiledWorkflow::from_definition(workflow_def.clone())
                .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))?;
        let start_req = crate::workflow::engine::WorkflowStartRequest::new(&self.workspace_root)
            .with_parameter("task_spec", "Autonomous roadmap phase implementation");
        engine
            .start_workflow(&compiled, start_req)
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))
    }

    /// Advance an active workflow run by one step.
    pub async fn advance_workflow_run(
        &self,
        run_id: WorkflowRunId,
        workflow_def: &WorkflowDefinition,
    ) -> Result<crate::workflow::state::WorkflowRunState, M31AError> {
        let engine = self.create_workflow_engine(self.prompt_catalog_arc());
        let compiled =
            crate::workflow::compiler::CompiledWorkflow::from_definition(workflow_def.clone())
                .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))?;
        engine
            .advance_workflow(run_id, &compiled)
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))
    }

    /// Approve a pending workflow step that is paused at a human approval gate.
    pub async fn approve_workflow_step(
        &self,
        run_id: WorkflowRunId,
        step_key: &str,
        workflow_def: &WorkflowDefinition,
    ) -> Result<crate::workflow::state::WorkflowRunState, M31AError> {
        let engine = self.create_workflow_engine(self.prompt_catalog_arc());
        let compiled =
            crate::workflow::compiler::CompiledWorkflow::from_definition(workflow_def.clone())
                .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))?;
        engine
            .approve_step(run_id, step_key, &compiled)
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))
    }

    /// Deny a pending workflow step that is paused at a human approval gate.
    pub async fn deny_workflow_step(
        &self,
        run_id: WorkflowRunId,
        step_key: &str,
        reason: &str,
        workflow_def: &WorkflowDefinition,
    ) -> Result<crate::workflow::state::WorkflowRunState, M31AError> {
        let engine = self.create_workflow_engine(self.prompt_catalog_arc());
        let compiled =
            crate::workflow::compiler::CompiledWorkflow::from_definition(workflow_def.clone())
                .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))?;
        engine
            .deny_step(run_id, step_key, reason, &compiled)
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))
    }

    /// Resume a paused or awaiting-approval workflow run.
    pub async fn resume_workflow_run(
        &self,
        run_id: WorkflowRunId,
        workflow_def: &WorkflowDefinition,
    ) -> Result<crate::workflow::state::WorkflowRunState, M31AError> {
        let engine = self.create_workflow_engine(self.prompt_catalog_arc());
        let compiled =
            crate::workflow::compiler::CompiledWorkflow::from_definition(workflow_def.clone())
                .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))?;
        engine
            .resume_workflow(run_id, &compiled)
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))
    }

    /// Retrieve an inspection snapshot of an active or historical workflow run.
    pub async fn get_workflow_snapshot(
        &self,
        run_id: WorkflowRunId,
    ) -> Result<crate::workflow::engine::WorkflowExecutionSnapshot, M31AError> {
        let engine = self.create_workflow_engine(self.prompt_catalog_arc());
        engine
            .inspect_workflow(run_id)
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))
    }

    /// Retrieve the current planning state projection from `.planning/STATE.md` if available.
    pub async fn get_genesis_state(
        &self,
        projection_dir: &str,
    ) -> Result<Option<crate::workflow::planning::PlanningState>, M31AError> {
        let state_file = self.workspace_root.join(projection_dir).join("STATE.md");
        if !state_file.exists() {
            return Ok(None);
        }
        let content = tokio::fs::read_to_string(&state_file)
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e)))?;
        let state = crate::workflow::planning::PlanningState::from_markdown(&content)
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))?;
        Ok(Some(state))
    }

    /// Canonical owner for `WorkflowResumeRequested` (single mutation path).
    ///
    /// Both the CLI runner and the TUI bridge route through here. The runtime
    /// decides: the run is inspected via the canonical `WorkflowEngine`; a
    /// `Blocked` run is resumed with its manifest-recompiled definition; any
    /// other state yields a truthful status report with no mutation.
    /// Manifest lookup is confined to `<workspace>/.planning/workflows/`.
    pub async fn handle_workflow_resume(&self, run_id_str: &str) -> Result<String, M31AError> {
        use std::str::FromStr;
        let run_id = crate::ids::WorkflowRunId::from_str(run_id_str)
            .map_err(|e| M31AError::validation(format!("invalid workflow run id: {e}")))?;
        let snapshot = self.get_workflow_snapshot(run_id).await?;
        let status = snapshot.run.status.to_string();
        if snapshot.run.status != crate::workflow::state::WorkflowRunState::Blocked {
            return Ok(format!(
                "Workflow run {run_id} is '{status}': only Blocked (paused) runs can be resumed. Ready: [{}]. Blocked: [{}].",
                snapshot.ready_step_keys.join(", "),
                snapshot.blocked_step_keys.join(", "),
            ));
        }
        let compiled = self
            .resolve_workflow_compiled(&snapshot.run.definition_id)
            .await?;
        let prompt_catalog: Arc<dyn crate::prompt::PromptCatalog> = self.prompt_catalog_arc();
        let engine = self.create_workflow_engine(prompt_catalog);
        let next = engine
            .resume_workflow(run_id, &compiled)
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))?;
        Ok(format!("Workflow run {run_id} resumed: now '{next}'.",))
    }

    /// Canonical owner for `WorkflowApprovalSubmitted` (single mutation path).
    ///
    /// Approves (or denies with reason) a step genuinely parked in
    /// `AwaitingApproval`; any other step state fails closed with the real
    /// state reported. Definition is recompiled from the run's own manifest —
    /// never synthesized.
    pub async fn handle_workflow_approval(
        &self,
        run_id_str: &str,
        step_key: &str,
        approved: bool,
        reason: Option<String>,
    ) -> Result<String, M31AError> {
        use std::str::FromStr;
        let run_id = crate::ids::WorkflowRunId::from_str(run_id_str)
            .map_err(|e| M31AError::validation(format!("invalid workflow run id: {e}")))?;
        let snapshot = self.get_workflow_snapshot(run_id).await?;
        let step = snapshot
            .step_runs
            .iter()
            .find(|s| s.step_key == step_key)
            .ok_or_else(|| {
                M31AError::validation(format!(
                    "unknown step '{step_key}' in workflow run {run_id}"
                ))
            })?;
        if step.status != crate::workflow::state::WorkflowStepState::AwaitingApproval {
            return Err(M31AError::validation(format!(
                "step '{step_key}' is '{}', not AwaitingApproval: approval refused",
                step.status
            )));
        }
        let compiled = self
            .resolve_workflow_compiled(&snapshot.run.definition_id)
            .await?;
        let prompt_catalog: Arc<dyn crate::prompt::PromptCatalog> = self.prompt_catalog_arc();
        let engine = self.create_workflow_engine(prompt_catalog);
        if approved {
            let next = engine
                .approve_step(run_id, step_key, &compiled)
                .await
                .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))?;
            Ok(format!(
                "Workflow run {run_id} step '{step_key}' approved: run now '{next}'."
            ))
        } else {
            let why = reason.unwrap_or_else(|| "denied by operator".to_string());
            let next = engine
                .deny_step(run_id, step_key, &why, &compiled)
                .await
                .map_err(|e| M31AError::Internal(anyhow::anyhow!(e.to_string())))?;
            Ok(format!(
                "Workflow run {run_id} step '{step_key}' denied ({why}): run now '{next}'."
            ))
        }
    }

    /// Canonical owner for `WorkflowPauseRequested` (single mutation path).
    ///
    /// Pauses a `Running` workflow to `Blocked` via the canonical
    /// `WorkflowEngine`; any other state fails closed with the real state
    /// reported. Durable state lives in the workflow repository, so any
    /// per-call engine instance observes the same run.
    pub async fn pause_workflow_run(
        &self,
        run_id_str: &str,
        reason: &str,
    ) -> Result<String, M31AError> {
        use std::str::FromStr;
        let run_id = crate::ids::WorkflowRunId::from_str(run_id_str)
            .map_err(|e| M31AError::validation(format!("invalid workflow run id: {e}")))?;
        let engine = self.create_workflow_engine(self.prompt_catalog_arc());
        engine
            .pause_workflow(run_id, reason)
            .await
            .map_err(|e| M31AError::validation(format!("workflow pause refused: {e}")))?;
        Ok(format!("Workflow run {run_id} paused: now 'blocked'."))
    }

    /// Canonical owner for `WorkflowCancelRequested` (single mutation path).
    ///
    /// Cancels non-terminal runs and steps via the canonical
    /// `WorkflowEngine`; terminal runs are left untouched (idempotent).
    pub async fn cancel_workflow_run(
        &self,
        run_id_str: &str,
        reason: &str,
    ) -> Result<String, M31AError> {
        use std::str::FromStr;
        let run_id = crate::ids::WorkflowRunId::from_str(run_id_str)
            .map_err(|e| M31AError::validation(format!("invalid workflow run id: {e}")))?;
        let engine = self.create_workflow_engine(self.prompt_catalog_arc());
        engine
            .cancel_workflow(run_id, reason)
            .await
            .map_err(|e| M31AError::validation(format!("workflow cancel refused: {e}")))?;
        Ok(format!("Workflow run {run_id} cancelled."))
    }

    /// Canonical owner for `WorkflowInspectRequested` (read-only inspection path).
    ///
    /// Renders the persisted run snapshot (status, ready/blocked steps,
    /// artifacts). Never mutates workflow, mission, or scheduler state.
    pub async fn inspect_workflow_run(&self, run_id_str: &str) -> Result<String, M31AError> {
        use std::str::FromStr;
        let run_id = crate::ids::WorkflowRunId::from_str(run_id_str)
            .map_err(|e| M31AError::validation(format!("invalid workflow run id: {e}")))?;
        let snapshot = self.get_workflow_snapshot(run_id).await?;
        let mut out = format!(
            "Workflow run {} (definition '{}', v{}): status '{}'.",
            snapshot.run.id,
            snapshot.run.definition_id,
            snapshot.run.definition_version,
            snapshot.run.status
        );
        out.push_str(&format!(
            "\nReady steps: [{}].\nBlocked steps: [{}].",
            snapshot.ready_step_keys.join(", "),
            snapshot.blocked_step_keys.join(", ")
        ));
        if snapshot.step_runs.is_empty() {
            out.push_str("\nNo step runs recorded.");
        } else {
            out.push_str("\nSteps:");
            for step in &snapshot.step_runs {
                out.push_str(&format!("\n  {} — {}", step.step_key, step.status));
            }
        }
        out.push_str(&format!("\nArtifacts: {}.", snapshot.artifacts.len()));
        Ok(out)
    }

    /// Recompile the `CompiledWorkflow` for a run from its stored manifest.
    ///
    /// Looks only in `<workspace>/.planning/workflows/<definition_id>.toml`.
    /// Fails closed when the manifest is absent: the caller must surface the
    /// honest error rather than synthesize a definition.
    async fn resolve_workflow_compiled(
        &self,
        definition_id: &str,
    ) -> Result<crate::workflow::compiler::CompiledWorkflow, M31AError> {
        // Reject path traversal in the definition id itself.
        if definition_id.contains('/')
            || definition_id.contains('\\')
            || definition_id.contains("..")
        {
            return Err(M31AError::validation(format!(
                "invalid workflow definition id: {definition_id:?}"
            )));
        }
        let manifest_path = self
            .workspace_root
            .join(".planning")
            .join("workflows")
            .join(format!("{definition_id}.toml"));
        if !manifest_path.exists() {
            return Err(M31AError::validation(format!(
                "workflow definition manifest not found at {}: resume/approval requires the stored manifest; refusing to synthesize one",
                manifest_path.display()
            )));
        }
        let manifest = crate::workflow::manifest::WorkflowManifest::from_file(
            &manifest_path,
            Some(&self.workspace_root),
        )
        .map_err(|e| M31AError::validation(format!("invalid workflow manifest: {e}")))?;
        // Compile against the runtime-shared catalog (one prompt authority).
        crate::workflow::compiler::WorkflowCompiler::new(self.prompt_catalog.as_ref())
            .compile(&manifest, Some(definition_id), Some(&self.workspace_root))
            .map_err(|e| M31AError::validation(format!("workflow manifest failed to compile: {e}")))
    }
}
