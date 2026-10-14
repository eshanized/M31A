//! October 14, 2026 Audit Findings Integration & Regression Test Suite.
//!
//! Empirically verifies the logical bug remediations:
//! - F-03 & F-12: Pause protocol halts controller at safe boundary, records `Paused` (not `Failed`), emits `MissionPaused`, preserves grants.
//! - F-04: Resuming an eligible paused mission loads run, validates paused status, resets in-flight tasks, and dispatches worker. Rejects non-paused/terminal runs.
//! - F-05: Resuming with corrupt manifest JSON or mismatched hash fails closed with integrity errors.
//! - F-07: Approval coordinator registration ordering eliminates waiter notification race condition.
//! - F-10: SafeResumeEngine hydrates task output files from artifacts, change_provenance, and tasks.result, invalidating tasks when output files are modified or deleted.
//! - F-13: `--wait-for-approval=false` immediately converts ASK to DENY without waiting for operator input.
//! - F-14: Workspace fingerprint fallback for non-Git workspaces produces deterministic hashes that dynamically reflect file modifications.

mod support;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use sha2::{Digest, Sha256};
use tempfile::tempdir;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use m31a::checkpoint::manager::{CheckpointManager, StagedArtifactInput};
use m31a::checkpoint::manifest::CheckpointManifest;
use m31a::checkpoint::resume::SafeResumeEngine;
use m31a::controller::{AutonomyController, ControllerHaltReason, StageOutcome};
use m31a::error::M31AError;
use m31a::events::EventType;
use m31a::events::bus::{EventBus, EventFilter};
use m31a::ids::{
    AgentId, ApprovalRequestId, ArtifactId, CheckId, CheckpointId, ExecutionRunId, MissionId,
    TaskId, ToolCallId,
};
use m31a::kernel::plan::{ResourceEstimate, VerificationStrategy};
use m31a::persistence::artifacts::fs_store::FsArtifactStore;
use m31a::persistence::sqlite::repositories::{SqliteExecutionRunRepository, SqliteTaskRepository};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::policy::approval::coordinator::ApprovalCoordinator;
use m31a::policy::approval::{
    ApprovalAction, ApprovalChannel, ApprovalError, ApprovalExplanationPacket, ApprovalRequest,
};
use m31a::repo::drift::RepositoryBaseline;
use m31a::runtime::AppRuntime;
use m31a::state::PolicyContext;
use m31a::state::intake::AutonomyMode;
use m31a::state_machine::{ExecutionRunState, TaskState};
use m31a::tools::risk::RiskClass;

// ============================================================================
// Test Doubles & Helpers
// ============================================================================

#[derive(Default, Clone)]
struct MockApprovalChannel {
    pub notified: Arc<Mutex<Vec<ApprovalExplanationPacket>>>,
}

#[async_trait]
impl ApprovalChannel for MockApprovalChannel {
    async fn notify_request(
        &self,
        packet: &ApprovalExplanationPacket,
    ) -> Result<(), ApprovalError> {
        let mut list = self.notified.lock().await;
        list.push(packet.clone());
        Ok(())
    }

    async fn poll_response(
        &self,
        _id: ApprovalRequestId,
    ) -> Result<Option<ApprovalAction>, ApprovalError> {
        Ok(None)
    }
}

/// Channel that immediately and synchronously resolves requests upon notification.
/// Verifies that registering the waiter before notification eliminates the race (F-07).
#[derive(Default, Clone)]
struct SynchronousApprovalChannel {
    pub notified: Arc<Mutex<Vec<ApprovalExplanationPacket>>>,
    pub coordinator: Arc<tokio::sync::RwLock<Option<Arc<ApprovalCoordinator>>>>,
}

#[async_trait]
impl ApprovalChannel for SynchronousApprovalChannel {
    async fn notify_request(
        &self,
        packet: &ApprovalExplanationPacket,
    ) -> Result<(), ApprovalError> {
        {
            let mut list = self.notified.lock().await;
            list.push(packet.clone());
        }
        if let Some(coord) = self.coordinator.read().await.as_ref() {
            coord
                .resolve_request(
                    packet.request_id,
                    ApprovalAction::AllowOnce,
                    "sync_operator",
                )
                .await?;
        }
        Ok(())
    }

    async fn poll_response(
        &self,
        _id: ApprovalRequestId,
    ) -> Result<Option<ApprovalAction>, ApprovalError> {
        Ok(None)
    }
}

#[derive(Debug, Default, Clone)]
struct RemediationModelCaller;

#[async_trait]
impl m31a::agent::model_policy::ModelCaller for RemediationModelCaller {
    async fn call_model(&self, context: &str) -> Result<m31a::model::types::ModelProposal, String> {
        if context.contains("planning.decompose")
            || context.contains("Lead Planner")
            || context.contains("Task Decomposition")
        {
            let decomp = r#"{
                "tasks": [
                    {
                        "id": "TASK-01",
                        "title": "Execute mission objective",
                        "description": "Research task",
                        "depends_on": [],
                        "role": "researcher",
                        "required_capabilities": ["repo.read", "fs.read"],
                        "verification": "artifact_inspection"
                    }
                ]
            }"#;
            Ok(m31a::model::types::ModelProposal::Complete {
                summary: decomp.to_string(),
                artifacts: Vec::new(),
            })
        } else {
            Ok(m31a::model::types::ModelProposal::Complete {
                summary: "Task executed successfully".to_string(),
                artifacts: Vec::new(),
            })
        }
    }
}

async fn setup_test_runtime() -> (tempfile::TempDir, Arc<AppRuntime>, sqlx::SqlitePool) {
    let dir = tempdir().expect("tempdir");
    let caller = Arc::new(RemediationModelCaller);
    let runtime = Arc::new(
        AppRuntime::new(dir.path())
            .await
            .expect("AppRuntime::new")
            .with_model_caller(caller),
    );
    let pool = runtime.pool().clone();
    (dir, runtime, pool)
}

fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

// ============================================================================
// F-03 & F-12: Pause Protocol, Controller Halted Reason, State Invariants
// ============================================================================

#[tokio::test]
async fn test_f03_f12_controller_pause_and_runtime_state_preservation() {
    let (_dir, runtime, pool) = setup_test_runtime().await;
    let run_repo = SqliteExecutionRunRepository::new(pool.clone());

    let run_id = ExecutionRunId::new();
    let now = chrono::Utc::now().to_rfc3339();
    let policy_context_json = serde_json::to_string(&PolicyContext::standard()).unwrap();

    // Seed execution run in Executing status
    sqlx::query(
        r#"
        INSERT INTO execution_runs (
            id, objective, status, mode, workspace_root, policy_context, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(run_id.as_bytes().as_slice())
    .bind("Pause test objective")
    .bind("Executing")
    .bind("Safe")
    .bind(runtime.workspace_root().to_string_lossy().as_ref())
    .bind(&policy_context_json)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .expect("insert execution_runs");

    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(run_id.as_bytes().as_slice())
    .bind("Pause test objective")
    .bind("Executing")
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .expect("insert missions");

    // Subscribe to event bus to monitor pause events
    let mut rx = runtime.event_bus().subscribe(EventFilter::all()).await;

    // Pause execution via canonical runtime method
    runtime
        .pause_execution(run_id, "Operator test pause")
        .await
        .expect("pause_execution should succeed");

    // 1. Verify durable state is updated to Paused (F-12: not Failed)
    let run = run_repo
        .get(run_id)
        .await
        .expect("get run")
        .expect("run exists");
    assert_eq!(
        run.status,
        ExecutionRunState::Paused,
        "Durable status must be Paused, never Failed"
    );

    // 2. Verify EventType::MissionPaused was emitted
    let mut received_paused_event = false;
    let deadline = tokio::time::sleep(Duration::from_secs(3));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => break,
            item = rx.next() => {
                if let Some(Ok(envelope)) = item {
                    if let EventType::MissionPaused {
                        mission_id,
                        reason,
                    } = envelope.event_type
                    {
                        if mission_id == run_id && reason == "Operator test pause" {
                            received_paused_event = true;
                            break;
                        }
                    }
                }
            }
        }
    }
    assert!(
        received_paused_event,
        "EventType::MissionPaused must be emitted on pause"
    );

    // 3. Verify idempotency: pausing already-paused run succeeds immediately
    let second_pause = runtime.pause_execution(run_id, "Duplicate pause").await;
    assert!(
        second_pause.is_ok(),
        "Repeated pause on already paused run must succeed idempotently"
    );

    // 4. Verify terminal guard: pausing a Completed run fails closed with validation error
    let comp_id = ExecutionRunId::new();
    sqlx::query(
        r#"
        INSERT INTO execution_runs (
            id, objective, status, mode, workspace_root, policy_context, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(comp_id.as_bytes().as_slice())
    .bind("Completed run")
    .bind("Completed")
    .bind("Safe")
    .bind(runtime.workspace_root().to_string_lossy().as_ref())
    .bind(&policy_context_json)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let err = runtime
        .pause_execution(comp_id, "Pause completed run")
        .await
        .unwrap_err();
    assert!(
        matches!(err, M31AError::ValidationError(_)),
        "Attempting to pause a terminal run must return validation error, got: {err:?}"
    );

    // 5. Verify AutonomyController pauses at stage boundary when pause token is cancelled (F-03)
    let pause_token = CancellationToken::new();
    let cancel_token = CancellationToken::new();
    let mut controller = AutonomyController::new(
        run_id,
        AutonomyMode::Safe,
        runtime.dependencies().clone(),
        runtime.event_bus().clone(),
        cancel_token,
    )
    .with_pause_token(pause_token.clone());

    // Cancel pause token before step
    pause_token.cancel();

    let outcome = controller
        .step()
        .await
        .expect("step with cancelled pause token");
    assert_eq!(
        outcome,
        StageOutcome::Halt(ControllerHaltReason::Paused),
        "Controller step must halt with ControllerHaltReason::Paused when pause token is cancelled"
    );
}

// ============================================================================
// F-04: Resume Canonical Dispatch, Eligibility Validation, Task Reset
// ============================================================================

#[tokio::test]
async fn test_f04_resume_canonical_dispatch_eligibility_and_reset() {
    let (_dir, runtime, pool) = setup_test_runtime().await;
    let run_repo = SqliteExecutionRunRepository::new(pool.clone());
    let task_repo = SqliteTaskRepository::new(pool.clone());

    let run_id = ExecutionRunId::new();
    let task_id = TaskId::new();
    let now = chrono::Utc::now().to_rfc3339();
    let policy_context_json = serde_json::to_string(&PolicyContext::standard()).unwrap();
    let verification_json = serde_json::to_string(&VerificationStrategy::Compilation).unwrap();
    let estimates_json = serde_json::to_string(&ResourceEstimate::new(10, 60, 1000, 0.0)).unwrap();

    // 1. Ineligible run: Executing cannot be resumed
    sqlx::query(
        r#"
        INSERT INTO execution_runs (
            id, objective, status, mode, workspace_root, policy_context, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(run_id.as_bytes().as_slice())
    .bind("Executing run")
    .bind("Executing")
    .bind("Safe")
    .bind(runtime.workspace_root().to_string_lossy().as_ref())
    .bind(&policy_context_json)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let err = runtime.resume_mission(run_id).await.unwrap_err();
    assert!(
        matches!(err, M31AError::ValidationError(_)),
        "Resuming a non-paused run must fail with validation error: {err:?}"
    );

    // 2. Transition run to Paused, insert an in-flight 'running' task
    sqlx::query("UPDATE execution_runs SET status = 'Paused' WHERE id = ?")
        .bind(run_id.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        r#"
        INSERT INTO tasks (
            id, mission_id, title, status, verification, estimates, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(task_id.as_bytes().as_slice())
    .bind(run_id.as_bytes().as_slice())
    .bind("In-flight task before pause")
    .bind("running")
    .bind(&verification_json)
    .bind(&estimates_json)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let mut rx = runtime.event_bus().subscribe(EventFilter::all()).await;

    // Resume the mission
    runtime
        .resume_mission(run_id)
        .await
        .expect("resume_mission on paused run should succeed");

    // Verify task was reset from running to pending
    let updated_task = task_repo.get(task_id).await.unwrap().unwrap();
    assert_eq!(
        updated_task.status,
        TaskState::Pending,
        "In-flight uncheckpointed tasks must be reset to Pending upon resume"
    );

    // Verify EventType::MissionResumed was emitted and run transitioned to Executing or Completed
    let mut received_resumed = false;
    let deadline = tokio::time::sleep(Duration::from_secs(5));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => break,
            item = rx.next() => {
                if let Some(Ok(envelope)) = item {
                    if let EventType::MissionResumed { mission_id } = envelope.event_type {
                        if mission_id == run_id {
                            received_resumed = true;
                            break;
                        }
                    }
                }
            }
        }
    }
    assert!(
        received_resumed,
        "EventType::MissionResumed must be emitted upon canonical resume"
    );

    // Verify run status transitioned to Executing or Completed as worker dispatches
    let updated_run = run_repo.get(run_id).await.unwrap().unwrap();
    assert!(
        updated_run.status == ExecutionRunState::Executing
            || updated_run.status == ExecutionRunState::Completed,
        "Run status must be active Executing or Completed, got: {:?}",
        updated_run.status
    );
}

// ============================================================================
// F-05: Manifest Corruption & Hash Mismatch Fail Closed
// ============================================================================

#[tokio::test]
async fn test_f05_resume_fails_closed_on_manifest_corruption_or_hash_mismatch() {
    let (_dir, runtime, pool) = setup_test_runtime().await;
    let run_repo = SqliteExecutionRunRepository::new(pool.clone());

    let run_id = ExecutionRunId::new();
    let cp_id = CheckpointId::new();
    let now = chrono::Utc::now().to_rfc3339();
    let policy_context_json = serde_json::to_string(&PolicyContext::standard()).unwrap();

    // Seed Paused execution run
    sqlx::query(
        r#"
        INSERT INTO execution_runs (
            id, objective, status, mode, workspace_root, policy_context, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(run_id.as_bytes().as_slice())
    .bind("Integrity resume test")
    .bind("Paused")
    .bind("Safe")
    .bind(runtime.workspace_root().to_string_lossy().as_ref())
    .bind(&policy_context_json)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    // 1. Insert checkpoint with corrupt manifest JSON
    sqlx::query(
        r#"
        INSERT INTO checkpoints (
            id, run_id, mission_id, sequence, stage, cycle, state_summary,
            manifest_json, manifest_hash, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(cp_id.as_bytes().as_slice())
    .bind(run_id.as_bytes().as_slice())
    .bind(run_id.as_bytes().as_slice())
    .bind(1)
    .bind("Observe")
    .bind(1)
    .bind("Summary")
    .bind(r#"{"corrupt": unparseable_json"#)
    .bind("dummy_hash")
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let err = runtime.resume_mission(run_id).await.unwrap_err();
    assert!(
        matches!(err, M31AError::ReconstructionError(_)),
        "Corrupt manifest JSON must fail closed with integrity ReconstructionError: {err:?}"
    );

    // Verify run status did NOT change to Executing
    let run = run_repo.get(run_id).await.unwrap().unwrap();
    assert_eq!(
        run.status,
        ExecutionRunState::Paused,
        "Run state must remain Paused when resume fails integrity validation"
    );

    // 2. Insert valid manifest JSON but with mismatched manifest_hash
    let manifest = CheckpointManifest::new(
        cp_id,
        run_id,
        1,
        "Observe",
        1,
        "test_snapshot_id",
        BTreeMap::new(),
        BTreeMap::new(),
        "policy_context_hash_1",
        Vec::new(),
        Vec::new(),
        "baseline_hash_1".to_string(),
    );
    let valid_manifest_json = serde_json::to_string(&manifest).unwrap();
    let tampered_hash = "0000000000000000000000000000000000000000000000000000000000000000";

    sqlx::query("UPDATE checkpoints SET manifest_json = ?, manifest_hash = ? WHERE id = ?")
        .bind(&valid_manifest_json)
        .bind(tampered_hash)
        .bind(cp_id.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    let err2 = runtime.resume_mission(run_id).await.unwrap_err();
    assert!(
        matches!(err2, M31AError::ReconstructionError(_)),
        "Mismatched manifest hash must fail closed with integrity ReconstructionError: {err2:?}"
    );

    // State remains Paused
    let run2 = run_repo.get(run_id).await.unwrap().unwrap();
    assert_eq!(run2.status, ExecutionRunState::Paused);
}

// ============================================================================
// F-07: Approval Coordinator Ordering Race Elimination
// ============================================================================

#[tokio::test]
async fn test_f07_approval_coordinator_ordering_race_eliminated() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("coord_test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let channel = Arc::new(SynchronousApprovalChannel::default());
    let coordinator = Arc::new(ApprovalCoordinator::new(
        Some(pool.clone()),
        Some(channel.clone()),
    ));
    *channel.coordinator.write().await = Some(coordinator.clone());

    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind("Approval test mission")
    .bind("in_progress")
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(task_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind("Approval test task")
    .bind("running")
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO agents (id, mission_id, role, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(agent_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind("worker")
    .bind("active")
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let req = ApprovalRequest::new(
        mission_id,
        Some(task_id),
        Some(agent_id),
        ToolCallId::new(),
        "execute_code",
        serde_json::json!({ "code": "println!(\"hello\");" }),
        vec!["workspace".to_string()],
        RiskClass::HighRiskMutation,
        Some("rule-code".to_string()),
        "hash-123",
        "Code execution requires human sign-off",
    );

    // Request approval: because waiters is registered before channel notification or event emission,
    // immediate synchronous resolution inside notify_request does not race or deadlock.
    let decision = coordinator
        .request_approval(req, AutonomyMode::Safe, Duration::from_secs(5))
        .await
        .expect("request_approval should resolve cleanly without race or timeout");

    assert_eq!(
        decision,
        ApprovalAction::AllowOnce,
        "Waiter must receive the exact approved decision delivered via synchronous channel notification"
    );
}

// ============================================================================
// F-10: Task Output Files Hydration & Tamper Detection in SafeResumeEngine
// ============================================================================

#[tokio::test]
async fn test_f10_safe_resume_hydrates_task_output_files_and_detects_tampering() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("safe_resume_hydrate.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let workspace_dir = dir.path().join("workspace_resume");
    std::fs::create_dir_all(&workspace_dir).unwrap();

    let artifacts_dir = dir.path().join("artifacts");
    let artifact_store = Arc::new(FsArtifactStore::new(&artifacts_dir));

    let mission_id = MissionId::new();
    let task_1 = TaskId::new();
    let task_2 = TaskId::new();
    let now = chrono::Utc::now().to_rfc3339();

    // Seed mission
    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind("Safe resume hydrate mission")
    .bind("in_progress")
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    // Seed tasks (both succeeded initially)
    for (tid, title) in [(task_1, "Task 1"), (task_2, "Task 2")] {
        sqlx::query(
            "INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(tid.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind(title)
        .bind("succeeded")
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();
    }

    // Write real workspace output files
    let file1_rel = "output_task1.txt";
    let file1_path = workspace_dir.join(file1_rel);
    std::fs::write(&file1_path, b"task 1 output data\n").unwrap();

    let file2_rel = "output_task2.txt";
    let file2_path = workspace_dir.join(file2_rel);
    std::fs::write(&file2_path, b"task 2 output data\n").unwrap();

    // Capture and save repository baseline to database
    let baseline = RepositoryBaseline::capture(&workspace_dir, mission_id, None, None).unwrap();
    baseline.save_to_db(&pool).await.unwrap();

    let mut hasher = Sha256::new();
    let fp_json = serde_json::to_string(&baseline.file_hashes).unwrap();
    hasher.update(fp_json.as_bytes());
    let snapshot_identity = format!("{:x}", hasher.finalize());

    // Create verification artifacts for Task 1 and Task 2
    let art_1 = ArtifactId::new();
    let art_1_data = b"task 1 verification log".to_vec();
    let art_1_hash = compute_sha256(&art_1_data);
    let art_1_size = art_1_data.len() as u64;

    let art_2 = ArtifactId::new();
    let art_2_data = b"task 2 verification log".to_vec();
    let art_2_hash = compute_sha256(&art_2_data);
    let art_2_size = art_2_data.len() as u64;

    // Record verification checks in SQLite for Task 1 and Task 2
    let check_1 = CheckId::new();
    let check_2 = CheckId::new();
    for (check_id, tid, aid) in [(check_1, task_1, art_1), (check_2, task_2, art_2)] {
        sqlx::query(
            r#"
            INSERT INTO verification_checks (id, mission_id, task_id, tier, status, command_or_tool, inputs_normalized, evidence_artifact_id, summary, snapshot_hash, created_at)
            VALUES (?, ?, ?, 3, 'passed', 'cargo test', '{}', ?, 'Check passed', ?, ?)
            "#,
        )
        .bind(check_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind(tid.as_bytes().as_slice())
        .bind(aid.as_bytes().as_slice())
        .bind(&snapshot_identity)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();
    }

    // 1. Insert artifact pointing to file1_rel in artifacts table
    sqlx::query(
        r#"
        INSERT INTO artifacts (
            id, name, logical_path, content_hash, size_bytes, extension,
            mission_id, task_id, created_at
        ) VALUES (?, 'output1', ?, ?, ?, 'txt', ?, ?, ?)
        "#,
    )
    .bind(art_1.as_bytes().as_slice())
    .bind(file1_rel)
    .bind(&art_1_hash)
    .bind(art_1_size as i64)
    .bind(mission_id.as_bytes().as_slice())
    .bind(task_1.as_bytes().as_slice())
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    // 2. Insert change_provenance pointing to file2_rel
    let prop_id = ToolCallId::new();
    sqlx::query(
        r#"
        INSERT INTO change_provenance (
            id, proposal_id, task_id, mission_id, requirement_keys, affected_files,
            affected_symbols, pre_mutation_hash, post_mutation_hash, diff_summary,
            verification_passed, created_at
        ) VALUES (?, ?, ?, ?, '[]', ?, '[]', 'pre', 'post', 'summary', 1, ?)
        "#,
    )
    .bind(ToolCallId::new().as_bytes().as_slice())
    .bind(prop_id.as_bytes().as_slice())
    .bind(task_2.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(serde_json::json!([file2_rel]).to_string())
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    // Create checkpoint manifest capturing both tasks Succeeded
    let cp_id = CheckpointId::new();
    let mut task_states = BTreeMap::new();
    task_states.insert(task_1, TaskState::Succeeded);
    task_states.insert(task_2, TaskState::Succeeded);

    let manifest = CheckpointManifest::new(
        cp_id,
        mission_id,
        1,
        "Observe",
        1,
        &snapshot_identity,
        task_states,
        BTreeMap::new(),
        "policy_context_hash_1",
        vec![check_1, check_2],
        vec![
            (art_1, art_1_hash.clone(), art_1_size),
            (art_2, art_2_hash.clone(), art_2_size),
        ],
        "Cycle 1 completed checkpoint",
    );

    let staging_dir = dir.path().join("staging_resume");
    let manager = CheckpointManager::new(pool.clone(), artifact_store.clone(), &staging_dir);
    manager
        .create_checkpoint(
            &manifest,
            vec![
                StagedArtifactInput::new(art_1, art_1_data, "bin", "verification_log"),
                StagedArtifactInput::new(art_2, art_2_data, "bin", "verification_log"),
            ],
        )
        .await
        .unwrap();

    // Test hydration: SafeResumeEngine must discover both output files from DB
    let mut engine = SafeResumeEngine::new(pool.clone(), artifact_store.clone(), &workspace_dir);
    engine
        .hydrate_task_output_files(mission_id)
        .await
        .expect("hydrate_task_output_files should succeed");

    // Both tasks have valid output files on disk -> preserved
    let report = engine
        .resume_mission(mission_id, &manifest)
        .await
        .expect("resume_mission should succeed");
    assert!(
        report.preserved_tasks.contains(&task_1),
        "Task 1 must be preserved when output file is intact"
    );
    assert!(
        report.preserved_tasks.contains(&task_2),
        "Task 2 must be preserved when output file is intact"
    );
    assert!(report.invalidated_tasks.is_empty());

    // Now tamper with task 1's output file and delete task 2's output file
    std::fs::write(&file1_path, b"TAMPERED output content").unwrap();
    std::fs::remove_file(&file2_path).unwrap();

    // Reset tasks back to succeeded in DB for new resume test
    for tid in [task_1, task_2] {
        sqlx::query("UPDATE tasks SET status = 'succeeded' WHERE id = ?")
            .bind(tid.as_bytes().as_slice())
            .execute(&pool)
            .await
            .unwrap();
    }

    let mut engine_tampered =
        SafeResumeEngine::new(pool.clone(), artifact_store.clone(), &workspace_dir);
    engine_tampered
        .hydrate_task_output_files(mission_id)
        .await
        .unwrap();

    let report_tampered = engine_tampered
        .resume_mission(mission_id, &manifest)
        .await
        .expect("resume_mission should run");

    // Both tasks must now be INVALIDATED due to missing/tampered output files (F-10)
    assert!(
        report_tampered.invalidated_tasks.contains(&task_1),
        "Task 1 must be invalidated when its hydrated output file is modified"
    );
    assert!(
        report_tampered.invalidated_tasks.contains(&task_2),
        "Task 2 must be invalidated when its hydrated output file is missing"
    );
    assert!(report_tampered.preserved_tasks.is_empty());
}

// ============================================================================
// F-13: --wait-for-approval=false Strictly Converts ASK to DENY
// ============================================================================

#[tokio::test]
async fn test_f13_wait_for_approval_false_strictly_converts_ask_to_deny() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("f13_test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let channel = Arc::new(MockApprovalChannel::default());
    let coordinator = ApprovalCoordinator::new(Some(pool.clone()), Some(channel.clone()))
        .with_wait_for_approval(false);

    assert!(
        !coordinator.wait_for_approval(),
        "wait_for_approval must be false"
    );

    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind("F13 approval test")
    .bind("in_progress")
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let req = ApprovalRequest::new(
        mission_id,
        Some(task_id),
        None,
        ToolCallId::new(),
        "rmdir",
        serde_json::json!({ "path": "/test" }),
        vec!["filesystem".to_string()],
        RiskClass::HighRiskMutation,
        Some("rule-fs".to_string()),
        "hash-fs",
        "Directory removal requires approval",
    );

    // In Safe mode with a long timeout, but wait_for_approval == false
    let decision = coordinator
        .request_approval(req, AutonomyMode::Safe, Duration::from_secs(30))
        .await
        .expect("request_approval should return immediately");

    match decision {
        ApprovalAction::Deny { reason } => {
            assert!(
                reason.contains("--wait-for-approval is disabled"),
                "Deny reason must mention --wait-for-approval disabled: {reason}"
            );
        }
        other => panic!("Expected immediate Deny, got: {other:?}"),
    }

    // Now re-enable wait_for_approval
    coordinator.set_wait_for_approval(true);
    assert!(coordinator.wait_for_approval());
}

// ============================================================================
// F-14: Workspace Fingerprint Fallback for Non-Git Workspaces
// ============================================================================

#[tokio::test]
async fn test_f14_non_git_workspace_deterministic_fingerprint() {
    let dir = tempdir().unwrap();
    let ws_path = dir.path().join("non_git_workspace");
    std::fs::create_dir_all(&ws_path).unwrap();

    // Verify .git does not exist
    assert!(
        !ws_path.join(".git").exists(),
        "Workspace must not contain a .git directory"
    );

    // Write initial files
    std::fs::write(ws_path.join("file1.txt"), b"initial content 1").unwrap();
    std::fs::write(ws_path.join("file2.txt"), b"initial content 2").unwrap();

    // Initialize AppRuntime and AgentEngine
    let runtime = AppRuntime::new(&ws_path)
        .await
        .expect("AppRuntime::new on non-git workspace");
    let session_id = m31a::ids::SessionId::new();
    let engine = runtime.create_agent_engine(session_id);

    // 1. Compute fingerprint on non-git workspace
    let fp1 = engine.compute_workspace_fingerprint().await;
    assert_eq!(
        fp1.len(),
        64,
        "Fingerprint must be a 64-character SHA256 hex string, got: {fp1}"
    );
    assert!(
        fp1.chars().all(|c| c.is_ascii_hexdigit()),
        "Fingerprint must be hexadecimal"
    );

    // 2. Determinism: compute again without modifications
    let fp2 = engine.compute_workspace_fingerprint().await;
    assert_eq!(
        fp1, fp2,
        "Fingerprint must be deterministic for identical workspace contents"
    );

    // 3. Dynamic change detection: modify file1.txt
    std::fs::write(ws_path.join("file1.txt"), b"MODIFIED content 1").unwrap();

    let fp3 = engine.compute_workspace_fingerprint().await;
    assert_ne!(
        fp1, fp3,
        "Fingerprint must change when a workspace file is modified"
    );

    // 4. Dynamic change detection: add a new file
    std::fs::write(ws_path.join("file3.txt"), b"brand new file").unwrap();

    let fp4 = engine.compute_workspace_fingerprint().await;
    assert_ne!(
        fp3, fp4,
        "Fingerprint must change when a new file is added to the workspace"
    );
}
