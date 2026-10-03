//! Remediation Test Suite: Planner -> Dispatcher Seam Hardening (GAP-02).
//!
//! Verifies:
//! 1. `PlanValidator` rejects tasks violating role envelopes (e.g. Researcher + cargo.test).
//! 2. `PlanValidator` accepts tasks with allowed role capabilities (e.g. Researcher + fs.read).
//! 3. `FailureClassifier` deterministically classifies allocation and envelope violations as Architecture.
//! 4. `AutonomyController` gracefully transitions to `ClassifyFailure` upon worker allocation failure without terminating with fatal `SeamError`.

use std::sync::Arc;
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

use m31a::controller::AutonomyController;
use m31a::controller::dependencies::ControllerDependencies;
use m31a::controller::progress::LoopStage;
use m31a::controller::stage::StageOutcome;
use m31a::events::bus::{BroadcastEventBus, EventBus};
use m31a::ids::{AgentId, JobId, MissionId, TaskId};
use m31a::kernel::plan::{
    CandidatePlan, CandidateTask, CandidateTaskKey, CapabilityAccessMode, CapabilityRequirement,
    ResourceEstimate, VerificationStrategy,
};
use m31a::kernel::seams::execution::{
    ExecutionError, WorkExecutionHandle, WorkExecutionRequest, WorkExecutionResult,
    WorkerDispatcher,
};
use m31a::kernel::seams::recovery::{
    FailureClassification, FailureClassificationRequest, RecoveryEngine,
};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::planning::validation::{PlanValidator, ValidationError, ValidationReport};
use m31a::recovery::adapter::ProductionRecoveryEngine;
use m31a::state::Mission;
use m31a::state::intake::AutonomyMode;
use m31a::state_machine::agent::AgentRole;

fn sample_candidate_task(
    id: &str,
    role: AgentRole,
    caps: Vec<CapabilityRequirement>,
) -> CandidateTask {
    CandidateTask {
        id: CandidateTaskKey::new(id),
        objective: format!("Objective for {}", id),
        description: None,
        depends_on: vec![],
        capabilities: caps,
        role,
        verification: VerificationStrategy::Compilation,
        estimates: ResourceEstimate::new(5, 60, 1000, 0.10),
        ..Default::default()
    }
}

#[tokio::test]
async fn test_plan_validator_rejects_forbidden_role_capabilities() {
    let validator = PlanValidator::new();

    // Researcher attempting cargo.test (not allowed by Researcher envelope)
    let bad_task = sample_candidate_task(
        "task-researcher-cargo",
        AgentRole::researcher(),
        vec![CapabilityRequirement::new(
            "cargo.test",
            CapabilityAccessMode::Read,
        )],
    );
    let plan = CandidatePlan::new("plan-bad", "Bad plan", vec![bad_task]);
    let mut report = ValidationReport::new();
    validator.run_capability_pass(&plan, &mut report).await;

    assert!(!report.is_valid());
    let has_conflict = report.errors.iter().any(|e| match e {
        ValidationError::PolicyConflict {
            capability_id,
            reason,
            ..
        } => capability_id == "cargo.test" && reason.contains("capability envelope violation"),
        _ => false,
    });
    assert!(
        has_conflict,
        "Expected PolicyConflict for cargo.test in Researcher role, got: {:?}",
        report.errors
    );
}

#[tokio::test]
async fn test_plan_validator_accepts_allowed_role_capabilities() {
    let validator = PlanValidator::new();

    // Researcher requesting fs.read and repo.read (allowed by Researcher envelope)
    let good_task = sample_candidate_task(
        "task-researcher-read",
        AgentRole::researcher(),
        vec![
            CapabilityRequirement::new("fs.read", CapabilityAccessMode::Read),
            CapabilityRequirement::new("repo.read", CapabilityAccessMode::Read),
        ],
    );
    let plan = CandidatePlan::new("plan-good", "Good plan", vec![good_task]);
    let mut report = ValidationReport::new();
    validator.run_capability_pass(&plan, &mut report).await;

    assert!(
        report.is_valid(),
        "Expected valid report, got errors: {:?}",
        report.errors
    );
}

#[tokio::test]
async fn test_failure_classifier_architecture_on_allocation_and_envelope_failure() {
    let recovery = ProductionRecoveryEngine::new(None);
    let mission_id = MissionId::new();
    let task_id = TaskId::new();

    let req1 = FailureClassificationRequest {
        mission_id,
        task_id,
        error_message: "worker allocation failed: no worker satisfies capability requirements"
            .to_string(),
    };
    let class1 = recovery.classify_failure(req1).await.unwrap();
    assert_eq!(class1, FailureClassification::Architecture);

    let req2 = FailureClassificationRequest {
        mission_id,
        task_id,
        error_message: "forbidden by role envelope: cargo.test not in researcher profile"
            .to_string(),
    };
    let class2 = recovery.classify_failure(req2).await.unwrap();
    assert_eq!(class2, FailureClassification::Architecture);
}

/// Deterministic model stub returning a fixed single-task decomposition,
/// so the allocation-failure flow below exercises the real planner seam
/// (Phase 27: the model proposes; the runtime decides and executes).
struct SingleTaskModel;

#[async_trait::async_trait]
impl m31a::agent::model_policy::ModelCaller for SingleTaskModel {
    async fn call_model(
        &self,
        _context: &str,
    ) -> Result<m31a::agent::model_policy::ModelProposal, String> {
        Ok(m31a::agent::model_policy::ModelProposal::Complete {
            summary: serde_json::json!({
                "tasks": [
                    {
                        "id": "TASK-01",
                        "title": "Survey repository structure",
                        "description": "Read-only survey",
                        "role": "researcher",
                        "depends_on": [],
                        "required_capabilities": ["fs.read"]
                    }
                ]
            })
            .to_string(),
            artifacts: vec![],
        })
    }

    async fn call_model_with_context(
        &self,
        _compiled: &m31a::kernel::seams::context::CompiledContext,
        _cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<m31a::agent::model_policy::ModelProposal, String> {
        self.call_model("").await
    }
}

/// A mock dispatcher that fails worker allocation intentionally.
struct FailingWorkerDispatcher;

#[async_trait::async_trait]
impl WorkerDispatcher for FailingWorkerDispatcher {
    async fn allocate_worker(
        &self,
        _task_id: TaskId,
        _mission_id: MissionId,
        _capabilities: &[String],
    ) -> Result<AgentId, ExecutionError> {
        Err(ExecutionError::AllocationFailed(
            "no worker matching requested capabilities".to_string(),
        ))
    }

    async fn dispatch_work(
        &self,
        _req: WorkExecutionRequest,
    ) -> Result<WorkExecutionHandle, ExecutionError> {
        Err(ExecutionError::DispatchFailed("not reached".into()))
    }

    async fn collect_result(
        &self,
        _handle: &WorkExecutionHandle,
    ) -> Result<WorkExecutionResult, ExecutionError> {
        Err(ExecutionError::ExecutionFailed("not reached".into()))
    }

    async fn cancel_job(&self, _job_id: &JobId) -> bool {
        false
    }
}

#[tokio::test]
async fn test_controller_graceful_transition_on_allocation_failure() {
    let dir = tempdir().unwrap();
    let storage_root = dir.path().to_path_buf();
    let db_path = storage_root.join("test_alloc_fail.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let bus = Arc::new(BroadcastEventBus::new(64));

    let mut deps = ControllerDependencies::production(
        pool.clone(),
        storage_root.clone(),
        storage_root.clone(),
        Some(bus.clone()),
    );

    // Override the dispatcher with our failing dispatcher, and wire the
    // real planner seam to a deterministic model so planning succeeds
    // honestly (no fabricated tasks) before allocation fails.
    deps = deps
        .with_dispatcher(Arc::new(FailingWorkerDispatcher))
        .with_planner(Arc::new(
            m31a::planning::service::PlanServiceImpl::new(storage_root.clone())
                .with_model_caller(Arc::new(SingleTaskModel)),
        ));

    let mission_id = MissionId::new();
    let mission = Mission::new(
        mission_id,
        "Test controller allocation failure handling".to_string(),
    );
    deps.mission_repo()
        .unwrap()
        .insert(&mission)
        .await
        .unwrap();

    let cancel = CancellationToken::new();
    let mut controller = AutonomyController::new(
        mission_id,
        AutonomyMode::Safe,
        deps,
        bus as Arc<dyn EventBus>,
        cancel,
    );

    // Step 1: Observe -> IdentifyReadyWork
    let out1 = controller.step().await.unwrap();
    assert_eq!(out1, StageOutcome::Advance(LoopStage::IdentifyReadyWork));

    // Step 2: IdentifyReadyWork -> ValidatePolicyAndResources
    let out2 = controller.step().await.unwrap();
    assert_eq!(
        out2,
        StageOutcome::Advance(LoopStage::ValidatePolicyAndResources)
    );

    // Step 3: ValidatePolicyAndResources -> AllocateWorkers
    let out3 = controller.step().await.unwrap();
    assert_eq!(out3, StageOutcome::Advance(LoopStage::AllocateWorkers));

    // Step 4: AllocateWorkers -> MUST advance to ClassifyFailure (NOT SeamError!)
    let out4 = controller.step().await;
    assert!(
        out4.is_ok(),
        "Controller step failed unexpectedly: {:?}",
        out4.err()
    );
    let outcome = out4.unwrap();
    assert_eq!(
        outcome,
        StageOutcome::Advance(LoopStage::ClassifyFailure),
        "Expected Advance to ClassifyFailure, got: {:?}",
        outcome
    );

    // Ensure execution result was populated with the failure
    assert!(controller.last_execution_result.is_some());
    let exec_res = controller.last_execution_result.as_ref().unwrap();
    assert!(!exec_res.success);
    assert!(
        exec_res
            .error_detail
            .as_ref()
            .unwrap()
            .contains("worker allocation failed"),
        "Expected error detail to mention worker allocation failed, got: {:?}",
        exec_res.error_detail
    );
}
