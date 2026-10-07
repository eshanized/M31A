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
use crate::git::worktree::{WorktreeConfig, WorktreeManager};
use crate::ids::{AgentId, MissionId, TaskId, WorkflowRunId};
use crate::memory::repository::{EngineeringMemoryStore, SqliteEngineeringMemoryRepository};
use crate::persistence::artifacts::{ArtifactRecord, ArtifactService, FsArtifactStore};
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

/// Drop-guard owning a slash-command budget reservation.
///
/// Settlement is synchronous, so `Drop` guarantees EVERY exit path of
/// `execute_user_command` (success, denial, error, `?`) releases the worker
/// slot and records the accumulated ESTIMATED token consumption. Slash
/// commands synthesize usage estimates (chars/4); the guard keeps those
/// estimates in the estimated counters — counted against admission, never
/// presented as provider-reported usage.
struct SlashCommandBudgetGuard {
    enforcer: Arc<BudgetEnforcer>,
    receipt: Option<crate::budget::ReservationReceipt>,
    tokens_estimated: u64,
}

impl SlashCommandBudgetGuard {
    fn new(enforcer: Arc<BudgetEnforcer>, receipt: crate::budget::ReservationReceipt) -> Self {
        Self {
            enforcer,
            receipt: Some(receipt),
            tokens_estimated: 0,
        }
    }

    fn add_estimate(&mut self, tokens: u64) {
        self.tokens_estimated = self.tokens_estimated.saturating_add(tokens);
    }
}

impl Drop for SlashCommandBudgetGuard {
    fn drop(&mut self) {
        if let Some(receipt) = self.receipt.take() {
            self.enforcer.settle_estimated(
                &receipt,
                &crate::budget::enforcer::ActualUsage {
                    tokens: self.tokens_estimated,
                    cost_usd: 0.0,
                    artifact_bytes: 0,
                    steps: 1,
                    calls: 1,
                    retries: 0,
                },
            );
        }
    }
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
    auth_authority: Arc<crate::git::AuthorizationAuthority>,
    capability_registry: Arc<crate::capability::registry::CapabilityRegistry>,
    tool_registry: Arc<crate::tools::registry::ToolRegistry>,
    prompt_catalog: Arc<crate::prompt::InMemoryPromptCatalog>,
    slash_registry: Arc<crate::interaction::commands::SlashCommandRegistry>,
    user_command_report: Arc<crate::interaction::user_commands::UserCommandLoadReport>,
    command_snapshot_handle: crate::interaction::user_commands::CommandSnapshotHandle,
    /// Single canonical authority set. Every field above that is also a
    /// member of `RuntimeAuthorities` mirrors the SAME `Arc` instance held
    /// here; `sync_authorities` re-packs the set atomically after any
    /// mutation so derived consumers can never observe a fork.
    authorities: Arc<crate::runtime_authorities::RuntimeAuthorities>,
    active_mission_cancellations:
        Arc<tokio::sync::RwLock<std::collections::HashMap<MissionId, CancellationToken>>>,
    shutdown_token: CancellationToken,
    forwarder_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl AppRuntime {
    /// Construct a complete production runtime for the specified workspace root.
    ///
    /// Canonical storage: the SQLite database, credentials, cache, logs,
    /// and runtime state resolve through [`crate::storage::StorageLayout`]
    /// (platform user dirs). The workspace `.m31a/` retains only
    /// workspace-scoped state (config, identity, prompts, worktrees).
    /// Legacy project-local state is migrated once before opening the DB.
    pub async fn new(workspace_root: impl Into<PathBuf>) -> Result<Self, M31AError> {
        let root = workspace_root.into();
        let channel = crate::deployment::DeploymentChannel::current();
        let layout = crate::storage::StorageLayout::new(&root, channel);
        // Migrate legacy project-local state first (integrity-safe, idempotent).
        let _ = crate::storage::migrate_legacy_workspace_state(&layout);
        // Ensure canonical dirs exist (global + minimal workspace).
        let _ = layout.ensure_global_dirs();
        let _ = layout.ensure_workspace_dir();

        // Canonical database: platform user data (never `<ws>/.m31a/m31a.db`).
        let db_path = layout.global_db_path();
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| M31AError::Internal(anyhow::anyhow!(e)))?;
        }
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
        let channel = crate::deployment::DeploymentChannel::current();
        let layout = crate::storage::StorageLayout::new(&workspace_root, channel);
        // Minimal workspace dir only (identity/config/prompts/worktrees).
        // Global state lives under the platform dirs; never create
        // `m31a.db` / `credentials.json` / global cache/logs here.
        let storage_root = layout.workspace_dir();
        tokio::fs::create_dir_all(&storage_root)
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e)))?;
        let _ = layout.ensure_global_dirs();

        // Canonical global artifact authority (platform user data).
        // Workspace-specific execution staging remains workspace-local
        // in controller paths where project semantics require it.
        let artifacts_dir = layout.global_artifacts_dir();
        tokio::fs::create_dir_all(&artifacts_dir)
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e)))?;
        let artifact_store = Arc::new(FsArtifactStore::new(artifacts_dir));
        let artifact_service = Arc::new(ArtifactService::new(artifact_store.clone(), pool.clone()));

        let redactor = Arc::new(SecretRedactor::new());
        // Canonical global telemetry (platform user data). Project-specific
        // mission records remain queryable in SQLite; the NDJSON stream is
        // application-wide and must not pollute every workspace.
        let telemetry_dir = layout.global_telemetry_dir();
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

        let shutdown_token = CancellationToken::new();
        let forwarder_handle = Arc::new(tokio::sync::Mutex::new(None));

        // Spawn event bus forwarder to telemetry stream (F-12)
        let forwarder_bus = event_bus.clone();
        let forwarder_collector = telemetry_collector.clone();
        let forwarder_shutdown = shutdown_token.clone();
        let handle = tokio::spawn(async move {
            use futures::StreamExt;
            let mut rx = forwarder_bus
                .subscribe(crate::events::bus::EventFilter::all())
                .await;
            loop {
                tokio::select! {
                    _ = forwarder_shutdown.cancelled() => break,
                    evt = rx.next() => {
                        let Some(Ok(envelope)) = evt else { break };
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
                }
            }
        });
        if let Ok(mut lock) = forwarder_handle.try_lock() {
            *lock = Some(handle);
        }

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
        budget.max_model_calls = config.app_config.budget.max_model_calls;
        budget.max_tokens = config.app_config.budget.max_tokens;
        budget.max_wall_clock_seconds = config.app_config.budget.max_wall_clock_seconds;
        budget.max_cost_usd = config.app_config.budget.max_cost_usd;
        budget.max_retries = config.app_config.budget.max_retries;
        budget.max_concurrent_agents = Some(config.app_config.runtime.concurrency_limit);
        let budget_enforcer = Arc::new(BudgetEnforcer::new(budget));

        let scanner =
            StartupCrashRecoveryScanner::new(pool.clone(), artifact_store.clone(), &workspace_root);
        let _ = scanner.scan_all_in_flight().await;

        let approval_coordinator = Arc::new(
            ApprovalCoordinator::new(Some(pool.clone()), None)
                .with_event_bus(event_bus.clone() as Arc<dyn EventBus>),
        );

        let git_service: Arc<dyn crate::capability::traits::git::GitService> =
            if config.app_config.git.enabled {
                Arc::new(crate::capability::providers::CliGitProvider::new(
                    &workspace_root,
                ))
            } else {
                Arc::new(crate::capability::providers::DisabledGitProvider)
            };

        // Canonical global model-catalog cache (platform cache,
        // channel-isolated). Legacy workspace cache is the migration fallback:
        // prefer global, fall back to legacy so pre-migration discovery is
        // not lost, and future saves go to the global store.
        let cache_path = layout.global_model_catalog_file();
        let legacy_cache_path =
            crate::model::catalog::ModelCatalog::cache_path_for_channel(&workspace_root, channel);
        let catalog = crate::model::catalog::ModelCatalog::load_from_cache_file(&cache_path)
            .ok()
            .filter(|c| c.schema_version >= crate::model::catalog::CURRENT_CATALOG_SCHEMA_VERSION)
            .or_else(|| {
                crate::model::catalog::ModelCatalog::load_from_cache_file(&legacy_cache_path)
                    .ok()
                    .filter(|c| {
                        c.schema_version >= crate::model::catalog::CURRENT_CATALOG_SCHEMA_VERSION
                    })
            })
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
        if !config.app_config.git.enabled {
            capability_registry.register_git(git_service.clone());
        }
        // Canonical production job authority: the supervisor writes the
        // durable `jobs` ledger (same rows startup recovery reconciles), so
        // production submissions and restart reconciliation share ONE job
        // lifecycle. The registry's standalone supervisor is replaced.
        // Spool lives in platform runtime state (never per-project).
        // The SAME `Arc` is stored in `RuntimeAuthorities::job_supervisor`
        // so worktree scopes reuse it instead of `JobSupervisor::new(...)`.
        let job_spool_dir = layout.job_spool_dir();
        let pooled_supervisor = Arc::new(
            crate::process::job::JobSupervisor::new(job_spool_dir).with_pool(pool.clone()),
        );
        {
            let job_provider = Arc::new(crate::capability::providers::LocalJobProvider::new(
                workspace_root.clone(),
                pooled_supervisor.clone(),
            ));
            capability_registry.register_jobs(job_provider);
        }
        // Runtime-authoritative sandbox enforcement: the deployment
        // `sandbox_mode` controls real process execution (required isolation
        // fails closed when unavailable; network denied whenever sandboxed).
        // The registry's standalone provider is replaced with the enforced one.
        {
            let enforcement = crate::sandbox::SandboxEnforcement::from_sandbox_mode(
                &config.app_config.runtime.sandbox_mode,
            );
            let enforced_process_provider = Arc::new(
                crate::capability::providers::LocalProcessProvider::new(workspace_root.clone())
                    .with_enforcement(enforcement),
            );
            capability_registry.register_process(enforced_process_provider.clone());
            capability_registry.register_shell(enforced_process_provider);
        }
        let mut tool_reg =
            crate::tools::registry::ToolRegistry::new_default(capability_registry.clone());
        tool_reg.register(crate::tools::definition::CompleteTool);
        tool_reg.register_agentic_tools();
        let tool_registry = Arc::new(tool_reg);

        // Single-read global user commands: load definitions once in memory.
        let user_command_report =
            crate::interaction::user_commands::load_global_user_commands_for_channel(channel);
        let definitions: Vec<Arc<crate::interaction::user_commands::UserCommandDefinition>> =
            user_command_report
                .loaded
                .iter()
                .filter_map(|cmd| {
                    crate::interaction::user_commands::UserCommandDefinition::from_prompt_command(
                        cmd.clone(),
                    )
                    .ok()
                    .map(Arc::new)
                })
                .collect();

        // Canonical shared prompt catalog: workspace overrides and global user commands
        // populated from the single in-memory definitions snapshot.
        let mut prompt_catalog_builder =
            crate::prompt::InMemoryPromptCatalog::with_builtins_and_workspace(&workspace_root);
        let _ = prompt_catalog_builder.apply_user_command_definitions(&definitions);
        let prompt_catalog = Arc::new(prompt_catalog_builder);

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

        // Canonical shared prompt compiler: the ONE compilation
        // authority for all production context generation (worker
        // contexts, interactive stable layers, review/diagnosis, planning).
        // Components that need different rendering behavior select it via
        // typed CompilationOptions — never via a second compiler instance.
        let prompt_compiler: Arc<dyn crate::prompt::PromptCompiler> =
            Arc::new(crate::prompt::DefaultPromptCompiler::new());

        // Canonical shared context compiler: workspace + shared prompt
        // catalog + shared prompt compiler + memory store + role-stage
        // fallback. This ONE instance is
        // the context authority for engines, the pre-execution coordinator,
        // and (via `production_with_shared_authorities`) the controller.
        // The SAME memory `Arc` is stored in `RuntimeAuthorities` so
        // worktree compilers share long-horizon memory semantics.
        let memory_repo: Arc<dyn crate::memory::EngineeringMemoryStore> =
            Arc::new(SqliteEngineeringMemoryRepository::new(pool.clone()));
        let context_compiler: Arc<dyn crate::kernel::seams::ContextCompiler> =
            Arc::new(
                crate::context::compiler::ProductionContextCompiler::new()
                    .with_workspace_root(workspace_root.clone())
                    .with_prompt_catalog(
                        prompt_catalog.clone() as Arc<dyn crate::prompt::PromptCatalog>
                    )
                    .with_prompt_compiler(prompt_compiler.clone())
                    .with_memory_store(memory_repo.clone())
                    .with_role_stage_fallback(Arc::new(|role| {
                        crate::agent::registry::RoleRegistry::global()
                            .read()
                            .ok()
                            .and_then(|guard| guard.stage_for(role))
                    })),
            );

        // Canonical artifact lifecycle: capability artifact operations go
        // through the SAME store+ledger as `ArtifactService` (no competing
        // `FsArtifactStore::new(workspace_artifacts_dir)` authority).
        capability_registry.register_artifacts(Arc::new(
            crate::capability::providers::FsArtifactStoreProvider::from_service(
                artifact_service.clone(),
            ),
        ));

        // Canonical tool pipeline BEFORE controller assembly so the worker
        // dispatcher consumes THE SAME `Arc`s (no second registry/pipeline).
        let tool_pipeline = Arc::new(
            crate::pipeline::runner::ToolPipelineRunner::new(tool_registry.clone())
                .with_artifact_store(artifact_store.clone())
                .with_db_pool(pool.clone())
                .with_approval_coordinator(approval_coordinator.clone()),
        );

        // Canonical fail-closed dispatcher caller: when no provider is bound
        // the runtime still shares ONE misconfigured caller instance instead
        // of letting the dispatcher `RoutedModelCaller::new(...)` per scope.
        let dispatcher_model_caller: Arc<dyn crate::agent::model_policy::ModelCaller> =
            match model_caller.clone() {
                Some(caller) => caller,
                None => {
                    let schemas =
                        crate::runtime_authorities::RuntimeAuthorities::governed_tool_schemas(
                            &capability_registry,
                            &tool_registry,
                            &config.app_config.policy.denied_tools,
                            crate::runtime_authorities::AutonomyPrecedence::from_config(&config),
                        );
                    Arc::new(
                        crate::agent::model_policy::RoutedModelCaller::new(
                            model_provider.clone(),
                            crate::model::router::resolver::ModelTier::Standard,
                            schemas,
                        )
                        .with_model(config.active_model.clone())
                        .with_provider_status(
                            config.active_provider.clone(),
                            crate::model::types::ProviderCapabilityStatus::Misconfigured,
                        ),
                    )
                }
            };

        let mut dependencies = ControllerDependencies::production_with_shared_authorities(
            pool.clone(),
            workspace_root.clone(),
            storage_root.clone(),
            event_bus.clone(),
            model_caller.clone(),
            &config,
            approval_coordinator.clone(),
            policy.clone(),
            artifact_store.clone(),
            budget_enforcer.clone(),
            capability_registry.clone(),
            context_compiler.clone(),
            tool_registry.clone(),
            tool_pipeline.clone(),
            dispatcher_model_caller.clone(),
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
            prompt_compiler.clone(),
            tool_pipeline,
            artifact_store.clone(),
            artifact_service.clone(),
            event_bus.clone(),
            git_service.clone(),
            memory_repo.clone(),
            pooled_supervisor.clone(),
            workspace_root.clone(),
            storage_root.clone(),
            channel,
        ));

        let mut slash_registry_builder =
            crate::interaction::commands::SlashCommandRegistry::new_standard();
        let _ = slash_registry_builder.register_user_command_definitions(&definitions, &[]);
        let slash_registry = Arc::new(slash_registry_builder);
        let user_command_report = Arc::new(user_command_report);

        let command_snapshot = crate::interaction::user_commands::CommandSnapshot {
            generation: 1,
            slash_registry: slash_registry.clone(),
            prompt_catalog: prompt_catalog.clone(),
            report: user_command_report.clone(),
            definitions,
        };
        let command_snapshot_handle =
            crate::interaction::user_commands::CommandSnapshotHandle::new(command_snapshot);
        let active_mission_cancellations =
            Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new()));

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
            auth_authority: capability_registry.authorization_authority().clone(),
            capability_registry,
            tool_registry,
            prompt_catalog,
            slash_registry,
            user_command_report,
            command_snapshot_handle,
            authorities,
            active_mission_cancellations,
            shutdown_token,
            forwarder_handle,
        })
    }

    /// Gracefully shutdown the runtime, cancelling background forwarders and tasks.
    pub async fn shutdown(&self) {
        self.shutdown_token.cancel();
        if let Some(handle) = self.forwarder_handle.lock().await.take() {
            let _ = handle.await;
        }
    }

    /// Access the runtime shutdown cancellation token.
    pub fn shutdown_token(&self) -> &CancellationToken {
        &self.shutdown_token
    }

    /// Check if the runtime is shutting down or cancelled.
    pub fn is_shutting_down(&self) -> bool {
        self.shutdown_token.is_cancelled()
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

    /// Authorize one governed Git mutation through the canonical lifecycle.
    ///
    /// This is the ONLY production path that mints Git mutation authority:
    /// it evaluates the live policy for the exact tool action in the exact
    /// workspace with the exact role/mode identity, then mints a time-bound
    /// authorization bound to the concrete operations, workspace, mission,
    /// task, and policy generation:
    ///
    /// ```text
    /// Allow                       → mint (provenance: policy-allow)
    /// Ask + covering mission grant → mint (provenance: mission authorization)
    /// Ask (no covering grant)     → deny (fail closed, never auto-approved)
    /// Deny / Escalate             → deny (immutable veto)
    /// ```
    ///
    /// Stale policy generations never authorize.
    #[allow(clippy::too_many_arguments)]
    pub async fn authorize_git_operation(
        &self,
        mission_id: MissionId,
        task_id: Option<TaskId>,
        agent_id: Option<AgentId>,
        role: crate::state_machine::agent::AgentRole,
        mode: crate::state_machine::AutonomyMode,
        tool_action: &str,
        workspace_root: PathBuf,
        operation: crate::git::GitOperation,
        additional_operations: Vec<crate::git::GitOperation>,
        resource_scope: &str,
        provenance: &str,
        covering_mission_auth: Option<&crate::planning::review::ExecutionAuthorization>,
    ) -> Result<crate::git::GitGate, M31AError> {
        use crate::kernel::seams::policy::{PolicyEvaluationRequest, PolicyGate};
        // Policy evaluation requires a concrete task binding; mission-scoped
        // setup (no task yet) evaluates under the mission identity with the
        // workspace as its resource scope.
        //
        // The evaluation carries the workspace IDENTITY (typed) but no target
        // paths: runtime orchestration actions (`runtime.*`) are authorized by
        // action identity (role/mode/workspace + policy rules on the action
        // name), while the minted gate binds the EXACT operation and
        // workspace. Injecting the workspace as a target path would subject
        // the runtime's own isolation directories (e.g. `.m31a/worktrees`)
        // to model-facing protected-path vetoes. Model-invoked tools still
        // pass their real target paths through the pipeline.
        let eval_task = task_id.unwrap_or_default();
        let mut req = PolicyEvaluationRequest::new(mission_id, eval_task, tool_action)
            .with_role(role)
            .with_autonomy_mode(mode)
            .with_workspace(workspace_root.clone())
            .with_policy_hash(self.policy.active_policy_hash().to_string());
        if let Some(agent) = agent_id {
            req = req.with_agent_id(agent);
        }
        let decision = self.policy.evaluate(req).await.map_err(|e| {
            M31AError::Internal(anyhow::anyhow!(format!(
                "git authorization policy check failed: {e}"
            )))
        })?;
        use crate::kernel::seams::policy::PolicyDecision as PD;
        let provenance = match decision {
            PD::Allow => provenance.to_string(),
            PD::Ask => {
                // An eligible Ask resolves ONLY against a covering mission
                // execution authorization (verified below): never
                // auto-approved, never resolved by mere existence of a row.
                let covering = covering_mission_auth.ok_or_else(|| {
                    M31AError::Internal(anyhow::anyhow!(format!(
                        "git authorization denied: policy decision for '{tool_action}' is Ask with no covering mission authorization"
                    )))
                })?;
                self.verify_covering_mission_auth(mission_id, covering)
                    .await?;
                format!("mission-execution-authorization:{}", covering.id)
            }
            PD::Deny | PD::Escalate => {
                return Err(M31AError::Internal(anyhow::anyhow!(format!(
                    "git authorization denied: policy decision for '{tool_action}' is {decision:?} (immutable)"
                ))));
            }
        };
        let auth = self.auth_authority.mint_git_authorization(
            mission_id,
            task_id,
            agent_id,
            self.policy.active_policy_hash().to_string(),
            workspace_root,
            operation,
            additional_operations,
            resource_scope,
            provenance,
            crate::git::GIT_AUTH_DEFAULT_TTL,
        );
        crate::git::GitGate::authorized_verified(auth, &self.auth_authority).map_err(|e| {
            M31AError::Internal(anyhow::anyhow!(format!("git authorization rejected: {e}")))
        })
    }

    /// Verify a covering mission execution authorization for Ask-resolution.
    ///
    /// The grant must be currently Authorized, unexpired, bound to this
    /// mission's session, and bound to the live policy generation,
    /// workspace, execution role, and an autonomy mode compatible with the
    /// governed execution. Anything else fails closed.
    async fn verify_covering_mission_auth(
        &self,
        mission_id: MissionId,
        covering: &crate::planning::review::ExecutionAuthorization,
    ) -> Result<(), M31AError> {
        use crate::persistence::sqlite::repositories::lifecycle::{
            ResumeAuthExpectations, SqliteLifecycleRepository,
        };
        if covering.decision != crate::planning::review::AuthorizationDecision::Authorized {
            return Err(M31AError::internal(
                "covering mission authorization is not Authorized".to_string(),
            ));
        }
        if covering.invalidation_reason.is_some() {
            return Err(M31AError::internal(
                "covering mission authorization carries an invalidation".to_string(),
            ));
        }
        let lifecycle_repo = SqliteLifecycleRepository::new(self.pool.clone());
        let session_id = lifecycle_repo
            .session_for_mission(mission_id)
            .await
            .map_err(|e| {
                M31AError::internal(format!("covering authorization session lookup failed: {e}"))
            })?
            .ok_or_else(|| {
                M31AError::internal(
                    "covering mission authorization has no governed session".to_string(),
                )
            })?;
        if session_id != covering.session_id {
            return Err(M31AError::internal(
                "covering mission authorization governs a different session".to_string(),
            ));
        }
        let live_mode = crate::runtime_authorities::AutonomyPrecedence::from_config(&self.config);
        let expectations = ResumeAuthExpectations {
            policy_hash: self.policy.active_policy_hash().to_string(),
            workspace_root: self.workspace_root.display().to_string(),
            agent_role: crate::state_machine::agent::AgentRole::implementer().to_string(),
            autonomy_mode: live_mode.to_string(),
        };
        SqliteLifecycleRepository::verify_execution_surface(covering, &expectations).map_err(|e| {
            M31AError::internal(format!("covering mission authorization is stale: {e}"))
        })
    }

    /// Resolve the latest real task id for a mission from durable state.
    ///
    /// Used for commit provenance: the trailer must reference a task that
    /// actually exists, never a fabricated identifier. Returns `None` when
    /// the mission has no durable tasks.
    pub async fn latest_mission_task_id(&self, mission_id: MissionId) -> Option<TaskId> {
        let repo =
            crate::persistence::sqlite::repositories::SqliteTaskRepository::new(self.pool.clone());
        repo.list_by_mission(mission_id)
            .await
            .ok()
            .and_then(|tasks| tasks.last().map(|t| t.id))
    }

    /// Stage worktree changes for finalization (`add -A` + unstage scratch
    /// dirs) under an already-authorized staging gate. Every failure is
    /// returned — never swallowed.
    async fn finalize_worktree_staging(
        &self,
        wt_git: &crate::capability::providers::CliGitProvider,
        wt_path: &std::path::Path,
        gate: &crate::git::GitGate,
    ) -> Result<(), String> {
        crate::git::worktree::WorktreeManager::ensure_git_excludes(wt_path)
            .await
            .map_err(|e| format!("git exclude hygiene failed: {e}"))?;
        wt_git
            .add_all(gate)
            .await
            .map_err(|e| format!("git add failed: {e}"))?;
        wt_git
            .reset(&[".m31a", "target", "node_modules", "__pycache__"], gate)
            .await
            .map_err(|e| format!("git reset failed: {e}"))?;
        Ok(())
    }

    /// Stage workspace-root changes for finalization under an
    /// already-authorized staging gate.
    async fn finalize_root_staging(&self, gate: &crate::git::GitGate) -> Result<(), String> {
        crate::git::worktree::WorktreeManager::ensure_git_excludes(&self.workspace_root)
            .await
            .map_err(|e| format!("git exclude hygiene failed: {e}"))?;
        self.git_service
            .add_all(gate)
            .await
            .map_err(|e| format!("git add failed: {e}"))?;
        self.git_service
            .reset(&[".m31a", "target", "node_modules", "__pycache__"], gate)
            .await
            .map_err(|e| format!("git reset failed: {e}"))?;
        Ok(())
    }

    /// Commit worktree changes with real provenance and merge into the
    /// workspace root. Returns `Ok(true)` when merged, `Ok(false)` when there
    /// was nothing to merge because the commit itself failed... no — commit
    /// failure is `Err`. Returns `Ok(merge_succeeded)`.
    ///
    /// Provenance references the actual durable task, the executing role and
    /// model, and the authorization that allowed the commit. Git state is
    /// part of completion: any failure is returned for the caller to record
    /// as mission failure.
    #[allow(clippy::too_many_arguments)]
    async fn finalize_worktree_commit(
        &self,
        wt_git: &crate::capability::providers::CliGitProvider,
        wt_path: &std::path::Path,
        stage_gate: &crate::git::GitGate,
        mission_id: MissionId,
        task_id: TaskId,
        role: crate::state_machine::agent::AgentRole,
        mode: crate::state_machine::AutonomyMode,
        commit_msg_text: &str,
        wt_branch: &str,
        mission_auth: Option<&crate::planning::review::ExecutionAuthorization>,
    ) -> Result<bool, String> {
        use crate::git::trailers::CommitTrailers;
        self.finalize_worktree_staging(wt_git, wt_path, stage_gate)
            .await?;
        // Two-phase commit binding: draft trailers without the authorization
        // line, mint the gate over the stripped base message, then commit the
        // final message naming the very authorization being enforced.
        let draft_trailers =
            CommitTrailers::new(mission_id, task_id, role.clone(), &self.config.active_model);
        let draft_msg = CommitTrailers::embed_trailers(commit_msg_text, &draft_trailers)
            .map_err(|e| format!("commit message build failed: {e}"))?;
        let base_msg = crate::git::authorization_commit_base(&draft_msg);
        let commit_gate = self
            .authorize_git_operation(
                mission_id,
                Some(task_id),
                None,
                role.clone(),
                mode,
                "runtime.git_commit_finalize",
                wt_path.to_path_buf(),
                crate::git::GitOperation::Commit {
                    message: base_msg.clone(),
                },
                Vec::new(),
                &format!("commit:{mission_id}"),
                "mission-finalize:policy-allow:git_commit",
                mission_auth,
            )
            .await
            .map_err(|e| format!("git commit denied by policy: {e}"))?;
        let auth_id = commit_gate
            .authorization()
            .map(|a| a.authorization_id.clone())
            .unwrap_or_default();
        let trailers = CommitTrailers::for_governed_execution(
            mission_id,
            task_id,
            None,
            role.clone(),
            &self.config.active_model,
            auth_id,
        );
        let full_msg = CommitTrailers::embed_trailers(commit_msg_text, &trailers)
            .map_err(|e| format!("commit message build failed: {e}"))?;
        wt_git
            .commit(&full_msg, &commit_gate)
            .await
            .map_err(|e| format!("git commit failed: {e}"))?;

        crate::git::worktree::WorktreeManager::ensure_git_excludes(&self.workspace_root)
            .await
            .map_err(|e| format!("git exclude hygiene failed: {e}"))?;
        let merge_gate = self
            .authorize_git_operation(
                mission_id,
                Some(task_id),
                None,
                role.clone(),
                mode,
                "runtime.git_merge_finalize",
                self.workspace_root.clone(),
                crate::git::GitOperation::Merge {
                    source: wt_branch.to_string(),
                },
                vec![crate::git::GitOperation::MergeAbort],
                &format!("merge:{wt_branch}"),
                "mission-finalize:policy-allow:git_merge",
                mission_auth,
            )
            .await
            .map_err(|e| format!("git merge denied by policy: {e}"))?;
        match self.git_service.merge(wt_branch, true, &merge_gate).await {
            Ok(_) => Ok(true),
            Err(err) => {
                // Safely abort the attempted merge if Git entered MERGING
                // state. Abort failure is recorded, never swallowed.
                if let Err(abort_err) = self.git_service.merge_abort(&merge_gate).await {
                    return Err(format!(
                        "git merge failed ({err}) and merge abort failed ({abort_err})"
                    ));
                }
                Err(format!("git merge failed: {err}"))
            }
        }
    }

    /// Commit workspace-root changes with real provenance. Any failure is
    /// returned for the caller to record as mission failure.
    async fn finalize_root_commit(
        &self,
        mission_id: MissionId,
        task_id: TaskId,
        role: crate::state_machine::agent::AgentRole,
        mode: crate::state_machine::AutonomyMode,
        commit_msg_text: &str,
        mission_auth: Option<&crate::planning::review::ExecutionAuthorization>,
    ) -> Result<(), String> {
        use crate::git::trailers::CommitTrailers;
        let draft_trailers =
            CommitTrailers::new(mission_id, task_id, role.clone(), &self.config.active_model);
        let draft_msg = CommitTrailers::embed_trailers(commit_msg_text, &draft_trailers)
            .map_err(|e| format!("commit message build failed: {e}"))?;
        let base_msg = crate::git::authorization_commit_base(&draft_msg);
        let commit_gate = self
            .authorize_git_operation(
                mission_id,
                Some(task_id),
                None,
                role.clone(),
                mode,
                "runtime.git_commit_finalize",
                self.workspace_root.clone(),
                crate::git::GitOperation::Commit {
                    message: base_msg.clone(),
                },
                Vec::new(),
                &format!("commit:{mission_id}"),
                "mission-finalize:policy-allow:git_commit",
                mission_auth,
            )
            .await
            .map_err(|e| format!("git commit denied by policy: {e}"))?;
        let auth_id = commit_gate
            .authorization()
            .map(|a| a.authorization_id.clone())
            .unwrap_or_default();
        let trailers = CommitTrailers::for_governed_execution(
            mission_id,
            task_id,
            None,
            role,
            &self.config.active_model,
            auth_id,
        );
        let full_msg = CommitTrailers::embed_trailers(commit_msg_text, &trailers)
            .map_err(|e| format!("commit message build failed: {e}"))?;
        self.git_service
            .commit(&full_msg, &commit_gate)
            .await
            .map_err(|e| format!("git commit failed: {e}"))?;
        Ok(())
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

    /// Identity-bearing authorization trust root (RUNTIME_SHARED).
    /// `Arc::ptr_eq` with `capability_registry.auth_authority`,
    /// `authorities.auth_authority`, and Git gates must hold.
    pub fn auth_authority(&self) -> &Arc<crate::git::AuthorizationAuthority> {
        &self.auth_authority
    }

    /// Durable pooled job supervisor (RUNTIME_SHARED). Scoped capability
    /// registries reuse this `Arc`; `JobSupervisor::new(...)` in downstream
    /// production composition is a split-brain fork.
    pub fn job_supervisor(&self) -> &Arc<crate::process::job::JobSupervisor> {
        self.authorities.job_supervisor()
    }

    /// Long-horizon engineering memory authority (RUNTIME_SHARED, same
    /// SQLite pool). Canonical and worktree context compilers share this
    /// `Arc`.
    pub fn memory_store(&self) -> &Arc<dyn crate::memory::EngineeringMemoryStore> {
        self.authorities.memory_store()
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
                    crate::storage::StorageLayout::new(
                        &self.workspace_root,
                        self.authorities.channel(),
                    )
                    .workspace_staging_dir(),
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
        let pipeline_runner = self.authorities.tool_pipeline().clone();
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
            self.tool_registry.clone(),
            pipeline_runner,
            self.policy.clone(),
            self.approval_coordinator.clone(),
            completion_gate,
            self.authorities.context_compiler().clone(),
            Some(self.event_bus.clone() as Arc<dyn crate::events::bus::EventBus>),
            self.capability_registry.clone(),
        )
        .with_prompt_catalog(self.prompt_catalog_arc())
        .with_prompt_compiler(self.authorities.prompt_compiler().clone())
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

    /// Canonical slash command registry snapshot for this runtime.
    pub fn slash_registry(&self) -> &Arc<crate::interaction::commands::SlashCommandRegistry> {
        &self.slash_registry
    }

    /// Access the atomic thread-safe handle to the runtime command snapshot.
    pub fn command_snapshot_handle(
        &self,
    ) -> crate::interaction::user_commands::CommandSnapshotHandle {
        self.command_snapshot_handle.clone()
    }

    /// User command load report containing loaded commands and any rejected diagnostics.
    pub fn user_command_report(
        &self,
    ) -> &Arc<crate::interaction::user_commands::UserCommandLoadReport> {
        &self.user_command_report
    }

    /// Canonical tool execution pipeline runner (Phase B single authority).
    pub fn tool_pipeline(&self) -> &Arc<crate::pipeline::runner::ToolPipelineRunner> {
        self.authorities.tool_pipeline()
    }

    /// Create the canonical slash command registry populated with built-in commands
    /// and all global user-defined commands loaded for this runtime's deployment channel.
    #[deprecated(
        note = "Prefer runtime.slash_registry() to share the canonical snapshot instead of creating clones"
    )]
    pub fn create_slash_registry(&self) -> crate::interaction::commands::SlashCommandRegistry {
        (*self.slash_registry).clone()
    }

    /// Test fixture helper: create an isolated slash command registry populated
    /// from an explicit directory. Not used in canonical production routing.
    pub fn create_slash_registry_from_dir(
        &self,
        dir: &Path,
    ) -> crate::interaction::commands::SlashCommandRegistry {
        let mut reg = crate::interaction::commands::SlashCommandRegistry::new_standard();
        let report = crate::interaction::user_commands::load_global_user_commands_from_dir(dir);
        let _ = reg.register_user_commands(report.loaded, &[]);
        reg
    }

    /// Atomically apply a reloaded user command set to the runtime (Phases 1, 2, 3).
    ///
    /// Single-read guarantee: both PromptCatalog and SlashCommandRegistry are
    /// updated from the exact same in-memory definitions produced by `report`.
    /// Validation is all-or-nothing: if synthesis or registration fails, the
    /// previous valid snapshot and authorities are completely preserved.
    pub fn apply_command_reload(
        &mut self,
        report: crate::interaction::user_commands::UserCommandLoadReport,
    ) -> Result<usize, crate::prompt::PromptError> {
        let definitions: Vec<Arc<crate::interaction::user_commands::UserCommandDefinition>> =
            report
                .loaded
                .iter()
                .filter_map(|cmd| {
                    crate::interaction::user_commands::UserCommandDefinition::from_prompt_command(
                        cmd.clone(),
                    )
                    .ok()
                    .map(Arc::new)
                })
                .collect();

        // 1. Prepare candidate catalog snapshot
        let mut candidate_catalog = (*self.prompt_catalog).clone();
        let count = candidate_catalog.apply_user_command_definitions(&definitions)?;

        // 2. Prepare candidate registry snapshot
        let mut candidate_registry =
            crate::interaction::commands::SlashCommandRegistry::new_standard();
        let _rejections = candidate_registry.register_user_command_definitions(&definitions, &[]);

        // 3. Atomically publish the new snapshot
        let new_generation = self.command_snapshot_handle.generation() + 1;
        let candidate_catalog = Arc::new(candidate_catalog);
        let candidate_registry = Arc::new(candidate_registry);
        let report_arc = Arc::new(report);

        let snapshot = crate::interaction::user_commands::CommandSnapshot {
            generation: new_generation,
            slash_registry: candidate_registry.clone(),
            prompt_catalog: candidate_catalog.clone(),
            report: report_arc.clone(),
            definitions,
        };

        self.command_snapshot_handle.publish(snapshot);
        self.prompt_catalog = candidate_catalog;
        self.slash_registry = candidate_registry;
        self.user_command_report = report_arc;
        self.sync_authorities();
        self.rebuild_dependencies();

        Ok(count)
    }

    /// Reload global user commands from the global command directory for this runtime's channel.
    ///
    /// Atomically re-packs authorities and reassembles dependent coordinators so all
    /// components immediately observe the updated prompt contracts without restarting.
    /// If reload fails, retains the previous valid catalog and registry.
    pub fn reload_user_commands(&mut self) -> Result<usize, crate::prompt::PromptError> {
        let channel = crate::deployment::DeploymentChannel::current();
        let report =
            crate::interaction::user_commands::load_global_user_commands_for_channel(channel);
        self.apply_command_reload(report)
    }

    /// Reload global user commands from an explicit directory.
    ///
    /// Atomically re-packs authorities and reassembles dependent coordinators so all
    /// components immediately observe the updated prompt contracts without restarting.
    /// If reload fails, retains the previous valid catalog and registry.
    pub fn reload_user_commands_from_dir(
        &mut self,
        dir: &Path,
    ) -> Result<usize, crate::prompt::PromptError> {
        let report = crate::interaction::user_commands::load_global_user_commands_from_dir(dir);
        self.apply_command_reload(report)
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
    ///
    /// Single-authority persistence: saves to the canonical global platform
    /// cache (`StorageLayout::global_model_catalog_file`), never to the
    /// legacy workspace-local `.m31a/cache/model_catalog*.json` (read-only
    /// migration source). CLI/TUI/wizard/doctor read the same global
    /// authority (global-first, legacy fallback for migration).
    pub async fn refresh_model_catalog(
        &self,
    ) -> Result<crate::model::catalog::ModelCatalog, crate::model::types::ModelError> {
        if let Some(ref provider) = self.model_provider {
            match provider.discover_models().await {
                Ok(models) => {
                    let mut cat = self.model_catalog.write().await;
                    cat.update_from_provider(&self.config.active_provider, models);
                    let layout = crate::storage::StorageLayout::new(
                        &self.workspace_root,
                        self.authorities.channel(),
                    );
                    let cache_path = layout.global_model_catalog_file();
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
        .with_workspace_root(self.workspace_root.clone())
        // Full execution-surface binding for minted authorizations: the live
        // policy generation, the fixed governed execution role, and the
        // effective autonomy mode resolved from live configuration. A profile
        // switch between authorization and execution invalidates the grant.
        .with_policy_hash(self.policy.active_policy_hash().to_string())
        .with_execution_role(crate::state_machine::agent::AgentRole::implementer().to_string())
        .with_execution_mode(
            crate::runtime_authorities::AutonomyPrecedence::from_config(&self.config).to_string(),
        );
        if let Some(ref caller) = self.model_caller {
            coord = coord.with_model_caller(caller.clone());
        }
        // Reuse the runtime-shared prompt catalog AND prompt compiler
        // (one prompt authority: compilation and resolution never diverge).
        coord = coord.with_prompt_catalog(self.prompt_catalog_arc());
        coord = coord.with_prompt_compiler(self.authorities.prompt_compiler().clone());
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
            self.authorities.prompt_compiler().clone(),
            self.authorities.tool_pipeline().clone(),
            self.artifact_store.clone(),
            self.artifact_service.clone(),
            self.event_bus.clone(),
            self.git_service.clone(),
            self.authorities.memory_store().clone(),
            self.authorities.job_supervisor().clone(),
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
        budget.max_model_calls = config.app_config.budget.max_model_calls;
        budget.max_tokens = config.app_config.budget.max_tokens;
        budget.max_wall_clock_seconds = config.app_config.budget.max_wall_clock_seconds;
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
        let dispatcher_caller = self
            .authorities
            .canonical_model_caller_for_dispatch(self.model_caller.clone());
        self.dependencies = ControllerDependencies::production_with_shared_authorities(
            self.pool.clone(),
            self.workspace_root.clone(),
            self.storage_root.clone(),
            self.event_bus.clone(),
            self.model_caller.clone(),
            &self.config,
            self.approval_coordinator.clone(),
            self.policy.clone(),
            self.artifact_store.clone(),
            self.budget_enforcer.clone(),
            self.capability_registry.clone(),
            self.authorities.context_compiler().clone(),
            self.tool_registry.clone(),
            self.authorities.tool_pipeline().clone(),
            dispatcher_caller,
        );
        self.dependencies = self
            .dependencies
            .clone()
            .with_git_service(self.git_service.clone());
    }

    /// Build the worktree-scoped execution triple sharing canonical trust roots.
    ///
    /// Scope variation (NOT a new runtime): `wt_path`-bound capability
    /// registry, tool inventory, pipeline, and context compiler sharing the
    /// canonical authorization trust root, durable job supervisor, sandbox
    /// enforcement, artifact service/store, event bus, prompt authorities,
    /// and engineering memory. Only the filesystem/workspace binding differs.
    ///
    /// Public for authority-identity tests: proves worktree missions inherit
    /// the canonical trust roots instead of forking them.
    pub fn scoped_worktree_execution(
        &self,
        wt_path: &std::path::Path,
    ) -> (
        Arc<crate::capability::registry::CapabilityRegistry>,
        Arc<crate::tools::registry::ToolRegistry>,
        Arc<crate::pipeline::runner::ToolPipelineRunner>,
        Arc<dyn crate::kernel::seams::ContextCompiler>,
    ) {
        // Same trust root: authorizations minted by the canonical runtime
        // verify inside worktree tool execution.
        let scoped_caps = Arc::new(
            crate::capability::registry::CapabilityRegistry::production_with_auth(
                wt_path,
                Some(self.event_bus.clone()),
                self.model_caller.clone(),
                self.auth_authority.clone(),
            ),
        );
        if !self.config.app_config.git.enabled {
            scoped_caps.register_git(self.git_service.clone());
        }
        // Same durable job authority: shared pooled supervisor (same SQLite
        // pool + spool dir), never `JobSupervisor::new(...)`.
        scoped_caps.register_jobs(Arc::new(
            crate::capability::providers::LocalJobProvider::new(
                wt_path.to_path_buf(),
                self.authorities.job_supervisor().clone(),
            ),
        ));
        // Same sandbox enforcement value derived from the SAME config.
        let enforcement = crate::sandbox::SandboxEnforcement::from_sandbox_mode(
            &self.config.app_config.runtime.sandbox_mode,
        );
        let enforced = Arc::new(
            crate::capability::providers::LocalProcessProvider::new(wt_path.to_path_buf())
                .with_enforcement(enforcement),
        );
        scoped_caps.register_process(enforced.clone());
        scoped_caps.register_shell(enforced);
        // Same artifact lifecycle: capability writes go through the canonical
        // service/store + ledger, never a competing workspace store.
        scoped_caps.register_artifacts(Arc::new(
            crate::capability::providers::FsArtifactStoreProvider::from_service(
                self.artifact_service.clone(),
            ),
        ));
        let mut tool_reg = crate::tools::registry::ToolRegistry::new_default(scoped_caps.clone());
        tool_reg.register(crate::tools::definition::CompleteTool);
        tool_reg.register_agentic_tools();
        let scoped_tools = Arc::new(tool_reg);
        let scoped_pipeline = Arc::new(
            crate::pipeline::runner::ToolPipelineRunner::new(scoped_tools.clone())
                .with_artifact_store(self.artifact_store.clone())
                .with_db_pool(self.pool.clone())
                .with_approval_coordinator(self.approval_coordinator.clone()),
        );
        // Same prompt authorities + same engineering memory; only the
        // workspace root differs.
        let worktree_compiler: Arc<dyn crate::kernel::seams::ContextCompiler> = Arc::new(
            crate::context::compiler::ProductionContextCompiler::new()
                .with_workspace_root(wt_path.to_path_buf())
                .with_prompt_catalog(self.prompt_catalog_arc())
                .with_prompt_compiler(self.authorities.prompt_compiler().clone())
                .with_memory_store(self.authorities.memory_store().clone())
                .with_role_stage_fallback(Arc::new(|role| {
                    crate::agent::registry::RoleRegistry::global()
                        .read()
                        .ok()
                        .and_then(|guard| guard.stage_for(role))
                })),
        );
        (
            scoped_caps,
            scoped_tools,
            scoped_pipeline,
            worktree_compiler,
        )
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
        // single canonical autonomy authority: explicit mission profile arg
        // wins, otherwise the resolved configuration's effective autonomy
        // (profile metadata + CLI --autonomy). never defaults to Safe when
        // a non-Safe profile is configured.
        let mode = match profile_name {
            Some(p) => crate::runtime_authorities::AutonomyPrecedence::from_profile_name(Some(p)),
            None => crate::runtime_authorities::AutonomyPrecedence::from_config(&self.config),
        };

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
        let isolation_required = self.config.app_config.git.execution_isolation.is_required();
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
                    workspace_mode: crate::workflow::genesis::WorkspaceMode::Unknown,
                    detected_stack: None,
                    file_count: 0,
                    has_git: false,
                });
            let tier =
                crate::workflow::genesis::discovery::classify_workflow_tier(&full_prompt, &env);

            if (tier == crate::workflow::genesis::WorkflowTier::Greenfield
                || tier == crate::workflow::genesis::WorkflowTier::Standard
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

        self.execute_mission_loop(
            mission_id,
            prompt,
            full_prompt,
            mode,
            upstream_context,
            None,
        )
        .await
    }

    /// Execute an authorized mission whose TaskGraph has already been materialized.
    ///
    /// FINAL RUNTIME BOUNDARY (the name is not the authorization): the caller
    /// MUST present the typed [`ExecutionAuthorization`](crate::planning::review::ExecutionAuthorization)
    /// granted for this mission, and this method INDEPENDENTLY verifies it
    /// against live state immediately before side effects begin:
    ///
    /// ```text
    /// session binding → lifecycle stage → plan/task revisions →
    /// content hashes → policy generation → workspace → role → mode → expiry
    /// ```
    ///
    /// A stale, mismatched, or expired authorization fails closed — execution
    /// never starts. The execution autonomy mode comes from the VERIFIED
    /// authorization, never from caller choice. There is no headless lane
    /// through this entry: missions without a governed session cannot present
    /// an authorization and are refused here (use `run_mission` for
    /// non-governed lanes, which carry no authorization claim).
    pub async fn run_authorized_mission(
        &self,
        mission_id: MissionId,
        objective: &str,
        auth: crate::planning::review::ExecutionAuthorization,
    ) -> Result<MissionExecutionSummary, M31AError> {
        use crate::persistence::sqlite::repositories::lifecycle::{
            ResumeAuthError, ResumeAuthExpectations,
        };
        let lifecycle_repo =
            crate::persistence::sqlite::repositories::SqliteLifecycleRepository::new(
                self.pool.clone(),
            );
        // 0. Session binding: the authorization must govern THIS mission's
        // session. A cross-session replay fails closed here.
        let session_id = lifecycle_repo
            .session_for_mission(mission_id)
            .await
            .map_err(|e| {
                M31AError::internal(format!("execution authorization lookup failed: {e}"))
            })?
            .ok_or_else(|| {
                M31AError::internal(
                    "execution refused: mission has no governed session and presents no verifiable authorization lane".to_string(),
                )
            })?;
        if session_id != auth.session_id {
            return Err(M31AError::internal(
                "execution refused: authorization governs a different session".to_string(),
            ));
        }
        // 1-7. Independent revalidation against live state (revisions,
        // hashes, decision, invalidation) plus the live execution surface
        // (policy generation, workspace, role, mode, expiry).
        let live_mode = crate::runtime_authorities::AutonomyPrecedence::from_config(&self.config);
        let expectations = ResumeAuthExpectations {
            policy_hash: self.policy.active_policy_hash().to_string(),
            workspace_root: self.workspace_root.display().to_string(),
            agent_role: crate::state_machine::agent::AgentRole::implementer().to_string(),
            autonomy_mode: live_mode.to_string(),
        };
        lifecycle_repo
            .revalidate_authorization_for_resume_with(mission_id, Some(&expectations))
            .await
            .map_err(|e| match e {
                ResumeAuthError::Stale(reason) | ResumeAuthError::Unauthorized(reason) => {
                    M31AError::internal(format!("execution refused: stale authorization: {reason}"))
                }
                other => M31AError::internal(format!(
                    "execution authorization verification failed: {other}"
                )),
            })?;
        // The presented artifact must be the CURRENT durable authorization,
        // not a superseded one replayed from history.
        let current = lifecycle_repo
            .load_latest_execution_authorization(&session_id)
            .await
            .map_err(|e| {
                M31AError::internal(format!("execution authorization lookup failed: {e}"))
            })?
            .ok_or_else(|| {
                M31AError::internal("execution refused: no durable authorization".to_string())
            })?;
        if current.id != auth.id {
            return Err(M31AError::internal(
                "execution refused: presented authorization is not the current durable grant"
                    .to_string(),
            ));
        }
        // Execution latitude comes from the verified authorization.
        let mode: crate::state_machine::AutonomyMode = auth
            .autonomy_mode
            .as_deref()
            .unwrap_or("")
            .parse()
            .map_err(|_| {
                M31AError::internal(
                    "execution refused: authorization carries no parseable autonomy mode"
                        .to_string(),
                )
            })?;
        self.execute_mission_loop(
            mission_id,
            objective,
            objective.to_string(),
            mode,
            None,
            Some(auth),
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
        mission_auth: Option<crate::planning::review::ExecutionAuthorization>,
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
        let git_enabled = self.config.app_config.git.enabled;
        let isolation_required =
            git_enabled && self.config.app_config.git.execution_isolation.is_required();
        let worktree_opt = if git_enabled && self.workspace_root.join(".git").exists() {
            // Worktree creation is a governed Git mutation: authorize it
            // through the canonical policy lifecycle before any mutation.
            // Mission-scoped setup has no task yet (task_id None, truthful);
            // policy evaluates under the mission identity with the workspace
            // as resource scope and must Allow, else setup fails closed.
            let wt_path = self.worktree_manager.worktree_path(&mission_id);
            let wt_branch = crate::git::worktree::WorktreeManager::branch_name(&mission_id);
            let setup_gate = self
                .authorize_git_operation(
                    mission_id,
                    None,
                    None,
                    crate::state_machine::agent::AgentRole::implementer(),
                    mode,
                    "runtime.git_worktree_create",
                    self.workspace_root.clone(),
                    crate::git::GitOperation::WorktreeAdd {
                        path: wt_path.display().to_string(),
                        branch: wt_branch,
                    },
                    Vec::new(),
                    &format!("worktree:{}", wt_path.display()),
                    "mission-setup:policy-allow:git_worktree_create",
                    mission_auth.as_ref(),
                )
                .await;
            let setup_gate: Option<crate::git::GitGate> = match setup_gate {
                Ok(g) => Some(g),
                Err(e) => {
                    if isolation_required {
                        return Err(M31AError::Internal(anyhow::anyhow!(
                            "Execution blocked: git worktree setup denied by policy for mission {}: {}.",
                            mission_id,
                            e
                        )));
                    }
                    tracing::warn!(
                        mission_id = %mission_id,
                        error = %e,
                        "Git worktree setup denied by policy; continuing in primary workspace (best_effort)"
                    );
                    None
                }
            };
            match setup_gate {
                None => {
                    // Policy denied setup and isolation is best_effort:
                    // continue in the primary workspace without a worktree.
                    tracing::warn!(
                        mission = %mission_id,
                        isolation_policy = "best_effort",
                        "ISOLATION DOWNGRADE: Git worktree setup denied by policy; \
                         mission will proceed in primary workspace. \
                         Set git.execution_isolation = \"required\" to block this."
                    );
                    None
                }
                Some(ref gate) => match self
                    .worktree_manager
                    .create_worktree(&mission_id, None, gate)
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
                },
            }
        } else if isolation_required {
            return Err(M31AError::Internal(anyhow::anyhow!(
                "Execution blocked: git worktree isolation required by policy, but workspace \
                 '{}' is not a git repository (.git missing). Initialize a git repository \
                 or set git.execution_isolation = \"best_effort\" to allow unisolated execution.",
                self.workspace_root.display()
            )));
        } else {
            if git_enabled {
                tracing::warn!(
                    mission = %mission_id,
                    isolation_policy = "best_effort",
                    "ISOLATION DOWNGRADE: Workspace is not a git repository; \
                     mission will proceed in primary workspace. \
                     Set git.execution_isolation = \"required\" to block this."
                );
            } else {
                tracing::info!(
                    mission = %mission_id,
                    "Git integration disabled by user; mission proceeding directly in workspace."
                );
            }
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
        // Worktree scope rule (NOT a fork): the worktree is a scope variation
        // of the canonical runtime. Only genuinely workspace-bound state is
        // re-derived for `wt.path` (capability root, tool inventory, context
        // workspace binding, repo graph). Everything identity-bearing is
        // inherited by `Arc` clone: policy, budget, approval, artifact
        // store/service, event bus, model caller/provider, prompt
        // catalog/compiler, memory, authorization trust root, durable job
        // supervisor, and sandbox enforcement. See
        // `scoped_worktree_execution`.
        let active_deps = if let Some(ref wt) = worktree_opt {
            let (scoped_caps, scoped_tools, scoped_pipeline, worktree_compiler) =
                self.scoped_worktree_execution(&wt.path);
            let dispatcher_caller = self
                .authorities
                .canonical_model_caller_for_dispatch(self.model_caller.clone());
            ControllerDependencies::production_scoped_for_workspace(
                self.pool.clone(),
                wt.path.clone(),
                self.storage_root.clone(),
                self.event_bus.clone(),
                self.model_caller.clone(),
                &self.config,
                self.approval_coordinator.clone(),
                self.policy.clone(),
                self.artifact_store.clone(),
                self.budget_enforcer.clone(),
                scoped_caps,
                worktree_compiler,
                scoped_tools,
                scoped_pipeline,
                dispatcher_caller,
            )
            .with_git_service(self.git_service.clone())
        } else {
            self.dependencies.clone()
        };

        let cancel_token = CancellationToken::new();
        self.active_mission_cancellations
            .write()
            .await
            .insert(mission_id, cancel_token.clone());
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
        )
        .with_policy_role(crate::state_machine::agent::AgentRole::implementer());

        if let Some(ctx) = upstream_context {
            controller = controller.with_upstream_context(ctx);
        }

        // 5. Run controller loop to terminal state (F-01)
        let run_res = controller.run().await;
        let halt_reason = match run_res {
            Ok(reason) => reason,
            Err(e) => {
                // Error-path worktree cleanup is governed, never a bare
                // boolean: authorize removal under the mission identity. If
                // authorization itself fails, the failure is recorded in the
                // terminal evidence (the controller already failed; cleanup
                // denial must not mask the original error).
                if let Some(ref wt) = worktree_opt
                    && self.worktree_manager.config().retention_policy
                        == crate::git::worktree::WorktreeRetentionPolicy::AlwaysRemove
                {
                    let task_id = self.latest_mission_task_id(mission_id).await;
                    match self
                        .authorize_git_operation(
                            mission_id,
                            task_id,
                            None,
                            crate::state_machine::agent::AgentRole::implementer(),
                            mode,
                            "runtime.git_worktree_remove",
                            self.workspace_root.clone(),
                            crate::git::GitOperation::WorktreeRemove { force: true },
                            vec![crate::git::GitOperation::BranchDelete {
                                branch: wt.branch.clone(),
                            }],
                            &format!("worktree:{}", wt.path.display()),
                            "mission-error-cleanup:policy-allow:git_worktree_remove",
                            mission_auth.as_ref(),
                        )
                        .await
                    {
                        Err(auth_err) => {
                            tracing::error!(
                                mission_id = %mission_id,
                                "error-path worktree cleanup denied by policy: {auth_err}; worktree left at {}",
                                wt.path.display()
                            );
                        }
                        Ok(gate) => {
                            if let Err(cleanup_err) =
                                self.worktree_manager.remove_worktree(wt, true, &gate).await
                            {
                                tracing::error!(
                                    mission_id = %mission_id,
                                    "error-path worktree cleanup failed: {cleanup_err}; worktree left at {}",
                                    wt.path.display()
                                );
                            }
                        }
                    }
                }
                self.active_mission_cancellations
                    .write()
                    .await
                    .remove(&mission_id);

                // Fail-closed terminalization: persist Failed state and emit MissionFailed
                let mission_repo =
                    crate::persistence::sqlite::repositories::mission::SqliteMissionRepository::new(
                        self.pool.clone(),
                    );
                let _ = crate::persistence::sqlite::repositories::MissionRepository::update_status(
                    &mission_repo,
                    mission_id,
                    crate::state_machine::MissionState::Failed,
                )
                .await;

                let terminal_env = EventEnvelope::new(
                    0,
                    Some(mission_id),
                    None,
                    "runtime".to_string(),
                    EventType::MissionFailed {
                        mission_id,
                        reason: format!("Controller execution aborted: {e}"),
                    },
                );
                let _ = self.event_bus.publish(terminal_env).await;

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

        // Git finalization evidence: every required Git mutation below either
        // succeeds (recorded) or flips completion to Failed with truthful
        // durable state. No `let _ =` swallowing, no fabricated task ids.
        let mut git_finalize_error: Option<String> = None;
        if is_success
            && self.config.app_config.git.enabled
            && self.config.app_config.git.auto_commit
        {
            if let Some(ref wt) = worktree_opt {
                // Real provenance: the latest durable task of this mission.
                // A mission with no durable tasks has no executed work to
                // attribute; finalization fails closed instead of fabricating
                // a task identifier.
                let task_id: Option<TaskId> = match self.latest_mission_task_id(mission_id).await {
                    Some(t) => Some(t),
                    None => {
                        let msg = "git finalization refused: mission has no durable tasks to attribute the commit to".to_string();
                        tracing::error!(mission_id = %mission_id, "{msg}");
                        git_finalize_error = Some(msg);
                        is_success = false;
                        final_status = "Failed";
                        None
                    }
                };
                if let Some(task_id) = task_id {
                    let role = crate::state_machine::agent::AgentRole::implementer();
                    let wt_path = wt.path.clone();
                    let wt_branch = wt.branch.clone();
                    // Authorize the exact worktree finalization mutations.
                    let commit_msg_text = format!(
                        "fix: {}\n\nAutonomous mission completed successfully.",
                        prompt
                    );
                    // The commit message is finalized after the gate is
                    // minted (trailers embed the authorization id), so mint
                    // the gate in two steps: first authorize add/reset under
                    // a staging gate, then mint the commit gate over the exact
                    // final message. Both gates derive from one policy Allow.
                    let stage_gate = self
                        .authorize_git_operation(
                            mission_id,
                            Some(task_id),
                            None,
                            role.clone(),
                            mode,
                            "runtime.git_finalize_stage",
                            wt_path.clone(),
                            crate::git::GitOperation::Add {
                                paths: vec!["-A".to_string()],
                            },
                            vec![crate::git::GitOperation::Unstage {
                                paths: vec![
                                    ".m31a".to_string(),
                                    "target".to_string(),
                                    "node_modules".to_string(),
                                    "__pycache__".to_string(),
                                ],
                            }],
                            &format!("finalize-stage:{mission_id}"),
                            "mission-finalize:policy-allow:git_finalize_stage",
                            mission_auth.as_ref(),
                        )
                        .await;
                    match stage_gate {
                        Err(e) => {
                            let msg = format!("git finalization staging denied by policy: {e}");
                            tracing::error!(mission_id = %mission_id, "{msg}");
                            git_finalize_error = Some(msg);
                            is_success = false;
                            final_status = "Failed";
                        }
                        Ok(stage_gate) => {
                            let wt_git =
                                crate::capability::providers::CliGitProvider::new(&wt_path);
                            let finalize_res = self
                                .finalize_worktree_commit(
                                    &wt_git,
                                    &wt_path,
                                    &stage_gate,
                                    mission_id,
                                    task_id,
                                    role.clone(),
                                    mode,
                                    &commit_msg_text,
                                    &wt_branch,
                                    mission_auth.as_ref(),
                                )
                                .await;
                            match finalize_res {
                                Err(e) => {
                                    tracing::error!(
                                        mission_id = %mission_id,
                                        error = %e,
                                        "Git finalization failed; mission marked Failed"
                                    );
                                    git_finalize_error = Some(e);
                                    is_success = false;
                                    final_status = "Failed";
                                }
                                Ok(merge_succeeded) => {
                                    // Handle ephemeral worktree cleanup according to retention policy.
                                    // Cleanup failures are recorded; a required
                                    // removal that fails flips completion.
                                    let retention = self.worktree_manager.config().retention_policy;
                                    let need_remove = merge_succeeded
                                        && retention
                                            != crate::git::worktree::WorktreeRetentionPolicy::AlwaysKeep
                                        || retention
                                            == crate::git::worktree::WorktreeRetentionPolicy::AlwaysRemove;
                                    let force = !merge_succeeded;
                                    if need_remove {
                                        match self
                                            .authorize_git_operation(
                                                mission_id,
                                                Some(task_id),
                                                None,
                                                role.clone(),
                                                mode,
                                                "runtime.git_worktree_remove",
                                                self.workspace_root.clone(),
                                                crate::git::GitOperation::WorktreeRemove { force },
                                                if force {
                                                    vec![crate::git::GitOperation::BranchDelete {
                                                        branch: wt_branch.clone(),
                                                    }]
                                                } else {
                                                    Vec::new()
                                                },
                                                &format!("worktree:{}", wt_path.display()),
                                                "mission-finalize:policy-allow:git_worktree_remove",
                                                mission_auth.as_ref(),
                                            )
                                            .await
                                        {
                                            Err(e) => {
                                                let msg = format!(
                                                    "worktree cleanup denied by policy: {e}"
                                                );
                                                tracing::error!(
                                                    mission_id = %mission_id,
                                                    "{msg}"
                                                );
                                                git_finalize_error = Some(msg.clone());
                                                if retention
                                                    == crate::git::worktree::WorktreeRetentionPolicy::AlwaysRemove
                                                {
                                                    is_success = false;
                                                    final_status = "Failed";
                                                }
                                            }
                                            Ok(gate) => {
                                                if let Err(e) = self
                                                    .worktree_manager
                                                    .remove_worktree(wt, force, &gate)
                                                    .await
                                                {
                                                    let msg =
                                                        format!("worktree cleanup failed: {e}");
                                                    tracing::error!(
                                                        mission_id = %mission_id,
                                                        "{msg}"
                                                    );
                                                    git_finalize_error = Some(msg);
                                                    if retention
                                                        == crate::git::worktree::WorktreeRetentionPolicy::AlwaysRemove
                                                    {
                                                        is_success = false;
                                                        final_status = "Failed";
                                                    }
                                                } else {
                                                    worktree_cleaned = true;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else if self.workspace_root.join(".git").exists() {
                let task_id: Option<TaskId> = match self.latest_mission_task_id(mission_id).await {
                    Some(t) => Some(t),
                    None => {
                        let msg = "git finalization refused: mission has no durable tasks to attribute the commit to".to_string();
                        tracing::error!(mission_id = %mission_id, "{msg}");
                        git_finalize_error = Some(msg);
                        is_success = false;
                        final_status = "Failed";
                        None
                    }
                };
                if let Some(task_id) = task_id {
                    let role = crate::state_machine::agent::AgentRole::implementer();
                    let commit_msg_text = format!(
                        "fix: {}\n\nAutonomous mission completed successfully.",
                        prompt
                    );
                    let stage_gate = self
                        .authorize_git_operation(
                            mission_id,
                            Some(task_id),
                            None,
                            role.clone(),
                            mode,
                            "runtime.git_finalize_stage",
                            self.workspace_root.clone(),
                            crate::git::GitOperation::Add {
                                paths: vec!["-A".to_string()],
                            },
                            vec![crate::git::GitOperation::Unstage {
                                paths: vec![
                                    ".m31a".to_string(),
                                    "target".to_string(),
                                    "node_modules".to_string(),
                                    "__pycache__".to_string(),
                                ],
                            }],
                            &format!("finalize-stage:{mission_id}"),
                            "mission-finalize:policy-allow:git_finalize_stage",
                            mission_auth.as_ref(),
                        )
                        .await;
                    match stage_gate {
                        Err(e) => {
                            let msg = format!("git finalization staging denied by policy: {e}");
                            tracing::error!(mission_id = %mission_id, "{msg}");
                            git_finalize_error = Some(msg);
                            is_success = false;
                            final_status = "Failed";
                        }
                        Ok(stage_gate) => {
                            if let Err(e) =
                                crate::git::worktree::WorktreeManager::ensure_git_excludes(
                                    &self.workspace_root,
                                )
                                .await
                            {
                                let msg = format!("git exclude hygiene failed: {e}");
                                tracing::error!(mission_id = %mission_id, "{msg}");
                                git_finalize_error = Some(msg);
                                is_success = false;
                                final_status = "Failed";
                            } else if let Err(e) = self.finalize_root_staging(&stage_gate).await {
                                tracing::error!(
                                    mission_id = %mission_id,
                                    error = %e,
                                    "Git finalization staging failed; mission marked Failed"
                                );
                                git_finalize_error = Some(e);
                                is_success = false;
                                final_status = "Failed";
                            } else {
                                // Commit with real provenance (latest durable
                                // task + bound authorization, never fabricated).
                                if let Err(e) = self
                                    .finalize_root_commit(
                                        mission_id,
                                        task_id,
                                        role,
                                        mode,
                                        &commit_msg_text,
                                        mission_auth.as_ref(),
                                    )
                                    .await
                                {
                                    tracing::error!(
                                        mission_id = %mission_id,
                                        error = %e,
                                        "Git commit failed; mission marked Failed"
                                    );
                                    git_finalize_error = Some(e);
                                    is_success = false;
                                    final_status = "Failed";
                                }
                            }
                        }
                    }
                }
            }
        }

        // Guaranteed cleanup for worktrees if not already cleaned up.
        // Cleanup authorization is governed (never a bare boolean); cleanup
        // failure under AlwaysRemove flips completion to Failed.
        if !worktree_cleaned && let Some(ref wt) = worktree_opt {
            let retention = self.worktree_manager.config().retention_policy;
            if retention == crate::git::worktree::WorktreeRetentionPolicy::AlwaysRemove {
                let task_id = self.latest_mission_task_id(mission_id).await;
                match self
                    .authorize_git_operation(
                        mission_id,
                        task_id,
                        None,
                        crate::state_machine::agent::AgentRole::implementer(),
                        mode,
                        "runtime.git_worktree_remove",
                        self.workspace_root.clone(),
                        crate::git::GitOperation::WorktreeRemove { force: true },
                        vec![crate::git::GitOperation::BranchDelete {
                            branch: wt.branch.clone(),
                        }],
                        &format!("worktree:{}", wt.path.display()),
                        "mission-cleanup:policy-allow:git_worktree_remove",
                        mission_auth.as_ref(),
                    )
                    .await
                {
                    Err(e) => {
                        let msg = format!("guaranteed worktree cleanup denied by policy: {e}");
                        tracing::error!(mission_id = %mission_id, "{msg}");
                        git_finalize_error = Some(msg);
                        is_success = false;
                        final_status = "Failed";
                    }
                    Ok(gate) => {
                        if let Err(e) = self.worktree_manager.remove_worktree(wt, true, &gate).await
                        {
                            let msg = format!("guaranteed worktree cleanup failed: {e}");
                            tracing::error!(mission_id = %mission_id, "{msg}");
                            git_finalize_error = Some(msg);
                            is_success = false;
                            final_status = "Failed";
                        }
                    }
                }
            } else if is_success
                && retention != crate::git::worktree::WorktreeRetentionPolicy::AlwaysKeep
            {
                let task_id = self.latest_mission_task_id(mission_id).await;
                match self
                    .authorize_git_operation(
                        mission_id,
                        task_id,
                        None,
                        crate::state_machine::agent::AgentRole::implementer(),
                        mode,
                        "runtime.git_worktree_remove",
                        self.workspace_root.clone(),
                        crate::git::GitOperation::WorktreeRemove { force: false },
                        Vec::new(),
                        &format!("worktree:{}", wt.path.display()),
                        "mission-cleanup:policy-allow:git_worktree_remove",
                        mission_auth.as_ref(),
                    )
                    .await
                {
                    Err(e) => {
                        tracing::warn!(
                            mission_id = %mission_id,
                            error = %e,
                            "Optional worktree cleanup denied by policy; leaving worktree in place"
                        );
                    }
                    Ok(gate) => {
                        if let Err(e) = self
                            .worktree_manager
                            .remove_worktree(wt, false, &gate)
                            .await
                        {
                            tracing::warn!(
                                mission_id = %mission_id,
                                error = %e,
                                "Optional worktree cleanup failed; leaving worktree in place"
                            );
                        }
                    }
                }
            }
        }

        // Surface Git finalization evidence in the terminal summary: a mission
        // whose execution succeeded but whose required Git finalization failed
        // must never report a bare success.
        let git_evidence = git_finalize_error.clone();

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

        // Mission-scoped approval grants die with the mission: terminal state
        // invalidates every live grant so a later mission can never replay an
        // old approval. Invalidation failure is recorded, never swallowed.
        if let Err(e) = crate::policy::approval::PolicyGrantStore::new()
            .invalidate_mission_grants(&self.pool, mission_id)
            .await
        {
            tracing::warn!(
                mission_id = %mission_id,
                "mission grant invalidation failed after terminalization: {e}"
            );
        }

        let halt_summary = if !is_success && halt_reason == ControllerHaltReason::MissionCompleted {
            match git_evidence {
                Some(ref git_err) => format!("GitFinalizeFailed: {git_err}"),
                None => "MergeFailed".to_string(),
            }
        } else {
            match git_evidence {
                Some(ref git_err) if !is_success => {
                    format!("{:?}: GitFinalizeFailed: {git_err}", halt_reason)
                }
                _ => format!("{:?}", halt_reason),
            }
        };

        // 8. Publish terminal event
        let terminal_env = EventEnvelope::new(
            0,
            Some(mission_id),
            None,
            "runtime".to_string(),
            if is_success {
                EventType::MissionCompleted { mission_id }
            } else if final_status == "Cancelled" {
                EventType::MissionCancelled {
                    mission_id,
                    reason: halt_summary.clone(),
                }
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

        self.active_mission_cancellations
            .write()
            .await
            .remove(&mission_id);

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
        if let Some(token) = self
            .active_mission_cancellations
            .read()
            .await
            .get(&mission_id)
        {
            token.cancel();
        }

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

        let now_str = chrono::Utc::now().to_rfc3339();
        let _ = sqlx::query(
            "UPDATE tasks SET status = 'cancelled', completed_at = ?, updated_at = ? WHERE mission_id = ? AND LOWER(status) NOT IN ('succeeded', 'failed', 'cancelled', 'skipped')",
        )
        .bind(&now_str)
        .bind(&now_str)
        .bind(mission_id.as_bytes().as_slice())
        .execute(&self.pool)
        .await;

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

    /// Pause an active mission, updating persistent state and broadcasting `MissionPaused`.
    pub async fn pause_mission(
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
            crate::state_machine::MissionState::Paused,
        )
        .await?;

        let env = EventEnvelope::new(
            0,
            Some(mission_id),
            None,
            "runtime".to_string(),
            EventType::MissionPaused {
                mission_id,
                reason: reason.to_string(),
            },
        );
        let _ = self.event_bus.publish(env).await;
        Ok(())
    }

    /// Resume a paused mission, updating persistent state and broadcasting `MissionResumed`.
    pub async fn resume_mission(&self, mission_id: MissionId) -> Result<(), M31AError> {
        let mission_repo =
            crate::persistence::sqlite::repositories::mission::SqliteMissionRepository::new(
                self.pool.clone(),
            );
        crate::persistence::sqlite::repositories::MissionRepository::update_status(
            &mission_repo,
            mission_id,
            crate::state_machine::MissionState::Executing,
        )
        .await?;

        let env = EventEnvelope::new(
            0,
            Some(mission_id),
            None,
            "runtime".to_string(),
            EventType::MissionResumed { mission_id },
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

    fn normalize_workspace(path: &std::path::Path) -> PathBuf {
        crate::init::instance::canonicalize_workspace_root(path)
    }

    /// Resume an existing session and reconstruct its runtime conversation state.
    ///
    /// Workspace binding rule (fail closed): a session belongs to exactly one
    /// workspace root. Resuming the SAME workspace attaches normally;
    /// resuming a session from a DIFFERENT workspace is refused here — the
    /// caller MUST construct (or select) a runtime bound to that workspace
    /// and rebind through `InteractiveSessionRunner::rebind_runtime` before
    /// execution. This prevents cross-workspace execution against the wrong
    /// authority graph.
    pub async fn resume_session(
        &self,
        id: crate::ids::SessionId,
    ) -> Result<crate::interaction::session::Session, M31AError> {
        let repo = self.session_repo();
        let session = repo
            .get_session(id)
            .await?
            .ok_or_else(|| M31AError::NotFound(format!("Session '{id}' not found")))?;
        if Self::normalize_workspace(&session.workspace_root)
            != Self::normalize_workspace(&self.workspace_root)
        {
            return Err(M31AError::internal(format!(
                "refusing to resume session '{id}': session workspace '{}' != runtime workspace '{}'; rebind to the session workspace before execution",
                session.workspace_root.display(),
                self.workspace_root.display()
            )));
        }

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
    ///
    /// Operator-initiated (`/commit`) but still governed: the commit is
    /// authorized through the canonical policy lifecycle and carries real
    /// provenance (the mission's latest durable task, never a fabricated
    /// identifier). A missing mission or a mission with no durable tasks
    /// fails closed instead of committing unattributed history.
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

        // Real provenance: the operator's commit must trace to an actual
        // mission and its latest durable task. Fabricating either identifier
        // would unattributably rewrite history.
        let mid = mission_id.ok_or_else(|| {
            M31AError::validation(
                "Commit refused: no mission identity; refusing unattributed commit".to_string(),
            )
        })?;
        let task_id = self.latest_mission_task_id(mid).await.ok_or_else(|| {
            M31AError::validation(
                "Commit refused: mission has no durable tasks to attribute the commit to"
                    .to_string(),
            )
        })?;
        let commit_msg = message.unwrap_or("Autonomous changes verified by M31A");
        let role = crate::state_machine::agent::AgentRole::implementer();
        let stage_gate = self
            .authorize_git_operation(
                mid,
                Some(task_id),
                None,
                role.clone(),
                crate::state_machine::AutonomyMode::Assisted,
                "runtime.git_finalize_stage",
                self.workspace_root.clone(),
                crate::git::GitOperation::Add {
                    paths: vec!["-A".to_string()],
                },
                Vec::new(),
                &format!("operator-commit-stage:{mid}"),
                "operator-commit:policy-allow:git_finalize_stage",
                None,
            )
            .await?;
        self.finalize_root_staging(&stage_gate)
            .await
            .map_err(|e| M31AError::Internal(anyhow::anyhow!(e)))?;
        self.finalize_root_commit(
            mid,
            task_id,
            role,
            crate::state_machine::AutonomyMode::Assisted,
            &format!("fix: {commit_msg}\n\nCommitted via M31A interactive session."),
            None,
        )
        .await
        .map_err(|e| M31AError::Internal(anyhow::anyhow!(e)))?;

        Ok(format!("Committed changes successfully for mission {mid}"))
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

    /// Execute a global user-defined slash command through the canonical
    /// runtime path:
    ///
    /// ```text
    /// PromptCommand (typed contract from <global_config_dir>/prompts/commands/*.toml)
    ///     ↓
    /// PromptReference → canonical PromptCatalog
    ///     ↓
    /// canonical PromptCompiler
    ///     ↓
    /// ModelInvocationKind::UserCommand → ModelCaller
    ///     ↓
    /// model tool proposals → PolicyGate → ApprovalCoordinator → ToolPipeline
    ///     ↓
    /// Verification → CommandOutput
    /// ```
    ///
    /// The command achieves its effects through governed tools, never through
    /// direct side effects. Capability requests declared in the TOML are
    /// advisory only — the canonical capability registry, policy gate, approval
    /// coordinator, and tool pipeline remain authoritative.
    pub async fn execute_user_command(
        &self,
        command: &str,
        args: Vec<String>,
        session_id: crate::ids::SessionId,
    ) -> Result<(), M31AError> {
        let clean_name = command.trim_start_matches('/').to_lowercase();

        // 1. Locate the UserCommandDefinition from the immutable active registry snapshot (Phase E).
        let def = self
            .slash_registry
            .get_user_command_definition(&clean_name)
            .ok_or_else(|| {
                M31AError::not_found(format!("global user command '/{}' not found", command))
            })?;
        let cmd = &def.command;
        let contract = &def.contract;

        // 2. Bind arguments against the typed command schema.
        let bound = cmd
            .bind_arguments(&args)
            .map_err(|e| M31AError::validation(e.to_string()))?;

        if bound.help_requested {
            let session_repo = self.session_repo();
            if let Ok(seq) = session_repo.next_sequence(session_id).await {
                let _ = session_repo
                    .append_turn(
                        session_id,
                        &crate::interaction::session::ConversationTurn::AssistantMessage {
                            id: uuid::Uuid::now_v7(),
                            sequence: seq,
                            content: cmd.describe(),
                            created_at: chrono::Utc::now(),
                        },
                    )
                    .await;
            }
            tracing::info!(command = %cmd.name, "User command help displayed");
            return Ok(());
        }

        // 3. Role validation through RoleRegistry (fails closed if unknown).
        let role_name = &cmd.role;
        let agent_role = match role_name.as_str() {
            "integrator" => crate::state_machine::agent::AgentRole::integrator(),
            "architect" => crate::state_machine::agent::AgentRole::architect(),
            "implementer" => crate::state_machine::agent::AgentRole::implementer(),
            "reviewer" => crate::state_machine::agent::AgentRole::reviewer(),
            "diagnostician" => crate::state_machine::agent::AgentRole::diagnostician(),
            other => crate::state_machine::agent::AgentRole::new(other),
        };
        let role_valid = crate::agent::registry::RoleRegistry::global()
            .read()
            .map(|g| g.contains(&agent_role))
            .unwrap_or(false);
        if !role_valid {
            return Err(M31AError::validation(format!(
                "user command '/{}' specifies unknown role '{}'",
                cmd.name, cmd.role
            )));
        }

        // Autonomy mode: respect session/runtime autonomy precedence (never elevated by TOML).
        let autonomy_mode =
            crate::runtime_authorities::AutonomyPrecedence::from_config(&self.config);

        // 4. Resolve session and establish durable Mission -> Task -> Agent identity (Phase C & D).
        let session_repo = self.session_repo();
        let session = session_repo.get_session(session_id).await?;

        let mission_id = match session.as_ref().and_then(|s| s.active_mission_id) {
            Some(id) => id,
            None => {
                let id = crate::ids::MissionId::new();
                let mission_repo =
                    crate::persistence::sqlite::repositories::SqliteMissionRepository::new(
                        self.pool.clone(),
                    );
                let mission = crate::state::Mission::new(id, format!("User command /{}", cmd.name));
                mission_repo.insert(&mission).await.map_err(|e| {
                    M31AError::persistence(format!(
                        "failed to persist mission for user command: {e}"
                    ))
                })?;
                id
            }
        };
        let task_id = crate::ids::TaskId::new();
        let agent_id = crate::ids::AgentId::new();

        // Persist real Task aggregate in SQLite repository.
        let task_repo =
            crate::persistence::sqlite::repositories::SqliteTaskRepository::new(self.pool.clone());
        let mut task =
            crate::state::Task::new(task_id, mission_id, format!("Execute /{}", cmd.name));
        task.role = agent_role.clone();
        task.status = crate::state_machine::TaskState::Running;
        task.started_at = Some(chrono::Utc::now());
        task.prompt_ref = Some(def.prompt_ref.clone());
        task_repo.insert(&task).await.map_err(|e| {
            M31AError::persistence(format!("failed to persist task for user command: {e}"))
        })?;

        // Persist real Agent aggregate in SQLite repository.
        let agent_repo =
            crate::persistence::sqlite::repositories::SqliteAgentRepository::new(self.pool.clone());
        let max_steps = cmd.max_steps.max(1);
        let mut agent = crate::state::agent::Agent::new(agent_id, mission_id, cmd.role.to_string())
            .with_runtime_details(
                Some(task_id),
                format!("role:{}", cmd.role),
                max_steps,
                self.config.active_model.clone(),
            );
        agent.status = crate::state_machine::AgentState::Running;
        agent_repo.insert(&agent).await.map_err(|e| {
            M31AError::persistence(format!("failed to persist agent for user command: {e}"))
        })?;

        // Canonical budget admission: slash commands consume the SAME
        // budget authority as normal missions (no command-specific
        // accounting lane). Exhaustion fails closed before any model call,
        // with truthful durable task/agent state. Settlement is owned by a
        // drop guard so EVERY exit path (success, denial, error, `?`)
        // releases the reservation and records estimated consumption —
        // provider usage here is estimated by construction (see below).
        let budget_receipt = {
            let estimates = crate::budget::enforcer::TaskEstimates {
                estimated_tokens: (max_steps as u64).saturating_mul(2000),
                estimated_cost_usd: 0.01,
                requires_worker: true,
                estimated_artifact_bytes: 4096,
            };
            match self.budget_enforcer.reserve(&estimates, false) {
                Ok(receipt) => receipt,
                Err(action) => {
                    task.status = crate::state_machine::TaskState::Failed;
                    task.completed_at = Some(chrono::Utc::now());
                    let _ = task_repo.insert(&task).await;
                    agent.status = crate::state_machine::AgentState::Failed;
                    agent.completed_at = Some(chrono::Utc::now());
                    let _ = agent_repo.insert(&agent).await;
                    return Err(M31AError::internal(format!(
                        "user command '/{}' refused: budget exhausted ({action:?})",
                        cmd.name
                    )));
                }
            }
        };
        let mut budget_guard =
            SlashCommandBudgetGuard::new(self.budget_enforcer.clone(), budget_receipt);

        // 5. Build prompt context with bound argument values.
        let mut prompt_ctx = crate::prompt::context::PromptContext::new(
            format!("user-cmd-{}-{}", cmd.name, uuid::Uuid::now_v7()),
            mission_id.to_string(),
            task_id.to_string(),
            agent_role.clone(),
            contract
                .stage
                .unwrap_or(crate::prompt::context::MissionStage::Execute),
            contract.description.clone(),
        );

        for (k, v) in &bound.values {
            prompt_ctx.custom_parameters.insert(k.clone(), v.clone());
        }

        // 6. Compile the prompt through the canonical compiler.
        let compiler = self.authorities.prompt_compiler();
        let effective = compiler
            .compile(
                contract,
                &prompt_ctx,
                &crate::prompt::compiler::CompilationOptions::default()
                    .with_source_kind(
                        crate::prompt::provenance::PromptSourceKind::GlobalUserCommand,
                    )
                    .with_strategy(crate::prompt::strategy::PromptStrategy::Standard),
            )
            .map_err(|e| M31AError::internal(format!("prompt compilation failed: {e}")))?;

        // 7. Typed ModelInvocation with ModelInvocationKind::UserCommand and full execution identity.
        let invocation = crate::runtime_authorities::ModelInvocation::new(
            effective,
            def.prompt_ref.clone(),
            agent_role.clone(),
            crate::runtime_authorities::ModelInvocationKind::UserCommand,
            autonomy_mode,
        )
        .with_mission_id(mission_id)
        .with_task_id(task_id)
        .with_agent_id(agent_id);

        // 8. Record user command invocation turn in session repository.
        if let Ok(seq) = session_repo.next_sequence(session_id).await {
            let _ = session_repo
                .append_turn(
                    session_id,
                    &crate::interaction::session::ConversationTurn::UserMessage {
                        id: uuid::Uuid::now_v7(),
                        sequence: seq,
                        content: format!("/{} {}", cmd.name, args.join(" ")),
                        raw_text: format!("/{} {}", cmd.name, args.join(" ")),
                        mentions: Vec::new(),
                        created_at: chrono::Utc::now(),
                    },
                )
                .await;
        }

        // 9. Operator approval gate if required by command definition (binds exact mission/task/agent identity).
        if cmd.requires_approval {
            let inv_id = uuid::Uuid::now_v7();
            let approval_req = crate::policy::approval::ApprovalRequest::new_user_command(
                mission_id,
                Some(task_id),
                Some(agent_id),
                cmd.name.clone(),
                cmd.version,
                inv_id,
                format!("command.{}", cmd.name),
                serde_json::json!({
                    "command": cmd.name,
                    "arguments": args,
                }),
                vec![format!("workspace:{}", self.workspace_root.display())],
                crate::tools::risk::RiskClass::HighRiskMutation,
                None,
                self.policy.active_policy_hash(),
                format!(
                    "Command-level authorization: User command '/{}' (version {}) requires operator authorization before execution",
                    cmd.name, cmd.version
                ),
            );
            let decision = self
                .approval_coordinator
                .request_approval(
                    approval_req,
                    autonomy_mode,
                    crate::policy::approval::coordinator::DEFAULT_APPROVAL_TIMEOUT,
                )
                .await
                .map_err(|e| M31AError::internal(format!("approval request failed: {e}")))?;

            if !decision.is_allowed() {
                task.status = crate::state_machine::TaskState::Failed;
                task.completed_at = Some(chrono::Utc::now());
                task_repo.insert(&task).await.map_err(|e| {
                    M31AError::persistence(format!(
                        "failed to persist task failure on approval denial: {e}"
                    ))
                })?;

                agent.status = crate::state_machine::AgentState::Failed;
                agent.completed_at = Some(chrono::Utc::now());
                agent_repo.insert(&agent).await.map_err(|e| {
                    M31AError::persistence(format!(
                        "failed to persist agent failure on approval denial: {e}"
                    ))
                })?;

                return Err(M31AError::validation(format!(
                    "execution of user command '/{}' denied by operator approval: {:?}",
                    cmd.name, decision
                )));
            }
        }

        // 10. Prepare governed tool execution context with all three identities (Phase D).
        let cancel_token = tokio_util::sync::CancellationToken::new();
        let envelope =
            crate::agent::profile::AgentProfile::built_in(agent_role.clone()).capability_policy;
        let tool_ctx = crate::tools::ToolExecutionContext::new(
            self.capability_registry.clone(),
            self.workspace_root.clone(),
            cancel_token.clone(),
        )
        .with_role_envelope(envelope)
        .with_agent_role(agent_role.clone())
        .with_autonomy_mode(autonomy_mode)
        .with_policy_hash(self.policy.active_policy_hash().to_string())
        .with_mission_id(mission_id)
        .with_task_id(task_id)
        .with_agent_id(agent_id);

        // Canonical runtime authority tool pipeline runner (Phase B single authority).
        let pipeline_runner = self.authorities.tool_pipeline();

        let model_caller = self
            .model_caller()
            .ok_or_else(|| M31AError::validation("no model provider configured"))?;

        let mut current_invocation = invocation;
        let mut created_commits: Vec<String> = Vec::new();
        let mut steps_executed = 0;
        let mut command_completed = false;
        let model_inv_repo =
            crate::model::persistence::invocation::SqliteModelInvocationRepository::new(
                self.pool.clone(),
            );

        while steps_executed < max_steps {
            steps_executed += 1;
            agent.steps_consumed = steps_executed;
            agent_repo.insert(&agent).await.map_err(|e| {
                M31AError::persistence(format!("failed to persist agent step update: {e}"))
            })?;

            let proposal_res = model_caller
                .call_model_with_invocation(&current_invocation, &cancel_token)
                .await;

            let proposal = match proposal_res {
                Ok(p) => p,
                Err(e) => {
                    task.status = crate::state_machine::TaskState::Failed;
                    task.completed_at = Some(chrono::Utc::now());
                    let _ = task_repo.insert(&task).await;

                    agent.status = crate::state_machine::AgentState::Failed;
                    agent.completed_at = Some(chrono::Utc::now());
                    let _ = agent_repo.insert(&agent).await;

                    return Err(M31AError::internal(format!("model call failed: {e}")));
                }
            };

            // Estimate tokens and persist authoritative ModelInvocationRecord.
            let prompt_tokens = (current_invocation.prompt.total_bytes / 4).max(1);
            let completion_tokens = match &proposal {
                crate::agent::model_policy::ModelProposal::AssistantText { content } => {
                    (content.len() / 4).max(1)
                }
                crate::agent::model_policy::ModelProposal::Complete { summary, .. } => {
                    (summary.len() / 4).max(1)
                }
                crate::agent::model_policy::ModelProposal::AskUser { question, .. } => {
                    (question.len() / 4).max(1)
                }
                crate::agent::model_policy::ModelProposal::Handoff { reason, .. } => {
                    (reason.len() / 4).max(1)
                }
                crate::agent::model_policy::ModelProposal::ToolCalls { calls } => {
                    let total_chars: usize = calls
                        .iter()
                        .map(|c| c.arguments.to_string().len() + c.name.len())
                        .sum();
                    (total_chars / 4).max(1)
                }
            };
            let usage = crate::model::types::TokenUsage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
                reasoning_tokens: 0,
                source: crate::model::types::UsageSource::Estimated,
            };
            // Slash-command usage is estimated by construction: accumulate
            // into the settlement guard (estimated counters, never
            // authoritative).
            budget_guard.add_estimate(usage.total_tokens as u64);
            let outcome = match &proposal {
                crate::agent::model_policy::ModelProposal::ToolCalls { calls } => {
                    format!("tool_calls:{}", calls.len())
                }
                crate::agent::model_policy::ModelProposal::Complete { .. } => {
                    "complete".to_string()
                }
                crate::agent::model_policy::ModelProposal::AssistantText { .. } => {
                    "assistant_text".to_string()
                }
                crate::agent::model_policy::ModelProposal::AskUser { .. } => "ask_user".to_string(),
                crate::agent::model_policy::ModelProposal::Handoff { .. } => "handoff".to_string(),
            };
            let prov_json = serde_json::to_string(&current_invocation.prompt.provenance)
                .ok()
                .or_else(|| {
                    Some(
                        serde_json::json!({
                            "prompt_ref": current_invocation.prompt_ref,
                            "contract_hash": current_invocation.prompt.contract_hash,
                            "content_hash": current_invocation.prompt.content_hash,
                        })
                        .to_string(),
                    )
                });
            let mut inv_record = crate::model::persistence::invocation::ModelInvocationRecord::new(
                mission_id,
                task_id,
                agent_id,
                steps_executed,
                &self.config.active_provider,
                &self.config.active_model,
                1,
                outcome,
                &usage,
                format!("user_command:{}", cmd.name),
            );
            inv_record.prompt_provenance = prov_json;
            model_inv_repo
                .insert_invocation(&inv_record)
                .await
                .map_err(|e| {
                    M31AError::persistence(format!(
                        "failed to persist model invocation record: {e}"
                    ))
                })?;

            match proposal {
                crate::agent::model_policy::ModelProposal::ToolCalls { calls } => {
                    if calls.is_empty() {
                        command_completed = true;
                        break;
                    }
                    let mut tool_results_summary = Vec::new();
                    for call in calls {
                        let action_req = crate::agent::runner::ActionRequest {
                            id: call.id.clone(),
                            tool_name: call.name.clone(),
                            parameters: call.arguments.clone(),
                        };
                        let start_time = std::time::Instant::now();
                        let action_res = pipeline_runner
                            .execute_action(
                                &action_req,
                                &tool_ctx,
                                self.policy().as_ref(),
                                autonomy_mode,
                            )
                            .await;
                        let duration_ms = start_time.elapsed().as_millis() as i64;

                        // Persist tool execution record to SQLite tool_executions table.
                        let tool_exec_id = uuid::Uuid::now_v7();
                        let tool_now = chrono::Utc::now().to_rfc3339();
                        sqlx::query(
                            r#"
                            INSERT INTO tool_executions (
                                id, mission_id, task_id, agent_id, tool_id, attempt_number,
                                success, duration_ms, error_category, error_code, artifact_id,
                                effective_risk, created_at
                            ) VALUES (?, ?, ?, ?, ?, 1, ?, ?, ?, ?, NULL, 'governed', ?)
                            "#,
                        )
                        .bind(tool_exec_id.as_bytes().as_slice())
                        .bind(mission_id.as_bytes().as_slice())
                        .bind(task_id.as_bytes().as_slice())
                        .bind(agent_id.as_bytes().as_slice())
                        .bind(&call.name)
                        .bind(if action_res.success { 1 } else { 0 })
                        .bind(duration_ms)
                        .bind(action_res.error.as_deref())
                        .bind(action_res.error.as_deref())
                        .bind(&tool_now)
                        .execute(&self.pool)
                        .await
                        .map_err(|e| {
                            M31AError::persistence(format!(
                                "failed to persist tool execution record: {e}"
                            ))
                        })?;

                        if !action_res.success {
                            task.status = crate::state_machine::TaskState::Failed;
                            task.completed_at = Some(chrono::Utc::now());
                            task_repo.insert(&task).await.map_err(|e| {
                                M31AError::persistence(format!(
                                    "failed to persist task failure on tool error: {e}"
                                ))
                            })?;

                            agent.status = crate::state_machine::AgentState::Failed;
                            agent.completed_at = Some(chrono::Utc::now());
                            agent_repo.insert(&agent).await.map_err(|e| {
                                M31AError::persistence(format!(
                                    "failed to persist agent failure on tool error: {e}"
                                ))
                            })?;

                            return Err(M31AError::internal(format!(
                                "tool '{}' execution failed: {}",
                                call.name,
                                action_res
                                    .error
                                    .unwrap_or_else(|| "unknown error".to_string())
                            )));
                        }

                        if call.name == "git_commit" {
                            if let Ok(val) =
                                serde_json::from_str::<serde_json::Value>(&action_res.output)
                            {
                                if let Some(hash) = val.get("commit_hash").and_then(|h| h.as_str())
                                {
                                    created_commits.push(hash.to_string());
                                }
                            }
                        }

                        tool_results_summary.push(format!(
                            "Tool {} (call {}): success, output: {}",
                            call.name, call.id, action_res.output
                        ));
                    }

                    // Feed tool execution results back to model context for next step
                    let additional_text = format!(
                        "\n\nTool execution results:\n{}",
                        tool_results_summary.join("\n")
                    );
                    current_invocation
                        .prompt
                        .assembled_text
                        .push_str(&additional_text);
                }
                crate::agent::model_policy::ModelProposal::Complete { summary, .. } => {
                    tracing::info!(
                        command = %cmd.name,
                        summary = %summary,
                        "User command completed by model proposal"
                    );
                    command_completed = true;
                    break;
                }
                crate::agent::model_policy::ModelProposal::AssistantText { content } => {
                    tracing::info!(
                        command = %cmd.name,
                        content = %content,
                        "User command finished with assistant text"
                    );
                    command_completed = true;
                    break;
                }
                crate::agent::model_policy::ModelProposal::AskUser { question, .. } => {
                    tracing::info!(
                        command = %cmd.name,
                        question = %question,
                        "User command requested operator input"
                    );
                    task.status = crate::state_machine::TaskState::Blocked;
                    task.completed_at = Some(chrono::Utc::now());
                    task_repo.insert(&task).await.map_err(|e| {
                        M31AError::persistence(format!(
                            "failed to persist task blocked status: {e}"
                        ))
                    })?;

                    agent.status = crate::state_machine::AgentState::Paused;
                    agent.completed_at = Some(chrono::Utc::now());
                    agent_repo.insert(&agent).await.map_err(|e| {
                        M31AError::persistence(format!(
                            "failed to persist agent paused status: {e}"
                        ))
                    })?;

                    return Ok(());
                }
                crate::agent::model_policy::ModelProposal::Handoff { reason, .. } => {
                    tracing::info!(
                        command = %cmd.name,
                        reason = %reason,
                        "User command handed off"
                    );
                    task.status = crate::state_machine::TaskState::NeedsReview;
                    task.completed_at = Some(chrono::Utc::now());
                    task_repo.insert(&task).await.map_err(|e| {
                        M31AError::persistence(format!("failed to persist task review status: {e}"))
                    })?;

                    agent.status = crate::state_machine::AgentState::Paused;
                    agent.completed_at = Some(chrono::Utc::now());
                    agent_repo.insert(&agent).await.map_err(|e| {
                        M31AError::persistence(format!(
                            "failed to persist agent paused status: {e}"
                        ))
                    })?;

                    return Ok(());
                }
            }
        }

        if !command_completed {
            task.status = crate::state_machine::TaskState::Failed;
            task.completed_at = Some(chrono::Utc::now());
            task_repo.insert(&task).await.map_err(|e| {
                M31AError::persistence(format!(
                    "failed to persist task failure on step exhaustion: {e}"
                ))
            })?;

            agent.status = crate::state_machine::AgentState::Failed;
            agent.completed_at = Some(chrono::Utc::now());
            agent_repo.insert(&agent).await.map_err(|e| {
                M31AError::persistence(format!(
                    "failed to persist agent failure on step exhaustion: {e}"
                ))
            })?;

            return Err(M31AError::validation(format!(
                "user command '/{}' exhausted step budget of {} steps without completion",
                cmd.name, max_steps
            )));
        }

        // 11. Run verification checks if required by command contract and persist records (Phases 9, 10, 11, 26).
        let is_dry_run = bound
            .values
            .get("dry_run")
            .map(|s| s == "true")
            .unwrap_or(false);

        if cmd.verification_required {
            if is_dry_run {
                tracing::info!(command = %cmd.name, "Dry run active; recording dry_run_validation verification check");
                let check_id = uuid::Uuid::now_v7();
                let now = chrono::Utc::now().to_rfc3339();
                let snapshot_hash = self
                    .completion_gate()
                    .capture_current_snapshot_hash(mission_id, Some(task_id))
                    .unwrap_or_else(|_| "unhashed".to_string());
                let inputs_normalized = serde_json::json!({
                    "check": "dry_run_validation",
                    "command": cmd.name,
                    "arguments": args,
                })
                .to_string();

                sqlx::query(
                    r#"
                    INSERT INTO verification_checks (id, mission_id, task_id, tier, status, command_or_tool, inputs_normalized, summary, snapshot_hash, created_at)
                    VALUES (?, ?, ?, 1, 'passed', 'dry_run_validation', ?, 'dry run mode: inspection complete, no mutations applied', ?, ?)
                    "#,
                )
                .bind(check_id.as_bytes().as_slice())
                .bind(mission_id.as_bytes().as_slice())
                .bind(task_id.as_bytes().as_slice())
                .bind(&inputs_normalized)
                .bind(&snapshot_hash)
                .bind(&now)
                .execute(&self.pool)
                .await
                .map_err(|e| {
                    M31AError::persistence(format!(
                        "failed to persist verification check record: {e}"
                    ))
                })?;
            } else {
                let git = self.git_service();
                for check in &cmd.verification_checks {
                    let mut check_passed = false;
                    let mut summary = String::new();

                    match check.as_str() {
                        "working_tree_status" => {
                            let status = git.status().await.map_err(|e| {
                                M31AError::internal(format!(
                                    "verification 'working_tree_status' failed to query git status: {e}"
                                ))
                            })?;
                            if status.is_clean {
                                check_passed = true;
                                summary = "working tree is clean".to_string();
                            } else {
                                summary = format!(
                                    "working tree is dirty (staged: {:?}, unstaged: {:?}, untracked: {:?})",
                                    status.staged, status.unstaged, status.untracked
                                );
                            }
                        }
                        "commit_contains_single_file" => {
                            let commits_to_check = if created_commits.is_empty() {
                                let log = git.log(1).await.map_err(|e| {
                                M31AError::internal(format!(
                                    "verification 'commit_contains_single_file' failed to query git log: {e}"
                                ))
                            })?;
                                log.into_iter().map(|c| c.commit_hash).collect()
                            } else {
                                created_commits.clone()
                            };

                            if commits_to_check.is_empty() {
                                summary = "no commits found to inspect".to_string();
                            } else {
                                let mut all_single = true;
                                for commit_hash in &commits_to_check {
                                    let show_output = git.show(commit_hash).await.map_err(|e| {
                                        M31AError::internal(format!(
                                            "failed to inspect commit {commit_hash}: {e}"
                                        ))
                                    })?;
                                    let file_count = show_output
                                        .lines()
                                        .filter(|l| l.starts_with("diff --git "))
                                        .count();
                                    if file_count != 1 {
                                        all_single = false;
                                        summary = format!(
                                            "commit {} touches {} files (expected exactly 1)",
                                            commit_hash, file_count
                                        );
                                        break;
                                    }
                                }
                                if all_single {
                                    check_passed = true;
                                    summary = "each commit touches exactly one file".to_string();
                                }
                            }
                        }
                        "conventional_commit_message" => {
                            let commits_to_check = if created_commits.is_empty() {
                                let log = git.log(10).await.map_err(|e| {
                                M31AError::internal(format!(
                                    "failed to query git log for conventional commit verification: {e}"
                                ))
                            })?;
                                log.into_iter().map(|c| c.commit_hash).collect()
                            } else {
                                created_commits.clone()
                            };

                            if commits_to_check.is_empty() {
                                summary = "no commits found to inspect".to_string();
                            } else {
                                let log = git.log(10).await.map_err(|e| {
                                M31AError::internal(format!(
                                    "failed to query git log for conventional commit verification: {e}"
                                ))
                            })?;
                                let mut all_conv = true;
                                for commit_hash in &commits_to_check {
                                    let info = log
                                    .iter()
                                    .find(|c| {
                                        c.commit_hash == *commit_hash
                                            || commit_hash.starts_with(&c.commit_hash)
                                    })
                                    .ok_or_else(|| {
                                        M31AError::validation(format!(
                                            "verification 'conventional_commit_message' failed: commit {} not found in log",
                                            commit_hash
                                        ))
                                    })?;

                                    if !crate::interaction::user_commands::is_conventional_commit_message(
                                    &info.message,
                                ) {
                                    all_conv = false;
                                    summary = format!(
                                        "commit message '{}' is not conventional",
                                        info.message
                                    );
                                    break;
                                }
                                }
                                if all_conv {
                                    check_passed = true;
                                    summary = "all commit messages follow conventional commits"
                                        .to_string();
                                }
                            }
                        }
                        unknown => {
                            summary =
                                format!("unknown verification check '{}'; failing closed", unknown);
                        }
                    }

                    // Persist verification check into SQLite verification_checks table (Phases 10 & 11)
                    let check_id = uuid::Uuid::now_v7();
                    let now = chrono::Utc::now().to_rfc3339();
                    let snapshot_hash = self
                        .completion_gate()
                        .capture_current_snapshot_hash(mission_id, Some(task_id))
                        .unwrap_or_else(|_| "unhashed".to_string());
                    let inputs_normalized = serde_json::json!({
                        "check": check.as_str(),
                        "command": cmd.name,
                        "arguments": args,
                    })
                    .to_string();

                    sqlx::query(
                    r#"
                    INSERT INTO verification_checks (id, mission_id, task_id, tier, status, command_or_tool, inputs_normalized, summary, snapshot_hash, created_at)
                    VALUES (?, ?, ?, 1, ?, ?, ?, ?, ?, ?)
                    "#,
                )
                .bind(check_id.as_bytes().as_slice())
                .bind(mission_id.as_bytes().as_slice())
                .bind(task_id.as_bytes().as_slice())
                .bind(if check_passed { "passed" } else { "failed" })
                .bind(check.as_str())
                .bind(&inputs_normalized)
                .bind(&summary)
                .bind(&snapshot_hash)
                .bind(&now)
                .execute(&self.pool)
                .await
                .map_err(|e| {
                    M31AError::persistence(format!(
                        "failed to persist verification check record: {e}"
                    ))
                })?;

                    if !check_passed {
                        task.status = crate::state_machine::TaskState::Failed;
                        task.completed_at = Some(chrono::Utc::now());
                        task_repo.insert(&task).await.map_err(|e| {
                            M31AError::persistence(format!(
                                "failed to persist task failure on verification error: {e}"
                            ))
                        })?;

                        agent.status = crate::state_machine::AgentState::Failed;
                        agent.completed_at = Some(chrono::Utc::now());
                        agent_repo.insert(&agent).await.map_err(|e| {
                            M31AError::persistence(format!(
                                "failed to persist agent failure on verification error: {e}"
                            ))
                        })?;

                        return Err(M31AError::validation(format!(
                            "verification '{}' failed: {}",
                            check, summary
                        )));
                    }
                }
            }
        }

        // 12. Final success state persistence in SQLite repositories and session history.
        task.status = crate::state_machine::TaskState::Succeeded;
        task.completed_at = Some(chrono::Utc::now());
        task_repo.insert(&task).await.map_err(|e| {
            M31AError::persistence(format!("failed to persist task success state: {e}"))
        })?;

        agent.status = crate::state_machine::AgentState::Completed;
        agent.completed_at = Some(chrono::Utc::now());
        agent_repo.insert(&agent).await.map_err(|e| {
            M31AError::persistence(format!("failed to persist agent completed state: {e}"))
        })?;

        if let Ok(seq) = session_repo.next_sequence(session_id).await {
            let _ = session_repo
                .append_turn(
                    session_id,
                    &crate::interaction::session::ConversationTurn::AssistantMessage {
                        id: uuid::Uuid::now_v7(),
                        sequence: seq,
                        content: format!(
                            "Command '/{}' executed successfully. Verified {} check(s).",
                            cmd.name,
                            if cmd.verification_required {
                                cmd.verification_checks.len()
                            } else {
                                0
                            }
                        ),
                        created_at: chrono::Utc::now(),
                    },
                )
                .await;
        }

        Ok(())
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
