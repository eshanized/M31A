//! Remediation Test Suite: Recovery / Reconciliation Hardening (GAP-03).
//!
//! Verifies:
//! 1. Corrupted/uncompilable workspace edits on tracked files are safely rolled back to clean git HEAD on recovery.
//! 2. Failure diagnostics and error outputs are preserved in `step_history` and injected into subsequent context.
//! 3. Recovery budget is bounded: repeated retries exhaust budget and escalate to Replan/Abort rather than compounding.

use std::fs;
use std::process::Command;
use std::sync::Arc;
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

use m31a::controller::AutonomyController;
use m31a::controller::dependencies::ControllerDependencies;
use m31a::controller::progress::LoopStage;
use m31a::controller::stage::StageOutcome;
use m31a::events::bus::{BroadcastEventBus, EventBus};
use m31a::ids::{MissionId, TaskId};
use m31a::kernel::seams::context::{ContextCompilationRequest, ContextCompiler};
use m31a::kernel::seams::execution::WorkExecutionResult;
use m31a::kernel::seams::recovery::{
    FailureClassification, RecoveryAction, RecoveryEngine, RecoveryStrategyRequest,
};
use m31a::kernel::seams::scheduler::WorkItem;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::recovery::adapter::ProductionRecoveryEngine;
use m31a::state::Mission;
use m31a::state::intake::AutonomyMode;

fn init_git_repo(path: &std::path::Path) {
    Command::new("git")
        .args(["init"])
        .current_dir(path)
        .output()
        .expect("git init failed");
    Command::new("git")
        .args(["config", "user.email", "test@m31a.dev"])
        .current_dir(path)
        .output()
        .expect("git config email failed");
    Command::new("git")
        .args(["config", "user.name", "M31A Test"])
        .current_dir(path)
        .output()
        .expect("git config name failed");

    fs::create_dir_all(path.join("src")).unwrap();
    fs::write(
        path.join("src/lib.rs"),
        "pub fn original_lib() -> &'static str { \"ok\" }\n",
    )
    .unwrap();
    fs::write(
        path.join("src/storage.rs"),
        "pub struct Storage {\n    pub counter: u64,\n}\nimpl Storage {\n    pub fn new() -> Self { Self { counter: 0 } }\n}\n",
    )
    .unwrap();

    Command::new("git")
        .args(["add", "."])
        .current_dir(path)
        .output()
        .expect("git add failed");
    Command::new("git")
        .args(["commit", "-m", "Initial baseline"])
        .current_dir(path)
        .output()
        .expect("git commit failed");
}

#[tokio::test]
async fn test_dirty_tracked_files_rolled_back_on_retry() {
    let dir = tempdir().unwrap();
    let workspace_root = dir.path().to_path_buf();
    init_git_repo(&workspace_root);

    let db_path = workspace_root.join("test_recovery.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let bus = Arc::new(BroadcastEventBus::new(64));

    let deps = ControllerDependencies::production(
        pool.clone(),
        workspace_root.clone(),
        workspace_root.clone(),
        Some(bus.clone()),
    );

    let mission_id = MissionId::new();
    let mission = Mission::new(mission_id, "Test recovery git rollback".to_string());
    deps.mission_repo().unwrap().insert(&mission).await.unwrap();

    let cancel = CancellationToken::new();
    let mut controller = AutonomyController::new(
        mission_id,
        AutonomyMode::Safe,
        deps,
        bus as Arc<dyn EventBus>,
        cancel,
    )
    .with_workspace_root(&workspace_root);

    // Corrupt storage.rs with uncompilable edits
    let corrupted_content = "pub fn corrupted() { SYNTAX ERROR";
    fs::write(workspace_root.join("src/storage.rs"), corrupted_content).unwrap();

    // Verify git status shows modified storage.rs
    let git_status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&workspace_root)
        .output()
        .unwrap();
    let status_str = String::from_utf8_lossy(&git_status.stdout);
    assert!(status_str.contains("src/storage.rs"));

    // Set up active task and failed execution result (simulating compilation failure)
    let task_id = TaskId::new();
    controller.active_task = Some(WorkItem {
        task_id,
        title: "Corrupted Task".to_string(),
        estimated_tokens: 1000,
        required_capabilities: vec!["fs.write".to_string()],
        description: None,
        completion_criteria: Vec::new(),
        requirement_keys: Vec::new(),
        assumptions: Vec::new(),
        verification: None,
    
        prompt_ref: None,});
    controller.last_execution_result = Some(WorkExecutionResult {
        task_id,
        success: false,
        output: "error[E0425]: cannot find value in this scope".to_string(),
        error_detail: Some("error[E0425]: cannot find value in this scope".to_string()),
        token_usage: None,
    });

    // Step ClassifyFailure -> RecoverOrReplan
    controller.progress.current_stage = LoopStage::ClassifyFailure;
    let out = controller.step().await.unwrap();
    assert_eq!(out, StageOutcome::Advance(LoopStage::RecoverOrReplan));
    assert_eq!(
        controller.last_failure_class,
        Some(FailureClassification::Compilation)
    );

    // Diagnostics must be recorded in step_history
    assert_eq!(controller.step_history.len(), 1);
    assert!(
        controller.step_history[0]
            .error
            .as_ref()
            .unwrap()
            .contains("error[E0425]")
    );

    // Step RecoverOrReplan -> must rollback modified tracked files and advance/skip
    let out2 = controller.step().await.unwrap();
    assert_eq!(out2, StageOutcome::SkipTo(LoopStage::Observe));

    // VERIFICATION: storage.rs MUST be restored to clean baseline!
    let restored_content = fs::read_to_string(workspace_root.join("src/storage.rs")).unwrap();
    assert!(
        restored_content.contains("pub struct Storage"),
        "Expected clean restored baseline, got: {}",
        restored_content
    );
    assert!(!restored_content.contains("SYNTAX ERROR"));

    // Verification: reconciliation action recorded in step_history
    assert_eq!(controller.step_history.len(), 2);
    assert_eq!(
        controller.step_history[1].tool_name,
        "workspace_reconciliation"
    );
}

#[tokio::test]
async fn test_corrupted_work_not_compounded_context_updated() {
    let compiler = m31a::context::compiler::ProductionContextCompiler::new();

    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let history = vec![
        m31a::kernel::seams::context::StepRecordDto {
            step_number: 1,
            tool_name: "edit_file".to_string(),
            parameters: serde_json::json!({"path": "src/storage.rs"}),
            success: false,
            output: String::new(),
            error: Some("error[E0308]: mismatched types".to_string()),
        },
        m31a::kernel::seams::context::StepRecordDto {
            step_number: 2,
            tool_name: "workspace_reconciliation".to_string(),
            parameters: serde_json::json!({"action": "git_checkout_head"}),
            success: true,
            output: "Restored modified tracked files to clean git commit baseline".to_string(),
            error: None,
        },
    ];

    let req = ContextCompilationRequest::new(mission_id, task_id, 4096)
        .with_task_objective("Fix storage implementation")
        .with_step_history(history);

    let compiled = compiler.compile_context(req).await.unwrap();

    // Context contains error diagnostic history so agent knows what failed and that files are clean
    assert!(
        compiled
            .system_prompt
            .contains("error[E0308]: mismatched types")
    );
    assert!(
        compiled
            .system_prompt
            .contains("Restored modified tracked files to clean git commit baseline")
    );
}

#[tokio::test]
async fn test_recovery_budget_is_bounded() {
    let recovery = ProductionRecoveryEngine::new(None);
    let mission_id = MissionId::new();
    let task_id = TaskId::new();

    // Turn 0: permitted retry
    let req0 = RecoveryStrategyRequest {
        mission_id,
        task_id,
        failure_class: FailureClassification::Compilation,
        retry_count: 0,
        error_message: None,
    };
    let act0 = recovery.determine_recovery(req0).await.unwrap();
    assert!(matches!(act0, RecoveryAction::Retry { .. }));

    // Turn 1: permitted retry
    let req1 = RecoveryStrategyRequest {
        mission_id,
        task_id,
        failure_class: FailureClassification::Compilation,
        retry_count: 1,
        error_message: None,
    };
    let act1 = recovery.determine_recovery(req1).await.unwrap();
    assert!(matches!(act1, RecoveryAction::Retry { .. }));

    // Exhausted retries (e.g. 5 retries): escalates to Replan or Abort, NOT infinite retry
    let req_exhausted = RecoveryStrategyRequest {
        mission_id,
        task_id,
        failure_class: FailureClassification::Compilation,
        retry_count: 10,
        error_message: None,
    };
    let act_exhausted = recovery.determine_recovery(req_exhausted).await.unwrap();
    assert!(
        matches!(
            act_exhausted,
            RecoveryAction::Replan { .. } | RecoveryAction::AbortMission { .. }
        ),
        "Expected Replan or Abort on exhausted retries, got: {:?}",
        act_exhausted
    );
}
