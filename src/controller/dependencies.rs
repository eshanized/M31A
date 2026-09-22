use crate::capability::traits::git::GitService;
use crate::events::bus::BroadcastEventBus;
use crate::kernel::seams::{
    ContextCompiler, EscalationChannel, PlanService, PolicyGate, RecoveryEngine,
    VerificationEngine, WorkScheduler, WorkerDispatcher,
};
use crate::persistence::sqlite::repositories::MissionRepository;
use crate::persistence::sqlite::transaction::SqliteTransactionManager;
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;

/// Bundles the 8 downstream subsystem seams without generic parameter propagation (D-06).
#[derive(Clone)]
pub struct ControllerDependencies {
    pub planner: Arc<dyn PlanService>,
    pub scheduler: Arc<dyn WorkScheduler>,
    pub policy: Arc<dyn PolicyGate>,
    pub context: Arc<dyn ContextCompiler>,
    pub dispatcher: Arc<dyn WorkerDispatcher>,
    pub verifier: Arc<dyn VerificationEngine>,
    pub recovery: Arc<dyn RecoveryEngine>,
    pub escalation: Arc<dyn EscalationChannel>,
    pub transaction_manager: Option<Arc<SqliteTransactionManager>>,
    pub mission_repo: Option<Arc<dyn MissionRepository>>,
    pub report_generator: Option<Arc<crate::report::ReportGenerator>>,
    pub budget_enforcer: Option<Arc<crate::budget::enforcer::BudgetEnforcer>>,
    pub workspace_root: Option<PathBuf>,
    pub approval_coordinator: Option<Arc<crate::policy::approval::ApprovalCoordinator>>,
    pub checkpoint_manager: Option<Arc<crate::checkpoint::manager::CheckpointManager>>,
    pub git_service: Option<Arc<dyn GitService>>,
    pub change_authority: Option<Arc<crate::change::authority::ChangeAuthority>>,
    /// Engineering memory store for execution-time diagnosis persistence.
    /// `None` disables memory writes (fail-safe: the autonomy loop never
    /// depends on memory availability).
    pub memory_store: Option<Arc<dyn crate::memory::EngineeringMemoryStore>>,
}

impl ControllerDependencies {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        planner: Arc<dyn PlanService>,
        scheduler: Arc<dyn WorkScheduler>,
        policy: Arc<dyn PolicyGate>,
        context: Arc<dyn ContextCompiler>,
        dispatcher: Arc<dyn WorkerDispatcher>,
        verifier: Arc<dyn VerificationEngine>,
        recovery: Arc<dyn RecoveryEngine>,
        escalation: Arc<dyn EscalationChannel>,
    ) -> Self {
        Self {
            planner,
            scheduler,
            policy,
            context,
            dispatcher,
            verifier,
            recovery,
            escalation,
            transaction_manager: None,
            mission_repo: None,
            report_generator: None,
            budget_enforcer: None,
            workspace_root: None,
            approval_coordinator: None,
            checkpoint_manager: None,
            git_service: None,
            change_authority: None,
            memory_store: None,
        }
    }

    /// Attach an engineering memory store for execution-time diagnosis persistence.
    pub fn with_memory_store(
        mut self,
        store: Arc<dyn crate::memory::EngineeringMemoryStore>,
    ) -> Self {
        self.memory_store = Some(store);
        self
    }

    /// Attach a centralized git service for version control operations.
    pub fn with_git_service(mut self, git_service: Arc<dyn GitService>) -> Self {
        self.git_service = Some(git_service);
        self
    }

    /// Attach an approval coordinator for interactive human authorization.
    pub fn with_approval_coordinator(
        mut self,
        coordinator: Arc<crate::policy::approval::ApprovalCoordinator>,
    ) -> Self {
        self.approval_coordinator = Some(coordinator);
        self
    }

    /// Attach a checkpoint manager for authoritative checkpoint creation and restoration.
    pub fn with_checkpoint_manager(
        mut self,
        checkpoint_manager: Arc<crate::checkpoint::manager::CheckpointManager>,
    ) -> Self {
        self.checkpoint_manager = Some(checkpoint_manager);
        self
    }

    /// Attach a change authority for closed-loop repair and mutation lifecycle.
    pub fn with_change_authority(
        mut self,
        change_authority: Arc<crate::change::authority::ChangeAuthority>,
    ) -> Self {
        self.change_authority = Some(change_authority);
        self
    }

    /// Assemble production dependencies connecting all real subsystems (GAP-03, AUT-01, BLK-02).
    pub fn production(
        pool: SqlitePool,
        workspace_root: PathBuf,
        storage_root: PathBuf,
        bus: Option<Arc<BroadcastEventBus>>,
    ) -> Self {
        Self::production_with_model(pool, workspace_root, storage_root, bus, None)
    }

    /// Assemble production dependencies with an optional custom model caller (GAP-03, GAP-05).
    pub fn production_with_model(
        pool: SqlitePool,
        workspace_root: PathBuf,
        storage_root: PathBuf,
        bus: Option<Arc<BroadcastEventBus>>,
        model_caller: Option<Arc<dyn crate::agent::model_policy::ModelCaller>>,
    ) -> Self {
        Self::production_with_model_and_config(
            pool,
            workspace_root,
            storage_root,
            bus,
            model_caller,
            None,
        )
    }

    /// Assemble production dependencies with authoritative configuration.
    pub fn production_with_model_and_config(
        pool: SqlitePool,
        workspace_root: PathBuf,
        storage_root: PathBuf,
        bus: Option<Arc<BroadcastEventBus>>,
        model_caller: Option<Arc<dyn crate::agent::model_policy::ModelCaller>>,
        config: Option<&crate::config::ResolvedConfiguration>,
    ) -> Self {
        Self::production_with_model_config_and_coordinator(
            pool,
            workspace_root,
            storage_root,
            bus,
            model_caller,
            config,
            None,
        )
    }

    /// Assemble production dependencies with explicit approval coordinator wiring (P0-A).
    pub fn production_with_model_config_and_coordinator(
        pool: SqlitePool,
        workspace_root: PathBuf,
        storage_root: PathBuf,
        bus: Option<Arc<BroadcastEventBus>>,
        model_caller: Option<Arc<dyn crate::agent::model_policy::ModelCaller>>,
        config: Option<&crate::config::ResolvedConfiguration>,
        coordinator: Option<Arc<crate::policy::approval::ApprovalCoordinator>>,
    ) -> Self {
        // Default authorities: each built once here and shared throughout
        // the bundle (dispatcher gate, verifier, reports, checkpoints).
        let policy_cfg = config.map(|c| &c.app_config.policy);
        let policy = Arc::new(
            crate::policy::effective::EffectivePolicy::standard_with_policy_config(
                &workspace_root,
                policy_cfg,
            ),
        );
        let artifacts = Arc::new(crate::persistence::artifacts::FsArtifactStore::new(
            storage_root.join("artifacts"),
        ));
        let budget = Arc::new(crate::budget::enforcer::BudgetEnforcer::new(
            Self::budget_for_config(config),
        ));
        Self::assemble_with_shared_authorities(
            pool,
            workspace_root,
            storage_root,
            bus,
            model_caller,
            config,
            coordinator,
            policy,
            artifacts,
            budget,
            None,
        )
    }

    /// Assemble production dependencies reusing the runtime's canonical
    /// shared authorities (one policy / budget / artifact authority per intended scope).
    ///
    /// Scope contract: `policy` is RUNTIME_SHARED immutable (replaced on
    /// `with_config`, never mutated); `budget` is RUNTIME_SHARED mutable via
    /// `update_limits` so consumption counters survive reconfiguration;
    /// `artifacts` is PROCESS_SHARED stateless (same base dir).
    #[allow(clippy::too_many_arguments)]
    pub fn production_with_shared_authorities(
        pool: SqlitePool,
        workspace_root: PathBuf,
        storage_root: PathBuf,
        bus: Option<Arc<BroadcastEventBus>>,
        model_caller: Option<Arc<dyn crate::agent::model_policy::ModelCaller>>,
        config: Option<&crate::config::ResolvedConfiguration>,
        coordinator: Option<Arc<crate::policy::approval::ApprovalCoordinator>>,
        policy: Arc<crate::policy::effective::EffectivePolicy>,
        artifacts: Arc<crate::persistence::artifacts::FsArtifactStore>,
        budget: Arc<crate::budget::enforcer::BudgetEnforcer>,
        capabilities: Option<Arc<crate::capability::registry::CapabilityRegistry>>,
    ) -> Self {
        Self::assemble_with_shared_authorities(
            pool,
            workspace_root,
            storage_root,
            bus,
            model_caller,
            config,
            coordinator,
            policy,
            artifacts,
            budget,
            capabilities,
        )
    }

    /// Resolve budget limits from authoritative configuration.
    fn budget_for_config(
        config: Option<&crate::config::ResolvedConfiguration>,
    ) -> crate::state::budget::ResourceBudget {
        if let Some(cfg) = config {
            crate::state::budget::ResourceBudget {
                max_agent_steps: cfg.app_config.budget.max_agent_steps,
                max_tokens: cfg.app_config.budget.max_tokens,
                max_wall_clock_seconds: cfg
                    .app_config
                    .budget
                    .max_wall_clock_seconds
                    .or(Some(cfg.app_config.runtime.timeout_secs)),
                max_cost_usd: cfg.app_config.budget.max_cost_usd,
                max_retries: cfg.app_config.budget.max_retries,
                max_concurrent_agents: Some(cfg.app_config.runtime.concurrency_limit),
                ..Default::default()
            }
        } else {
            crate::state::budget::ResourceBudget {
                max_agent_steps: Some(100),
                max_tokens: Some(1_000_000),
                max_wall_clock_seconds: Some(300),
                max_cost_usd: Some(5.0),
                max_retries: Some(3),
                max_concurrent_agents: Some(4),
                ..Default::default()
            }
        }
    }

    /// Shared assembly using caller-provided canonical authorities.
    #[allow(clippy::too_many_arguments)]
    fn assemble_with_shared_authorities(
        pool: SqlitePool,
        workspace_root: PathBuf,
        storage_root: PathBuf,
        bus: Option<Arc<BroadcastEventBus>>,
        model_caller: Option<Arc<dyn crate::agent::model_policy::ModelCaller>>,
        config: Option<&crate::config::ResolvedConfiguration>,
        coordinator: Option<Arc<crate::policy::approval::ApprovalCoordinator>>,
        policy: Arc<crate::policy::effective::EffectivePolicy>,
        artifacts: Arc<crate::persistence::artifacts::FsArtifactStore>,
        budget: Arc<crate::budget::enforcer::BudgetEnforcer>,
        capabilities: Option<Arc<crate::capability::registry::CapabilityRegistry>>,
    ) -> Self {
        let coord = coordinator.unwrap_or_else(|| {
            let base = crate::policy::approval::ApprovalCoordinator::new(Some(pool.clone()), None);
            if let Some(ref b) = bus {
                Arc::new(base.with_event_bus(b.clone() as Arc<dyn crate::events::EventBus>))
            } else {
                Arc::new(base)
            }
        });

        let memory_repo = Arc::new(crate::memory::SqliteEngineeringMemoryRepository::new(
            pool.clone(),
        ));
        let context = Arc::new(
            crate::context::compiler::ProductionContextCompiler::new()
                .with_workspace_root(workspace_root.clone())
                .with_memory_store(memory_repo.clone())
                .with_role_stage_fallback(Arc::new(|role| {
                    crate::agent::registry::RoleRegistry::global()
                        .read()
                        .ok()
                        .and_then(|guard| guard.stage_for(role))
                })),
        );

        let mut disp =
            crate::agent::dispatcher::ProductionWorkerDispatcher::new_with_roots_and_config(
                &workspace_root,
                &storage_root,
                config,
            )
            .with_db_pool(pool.clone())
            .with_approval_coordinator(coord.clone());
        if let Some(ref caps) = capabilities {
            disp = disp.with_capabilities(caps.clone());
        } else if let Some(ref b) = bus {
            disp = disp.with_capabilities(Arc::new(
                crate::capability::registry::CapabilityRegistry::production(
                    &workspace_root,
                    Some(b.clone()),
                    None,
                ),
            ));
        }
        if let Some(ref caller) = model_caller {
            disp = disp.with_model_caller(caller.clone());
        }

        let planner = Arc::new(
            crate::planning::service::PlanServiceImpl::new_with_roots(
                &workspace_root,
                &storage_root,
            )
            .with_model_caller(disp.model_caller.clone()),
        );

        let concurrency_limit = config
            .map(|c| c.app_config.runtime.concurrency_limit)
            .unwrap_or(4);

        let resource_manager = Arc::new(crate::scheduler::resources::ResourceManager::new(
            Some(pool.clone()),
            concurrency_limit as u64,
        ));
        let limits = crate::scheduler::concurrency::ConcurrencyLimits {
            max_global_workers: concurrency_limit,
            ..Default::default()
        };

        let scheduler_bus = bus.clone().map(|b| b as Arc<dyn crate::events::EventBus>);
        let scheduler = Arc::new(crate::scheduler::engine::SchedulerEngine::new(
            pool.clone(),
            resource_manager,
            limits,
            scheduler_bus,
        ));

        let dispatcher =
            Arc::new(disp.with_policy_gate(Arc::clone(&policy) as Arc<dyn PolicyGate>));

        let hierarchy = if let Some(cfg) = config {
            Arc::new(
                crate::verification::hierarchy::VerificationHierarchyEngine::for_workspace_with_config(
                    &workspace_root,
                    cfg,
                ),
            )
        } else {
            Arc::new(
                crate::verification::hierarchy::VerificationHierarchyEngine::for_workspace(
                    &workspace_root,
                    None,
                    None,
                ),
            )
        };

        let verifier = Arc::new(
            crate::verification::gate::EvidenceCompletionGate::new(
                pool.clone(),
                artifacts.clone(),
                workspace_root.clone(),
            )
            .with_hierarchy_engine(hierarchy),
        );

        let mut recovery =
            crate::recovery::adapter::ProductionRecoveryEngine::new(Some(pool.clone()));
        // Evidence-based diagnosis: attach the model caller so recovery
        // hypotheses come from model reasoning over failure evidence, with
        // deterministic heuristic fallback when no caller is configured.
        // Without this, recovery degrades to static keyword rules even when
        // a model is available.
        if let Some(ref caller) = model_caller {
            recovery = recovery.with_diagnostician(Arc::new(
                crate::verification::diagnostician::ModelDiagnostician::new()
                    .with_model_caller(caller.clone()),
            ));
        }
        let recovery = Arc::new(recovery);

        let escalation = Arc::new(
            crate::policy::approval::adapter::ProductionEscalationChannel::new(Some(pool.clone()))
                .with_coordinator(coord.clone()),
        );

        let transaction_manager = Arc::new(SqliteTransactionManager::new(pool.clone()));
        let mission_repo = Arc::new(
            crate::persistence::sqlite::repositories::SqliteMissionRepository::new(pool.clone()),
        );

        let report_repo = Arc::new(
            crate::persistence::sqlite::repositories::report::SqliteReportRepository::new(
                pool.clone(),
            ),
        );
        let redactor = Arc::new(crate::telemetry::redactor::SecretRedactor::new());
        let report_generator = Arc::new(crate::report::ReportGenerator::new(
            pool.clone(),
            artifacts.clone(),
            report_repo,
            redactor,
        ));

        let staging_dir = storage_root.join("staging");
        let checkpoint_manager = Arc::new(crate::checkpoint::manager::CheckpointManager::new(
            pool.clone(),
            artifacts.clone(),
            staging_dir,
        ));

        Self {
            planner,
            scheduler,
            policy,
            context,
            dispatcher,
            verifier,
            recovery,
            escalation,
            transaction_manager: Some(transaction_manager),
            mission_repo: Some(mission_repo),
            report_generator: Some(report_generator),
            budget_enforcer: Some(budget),
            workspace_root: Some(workspace_root),
            approval_coordinator: Some(coord),
            checkpoint_manager: Some(checkpoint_manager),
            git_service: None,
            change_authority: Some(Arc::new(
                crate::change::authority::ChangeAuthority::new().with_pool(pool.clone()),
            )),
            memory_store: Some(
                memory_repo.clone() as Arc<dyn crate::memory::EngineeringMemoryStore>
            ),
        }
    }

    /// Wire optional persistence components for ACID aggregate and checkpoint commits (PST-01, PST-06, FINDING-05).
    pub fn with_persistence(
        mut self,
        transaction_manager: Arc<SqliteTransactionManager>,
        mission_repo: Arc<dyn MissionRepository>,
    ) -> Self {
        self.transaction_manager = Some(transaction_manager);
        self.mission_repo = Some(mission_repo);
        self
    }

    /// Wire production or custom VerificationEngine (VER-05).
    pub fn with_verifier(mut self, verifier: Arc<dyn VerificationEngine>) -> Self {
        self.verifier = verifier;
        self
    }

    /// Wire production or custom WorkScheduler (SCHED-01).
    pub fn with_scheduler(mut self, scheduler: Arc<dyn WorkScheduler>) -> Self {
        self.scheduler = scheduler;
        self
    }

    pub fn with_report_generator(mut self, generator: Arc<crate::report::ReportGenerator>) -> Self {
        self.report_generator = Some(generator);
        self
    }

    pub fn with_budget_enforcer(
        mut self,
        enforcer: Arc<crate::budget::enforcer::BudgetEnforcer>,
    ) -> Self {
        self.budget_enforcer = Some(enforcer);
        self
    }
}
