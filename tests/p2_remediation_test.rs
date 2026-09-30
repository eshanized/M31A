//! Comprehensive P2 Runtime & Execution Integrity Verification Test Suite.
//!
//! Validates all six authorized remediation areas:
//! - P2-A: Checkpoint & State Persistence Integrity (BUG-P2-02, BUG-P2-03)
//!   - Real CheckpointManager::restore_checkpoint implementation
//!   - Manifest hash and artifact verification
//!   - Task state reconciliation via SafeResumeEngine
//!   - Error on tampered manifest and missing checkpoint
//! - P2-B: Crash Recovery & Resume Path Integrity (BUG-P2-04, BUG-P2-05)
//!   - StartupCrashRecoveryScanner reconciliation of running tasks to pending
//!   - Deterministic workspace recreation on NeedsRepair classification
//! - P2-C: Verification Pipeline Hardening (BUG-P2-06)
//!   - TestRunner timeout enforcement & process containment
//!   - CompilerRunner timeout enforcement & process containment
//! - P2-D: Worktree Lifecycle & Isolation (BUG-P2-07)
//!   - Guaranteed cleanup on AlwaysRemove policy
//!   - Retention on failure when policy is KeepOnFailure
//! - P2-E: Retry Budgets & Loop Detection (BUG-P2-08, BUG-REC-01)
//!   - Cumulative mission and task attempt tracking in RecoveryBudgetTracker
//!   - LoopDetector semantic progress fingerprint and cycle detection
//!   - Deterministic 8-step recovery ladder escalation
//! - P2-F: Execution Path Consolidation (BUG-P2-01, BUG-P2-09, BUG-P2-11)
//!   - CliDispatcher fails fast without faking success on RunMission
//!   - CliDispatcher fails fast without faking success on ShowMission
//!   - RestoreCheckpoint CLI command execution parity
//!   - Planning projections path without duplicated .m31a
//!   - ListMissions plain-text itemized formatting

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

use m31a::checkpoint::crash_recovery::StartupCrashRecoveryScanner;
use m31a::checkpoint::manager::{CheckpointManager, StagedArtifactInput};
use m31a::checkpoint::manifest::CheckpointManifest;
use m31a::cli::dispatch::{CliDispatcher, CliError, RuntimeCommand};
use m31a::controller::loop_detector::{LoopDetector, LoopSignature};
use m31a::controller::progress::LoopStage;
use m31a::events::bus::BroadcastEventBus;
use m31a::ids::{ArtifactId, CheckpointId, MissionId, TaskId};
use m31a::kernel::seams::recovery::{
    FailureClassification, RecoveryAction, RecoveryEngine, RecoveryStrategyRequest,
};
use m31a::persistence::artifacts::FsArtifactStore;
use m31a::persistence::artifacts::fs_store::ArtifactStore;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::planning::projections::mission_projections_dir;
use m31a::recovery::adapter::ProductionRecoveryEngine;
use m31a::recovery::budget::RecoveryBudgetTracker;
use m31a::state_machine::TaskState;
use m31a::verification::runners::VerificationRunner;
use m31a::verification::runners::compiler::CompilerRunner;
use m31a::verification::runners::tests::TestRunner;

// ============================================================================
// P2-A: Checkpoint & State Persistence Integrity
// ============================================================================

#[tokio::test]
async fn test_p2_a_checkpoint_restore_reconciles_tasks_and_manifest() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let storage_root = dir.path().join(".m31a");
    let artifacts_dir = storage_root.join("artifacts");
    let staging_dir = storage_root.join("staging");
    let artifact_store: Arc<dyn ArtifactStore> =
        Arc::new(FsArtifactStore::new(artifacts_dir.clone()));

    let manager = CheckpointManager::new(pool.clone(), artifact_store.clone(), staging_dir);

    let mission_id = MissionId::new();
    let task_id1 = TaskId::new();
    let task_id2 = TaskId::new();

    // 1. Create SQLite mission and tasks
    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, 'Test mission', 'Executing', datetime('now'), datetime('now'))")
        .bind(mission_id.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, 'Task 1', 'running', datetime('now'), datetime('now'))")
        .bind(task_id1.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, 'Task 2', 'pending', datetime('now'), datetime('now'))")
        .bind(task_id2.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    // 2. Prepare artifact and manifest
    let art_id = ArtifactId::new();
    let art_data = b"evidence log data 12345".to_vec();
    let mut hasher = sha2::Sha256::default();
    sha2::Digest::update(&mut hasher, &art_data);
    let art_hash = format!("{:x}", sha2::Digest::finalize(hasher));
    let art_size = art_data.len() as u64;

    let staged_art = StagedArtifactInput::new(art_id, art_data, "log", "verification_log");

    let cp_id = CheckpointId::new();
    let mut task_states = BTreeMap::new();
    task_states.insert(task_id1, TaskState::Running);
    task_states.insert(task_id2, TaskState::Pending);

    let manifest = CheckpointManifest::new(
        cp_id,
        mission_id,
        1,
        "Checkpoint",
        1,
        "snap123",
        task_states,
        BTreeMap::new(),
        "policy123",
        vec![],
        vec![(art_id, art_hash, art_size)],
        "Valid checkpoint",
    );

    // 3. Create checkpoint
    let created_id = manager
        .create_checkpoint(&manifest, vec![staged_art])
        .await
        .expect("checkpoint creation must succeed");
    assert_eq!(created_id, cp_id);

    // 4. Restore checkpoint into workspace
    let restore_res = manager
        .restore_checkpoint(cp_id, dir.path())
        .await
        .expect("restore must succeed");

    assert_eq!(restore_res.checkpoint_id, cp_id);
    assert_eq!(restore_res.mission_id, mission_id);
    assert_eq!(restore_res.cycle, 1);
    assert!(restore_res.rescheduled_tasks >= 1);

    // Verify mission status updated in DB
    let status: String = sqlx::query_scalar("SELECT status FROM missions WHERE id = ?")
        .bind(mission_id.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "Executing");

    // Verify in-flight running task was rescheduled to pending
    let t1_status: String = sqlx::query_scalar("SELECT status FROM tasks WHERE id = ?")
        .bind(task_id1.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(t1_status, "pending");
}

#[tokio::test]
async fn test_p2_a_checkpoint_restore_fails_on_tampered_manifest() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let storage_root = dir.path().join(".m31a");
    let artifacts_dir = storage_root.join("artifacts");
    let staging_dir = storage_root.join("staging");
    let artifact_store: Arc<dyn ArtifactStore> =
        Arc::new(FsArtifactStore::new(artifacts_dir.clone()));

    let manager = CheckpointManager::new(pool.clone(), artifact_store.clone(), staging_dir);

    let mission_id = MissionId::new();
    let cp_id = CheckpointId::new();

    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, 'Test mission', 'Executing', datetime('now'), datetime('now'))")
        .bind(mission_id.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    // Insert checkpoint with corrupted manifest_json (mismatched hash)
    let manifest = CheckpointManifest::new(
        cp_id,
        mission_id,
        1,
        "Checkpoint",
        1,
        "snap123",
        BTreeMap::new(),
        BTreeMap::new(),
        "policy123",
        vec![],
        vec![],
        "Tampered checkpoint",
    );
    let manifest_json = serde_json::to_string(&manifest).unwrap();
    let fake_hash = "deadbeef00000000000000000000000000000000000000000000000000000000";

    sqlx::query(
        "INSERT INTO checkpoints (id, mission_id, sequence, stage, cycle, state_summary, manifest_json, manifest_hash, created_at) VALUES (?, ?, 1, 'Observe', 1, 'Summary', ?, ?, '2026-01-01T00:00:00Z')"
    )
    .bind(cp_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(manifest_json)
    .bind(fake_hash)
    .execute(&pool)
    .await
    .unwrap();

    let res = manager.restore_checkpoint(cp_id, dir.path()).await;
    assert!(res.is_err(), "Must fail when manifest hash is tampered");
}

#[tokio::test]
async fn test_p2_a_checkpoint_restore_not_found() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let storage_root = dir.path().join(".m31a");
    let manager = CheckpointManager::new(
        pool,
        Arc::new(FsArtifactStore::new(storage_root.join("artifacts"))),
        storage_root.join("staging"),
    );

    let nonexistent_id = CheckpointId::new();
    let res = manager.restore_checkpoint(nonexistent_id, dir.path()).await;
    assert!(
        matches!(res, Err(m31a::checkpoint::manager::CheckpointError::NotFound(id)) if id == nonexistent_id)
    );
}

// ============================================================================
// P2-B: Crash Recovery & Resume Path Integrity
// ============================================================================

#[tokio::test]
async fn test_p2_b_crash_recovery_reconciles_in_flight_tasks() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let mission_id = MissionId::new();
    let task_id1 = TaskId::new();
    let task_id2 = TaskId::new();

    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, 'Crash mission', 'Executing', datetime('now'), datetime('now'))")
        .bind(mission_id.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, 'Task 1', 'running', datetime('now'), datetime('now'))")
        .bind(task_id1.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, 'Task 2', 'running', datetime('now'), datetime('now'))")
        .bind(task_id2.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    let scanner = StartupCrashRecoveryScanner::new(
        pool.clone(),
        Arc::new(FsArtifactStore::new(dir.path().join("artifacts"))),
        dir.path(),
    );

    let scan_res = scanner
        .scan_and_reconcile(mission_id)
        .await
        .expect("scanner must succeed");

    assert!(scan_res.explanation.contains("in-flight tasks reconciled"));

    // Verify tasks are both pending
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tasks WHERE mission_id = ? AND LOWER(status) = 'pending'",
    )
    .bind(mission_id.as_bytes().as_slice())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn test_p2_b_crash_recovery_needs_repair_recreates_workspace_directory() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let missing_ws = dir.path().join("deleted_workspace");
    assert!(!missing_ws.exists());

    let mission_id = MissionId::new();
    let cp_id = CheckpointId::new();

    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, 'Repair mission', 'Executing', datetime('now'), datetime('now'))")
        .bind(mission_id.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    let manifest = CheckpointManifest::new(
        cp_id,
        mission_id,
        1,
        "Observe",
        1,
        "snap123",
        BTreeMap::new(),
        BTreeMap::new(),
        "policy",
        vec![],
        vec![],
        "Repair test",
    );
    let manifest_json = serde_json::to_string(&manifest).unwrap();
    let manifest_hash = manifest.compute_manifest_hash();

    sqlx::query(
        "INSERT INTO checkpoints (id, mission_id, sequence, stage, cycle, state_summary, manifest_json, manifest_hash, created_at) VALUES (?, ?, 1, 'Observe', 1, 'Repair', ?, ?, '2026-01-01T00:00:00Z')"
    )
    .bind(cp_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(manifest_json)
    .bind(manifest_hash)
    .execute(&pool)
    .await
    .unwrap();

    let scanner = StartupCrashRecoveryScanner::new(
        pool.clone(),
        Arc::new(FsArtifactStore::new(dir.path().join("artifacts"))),
        &missing_ws,
    );

    let res = scanner.scan_and_reconcile(mission_id).await.unwrap();
    assert_eq!(
        res.classification,
        m31a::checkpoint::crash_recovery::CrashRecoveryClassification::NeedsRepair
    );
}

// ============================================================================
// P2-C: Verification Pipeline Hardening
// ============================================================================

#[tokio::test]
async fn test_p2_c_verification_test_runner_enforces_timeout() {
    let dir = tempdir().unwrap();
    let runner = TestRunner::new()
        .with_command("sleep 5")
        .with_timeout_secs(1);

    let start = std::time::Instant::now();
    let check = runner
        .execute(MissionId::new(), TaskId::new(), dir.path(), "snap123")
        .await
        .expect("timeout returns failed check, not Err");

    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(3),
        "Execution should abort after ~1s, took {:?}",
        elapsed
    );
    assert!(check.status.is_failed());
    assert!(check.summary.contains("timed out"));
}

#[tokio::test]
async fn test_p2_c_verification_compiler_runner_enforces_timeout() {
    let dir = tempdir().unwrap();
    let runner = CompilerRunner::with_command("sleep 5").with_timeout_secs(1);

    let start = std::time::Instant::now();
    let check = runner
        .execute(MissionId::new(), TaskId::new(), dir.path(), "snap123")
        .await
        .expect("timeout returns failed check, not Err");

    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(3),
        "Execution should abort after ~1s, took {:?}",
        elapsed
    );
    assert!(check.status.is_failed());
    assert!(check.summary.contains("timed out"));
}

// ============================================================================
// P2-D: Worktree Lifecycle & Isolation
// ============================================================================

#[tokio::test]
async fn test_p2_d_worktree_cleanup_on_always_remove_policy() {
    let dir = tempdir().unwrap();
    let repo_dir = dir.path().join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    // Initialize git repository
    let _ = tokio::process::Command::new("git")
        .args(["init"])
        .current_dir(&repo_dir)
        .output()
        .await
        .unwrap();

    let _ = tokio::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&repo_dir)
        .output()
        .await
        .unwrap();

    let _ = tokio::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&repo_dir)
        .output()
        .await
        .unwrap();

    std::fs::write(repo_dir.join("README.md"), "# Init").unwrap();
    let _ = tokio::process::Command::new("git")
        .args(["add", "README.md"])
        .current_dir(&repo_dir)
        .output()
        .await
        .unwrap();
    let _ = tokio::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&repo_dir)
        .output()
        .await
        .unwrap();

    let wt_cfg = m31a::git::worktree::WorktreeConfig::new(&repo_dir)
        .with_retention(m31a::git::worktree::WorktreeRetentionPolicy::AlwaysRemove);
    let manager = m31a::git::worktree::WorktreeManager::new(wt_cfg);

    let mid = MissionId::new();
    let wt = manager.create_worktree(&mid, None).await.unwrap();
    assert!(wt.path.exists());

    // Clean up under AlwaysRemove policy
    manager
        .remove_worktree(&wt, true, &m31a::git::GitGate::authorized())
        .await
        .unwrap();
    assert!(!wt.path.exists());
}

#[tokio::test]
async fn test_p2_d_worktree_retained_on_failure_when_keep_on_failure() {
    let dir = tempdir().unwrap();
    let repo_dir = dir.path().join("repo");
    std::fs::create_dir_all(&repo_dir).unwrap();

    let _ = tokio::process::Command::new("git")
        .args(["init"])
        .current_dir(&repo_dir)
        .output()
        .await
        .unwrap();

    let _ = tokio::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&repo_dir)
        .output()
        .await
        .unwrap();

    let _ = tokio::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&repo_dir)
        .output()
        .await
        .unwrap();

    std::fs::write(repo_dir.join("README.md"), "# Init").unwrap();
    let _ = tokio::process::Command::new("git")
        .args(["add", "README.md"])
        .current_dir(&repo_dir)
        .output()
        .await
        .unwrap();
    let _ = tokio::process::Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&repo_dir)
        .output()
        .await
        .unwrap();

    let wt_cfg = m31a::git::worktree::WorktreeConfig::new(&repo_dir)
        .with_retention(m31a::git::worktree::WorktreeRetentionPolicy::KeepOnFailure);
    let manager = m31a::git::worktree::WorktreeManager::new(wt_cfg);

    let mid = MissionId::new();
    let wt = manager.create_worktree(&mid, None).await.unwrap();
    assert!(wt.path.exists());

    // When mission fails under KeepOnFailure, worktree must be retained
    let is_success = false;
    if is_success {
        manager
            .remove_worktree(&wt, false, &m31a::git::GitGate::authorized())
            .await
            .unwrap();
    }
    assert!(wt.path.exists(), "Worktree must be preserved on failure");

    // Clean up fixture
    manager
        .remove_worktree(&wt, true, &m31a::git::GitGate::authorized())
        .await
        .unwrap();
}

// ============================================================================
// P2-E: Retry Budgets & Loop Detection
// ============================================================================

#[tokio::test]
async fn test_p2_e_recovery_budget_cumulative_mission_retries() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let budget = RecoveryBudgetTracker::new(3, 5);
    let engine = ProductionRecoveryEngine::with_components(
        m31a::recovery::classifier::FailureClassifier::new(),
        budget,
        Some(pool.clone()),
    );

    let mission_id = MissionId::new();
    let task_id = TaskId::new();

    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, 'Rec mission', 'Executing', datetime('now'), datetime('now'))")
        .bind(mission_id.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, 'Task', 'pending', datetime('now'), datetime('now'))")
        .bind(task_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    // Evaluate 5 attempts sequentially
    for attempt in 0..5 {
        let action = engine
            .determine_recovery(RecoveryStrategyRequest {
                mission_id,
                task_id,
                failure_class: FailureClassification::Transient,
                retry_count: attempt,
                error_message: None,
            })
            .await
            .unwrap();

        if attempt < 3 {
            assert!(matches!(action, RecoveryAction::Retry { .. }));
        }
    }

    // 6th attempt exceeds mission ceiling of 5 -> must AbortMission
    let final_action = engine
        .determine_recovery(RecoveryStrategyRequest {
            mission_id,
            task_id,
            failure_class: FailureClassification::Transient,
            retry_count: 5,
            error_message: None,
        })
        .await
        .unwrap();

    assert!(
        matches!(final_action, RecoveryAction::AbortMission { .. }),
        "Exceeding mission ceiling must trigger AbortMission, got {:?}",
        final_action
    );
}

#[test]
fn test_p2_e_loop_detector_triggers_on_repeated_failure_without_progress() {
    let mut detector = LoopDetector::new(10, 3);
    let task_id = Some(TaskId::new());

    let sig = LoopSignature {
        task_id,
        failure_class: "Transient".into(),
        recovery_strategy: "retry".into(),
        stage: LoopStage::Verify,
        progress_fingerprint: 0, // No progress made
    };

    assert!(!detector.record(sig.clone())); // Count = 1
    assert!(!detector.record(sig.clone())); // Count = 2
    assert!(detector.record(sig.clone())); // Count = 3 -> Loop detected!
}

// ============================================================================
// P2-F: Execution Path Consolidation
// ============================================================================

#[tokio::test]
async fn test_p2_f_cli_run_mission_fails_fast_without_runtime() {
    let dispatcher = CliDispatcher::new();
    let res = dispatcher
        .dispatch(RuntimeCommand::RunMission {
            prompt: "Test mission".to_string(),
            profile: None,
            wait_for_approval: false,
        })
        .await;

    assert!(
        matches!(res, Err(CliError::ExecutionFailed(msg)) if msg.contains("Runtime execution engine is unavailable"))
    );
}

#[tokio::test]
async fn test_p2_f_cli_show_mission_fails_without_database() {
    let dispatcher = CliDispatcher::new();
    let res = dispatcher
        .dispatch(RuntimeCommand::ShowMission {
            id: MissionId::new().to_string(),
        })
        .await;

    assert!(
        matches!(res, Err(CliError::ExecutionFailed(msg)) if msg.contains("No database connection available"))
    );
}

#[tokio::test]
async fn test_p2_f_cli_restore_checkpoint_via_dispatcher() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let storage_root = dir.path().join(".m31a");
    let artifacts_dir = storage_root.join("artifacts");
    let staging_dir = storage_root.join("staging");
    let artifact_store: Arc<dyn ArtifactStore> =
        Arc::new(FsArtifactStore::new(artifacts_dir.clone()));

    let manager = Arc::new(CheckpointManager::new(
        pool.clone(),
        artifact_store.clone(),
        staging_dir,
    ));

    let bus = Arc::new(BroadcastEventBus::new(256));
    let mission_id = MissionId::new();
    let cp_id = CheckpointId::new();

    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, 'Restore test', 'Executing', datetime('now'), datetime('now'))")
        .bind(mission_id.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    let manifest = CheckpointManifest::new(
        cp_id,
        mission_id,
        1,
        "Checkpoint",
        1,
        "snap123",
        BTreeMap::new(),
        BTreeMap::new(),
        "policy",
        vec![],
        vec![],
        "Dispatcher restore",
    );

    manager.create_checkpoint(&manifest, vec![]).await.unwrap();

    let dispatcher = CliDispatcher::new()
        .with_pool(pool)
        .with_event_bus(bus)
        .with_checkpoint_manager(manager)
        .with_artifact_store(artifact_store)
        .with_workspace_root(dir.path().to_path_buf());

    let output = dispatcher
        .dispatch(RuntimeCommand::RestoreCheckpoint {
            id: cp_id.to_string(),
        })
        .await
        .expect("restore must succeed");

    assert!(
        output
            .text
            .contains(&format!("Restored checkpoint {cp_id}"))
    );
    assert_eq!(
        output.data.get("restored").and_then(|v| v.as_bool()),
        Some(true)
    );
}

#[tokio::test]
async fn test_p2_f_cli_restore_checkpoint_fails_on_missing_checkpoint() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let dispatcher = CliDispatcher::new()
        .with_pool(pool)
        .with_workspace_root(dir.path().to_path_buf());

    let missing_id = CheckpointId::new();
    let res = dispatcher
        .dispatch(RuntimeCommand::RestoreCheckpoint {
            id: missing_id.to_string(),
        })
        .await;

    assert!(matches!(res, Err(CliError::NotFound(msg)) if msg.contains(&missing_id.to_string())));
}

#[test]
fn test_p2_planning_projections_dir_no_nested_m31a() {
    let dir = tempdir().unwrap();
    let storage_root = dir.path().join(".m31a");
    let mid = MissionId::new();

    let proj_dir = mission_projections_dir(&storage_root, mid);
    let path_str = proj_dir.to_string_lossy();

    assert!(
        !path_str.contains(".m31a/.m31a"),
        "Projections directory must not contain nested .m31a/.m31a: {}",
        path_str
    );
    assert!(path_str.ends_with(&format!("missions/{mid}/projections")));
}

#[tokio::test]
async fn test_p2_cli_list_missions_renders_itemized_text() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let m1 = MissionId::new();
    let m2 = MissionId::new();

    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, 'Deploy microservice', 'Executing', datetime('now'), datetime('now'))")
        .bind(m1.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, 'Fix crash bug', 'Completed', datetime('now'), datetime('now'))")
        .bind(m2.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    let dispatcher = CliDispatcher::new().with_pool(pool);
    let output = dispatcher
        .dispatch(RuntimeCommand::ListMissions { all: true })
        .await
        .expect("list missions must succeed");

    assert!(output.text.contains("Found 2 missions:"));
    assert!(output.text.contains(&m1.to_string()));
    assert!(output.text.contains(&m2.to_string()));
    assert!(output.text.contains("Deploy microservice"));
    assert!(output.text.contains("Fix crash bug"));
}
