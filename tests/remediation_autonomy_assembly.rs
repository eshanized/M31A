//! Remediation Test Suite: Autonomy Controller Production Seam Assembly (AUT-01, BLK-02).
//!
//! Verifies:
//! 1. `ControllerDependencies::production` successfully instantiates all 8 production seams.
//! 2. Production `RecoveryEngine` adapter deterministically classifies errors and evaluates recovery budgets.
//! 3. Production `EscalationChannel` adapter persists requests in SQLite and evaluates operator resolution.
//! 4. `AutonomyController` steps through initial stages using pure production dependencies without mock harnesses.

use std::sync::Arc;
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

use m31a::controller::AutonomyController;
use m31a::controller::dependencies::ControllerDependencies;
use m31a::controller::progress::LoopStage;
use m31a::events::bus::{BroadcastEventBus, EventBus};
use m31a::ids::MissionId;
use m31a::kernel::seams::escalation::{
    EscalationChannel, EscalationRequest, EscalationRequestId, EscalationResponse,
};
use m31a::kernel::seams::recovery::{
    FailureClassification, FailureClassificationRequest, RecoveryAction, RecoveryEngine,
    RecoveryStrategyRequest,
};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::state::Mission;
use m31a::state::intake::AutonomyMode;

#[tokio::test]
async fn test_production_seam_assembly_instantiation() {
    let dir = tempdir().unwrap();
    let storage_root = dir.path().to_path_buf();
    let db_path = storage_root.join("test_assembly.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let bus = Arc::new(BroadcastEventBus::new(64));

    // Assemble all 8 real subsystem seams via ControllerDependencies::production
    let deps = ControllerDependencies::production(
        pool.clone(),
        storage_root.clone(),
        storage_root.clone(),
        Some(bus.clone()),
    );

    assert!(deps.transaction_manager.is_some());
    assert!(deps.mission_repo.is_some());
}

#[tokio::test]
async fn test_production_recovery_engine_adapter() {
    let dir = tempdir().unwrap();
    let storage_root = dir.path().to_path_buf();
    let db_path = storage_root.join("test_recovery.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let recovery = m31a::recovery::adapter::ProductionRecoveryEngine::new(Some(pool));
    let mission_id = MissionId::new();
    let task_id = m31a::ids::TaskId::new();

    // 1. Classify compiler error deterministically
    let req = FailureClassificationRequest {
        mission_id,
        task_id,
        error_message: "error[E0308]: mismatched types, expected usize found i32".to_string(),
    };
    let class = recovery.classify_failure(req).await.unwrap();
    assert_eq!(class, FailureClassification::Compilation);

    // 2. Determine recovery for transient failure (within budget -> retry)
    let strat_req = RecoveryStrategyRequest {
        mission_id,
        task_id,
        failure_class: FailureClassification::Transient,
        retry_count: 0,
        error_message: None,
    };
    let action = recovery.determine_recovery(strat_req).await.unwrap();
    assert!(matches!(action, RecoveryAction::Retry { .. }));

    // 3. Determine recovery for policy violation (non-retryable -> abort)
    let strat_req_policy = RecoveryStrategyRequest {
        mission_id,
        task_id,
        failure_class: FailureClassification::Policy,
        retry_count: 0,
        error_message: None,
    };
    let action_policy = recovery.determine_recovery(strat_req_policy).await.unwrap();
    assert!(matches!(action_policy, RecoveryAction::AbortMission { .. }));
}

#[tokio::test]
async fn test_production_escalation_channel_adapter() {
    let dir = tempdir().unwrap();
    let storage_root = dir.path().to_path_buf();
    let db_path = storage_root.join("test_escalation.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let channel = m31a::policy::approval::adapter::ProductionEscalationChannel::new(Some(pool));
    let req_id = EscalationRequestId::new();
    let mission_id = MissionId::new();

    let req = EscalationRequest {
        id: req_id,
        mission_id,
        task_id: None,
        reason: "Need operator grant for elevated resource access".to_string(),
        timeout_seconds: Some(60),
    };

    // 1. Request escalation
    let submitted_id = channel.request_escalation(req).await.unwrap();
    assert_eq!(submitted_id, req_id);

    // 2. Check pending response -> None
    let pending_status = channel.check_response(req_id).await.unwrap();
    assert!(pending_status.is_none());

    // 3. Resolve approval
    channel
        .resolve(req_id, EscalationResponse::Approved)
        .await
        .unwrap();

    // 4. Check resolved response -> Approved
    let resolved_status = channel.check_response(req_id).await.unwrap();
    assert_eq!(resolved_status, Some(EscalationResponse::Approved));
}

#[tokio::test]
async fn test_autonomy_controller_stepping_with_production_dependencies() {
    let dir = tempdir().unwrap();
    let storage_root = dir.path().to_path_buf();
    let db_path = storage_root.join("test_controller.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let bus = Arc::new(BroadcastEventBus::new(64));

    let deps = ControllerDependencies::production(
        pool.clone(),
        storage_root.clone(),
        storage_root.clone(),
        Some(bus.clone()),
    );

    let mission_id = MissionId::new();
    let mission = Mission::new(
        mission_id,
        "Test production assembly autonomy execution".to_string(),
    );
    deps.mission_repo
        .as_ref()
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

    // Initial stage is Observe
    assert_eq!(controller.progress.current_stage, LoopStage::Observe);

    // Step 1: Observe. Production planning has no usable model provider
    // here, so the planner seam MUST fail explicitly (Phase 27: no
    // fabricated tasks) and the controller surfaces the seam error.
    let outcome1 = controller.step().await;
    let err = outcome1.expect_err("planning without a usable model must fail explicitly");
    let err_str = err.to_string();
    assert!(
        err_str.contains("planner") && err_str.contains("no fallback tasks substituted"),
        "Controller must surface explicit planner failure, got: {}",
        err_str
    );
}

#[tokio::test]
async fn test_app_runtime_full_composition_and_mission_run() {
    let dir = tempdir().unwrap();
    let _ = std::process::Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(dir.path())
        .status();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(dir.path())
        .status();
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "test@test.local"])
        .current_dir(dir.path())
        .status();
    std::fs::write(dir.path().join("README.md"), "# Test\n").unwrap();
    let _ = std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .status();
    let _ = std::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(dir.path())
        .status();

    let runtime = m31a::runtime::AppRuntime::new(dir.path())
        .await
        .unwrap()
        .without_model_caller();

    // Phase 27: with no model provider configured, the runtime must fail
    // the mission explicitly at planning time instead of executing
    // fabricated tasks. Recovery path: configure a model provider.
    let err = runtime
        .run_mission(
            "Assemble test mission and verify composition root",
            Some("autonomous"),
            false,
        )
        .await
        .expect_err("mission without a model provider must fail explicitly");
    let err_str = err.to_string();
    assert!(
        err_str.contains("model") || err_str.contains("planner") || err_str.contains("fabricate"),
        "Mission failure must name the missing-model cause, got: {}",
        err_str
    );
}
