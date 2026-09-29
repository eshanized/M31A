//! Integration tests proving authoritative checkpoint creation and restoration (CHK-01, CHK-04, CHK-05).
//!
//! Verifies:
//! 1. Autonomous mission creates a checkpoint through CheckpointManager
//! 2. Checkpoint row contains valid manifest_json
//! 3. Checkpoint row contains valid manifest_hash
//! 4. Manifest hash validates cryptographically
//! 5. Checkpoint can be restored via CheckpointManager
//! 6. Crash recovery scanner recognizes the checkpoint as valid (SafeToResume)
//! 7. Task state is reconciled correctly on restore (running -> pending)
//! 8. Corrupted manifest is rejected by validator, restore, and crash scanner
//! 9. Production path: AppRuntime -> AutonomyController -> CheckpointManager -> SQLite

use chrono::Utc;
use sqlx::Row;
use std::sync::Arc;
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

use m31a::checkpoint::crash_recovery::{CrashRecoveryClassification, StartupCrashRecoveryScanner};
use m31a::checkpoint::integrity::CheckpointIntegrityValidator;
use m31a::checkpoint::manifest::CheckpointManifest;
use m31a::controller::AutonomyController;
use m31a::controller::progress::LoopStage;
use m31a::events::bus::BroadcastEventBus;
use m31a::ids::{CheckpointId, MissionId, TaskId};
use m31a::runtime::AppRuntime;
use m31a::state::intake::AutonomyMode;

#[tokio::test]
async fn test_autonomous_controller_creates_checkpoint_through_checkpoint_manager() {
    let temp = tempdir().unwrap();
    let runtime = AppRuntime::new(temp.path()).await.unwrap();
    let pool = runtime.pool().clone();

    let mission_id = MissionId::new();
    let task_id1 = TaskId::new();
    let task_id2 = TaskId::new();
    let now_str = Utc::now().to_rfc3339();

    // Insert mission
    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind("Authoritative checkpoint integration test")
    .bind("Running")
    .bind(&now_str)
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    // Insert tasks: one running, one pending
    sqlx::query(
        "INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(task_id1.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind("Task 1: In Flight")
    .bind("running")
    .bind(&now_str)
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(task_id2.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind("Task 2: Pending")
    .bind("pending")
    .bind(&now_str)
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    // Instantiate AutonomyController using production runtime dependencies
    let cancel = CancellationToken::new();
    let bus = Arc::new(BroadcastEventBus::new(256));
    let mut controller = AutonomyController::new(
        mission_id,
        AutonomyMode::Autonomous,
        runtime.dependencies().clone(),
        bus,
        cancel,
    )
    .with_workspace_root(temp.path());

    // Advance controller to Checkpoint stage
    controller.progress.current_stage = LoopStage::Checkpoint;

    // Step the controller — should invoke CheckpointManager::create_checkpoint
    let outcome = controller.step().await.expect("step should succeed");
    assert_eq!(
        outcome,
        m31a::controller::StageOutcome::Advance(LoopStage::Observe)
    );

    // 1 & 2 & 3: Verify checkpoint row in SQLite directly
    let row = sqlx::query(
        r#"
        SELECT id, sequence, stage, cycle, manifest_json, manifest_hash, state_summary
        FROM checkpoints
        WHERE mission_id = ?
        ORDER BY cycle DESC, sequence DESC
        LIMIT 1
        "#,
    )
    .bind(mission_id.as_bytes().as_slice())
    .fetch_optional(&pool)
    .await
    .unwrap()
    .expect("checkpoint row must exist in SQLite");

    let cp_id_bytes: Vec<u8> = row.get("id");
    let mut cp_bytes = [0u8; 16];
    cp_bytes.copy_from_slice(&cp_id_bytes);
    let checkpoint_id = CheckpointId::from_bytes(cp_bytes);

    let manifest_json: Option<String> = row.get("manifest_json");
    let manifest_hash: Option<String> = row.get("manifest_hash");

    assert!(manifest_json.is_some(), "manifest_json must not be null");
    assert!(manifest_hash.is_some(), "manifest_hash must not be null");

    let json_str = manifest_json.unwrap();
    let recorded_hash = manifest_hash.unwrap();

    // 4. Manifest hash validates
    let manifest: CheckpointManifest = serde_json::from_str(&json_str)
        .expect("manifest_json must deserialize to CheckpointManifest");
    assert_eq!(manifest.checkpoint_id, checkpoint_id);
    assert_eq!(manifest.mission_id, mission_id);
    assert_eq!(manifest.cycle, 1);

    let computed_hash = manifest.compute_manifest_hash();
    assert_eq!(
        recorded_hash, computed_hash,
        "recorded manifest_hash must match computed hash of manifest_json"
    );

    // Cryptographic validation via CheckpointIntegrityValidator
    let validator = CheckpointIntegrityValidator::new(runtime.artifact_store().clone());
    validator
        .validate_with_recorded_hash(&manifest, Some(&recorded_hash))
        .await
        .expect("manifest integrity validation must pass");

    // 6. Crash recovery scanner recognizes controller checkpoint as SafeToResume
    let scanner = StartupCrashRecoveryScanner::new(
        pool.clone(),
        runtime.artifact_store().clone(),
        temp.path(),
    );
    let scan_res = scanner
        .scan_and_reconcile(mission_id)
        .await
        .expect("scan_and_reconcile must succeed");

    assert_eq!(
        scan_res.classification,
        CrashRecoveryClassification::SafeToResume,
        "classification must be SafeToResume; explanation: {}",
        scan_res.explanation
    );
    assert_eq!(scan_res.checkpoint_id, Some(checkpoint_id));

    // 5 & 7: Checkpoint can be restored and reconciles task states correctly
    let restore_res = runtime
        .checkpoint_manager()
        .restore_checkpoint(checkpoint_id, temp.path())
        .await
        .expect("checkpoint restore must succeed");

    assert_eq!(restore_res.checkpoint_id, checkpoint_id);
    assert_eq!(restore_res.cycle, 1);

    // Running task1 should be rescheduled to pending
    let t1_status: String = sqlx::query_scalar("SELECT status FROM tasks WHERE id = ?")
        .bind(task_id1.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(t1_status.to_lowercase(), "pending");

    // 8. Corrupted manifest is rejected by both restore and crash scanner
    let tampered_hash = "0000000000000000000000000000000000000000000000000000000000000000";
    sqlx::query("UPDATE checkpoints SET manifest_hash = ? WHERE id = ?")
        .bind(tampered_hash)
        .bind(checkpoint_id.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    let tampered_restore = runtime
        .checkpoint_manager()
        .restore_checkpoint(checkpoint_id, temp.path())
        .await;
    assert!(
        tampered_restore.is_err(),
        "restore of tampered checkpoint must fail"
    );

    let tampered_scan = scanner.scan_and_reconcile(mission_id).await.unwrap();
    assert_eq!(
        tampered_scan.classification,
        CrashRecoveryClassification::Corrupt,
        "tampered manifest must be classified as Corrupt by crash scanner"
    );
}

#[tokio::test]
async fn test_app_runtime_to_controller_to_checkpoint_manager_sqlite_pipeline() {
    let temp = tempdir().unwrap();
    let runtime = AppRuntime::new(temp.path()).await.unwrap();
    let pool = runtime.pool().clone();

    // Verify runtime.checkpoint_manager() is available and functional
    let cp_mgr = runtime.checkpoint_manager();
    assert!(cp_mgr.staging_dir().exists() || cp_mgr.staging_dir().parent().unwrap().exists());

    // Verify dependencies has checkpoint_manager wired
    assert!(
        runtime.dependencies().checkpoint_manager.is_some(),
        "runtime dependencies must have checkpoint_manager wired"
    );

    let mission_id = MissionId::new();
    let now_str = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind("Pipeline test")
    .bind("Executing")
    .bind(&now_str)
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    let cancel = CancellationToken::new();
    let bus = Arc::new(BroadcastEventBus::new(256));
    let mut controller = AutonomyController::new(
        mission_id,
        AutonomyMode::Autonomous,
        runtime.dependencies().clone(),
        bus,
        cancel,
    )
    .with_workspace_root(temp.path());

    // Step through Checkpoint stage
    controller.progress.current_stage = LoopStage::Checkpoint;
    let _ = controller.step().await.unwrap();

    // Query SQLite to prove CheckpointManager was invoked and produced row with manifest
    let row = sqlx::query(
        "SELECT id, manifest_json, manifest_hash FROM checkpoints WHERE mission_id = ?",
    )
    .bind(mission_id.as_bytes().as_slice())
    .fetch_one(&pool)
    .await
    .expect("row must exist");

    let m_json: String = row.get("manifest_json");
    let m_hash: String = row.get("manifest_hash");

    assert!(!m_json.is_empty());
    assert_eq!(m_hash.len(), 64);

    let manifest: CheckpointManifest = serde_json::from_str(&m_json).unwrap();
    assert_eq!(manifest.compute_manifest_hash(), m_hash);
}
