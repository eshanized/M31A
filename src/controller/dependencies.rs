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
///
/// Authority contract (wiring remediation v0.1.1): fields are PRIVATE.
/// Production code MUST NOT construct split-brain combinations such as
/// `policy = B, model_caller = A, tools = C` by direct field assignment.
/// The production path is `production_with_shared_authorities`, which
/// consumes already-composed runtime authorities; test/rotation seams use the
/// `with_*` builders, each of which replaces ONE derived seam without
/// forking shared authorities.
#[derive(Clone)]
pub struct ControllerDependencies {
    planner: Arc<dyn PlanService>,
    scheduler: Arc<dyn WorkScheduler>,
    policy: Arc<dyn PolicyGate>,
    context: Arc<dyn ContextCompiler>,
    dispatcher: Arc<dyn WorkerDispatcher>,
    verifier: Arc<dyn VerificationEngine>,
    recovery: Arc<dyn RecoveryEngine>,
    escalation: Arc<dyn EscalationChannel>,
    transaction_manager: Option<Arc<SqliteTransactionManager>>,
    mission_repo: Option<Arc<dyn MissionRepository>>,
    report_generator: Option<Arc<crate::report::ReportGenerator>>,
    budget_enforcer: Option<Arc<crate::budget::enforcer::BudgetEnforcer>>,
    workspace_root: Option<PathBuf>,
    approval_coordinator: Option<Arc<crate::policy::approval::ApprovalCoordinator>>,
    checkpoint_manager: Option<Arc<crate::checkpoint::manager::CheckpointManager>>,
    git_service: Option<Arc<dyn GitService>>,
    change_authority: Option<Arc<crate::change::authority::ChangeAuthority>>,
    /// Engineering memory store for execution-time diagnosis persistence.
    /// `None` disables memory writes (fail-safe: the autonomy loop never
    /// depends on memory availability).
    memory_store: Option<Arc<dyn crate::memory::EngineeringMemoryStore>>,
}

impl ControllerDependencies {
    /// Access the authoritative plan service seam.
    pub fn planner(&self) -> &Arc<dyn PlanService> {
        &self.planner
    }
    /// Access the authoritative work scheduler seam.
    pub fn scheduler(&self) -> &Arc<dyn WorkScheduler> {
        &self.scheduler
    }
    /// Access the authoritative policy gate seam.
    pub fn policy(&self) -> &Arc<dyn PolicyGate> {
        &self.policy
    }
    /// Access the authoritative context compiler seam.
    pub fn context(&self) -> &Arc<dyn ContextCompiler> {
        &self.context
    }
    /// Access the authoritative worker dispatcher seam.
    pub fn dispatcher(&self) -> &Arc<dyn WorkerDispatcher> {
        &self.dispatcher
    }
    /// Access the authoritative verification engine seam.
    pub fn verifier(&self) -> &Arc<dyn VerificationEngine> {
        &self.verifier
    }
    /// Access the authoritative recovery engine seam.
    pub fn recovery(&self) -> &Arc<dyn RecoveryEngine> {
        &self.recovery
    }
    /// Access the authoritative escalation channel seam.
    pub fn escalation(&self) -> &Arc<dyn EscalationChannel> {
        &self.escalation
    }
    /// Access the transaction manager, if wired.
    pub fn transaction_manager(&self) -> Option<&Arc<SqliteTransactionManager>> {
        self.transaction_manager.as_ref()
    }
    /// Access the mission repository, if wired.
    pub fn mission_repo(&self) -> Option<&Arc<dyn MissionRepository>> {
        self.mission_repo.as_ref()
    }
    /// Access the report generator, if wired.
    pub fn report_generator(&self) -> Option<&Arc<crate::report::ReportGenerator>> {
        self.report_generator.as_ref()
    }
    /// Access the budget enforcer, if wired.
    pub fn budget_enforcer(&self) -> Option<&Arc<crate::budget::enforcer::BudgetEnforcer>> {
        self.budget_enforcer.as_ref()
    }
    /// Access the workspace root, if wired.
    pub fn workspace_root(&self) -> Option<&std::path::Path> {
        self.workspace_root.as_deref()
    }
    /// Access the approval coordinator, if wired.
    pub fn approval_coordinator(
        &self,
    ) -> Option<&Arc<crate::policy::approval::ApprovalCoordinator>> {
        self.approval_coordinator.as_ref()
    }
    /// Access the checkpoint manager, if wired.
    pub fn checkpoint_manager(
        &self,
    ) -> Option<&Arc<crate::checkpoint::manager::CheckpointManager>> {
        self.checkpoint_manager.as_ref()
    }
    /// Access the git service, if wired.
    pub fn git_service(&self) -> Option<&Arc<dyn GitService>> {
        self.git_service.as_ref()
    }
    /// Access the change authority, if wired.
    pub fn change_authority(&self) -> Option<&Arc<crate::change::authority::ChangeAuthority>> {
        self.change_authority.as_ref()
    }
    /// Access the engineering memory store, if wired.
    pub fn memory_store(&self) -> Option<&Arc<dyn crate::memory::EngineeringMemoryStore>> {
        self.memory_store.as_ref()
    }
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

    /// Replace the plan service seam (test/rotation seam; shared authorities untouched).
    pub fn with_planner(mut self, planner: Arc<dyn PlanService>) -> Self {
        self.planner = planner;
        self
    }

    /// Replace the policy gate seam (test/rotation seam; shared authorities untouched).
    pub fn with_policy(mut self, policy: Arc<dyn PolicyGate>) -> Self {
        self.policy = policy;
        self
    }

    /// Replace the context compiler seam.
    ///
    /// The production path installs the runtime-shared compiler here so the
    /// controller observes the same context authority as engines and the
    /// pre-execution coordinator.
    pub fn with_context(mut self, context: Arc<dyn ContextCompiler>) -> Self {
        self.context = context;
        self
    }

    /// Replace the worker dispatcher seam (test/rotation seam; shared authorities untouched).
    pub fn with_dispatcher(mut self, dispatcher: Arc<dyn WorkerDispatcher>) -> Self {
        self.dispatcher = dispatcher;
        self
    }

    /// Replace the recovery engine seam (test/rotation seam; shared authorities untouched).
    pub fn with_recovery(mut self, recovery: Arc<dyn RecoveryEngine>) -> Self {
        self.recovery = recovery;
        self
    }

    /// Replace the escalation channel seam (test/rotation seam; shared authorities untouched).
    pub fn with_escalation(mut self, escalation: Arc<dyn EscalationChannel>) -> Self {
        self.escalation = escalation;
        self
    }

    /// Attach the workspace root this bundle is scoped to.
    pub fn with_workspace_root(mut self, workspace_root: PathBuf) -> Self {
        self.workspace_root = Some(workspace_root);
        self
    }

    /// Assemble production dependencies connecting all real subsystems (GAP-03, AUT-01, BLK-02).
    ///
    /// Compatibility shim: builds its own policy/artifact/budget authorities
    /// and delegates to the canonical path. Production code with a composed
    /// `RuntimeAuthorities` set MUST use `production_with_shared_authorities`
    /// instead; this shim exists for standalone/test callers only.
    pub fn production(
        pool: SqlitePool,
        workspace_root: PathBuf,
        storage_root: PathBuf,
        bus: Option<Arc<BroadcastEventBus>>,
    ) -> Self {
        Self::production_with_model(pool, workspace_root, storage_root, bus, None)
    }

    /// Assemble production dependencies with an optional custom model caller (GAP-03, GAP-05).
    ///
    /// Compatibility shim delegating toward the canonical path (see `production`).
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
    ///
    /// Compatibility shim delegating toward the canonical path (see `production`).
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
    ///
    /// Compatibility shim: builds its own default authorities, then funnels
    /// into `assemble_with_shared_authorities`. Canonical production callers
    /// pass already-composed authorities via `production_with_shared_authorities`.
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
            crate::deployment::DeploymentPaths::project_artifacts_dir(
                &workspace_root,
                crate::deployment::DeploymentChannel::current(),
            ),
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
            None,
        )
    }

    /// Assemble production dependencies reusing the runtime's canonical
    /// shared authorities (one policy / budget / artifact authority per intended scope).
    ///
    /// CANONICAL PRODUCTION PATH. `policy`, `artifacts`, `budget`, and
    /// `capabilities` are the runtime-shared instances — never rebuilt here.
    /// `context_compiler`, when provided, installs the runtime-shared context
    /// authority; when `None`, a legacy internal compiler is built for
    /// standalone/test callers (same inputs, but NOT the shared instance).
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
        context_compiler: Option<Arc<dyn ContextCompiler>>,
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
            context_compiler,
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
    ///
    /// The dispatcher is assembled via `from_shared_authorities` whenever the
    /// runtime-shared capability registry is provided (all production paths):
    /// no second registry, policy, provider, or caller is constructed. The
    /// legacy self-contained dispatcher construction below runs ONLY for
    /// standalone callers without composed authorities.
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
        context_compiler: Option<Arc<dyn ContextCompiler>>,
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
        // One context authority: the runtime-shared compiler when provided,
        // else a legacy internal build (same inputs) for standalone callers.
        let context: Arc<dyn ContextCompiler> = match context_compiler {
            Some(shared) => shared,
            None => Arc::new(
                crate::context::compiler::ProductionContextCompiler::new()
                    .with_workspace_root(workspace_root.clone())
                    .with_memory_store(memory_repo.clone())
                    .with_role_stage_fallback(Arc::new(|role| {
                        crate::agent::registry::RoleRegistry::global()
                            .read()
                            .ok()
                            .and_then(|guard| guard.stage_for(role))
                    })),
            ),
        };

        let artifact_trait_store: Arc<dyn crate::persistence::artifacts::ArtifactStore> =
            artifacts.clone() as Arc<dyn crate::persistence::artifacts::ArtifactStore>;
        let disp = match (capabilities, model_caller.clone()) {
            (Some(caps), caller) => {
                // Canonical path: everything shared, nothing constructed.
                crate::agent::dispatcher::ProductionWorkerDispatcher::from_shared_authorities(
                    workspace_root.clone(),
                    caps,
                    Arc::clone(&policy) as Arc<dyn PolicyGate>,
                    artifact_trait_store,
                    caller,
                    Some(coord.clone()),
                    Some(pool.clone()),
                    config,
                    Some(context.clone()),
                )
            }
            (None, caller) => {
                // Legacy standalone path (no composed authorities available):
                // self-contained dispatcher construction. Production callers
                // MUST pass `Some(capabilities)`; see `AppRuntime`.
                // The dispatcher still binds the scope's context compiler
                // (shared or scope-internal, never a worker-owned divergent
                // build): dispatch without a compiler fails closed.
                let mut legacy =
                    crate::agent::dispatcher::ProductionWorkerDispatcher::new_with_roots_and_config(
                        &workspace_root,
                        &storage_root,
                        config,
                    )
                    .with_db_pool(pool.clone())
                    .with_approval_coordinator(coord.clone())
                    .with_context_compiler(context.clone());
                if let Some(ref b) = bus {
                    legacy = legacy.with_capabilities(Arc::new(
                        crate::capability::registry::CapabilityRegistry::production(
                            &workspace_root,
                            Some(b.clone()),
                            None,
                        ),
                    ));
                }
                if let Some(caller) = caller {
                    legacy = legacy.with_model_caller(caller);
                }
                legacy
            }
        };

        // Shared prompt authority: the planner binds the SAME catalog the
        // context authority owns (never an isolated per-component build).
        let mut planner_service = crate::planning::service::PlanServiceImpl::new_with_roots(
            &workspace_root,
            &storage_root,
        )
        .with_model_caller(disp.model_caller().clone());
        if let Some(catalog) = context.prompt_catalog() {
            planner_service = planner_service.with_prompt_catalog(catalog.clone());
        }
        if let Some(compiler) = context.prompt_compiler() {
            planner_service = planner_service.with_prompt_compiler(compiler.clone());
        }
        let planner = Arc::new(planner_service);
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
        //
        // Shared prompt authorities: the diagnostician binds the SAME
        // catalog/compiler the context authority owns (never an isolated
        // per-component build).
        {
            let mut diagnostician = crate::verification::diagnostician::ModelDiagnostician::new();
            if let Some(catalog) = context.prompt_catalog() {
                diagnostician = diagnostician.with_catalog(catalog.clone());
            }
            if let Some(compiler) = context.prompt_compiler() {
                diagnostician = diagnostician.with_compiler(compiler.clone());
            }
            if let Some(ref caller) = model_caller {
                diagnostician = diagnostician.with_model_caller(caller.clone());
            }
            recovery = recovery.with_diagnostician(Arc::new(diagnostician));
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

        // Channel-aware staging: the controller checkpoint manager must
        // resolve the same per-channel directory as every other runtime
        // consumer (single staging authority per channel).
        let staging_dir = crate::deployment::DeploymentPaths::project_staging_dir(
            &workspace_root,
            crate::deployment::DeploymentChannel::current(),
        );
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

    /// Replace the transaction manager (test/rotation seam; shared authorities untouched).
    pub fn with_transaction_manager(
        mut self,
        transaction_manager: Arc<SqliteTransactionManager>,
    ) -> Self {
        self.transaction_manager = Some(transaction_manager);
        self
    }

    /// Replace the mission repository (test/rotation seam; shared authorities untouched).
    pub fn with_mission_repo(mut self, mission_repo: Arc<dyn MissionRepository>) -> Self {
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
