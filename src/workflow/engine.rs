//! WorkflowEngine coordinating durable multi-step workflow execution over M31A runtime authorities.
//!
//! # Core Invariant
//!
//! "The model proposes. The runtime decides."
//!
//! Under the unified authority model (AD-003), WorkflowEngine is a pure declarative
//! workflow lowering and canonical runtime orchestrator. There is exactly ONE production
//! execution path:
//!
//! ```text
//! Workflow definition
//!     ↓
//! WorkflowCompiler / CompiledWorkflow::lower
//!     ↓
//! LoweredWorkflow
//!     ↓
//! Canonical Mission + CandidatePlan
//!     ↓
//! AutonomyController::run
//!     ↓
//! SchedulerEngine
//!     ↓
//! WorkerDispatcher
//!     ↓
//! WorkerRunner
//! ```
//!
//! WorkflowEngine cannot independently calculate ready waves, allocate workers, or declare
//! task completion. All execution authority resides exclusively in `AutonomyController`
//! and `SchedulerEngine`.

use crate::controller::AutonomyController;
use crate::controller::dependencies::ControllerDependencies;
use crate::controller::halting::ControllerHaltReason;
use crate::events::bus::EventBus;
use crate::events::envelope::EventEnvelope;
use crate::events::types::EventType;
use crate::ids::{MissionId, WorkflowRunId};
use crate::prompt::PromptCatalog;
use crate::state::intake::AutonomyMode;
use crate::state::mission::Mission;
use crate::workflow::compiler::CompiledWorkflow;
use crate::workflow::error::WorkflowError;
use crate::workflow::repository::WorkflowRepository;
use crate::workflow::state::{
    WorkflowArtifact, WorkflowArtifactStatus, WorkflowMode, WorkflowRun, WorkflowRunState,
    WorkflowStepRun, WorkflowStepState,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

/// Request payload to initiate a workflow run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStartRequest {
    pub workspace_root: PathBuf,
    pub mode: WorkflowMode,
    #[serde(default)]
    pub initial_parameters: BTreeMap<String, String>,
}

impl WorkflowStartRequest {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            mode: WorkflowMode::Standard,
            initial_parameters: BTreeMap::new(),
        }
    }

    pub fn with_mode(mut self, mode: WorkflowMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_parameter(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.initial_parameters.insert(key.into(), value.into());
        self
    }
}

/// Durable handle returned when a workflow run is started.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowRunHandle {
    pub run_id: WorkflowRunId,
    pub definition_id: String,
    pub definition_version: u32,
    pub status: WorkflowRunState,
}

/// Comprehensive inspection snapshot of a workflow run and its current progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowExecutionSnapshot {
    pub run: WorkflowRun,
    pub step_runs: Vec<WorkflowStepRun>,
    pub artifacts: Vec<WorkflowArtifact>,
    pub ready_step_keys: Vec<String>,
    pub blocked_step_keys: Vec<String>,
}

/// Replan request payload passed to `WorkflowReplanner`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplanRequest {
    pub workflow_run_id: WorkflowRunId,
    pub failed_step_key: String,
    pub attempt: u32,
    pub reason: String,
    pub affected_artifacts: Vec<String>,
    pub invalidated_downstream_steps: Vec<String>,
}

/// Extension point seam for handling `RecoveryStrategy::Replan`.
pub trait WorkflowReplanner: Send + Sync {
    /// Request re-planning from runtime authorities.
    fn request_replan(&self, request: &ReplanRequest) -> Result<(), WorkflowError>;
}

/// Central Workflow Orchestration Engine lowering declarative compiled workflows
/// into canonical Missions and driving them through `AutonomyController`.
pub struct WorkflowEngine {
    repository: Arc<dyn WorkflowRepository>,
    prompt_catalog: Arc<dyn PromptCatalog>,
    event_bus: Option<Arc<dyn EventBus>>,
    replanner: Option<Arc<dyn WorkflowReplanner>>,
    active_cancellation_tokens: Arc<RwLock<HashMap<WorkflowRunId, CancellationToken>>>,
    artifact_service: Option<Arc<crate::persistence::ArtifactService>>,
    dependencies: Option<ControllerDependencies>,
}

impl WorkflowEngine {
    /// Construct a new WorkflowEngine instance.
    pub fn new(
        repository: Arc<dyn WorkflowRepository>,
        prompt_catalog: Arc<dyn PromptCatalog>,
        event_bus: Option<Arc<dyn EventBus>>,
    ) -> Self {
        Self {
            repository,
            prompt_catalog,
            event_bus,
            replanner: None,
            active_cancellation_tokens: Arc::new(RwLock::new(HashMap::new())),
            artifact_service: None,
            dependencies: None,
        }
    }

    /// Construct a fully configured WorkflowEngine directly from `ControllerDependencies`.
    pub fn from_dependencies(
        repository: Arc<dyn WorkflowRepository>,
        prompt_catalog: Arc<dyn PromptCatalog>,
        dependencies: ControllerDependencies,
        event_bus: Option<Arc<dyn EventBus>>,
    ) -> Self {
        Self::new(repository, prompt_catalog, event_bus).with_dependencies(dependencies)
    }

    /// Attach controller dependencies for canonical runtime execution.
    pub fn with_dependencies(mut self, dependencies: ControllerDependencies) -> Self {
        self.dependencies = Some(dependencies);
        self
    }

    pub fn dependencies(&self) -> Option<&ControllerDependencies> {
        self.dependencies.as_ref()
    }

    pub fn with_artifact_service(
        mut self,
        artifact_service: Arc<crate::persistence::ArtifactService>,
    ) -> Self {
        self.artifact_service = Some(artifact_service);
        self
    }

    pub fn artifact_service(&self) -> Option<&Arc<crate::persistence::ArtifactService>> {
        self.artifact_service.as_ref()
    }

    pub fn with_replanner(mut self, replanner: Arc<dyn WorkflowReplanner>) -> Self {
        self.replanner = Some(replanner);
        self
    }

    pub fn repository(&self) -> &Arc<dyn WorkflowRepository> {
        &self.repository
    }

    pub fn prompt_catalog(&self) -> &Arc<dyn PromptCatalog> {
        &self.prompt_catalog
    }

    // =========================================================================
    // Canonical Workflow Execution (AD-003)
    // =========================================================================

    /// Start executing a workflow run through the single canonical runtime spine (AD-003).
    ///
    /// # Pipeline:
    /// 1. Validates compiled workflow definition and workspace directory.
    /// 2. Lowers compiled workflow into canonical Mission DAG & CandidatePlan.
    /// 3. Writes canonical `.planning/projections/plan.json` projection.
    /// 4. Durably creates `WorkflowRun` in `Running` state.
    /// 5. Persists canonical `Mission` aggregate in `MissionRepository`.
    /// 6. Materializes `CandidatePlan` into `SchedulerEngine`.
    /// 7. Durably materializes `WorkflowStepRun` records linked to canonical `MissionId`.
    /// 8. Emits `WorkflowStarted` lifecycle event.
    /// 9. Drives execution exclusively via `AutonomyController`.
    /// 10. Synchronizes workflow run and step run states from canonical task completion.
    pub async fn start_workflow(
        &self,
        compiled: &CompiledWorkflow,
        request: WorkflowStartRequest,
    ) -> Result<WorkflowRunHandle, WorkflowError> {
        let deps = self.dependencies.as_ref().ok_or_else(|| {
            WorkflowError::ExecutionFailed {
                step_key: "root".to_string(),
                reason: "start_workflow requires configured ControllerDependencies (WorkScheduler, WorkerDispatcher, VerificationEngine)".to_string(),
            }
        })?;

        // 1. Structural validation
        compiled.definition.validate()?;

        if !request.workspace_root.exists() {
            tokio::fs::create_dir_all(&request.workspace_root)
                .await
                .map_err(|e| {
                    WorkflowError::PersistenceFailure(format!(
                        "failed to create workspace directory: {}",
                        e
                    ))
                })?;
        }

        // 2. Canonical workflow lowering into Mission DAG (AD-003)
        let mission_id = MissionId::new();
        let unblocked = compiled.compute_unblocked_step_keys(&std::collections::HashSet::new());
        let lowered = if unblocked.len() == compiled.definition.steps.len() {
            compiled.lower(mission_id)?
        } else {
            compiled.lower_subset(mission_id, Some(&unblocked), 0)?
        };

        // 3. Write canonical plan.json projection
        let projections_dir = request.workspace_root.join(".planning").join("projections");
        let _ = tokio::fs::create_dir_all(&projections_dir).await;
        if let Ok(plan_json) = serde_json::to_string_pretty(&lowered.candidate_plan) {
            let _ = tokio::fs::write(projections_dir.join("plan.json"), &plan_json).await;
            let mdir = crate::planning::projections::mission_projections_dir(
                &request.workspace_root,
                lowered.mission_id,
            );
            let _ = tokio::fs::create_dir_all(&mdir).await;
            let _ = tokio::fs::write(mdir.join("plan.json"), &plan_json).await;
        }

        // 4. Create durable WorkflowRun
        let mut run = WorkflowRun::new(
            compiled.definition.id.clone(),
            compiled.definition.version,
            request.workspace_root.clone(),
            request.mode,
        );
        run.transition_to(WorkflowRunState::Running, None)?;
        self.repository.create_run(&run).await?;

        // 5. Persist canonical Mission aggregate
        let mut mission =
            Mission::new(lowered.mission_id, lowered.candidate_plan.objective.clone());
        mission.workspace_root = request.workspace_root.clone();
        mission.mode = match request.mode {
            WorkflowMode::Autonomous => AutonomyMode::Autonomous,
            WorkflowMode::Interactive => AutonomyMode::Assisted,
            WorkflowMode::Standard => AutonomyMode::Safe,
        };
        mission
            .constraints
            .push(format!("workflow_run_id:{}", run.id));
        mission
            .constraints
            .push(format!("workflow_definition_id:{}", compiled.definition.id));
        for step in &compiled.definition.steps {
            mission.constraints.push(format!("step_key:{}", step.key));
        }

        if let Some(repo) = deps.mission_repo() {
            repo.insert(&mission)
                .await
                .map_err(|e| WorkflowError::MissionCreationFailed {
                    step_key: "root".to_string(),
                    reason: e.to_string(),
                })?;
        }

        // 6. Materialize plan in SchedulerEngine
        deps.scheduler()
            .materialize_plan(lowered.mission_id, &lowered.candidate_plan)
            .await
            .map_err(|e| WorkflowError::TaskSubmissionFailed {
                step_key: "root".to_string(),
                reason: e.to_string(),
            })?;

        // Propagate step recovery strategies to tasks table
        if let Some(pool) = self.repository.pool() {
            for step in &compiled.definition.steps {
                if let Some(candidate_key) = lowered.step_task_keys.get(&step.key) {
                    let strat = step
                        .recovery_strategy
                        .as_ref()
                        .unwrap_or(&compiled.definition.default_recovery_strategy);
                    let max_retries: i64 = match strat {
                        crate::workflow::definition::RecoveryStrategy::Retry { max_retries } => {
                            max_retries.saturating_sub(1) as i64
                        }
                        crate::workflow::definition::RecoveryStrategy::Fail => 0,
                        crate::workflow::definition::RecoveryStrategy::Replan => 0,
                        crate::workflow::definition::RecoveryStrategy::AskOperator => 0,
                        crate::workflow::definition::RecoveryStrategy::Skip => 0,
                    };
                    let task_repo =
                        crate::persistence::sqlite::repositories::SqliteTaskRepository::new(
                            pool.clone(),
                        );
                    let _ = task_repo
                        .update_max_retries_by_candidate_key(
                            lowered.mission_id,
                            candidate_key.as_str(),
                            max_retries as u32,
                        )
                        .await;
                }
            }
        }

        // 7. Materialize step runs in Pending status linked to canonical mission_id
        for step_def in &compiled.definition.steps {
            let mut step_run = WorkflowStepRun::new(run.id, step_def.key.clone());
            step_run.mission_id = Some(lowered.mission_id);
            self.repository.create_step_run(&step_run).await?;
        }

        // 8. Register cancellation token
        let token = CancellationToken::new();
        self.active_cancellation_tokens
            .write()
            .await
            .insert(run.id, token.clone());

        // 9. Emit WorkflowStarted event
        self.emit_event(
            0,
            Some(lowered.mission_id),
            EventType::WorkflowStarted {
                workflow_run_id: run.id,
                definition_id: compiled.definition.id.clone(),
            },
        )
        .await;

        // 10. Canonical AutonomyController execution loop
        let bus = self
            .event_bus
            .clone()
            .unwrap_or_else(|| Arc::new(crate::events::bus::BroadcastEventBus::new(100)));

        let mut controller =
            AutonomyController::new(lowered.mission_id, mission.mode, deps.clone(), bus, token)
                .with_mission_objective(lowered.candidate_plan.objective.clone())
                .with_workspace_root(request.workspace_root.clone())
                .with_policy_role(crate::state_machine::agent::AgentRole::implementer());

        let halt_outcome = controller
            .run()
            .await
            .map_err(|e| WorkflowError::ExecutionFailed {
                step_key: "root".to_string(),
                reason: format!("AutonomyController failed: {}", e),
            })?;

        // 11. Synchronize workflow and step run state from canonical controller outcome
        self.sync_workflow_from_controller_halt(
            run.id,
            lowered.mission_id,
            compiled,
            halt_outcome,
            deps,
        )
        .await?;

        let updated_run = self
            .repository
            .get_run(run.id)
            .await?
            .ok_or(WorkflowError::UnknownWorkflow(run.id))?;

        Ok(WorkflowRunHandle {
            run_id: run.id,
            definition_id: compiled.definition.id.clone(),
            definition_version: compiled.definition.version,
            status: updated_run.status,
        })
    }

    /// Execute a workflow through the canonical AutonomyController closed loop (AD-003)
    /// and return an execution snapshot.
    pub async fn execute_autonomously(
        &self,
        compiled: &CompiledWorkflow,
        request: WorkflowStartRequest,
    ) -> Result<WorkflowExecutionSnapshot, WorkflowError> {
        let handle = self
            .start_workflow(compiled, request.with_mode(WorkflowMode::Autonomous))
            .await?;
        self.inspect_workflow(handle.run_id).await
    }

    /// Advance or resume an existing workflow run via canonical `AutonomyController`.
    ///
    /// Does NOT independently compute ready steps or execute local waves.
    /// Resumes the canonical controller lifecycle for the associated `MissionId`.
    pub async fn advance_workflow(
        &self,
        run_id: WorkflowRunId,
        compiled: &CompiledWorkflow,
    ) -> Result<WorkflowRunState, WorkflowError> {
        let deps = self
            .dependencies
            .as_ref()
            .ok_or_else(|| WorkflowError::ExecutionFailed {
                step_key: "root".to_string(),
                reason: "advance_workflow requires configured ControllerDependencies".to_string(),
            })?;

        let mut run = self
            .repository
            .get_run(run_id)
            .await?
            .ok_or(WorkflowError::UnknownWorkflow(run_id))?;

        if run.status.is_terminal() {
            return Ok(run.status);
        }

        let step_runs = self.repository.list_step_runs(run_id).await?;
        let (mission_id, is_new) = if let Some(mid) = step_runs.iter().find_map(|s| s.mission_id) {
            (mid, false)
        } else {
            let mid = MissionId::new();
            let mut mission = Mission::new(mid, compiled.definition.name.clone());
            mission.workspace_root = run.workspace_root.clone();
            mission.mode = match run.mode {
                WorkflowMode::Autonomous => AutonomyMode::Autonomous,
                WorkflowMode::Interactive => AutonomyMode::Assisted,
                WorkflowMode::Standard => AutonomyMode::Safe,
            };
            mission
                .constraints
                .push(format!("workflow_run_id:{}", run.id));
            mission
                .constraints
                .push(format!("workflow_definition_id:{}", compiled.definition.id));
            for step in &compiled.definition.steps {
                mission.constraints.push(format!("step_key:{}", step.key));
            }
            if let Some(repo) = deps.mission_repo() {
                // Durable mission scope is REQUIRED: step runs reference this
                // mission. An insert failure aborts honestly instead of
                // running under a phantom mission identity.
                repo.insert(&mission)
                    .await
                    .map_err(|e| WorkflowError::ExecutionFailed {
                        step_key: "root".to_string(),
                        reason: format!("workflow mission persistence failed: {e}"),
                    })?;
            }
            for step_run in &step_runs {
                let mut updated = step_run.clone();
                updated.mission_id = Some(mid);
                let _ = self.repository.update_step_run(&updated).await;
            }
            (mid, true)
        };

        // Determine which steps are approved/completed
        let approved_keys: std::collections::HashSet<String> = step_runs
            .iter()
            .filter(|s| s.status == WorkflowStepState::Completed)
            .map(|s| s.step_key.clone())
            .collect();

        let unblocked = compiled.compute_unblocked_step_keys(&approved_keys);
        let lowered = if unblocked.len() == compiled.definition.steps.len() {
            compiled.lower_subset(mission_id, None, if is_new { 0 } else { 1 })?
        } else {
            compiled.lower_subset(mission_id, Some(&unblocked), if is_new { 0 } else { 1 })?
        };

        // Materialize or Reconcile plan into scheduler
        if is_new {
            deps.scheduler()
                .materialize_plan(mission_id, &lowered.candidate_plan)
                .await
                .map_err(|e| WorkflowError::TaskSubmissionFailed {
                    step_key: "root".to_string(),
                    reason: e.to_string(),
                })?;
        } else if deps
            .scheduler()
            .reconcile_plan(mission_id, &lowered.candidate_plan)
            .await
            .is_err()
        {
            let _ = deps
                .scheduler()
                .materialize_plan(mission_id, &lowered.candidate_plan)
                .await;
        }

        if let Some(pool) = self.repository.pool() {
            for step in &compiled.definition.steps {
                if let Some(candidate_key) = lowered.step_task_keys.get(&step.key) {
                    let strat = step
                        .recovery_strategy
                        .as_ref()
                        .unwrap_or(&compiled.definition.default_recovery_strategy);
                    let max_retries: i64 = match strat {
                        crate::workflow::definition::RecoveryStrategy::Retry { max_retries } => {
                            max_retries.saturating_sub(1) as i64
                        }
                        crate::workflow::definition::RecoveryStrategy::Fail => 0,
                        crate::workflow::definition::RecoveryStrategy::Replan => 0,
                        crate::workflow::definition::RecoveryStrategy::AskOperator => 0,
                        crate::workflow::definition::RecoveryStrategy::Skip => 0,
                    };
                    let task_repo =
                        crate::persistence::sqlite::repositories::SqliteTaskRepository::new(
                            pool.clone(),
                        );
                    let _ = task_repo
                        .update_max_retries_by_candidate_key(
                            mission_id,
                            candidate_key.as_str(),
                            max_retries as u32,
                        )
                        .await;
                }
            }
        }

        if run.status != WorkflowRunState::Running {
            run.transition_to(WorkflowRunState::Running, None)?;
            self.repository.update_run(&run).await?;
        }

        let token = self
            .active_cancellation_tokens
            .read()
            .await
            .get(&run_id)
            .cloned()
            .unwrap_or_default();

        let bus = self
            .event_bus
            .clone()
            .unwrap_or_else(|| Arc::new(crate::events::bus::BroadcastEventBus::new(100)));

        let mut controller = AutonomyController::new(
            mission_id,
            match run.mode {
                WorkflowMode::Autonomous => AutonomyMode::Autonomous,
                WorkflowMode::Interactive => AutonomyMode::Assisted,
                WorkflowMode::Standard => AutonomyMode::Safe,
            },
            deps.clone(),
            bus,
            token,
        )
        .with_mission_objective(compiled.definition.name.clone())
        .with_workspace_root(run.workspace_root.clone())
        .with_policy_role(crate::state_machine::agent::AgentRole::implementer());

        let halt_outcome = controller
            .run()
            .await
            .map_err(|e| WorkflowError::ExecutionFailed {
                step_key: "root".to_string(),
                reason: format!("AutonomyController resume failed: {}", e),
            })?;

        self.sync_workflow_from_controller_halt(run.id, mission_id, compiled, halt_outcome, deps)
            .await?;

        let updated_run = self
            .repository
            .get_run(run.id)
            .await?
            .ok_or(WorkflowError::UnknownWorkflow(run.id))?;

        Ok(updated_run.status)
    }

    /// Synchronize durable workflow and step run records from canonical controller halting state.
    async fn sync_workflow_from_controller_halt(
        &self,
        run_id: WorkflowRunId,
        mission_id: MissionId,
        compiled: &CompiledWorkflow,
        halt_outcome: ControllerHaltReason,
        _deps: &ControllerDependencies,
    ) -> Result<(), WorkflowError> {
        // 1. Sync tasks from canonical database into step runs via task repository
        let task_rows: Vec<(String, String, i64)> = if let Some(pool) = self.repository.pool() {
            let task_repo =
                crate::persistence::sqlite::repositories::SqliteTaskRepository::new(pool.clone());
            task_repo
                .list_candidate_summaries_by_mission(mission_id)
                .await
                .map_err(|e| WorkflowError::PersistenceFailure(e.to_string()))?
        } else {
            Vec::new()
        };

        for (candidate_key, status_str, retry_count) in &task_rows {
            if let Some(mut srun) = self
                .repository
                .get_step_run_by_key(run_id, candidate_key)
                .await?
            {
                let step_def = compiled
                    .definition
                    .steps
                    .iter()
                    .find(|s| &s.key == candidate_key);
                let is_awaiting_approval = step_def
                    .is_some_and(|s| s.quality_gate.require_human_approval)
                    && srun.status != WorkflowStepState::Completed;

                let mapped_state = match status_str.as_str() {
                    "completed" | "succeeded" => {
                        if is_awaiting_approval {
                            WorkflowStepState::AwaitingApproval
                        } else {
                            WorkflowStepState::Completed
                        }
                    }
                    "failed" => {
                        let strat = step_def
                            .and_then(|s| s.recovery_strategy.as_ref())
                            .unwrap_or(&compiled.definition.default_recovery_strategy);
                        if matches!(
                            strat,
                            crate::workflow::definition::RecoveryStrategy::AskOperator
                        ) {
                            WorkflowStepState::AwaitingInput
                        } else {
                            WorkflowStepState::Failed
                        }
                    }
                    "blocked" => WorkflowStepState::Blocked,
                    "in_progress" | "leased" => {
                        if matches!(
                            halt_outcome,
                            ControllerHaltReason::FatalError { .. }
                                | ControllerHaltReason::UnrecoverableState { .. }
                                | ControllerHaltReason::BudgetExhausted { .. }
                                | ControllerHaltReason::MaxCyclesExceeded { .. }
                                | ControllerHaltReason::LoopDetected { .. }
                        ) {
                            WorkflowStepState::Failed
                        } else {
                            WorkflowStepState::Running
                        }
                    }
                    "cancelled" => WorkflowStepState::Cancelled,
                    "skipped" => WorkflowStepState::Skipped,
                    _ => {
                        if matches!(
                            halt_outcome,
                            ControllerHaltReason::FatalError { .. }
                                | ControllerHaltReason::UnrecoverableState { .. }
                                | ControllerHaltReason::BudgetExhausted { .. }
                        ) && *retry_count > 0
                        {
                            WorkflowStepState::Failed
                        } else {
                            WorkflowStepState::Pending
                        }
                    }
                };

                srun.attempt_count = (retry_count + 1).max(srun.attempt_count as i64) as u32;

                if srun.status != mapped_state && !srun.status.is_terminal() {
                    if srun.status == WorkflowStepState::Pending
                        || srun.status == WorkflowStepState::Blocked
                    {
                        srun.transition_to(WorkflowStepState::Running, None)?;
                    }
                    let halt_msg = match &halt_outcome {
                        ControllerHaltReason::FatalError { failure_class } => {
                            Some(failure_class.clone())
                        }
                        ControllerHaltReason::UnrecoverableState { reason } => Some(reason.clone()),
                        ControllerHaltReason::BudgetExhausted { kind } => {
                            Some(format!("Budget exhausted: {:?}", kind))
                        }
                        ControllerHaltReason::MaxCyclesExceeded { limit } => {
                            Some(format!("Max cycles exceeded: {}", limit))
                        }
                        ControllerHaltReason::LoopDetected { signature } => {
                            Some(format!("Loop detected: {:?}", signature))
                        }
                        _ => None,
                    };
                    srun.transition_to(mapped_state, halt_msg)?;
                    self.repository.update_step_run(&srun).await?;
                }
            }
        }

        // 2. Record produced artifacts
        let mut run = self
            .repository
            .get_run(run_id)
            .await?
            .ok_or(WorkflowError::UnknownWorkflow(run_id))?;

        for step_def in &compiled.definition.steps {
            if let Some(srun) = self
                .repository
                .get_step_run_by_key(run_id, &step_def.key)
                .await?
                && srun.status == WorkflowStepState::Completed
            {
                for out_binding in &step_def.expected_outputs {
                    let full_path = run.workspace_root.join(&out_binding.relative_path);
                    if full_path.exists()
                        && let Ok(bytes) = tokio::fs::read(&full_path).await
                    {
                        let content_hash =
                            crate::persistence::artifacts::compute_artifact_hash(&bytes);
                        let artifact = WorkflowArtifact::new(
                            run.id,
                            srun.id,
                            out_binding.artifact_name.clone(),
                            out_binding.relative_path.clone(),
                            &content_hash,
                            1,
                        );
                        if let Some(ref service) = self.artifact_service {
                            let prov = crate::persistence::artifacts::ArtifactProvenance::for_workflow_step(
                                run.id,
                                srun.id,
                                step_def.role.as_str(),
                            );
                            let ext = out_binding
                                .relative_path
                                .extension()
                                .and_then(|s| s.to_str())
                                .unwrap_or("txt");
                            service
                                .create_and_store_with_id(
                                    artifact.id,
                                    &artifact.name,
                                    &bytes,
                                    ext,
                                    prov,
                                )
                                .await
                                .map_err(|e| WorkflowError::PersistenceFailure(e.to_string()))?;
                        }
                        self.repository.record_artifact(&artifact).await?;
                        self.emit_event(
                            0,
                            srun.mission_id,
                            EventType::WorkflowArtifactRecorded {
                                workflow_run_id: run.id,
                                step_run_id: srun.id,
                                artifact_id: artifact.id,
                                name: artifact.name.clone(),
                                path: artifact.path.to_string_lossy().to_string(),
                            },
                        )
                        .await;
                    }
                }
            }
        }

        // 3. Update workflow run state
        let mut awaiting_approval_step = None;
        let mut awaiting_input_step = None;
        let mut has_failed = false;
        for step_def in &compiled.definition.steps {
            if let Some(s) = self
                .repository
                .get_step_run_by_key(run_id, &step_def.key)
                .await?
            {
                if s.status == WorkflowStepState::AwaitingApproval {
                    awaiting_approval_step = Some((s.id, s.step_key.clone()));
                }
                if s.status == WorkflowStepState::AwaitingInput {
                    awaiting_input_step = Some((s.id, s.step_key.clone()));
                }
                if s.status == WorkflowStepState::Failed {
                    has_failed = true;
                }
            }
        }

        if let Some((step_run_id, step_key)) = awaiting_approval_step {
            run.transition_to(WorkflowRunState::AwaitingApproval, None)?;
            self.repository.update_run(&run).await?;
            self.emit_event(
                0,
                Some(mission_id),
                EventType::WorkflowStepAwaitingApproval {
                    workflow_run_id: run.id,
                    step_run_id,
                    step_key,
                },
            )
            .await;
            return Ok(());
        }

        if let Some((step_run_id, step_key)) = awaiting_input_step {
            run.transition_to(WorkflowRunState::AwaitingInput, None)?;
            self.repository.update_run(&run).await?;
            self.emit_event(
                0,
                Some(mission_id),
                EventType::WorkflowStepAwaitingInput {
                    workflow_run_id: run.id,
                    step_run_id,
                    step_key,
                },
            )
            .await;
            return Ok(());
        }

        let mut all_completed = true;
        for step_def in &compiled.definition.steps {
            let is_step_completed = if let Some(s) = self
                .repository
                .get_step_run_by_key(run_id, &step_def.key)
                .await?
            {
                s.status == WorkflowStepState::Completed
            } else {
                false
            };
            if !is_step_completed {
                all_completed = false;
                break;
            }
        }

        match halt_outcome {
            ControllerHaltReason::MissionCompleted => {
                if has_failed {
                    run.transition_to(
                        WorkflowRunState::Failed,
                        Some("Step execution failed".to_string()),
                    )?;
                } else if all_completed {
                    run.transition_to(WorkflowRunState::Completed, None)?;
                } else {
                    // Steps still pending or in progress; do not mark workflow completed
                }
                self.repository.update_run(&run).await?;
                if all_completed && !has_failed {
                    self.emit_event(
                        0,
                        Some(mission_id),
                        EventType::WorkflowCompleted {
                            workflow_run_id: run.id,
                        },
                    )
                    .await;
                }
            }
            ControllerHaltReason::Cancelled => {
                run.transition_to(
                    WorkflowRunState::Cancelled,
                    Some("Cancelled by controller".to_string()),
                )?;
                self.repository.update_run(&run).await?;
                self.emit_event(
                    0,
                    Some(mission_id),
                    EventType::WorkflowCancelled {
                        workflow_run_id: run.id,
                        reason: "Cancelled by controller".to_string(),
                    },
                )
                .await;
            }
            ControllerHaltReason::Paused => {
                run.transition_to(
                    WorkflowRunState::Blocked,
                    Some("Paused by controller".to_string()),
                )?;
                self.repository.update_run(&run).await?;
                self.emit_event(
                    0,
                    Some(mission_id),
                    EventType::WorkflowPaused {
                        workflow_run_id: run.id,
                        reason: "Paused by controller".to_string(),
                    },
                )
                .await;
            }
            ControllerHaltReason::Blocked => {
                run.transition_to(
                    WorkflowRunState::Blocked,
                    Some("Blocked in controller".to_string()),
                )?;
                self.repository.update_run(&run).await?;
            }
            other => {
                let reason_str = format!("Controller halted: {:?}", other);
                run.transition_to(WorkflowRunState::Failed, Some(reason_str.clone()))?;
                self.repository.update_run(&run).await?;
                self.emit_event(
                    0,
                    Some(mission_id),
                    EventType::WorkflowFailed {
                        workflow_run_id: run.id,
                        reason: reason_str,
                    },
                )
                .await;
            }
        }

        Ok(())
    }

    // =========================================================================
    // Static Wave Analysis (Pure DAG compilation/inspection)
    // =========================================================================

    /// Compute execution waves statically from the workflow DAG structure.
    ///
    /// Wave 0 contains all zero-dependency steps.
    /// Wave 1 contains steps whose dependencies are in Wave 0, etc.
    pub fn compute_execution_waves(
        &self,
        compiled: &CompiledWorkflow,
    ) -> Result<Vec<Vec<String>>, WorkflowError> {
        let keys: Vec<String> = compiled
            .definition
            .steps
            .iter()
            .map(|s| s.key.clone())
            .collect();
        let edges: Vec<(String, String)> = compiled
            .definition
            .steps
            .iter()
            .flat_map(|s| s.depends_on.iter().map(|dep| (dep.clone(), s.key.clone())))
            .collect();

        crate::dag::ops::compute_waves(keys, edges).map_err(|err| match err {
            crate::dag::ops::GraphError::CycleDetected { nodes, .. } => {
                WorkflowError::CycleDetected { cycle: nodes }
            }
            other => WorkflowError::InvalidDefinition(other.to_string()),
        })
    }

    // =========================================================================
    // Human Approval & Interactive Gates
    // =========================================================================

    /// Approve a step in `AwaitingApproval`, transitioning it to `Running` and resuming execution.
    pub async fn approve_step(
        &self,
        run_id: WorkflowRunId,
        step_key: &str,
        compiled: &CompiledWorkflow,
    ) -> Result<WorkflowRunState, WorkflowError> {
        let mut step_run = self
            .repository
            .get_step_run_by_key(run_id, step_key)
            .await?
            .ok_or_else(|| WorkflowError::UnknownStep {
                run_id,
                step_key: step_key.to_string(),
            })?;

        if step_run.status == WorkflowStepState::AwaitingApproval {
            step_run.transition_to(WorkflowStepState::Completed, None)?;
            self.repository.update_step_run(&step_run).await?;
        }

        self.emit_event(
            0,
            step_run.mission_id,
            EventType::WorkflowStepCompleted {
                workflow_run_id: run_id,
                step_run_id: step_run.id,
                step_key: step_key.to_string(),
            },
        )
        .await;

        let mut run = self
            .repository
            .get_run(run_id)
            .await?
            .ok_or(WorkflowError::UnknownWorkflow(run_id))?;

        if run.status == WorkflowRunState::AwaitingApproval {
            run.transition_to(WorkflowRunState::Running, None)?;
            self.repository.update_run(&run).await?;
        }

        self.advance_workflow(run_id, compiled).await
    }

    /// Deny a step in `AwaitingApproval`, transitioning it to `Failed`.
    pub async fn deny_step(
        &self,
        run_id: WorkflowRunId,
        step_key: &str,
        reason: &str,
        _compiled: &CompiledWorkflow,
    ) -> Result<WorkflowRunState, WorkflowError> {
        let mut step_run = self
            .repository
            .get_step_run_by_key(run_id, step_key)
            .await?
            .ok_or_else(|| WorkflowError::UnknownStep {
                run_id,
                step_key: step_key.to_string(),
            })?;

        step_run.transition_to(WorkflowStepState::Failed, Some(reason.to_string()))?;
        self.repository.update_step_run(&step_run).await?;

        let mut run = self
            .repository
            .get_run(run_id)
            .await?
            .ok_or(WorkflowError::UnknownWorkflow(run_id))?;

        run.transition_to(
            WorkflowRunState::Failed,
            Some(format!("Step '{}' denied: {}", step_key, reason)),
        )?;
        self.repository.update_run(&run).await?;

        self.emit_event(
            0,
            step_run.mission_id,
            EventType::WorkflowFailed {
                workflow_run_id: run_id,
                reason: format!("Step '{}' denied: {}", step_key, reason),
            },
        )
        .await;

        Ok(WorkflowRunState::Failed)
    }

    /// Provide input parameters for a step waiting in `AwaitingInput`, resuming execution.
    pub async fn provide_input(
        &self,
        run_id: WorkflowRunId,
        step_key: &str,
        _params: BTreeMap<String, String>,
        compiled: &CompiledWorkflow,
    ) -> Result<WorkflowRunState, WorkflowError> {
        let mut step_run = self
            .repository
            .get_step_run_by_key(run_id, step_key)
            .await?
            .ok_or_else(|| WorkflowError::UnknownStep {
                run_id,
                step_key: step_key.to_string(),
            })?;

        if step_run.status == WorkflowStepState::AwaitingInput {
            step_run.transition_to(WorkflowStepState::Running, None)?;
            self.repository.update_step_run(&step_run).await?;
        }

        let mut run = self
            .repository
            .get_run(run_id)
            .await?
            .ok_or(WorkflowError::UnknownWorkflow(run_id))?;

        if run.status == WorkflowRunState::AwaitingInput {
            run.transition_to(WorkflowRunState::Running, None)?;
            self.repository.update_run(&run).await?;
        }

        self.advance_workflow(run_id, compiled).await
    }

    // =========================================================================
    // Pause, Resume, Cancellation
    // =========================================================================

    /// Pause an active workflow run, preventing newly scheduled steps.
    pub async fn pause_workflow(
        &self,
        run_id: WorkflowRunId,
        reason: &str,
    ) -> Result<(), WorkflowError> {
        let mut run = self
            .repository
            .get_run(run_id)
            .await?
            .ok_or(WorkflowError::UnknownWorkflow(run_id))?;

        if run.status != WorkflowRunState::Running {
            return Err(WorkflowError::InvalidTransition {
                from: run.status.to_string(),
                to: WorkflowRunState::Blocked.to_string(),
                reason: "only running workflows can be paused".to_string(),
            });
        }

        run.transition_to(WorkflowRunState::Blocked, Some(reason.to_string()))?;
        self.repository.update_run(&run).await?;

        self.emit_event(
            0,
            None,
            EventType::WorkflowPaused {
                workflow_run_id: run_id,
                reason: reason.to_string(),
            },
        )
        .await;

        Ok(())
    }

    /// Resume a paused (`Blocked`) workflow run through canonical `AutonomyController`.
    pub async fn resume_workflow(
        &self,
        run_id: WorkflowRunId,
        compiled: &CompiledWorkflow,
    ) -> Result<WorkflowRunState, WorkflowError> {
        let mut run = self
            .repository
            .get_run(run_id)
            .await?
            .ok_or(WorkflowError::UnknownWorkflow(run_id))?;

        if run.status != WorkflowRunState::Blocked {
            return Err(WorkflowError::InvalidTransition {
                from: run.status.to_string(),
                to: WorkflowRunState::Running.to_string(),
                reason: "only blocked/paused workflows can be resumed".to_string(),
            });
        }

        run.transition_to(WorkflowRunState::Running, None)?;
        self.repository.update_run(&run).await?;

        self.emit_event(
            0,
            None,
            EventType::WorkflowResumed {
                workflow_run_id: run_id,
            },
        )
        .await;

        self.advance_workflow(run_id, compiled).await
    }

    /// Cancel a workflow run, stopping pending work and marking state cancelled.
    pub async fn cancel_workflow(
        &self,
        run_id: WorkflowRunId,
        reason: &str,
    ) -> Result<(), WorkflowError> {
        if let Some(token) = self
            .active_cancellation_tokens
            .write()
            .await
            .remove(&run_id)
        {
            token.cancel();
        }

        let mut run = self
            .repository
            .get_run(run_id)
            .await?
            .ok_or(WorkflowError::UnknownWorkflow(run_id))?;

        if !run.status.is_terminal() {
            run.transition_to(WorkflowRunState::Cancelled, Some(reason.to_string()))?;
            self.repository.update_run(&run).await?;
        }

        let step_runs = self.repository.list_step_runs(run_id).await?;
        for mut step in step_runs {
            if !step.status.is_terminal() {
                step.transition_to(
                    WorkflowStepState::Cancelled,
                    Some("workflow cancelled".to_string()),
                )?;
                self.repository.update_step_run(&step).await?;
            }
        }

        self.emit_event(
            0,
            None,
            EventType::WorkflowCancelled {
                workflow_run_id: run_id,
                reason: reason.to_string(),
            },
        )
        .await;

        Ok(())
    }

    // =========================================================================
    // Crash Recovery (Startup Scan)
    // =========================================================================

    /// Reconciles interrupted steps on startup and resumes workflows that were in `Running` state.
    pub async fn recover_incomplete_workflows(
        &self,
        compiled_workflows: &HashMap<String, CompiledWorkflow>,
    ) -> Result<Vec<WorkflowRunId>, WorkflowError> {
        let incomplete_runs = self.repository.list_incomplete_runs().await?;
        let mut recovered_ids = Vec::new();

        for run in incomplete_runs {
            let compiled = match compiled_workflows.get(&run.definition_id) {
                Some(c) => c,
                None => continue,
            };

            let step_runs = self.repository.list_step_runs(run.id).await?;
            for mut step in step_runs {
                if step.status == WorkflowStepState::Running {
                    step.transition_to(
                        WorkflowStepState::Blocked,
                        Some("crash recovery: process terminated in-flight".to_string()),
                    )?;
                    self.repository.update_step_run(&step).await?;
                }
            }

            if run.status == WorkflowRunState::Running {
                let token = CancellationToken::new();
                self.active_cancellation_tokens
                    .write()
                    .await
                    .insert(run.id, token);

                let _ = self.advance_workflow(run.id, compiled).await?;
            }

            recovered_ids.push(run.id);
        }

        Ok(recovered_ids)
    }

    // =========================================================================
    // Cascading Invalidation
    // =========================================================================

    /// Invalidate downstream steps when an upstream step's output changes or is modified.
    ///
    /// Transitive downstream steps are marked `Blocked` (or `Pending`), and their
    /// produced artifacts are marked `Superseded`.
    pub async fn invalidate_downstream_steps(
        &self,
        run_id: WorkflowRunId,
        changed_step_key: &str,
        compiled: &CompiledWorkflow,
    ) -> Result<Vec<String>, WorkflowError> {
        let downstream_keys = self.find_downstream_step_keys(changed_step_key, compiled);
        if downstream_keys.is_empty() {
            return Ok(Vec::new());
        }

        let all_artifacts = self.repository.list_artifacts(run_id).await?;

        for key in &downstream_keys {
            if let Some(mut step_run) = self.repository.get_step_run_by_key(run_id, key).await? {
                if step_run.status != WorkflowStepState::Pending {
                    step_run.status = WorkflowStepState::Blocked;
                    step_run.completed_at = None;
                    step_run.halt_reason = Some(format!(
                        "upstream step '{}' was invalidated",
                        changed_step_key
                    ));
                    self.repository.update_step_run(&step_run).await?;
                }

                for mut art in all_artifacts
                    .iter()
                    .filter(|a| a.step_run_id == step_run.id)
                    .cloned()
                {
                    if art.status == WorkflowArtifactStatus::Valid {
                        art.mark_superseded();
                        self.repository
                            .update_artifact_status(art.id, WorkflowArtifactStatus::Superseded)
                            .await?;
                    }
                }
            }
        }

        Ok(downstream_keys)
    }

    fn find_downstream_step_keys(
        &self,
        source_key: &str,
        compiled: &CompiledWorkflow,
    ) -> Vec<String> {
        let mut dependents: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for step in &compiled.definition.steps {
            for dep in &step.depends_on {
                dependents
                    .entry(dep.as_str())
                    .or_default()
                    .push(step.key.as_str());
            }
        }

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(source_key);

        while let Some(current) = queue.pop_front() {
            if let Some(children) = dependents.get(current) {
                for &child in children {
                    if visited.insert(child.to_string()) {
                        queue.push_back(child);
                    }
                }
            }
        }

        let mut result: Vec<String> = visited.into_iter().collect();
        result.sort();
        result
    }

    /// Validates whether an execution attempt is fresh against the durable step run.
    pub async fn validate_attempt_freshness(
        &self,
        run_id: WorkflowRunId,
        step_key: &str,
        attempt: u32,
    ) -> Result<(), WorkflowError> {
        let step_run = self
            .repository
            .get_step_run_by_key(run_id, step_key)
            .await?
            .ok_or_else(|| WorkflowError::UnknownStep {
                run_id,
                step_key: step_key.to_string(),
            })?;

        if attempt != step_run.attempt_count {
            return Err(WorkflowError::StaleExecution {
                step_key: step_key.to_string(),
                expected_attempt: step_run.attempt_count,
                received_attempt: attempt,
            });
        }

        Ok(())
    }

    // =========================================================================
    // Inspection & Observability
    // =========================================================================

    /// Inspect a workflow run and return a complete execution snapshot.
    pub async fn inspect_workflow(
        &self,
        run_id: WorkflowRunId,
    ) -> Result<WorkflowExecutionSnapshot, WorkflowError> {
        let run = self
            .repository
            .get_run(run_id)
            .await?
            .ok_or(WorkflowError::UnknownWorkflow(run_id))?;

        let step_runs = self.repository.list_step_runs(run_id).await?;
        let artifacts = self.repository.list_artifacts(run_id).await?;

        let mut completed_keys = std::collections::HashSet::new();
        for s in &step_runs {
            if s.status == WorkflowStepState::Completed {
                completed_keys.insert(s.step_key.clone());
            }
        }

        let plan_path = run
            .workspace_root
            .join(".planning")
            .join("projections")
            .join("plan.json");
        let task_deps: std::collections::HashMap<String, Vec<String>> = if let Ok(content) =
            std::fs::read_to_string(&plan_path)
        {
            if let Ok(plan) = serde_json::from_str::<crate::kernel::plan::CandidatePlan>(&content) {
                plan.tasks
                    .into_iter()
                    .map(|t| {
                        (
                            t.id.as_str().to_string(),
                            t.depends_on
                                .into_iter()
                                .map(|d| d.as_str().to_string())
                                .collect(),
                        )
                    })
                    .collect()
            } else {
                std::collections::HashMap::new()
            }
        } else {
            std::collections::HashMap::new()
        };

        let mut ready = Vec::new();
        let mut blocked = Vec::new();

        for s in &step_runs {
            match s.status {
                WorkflowStepState::Pending => {
                    if let Some(deps) = task_deps.get(&s.step_key) {
                        let all_deps_satisfied = deps.iter().all(|d| completed_keys.contains(d));
                        if all_deps_satisfied {
                            ready.push(s.step_key.clone());
                        } else {
                            blocked.push(s.step_key.clone());
                        }
                    } else {
                        ready.push(s.step_key.clone());
                    }
                }
                WorkflowStepState::Blocked => blocked.push(s.step_key.clone()),
                _ => {}
            }
        }

        Ok(WorkflowExecutionSnapshot {
            run,
            step_runs,
            artifacts,
            ready_step_keys: ready,
            blocked_step_keys: blocked,
        })
    }

    // =========================================================================
    // Helper Methods
    // =========================================================================

    async fn emit_event(
        &self,
        sequence: u64,
        mission_id: Option<MissionId>,
        event_type: EventType,
    ) {
        if let Some(ref bus) = self.event_bus {
            let env = EventEnvelope::new(
                sequence,
                mission_id,
                None,
                "workflow_engine".to_string(),
                event_type,
            );
            let _ = bus.publish(env).await;
        }
    }
}
