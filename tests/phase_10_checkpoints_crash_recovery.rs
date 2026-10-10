//! Phase 10 Integration Tests: Checkpoint Management, Crash Recovery Scanner & Safe Resume (CHK-01–CHK-05, D-13–D-16).

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;
use tempfile::tempdir;

use m31a::checkpoint::crash_recovery::{CrashRecoveryClassification, StartupCrashRecoveryScanner};
use m31a::checkpoint::integrity::{CheckpointIntegrityError, CheckpointIntegrityValidator};
use m31a::checkpoint::manager::{CheckpointManager, StagedArtifactInput};
use m31a::checkpoint::manifest::CheckpointManifest;
use m31a::checkpoint::resume::SafeResumeEngine;
use m31a::ids::{AgentId, ArtifactId, CheckId, CheckpointId, JobId, MissionId, TaskId};
use m31a::persistence::artifacts::fs_store::{ArtifactStore, FsArtifactStore};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::repo::drift::RepositoryBaseline;
use m31a::state_machine::TaskState;

// ============================================================================
// Test Database & Environment Helper
// ============================================================================

async fn setup_test_env() -> (
    tempfile::TempDir,
    sqlx::SqlitePool,
    Arc<FsArtifactStore>,
    MissionId,
    TaskId,
    AgentId,
) {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_checkpoint.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let artifacts_dir = dir.path().join("artifacts");
    let artifact_store = Arc::new(FsArtifactStore::new(&artifacts_dir));

    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind("Checkpoint and crash recovery integration mission")
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
    .bind("Primary verification task")
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

    (dir, pool, artifact_store, mission_id, task_id, agent_id)
}

fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

// ============================================================================
// Task 10-03-01: Two-Phase Checkpoint Creation & Integrity Validation (CHK-01, CHK-04, D-13)
// ============================================================================

#[tokio::test]
async fn test_two_phase_checkpoint_creation() {
    let (temp_dir, pool, artifact_store, mission_id, task_id, _agent_id) = setup_test_env().await;

    let staging_dir = temp_dir.path().join("staging");
    let manager = CheckpointManager::new(pool.clone(), artifact_store.clone(), &staging_dir);

    let checkpoint_id = CheckpointId::new();
    let artifact_1_id = ArtifactId::new();
    let artifact_1_data =
        b"Test verification execution log content for tier 3 test runner".to_vec();
    let artifact_1_hash = compute_sha256(&artifact_1_data);
    let artifact_1_size = artifact_1_data.len() as u64;

    let artifact_2_id = ArtifactId::new();
    let artifact_2_data = b"Tier 6 review verdict json payload report".to_vec();
    let artifact_2_hash = compute_sha256(&artifact_2_data);
    let artifact_2_size = artifact_2_data.len() as u64;

    let check_id = CheckId::new();
    let job_id = JobId::new();
    let snapshot_identity =
        "a1b2c3d4e5f67890123456789abcdef0123456789abcdef0123456789abcdef0".to_string();
    let policy_context_hash =
        "f0e1d2c3b4a5968778695a4b3c2d1e0ff0e1d2c3b4a5968778695a4b3c2d1e0f".to_string();

    let mut task_states = BTreeMap::new();
    task_states.insert(task_id, TaskState::Succeeded);

    let mut job_states = BTreeMap::new();
    job_states.insert(job_id, "completed".to_string());

    let artifact_references = vec![
        (artifact_1_id, artifact_1_hash.clone(), artifact_1_size),
        (artifact_2_id, artifact_2_hash.clone(), artifact_2_size),
    ];

    let manifest = CheckpointManifest::new(
        checkpoint_id,
        mission_id,
        1,
        "Verify",
        1,
        &snapshot_identity,
        task_states,
        job_states,
        &policy_context_hash,
        vec![check_id],
        artifact_references,
        "Checkpoint cycle 1 verification passed",
    );

    let staged_artifacts = vec![
        StagedArtifactInput::new(
            artifact_1_id,
            artifact_1_data,
            "bin",
            "verification_evidence",
        ),
        StagedArtifactInput::new(artifact_2_id, artifact_2_data, "json", "reviewer_verdict"),
    ];

    // 1. Execute two-phase checkpoint creation
    let created_id = manager
        .create_checkpoint(&manifest, staged_artifacts)
        .await
        .expect("Checkpoint creation should succeed");

    assert_eq!(created_id, checkpoint_id);

    // 2. Validate SQLite database records
    use sqlx::Row;
    let row = sqlx::query(
        r#"
        SELECT sequence, stage, cycle, manifest_hash, manifest_json
        FROM checkpoints
        WHERE id = ?
        "#,
    )
    .bind(checkpoint_id.as_bytes().as_slice())
    .fetch_one(&pool)
    .await
    .expect("Checkpoint row must exist in SQLite");

    let sequence: i64 = row.get("sequence");
    let stage: String = row.get("stage");
    let cycle: i64 = row.get("cycle");
    let manifest_hash: Option<String> = row.get("manifest_hash");

    assert_eq!(sequence, 1);
    assert_eq!(stage, "Verify");
    assert_eq!(cycle, 1);
    assert_eq!(manifest_hash, Some(manifest.compute_manifest_hash()));

    let artifact_rows = sqlx::query(
        r#"
        SELECT artifact_id, sha256_hash, size_bytes, artifact_type
        FROM checkpoint_artifacts
        WHERE checkpoint_id = ?
        ORDER BY size_bytes ASC
        "#,
    )
    .bind(checkpoint_id.as_bytes().as_slice())
    .fetch_all(&pool)
    .await
    .expect("Checkpoint artifact rows must exist");

    assert_eq!(artifact_rows.len(), 2);

    // 3. Validate authoritative FsArtifactStore contents
    let a1_bytes = artifact_store
        .retrieve(artifact_1_id, "bin")
        .await
        .expect("Artifact 1 must exist in store");
    assert_eq!(compute_sha256(&a1_bytes), artifact_1_hash);

    let a2_bytes = artifact_store
        .retrieve(artifact_2_id, "json")
        .await
        .expect("Artifact 2 must exist in store");
    assert_eq!(compute_sha256(&a2_bytes), artifact_2_hash);

    // 4. Validate integrity with CheckpointIntegrityValidator (CHK-04)
    let validator = CheckpointIntegrityValidator::new(artifact_store.clone())
        .with_repo_hash(&snapshot_identity)
        .with_policy_hash(&policy_context_hash);

    validator
        .validate_with_recorded_hash(&manifest, manifest_hash.as_deref())
        .await
        .expect("Integrity validator must pass for pristine checkpoint");

    // 5. Simulate artifact tampering on disk: corrupt artifact 1 in store
    artifact_store
        .store(
            artifact_1_id,
            b"Corrupted and tampered artifact content",
            "bin",
        )
        .await
        .unwrap();

    let tampered_result = validator.validate(&manifest).await;
    match tampered_result {
        Err(CheckpointIntegrityError::ArtifactHashMismatch { artifact_id, .. }) => {
            assert_eq!(artifact_id, artifact_1_id);
        }
        Err(CheckpointIntegrityError::ArtifactSizeMismatch { artifact_id, .. }) => {
            assert_eq!(artifact_id, artifact_1_id);
        }
        other => panic!("Expected hash or size mismatch error, got: {other:?}"),
    }

    // 6. Simulate missing artifact
    let non_existent_artifact_id = ArtifactId::new();
    let mut bad_manifest = manifest.clone();
    bad_manifest.artifact_references =
        vec![(non_existent_artifact_id, "dummyhash".to_string(), 100)];

    let missing_result = validator.validate(&bad_manifest).await;
    match missing_result {
        Err(CheckpointIntegrityError::ArtifactMissing(id)) => {
            assert_eq!(id, non_existent_artifact_id);
        }
        other => panic!("Expected ArtifactMissing error, got: {other:?}"),
    }
}

// ============================================================================
// Task 10-03-02: Staged Crash Recovery Scanner & Process Reconciliation (CHK-02, CHK-03, D-14, D-15)
// ============================================================================

#[tokio::test]
async fn test_crash_recovery_classification() {
    let (temp_dir, pool, artifact_store, mission_id, task_id, agent_id) = setup_test_env().await;

    // Create a mock workspace directory
    let workspace_dir = temp_dir.path().join("workspace");
    std::fs::create_dir_all(&workspace_dir).unwrap();
    std::fs::write(workspace_dir.join("main.rs"), b"fn main() {}\n").unwrap();

    let staging_dir = temp_dir.path().join("staging");
    let manager = CheckpointManager::new(pool.clone(), artifact_store.clone(), &staging_dir);

    // Save baseline
    let baseline = RepositoryBaseline::capture(&workspace_dir, mission_id, None, None).unwrap();
    baseline.save_to_db(&pool).await.unwrap();

    let mut hasher = Sha256::new();
    let fp_json = serde_json::to_string(&baseline.file_hashes).unwrap();
    hasher.update(fp_json.as_bytes());
    let snapshot_identity = format!("{:x}", hasher.finalize());

    // Create a valid checkpoint
    let cp_id = CheckpointId::new();
    let art_id = ArtifactId::new();
    let art_data = b"execution log evidence".to_vec();
    let art_hash = compute_sha256(&art_data);
    let art_size = art_data.len() as u64;

    let mut task_states = BTreeMap::new();
    task_states.insert(task_id, TaskState::Succeeded);
    let manifest = CheckpointManifest::new(
        cp_id,
        mission_id,
        1,
        "Verify",
        1,
        &snapshot_identity,
        task_states,
        BTreeMap::new(),
        "policy_context_hash_val",
        vec![],
        vec![(art_id, art_hash.clone(), art_size)],
        "Initial cycle checkpoint",
    );

    manager
        .create_checkpoint(
            &manifest,
            vec![StagedArtifactInput::new(
                art_id, art_data, "bin", "evidence",
            )],
        )
        .await
        .unwrap();

    let scanner =
        StartupCrashRecoveryScanner::new(pool.clone(), artifact_store.clone(), &workspace_dir);

    // Scenario 1: Clean state with valid checkpoint and no running jobs -> SafeToResume (CHK-02)
    let res1 = scanner.scan_and_reconcile(mission_id).await.unwrap();
    assert_eq!(
        res1.classification,
        CrashRecoveryClassification::SafeToResume
    );
    assert!(res1.classification.is_safe());

    // Scenario 2: Unexplained workspace drift -> Ambiguous (CHK-02, D-14)
    std::fs::write(workspace_dir.join("unexpected_secret.rs"), b"leaked = 1\n").unwrap();
    let res2 = scanner.scan_and_reconcile(mission_id).await.unwrap();
    assert_eq!(res2.classification, CrashRecoveryClassification::Ambiguous);
    assert!(!res2.classification.is_safe()); // CHK-03: never auto-promoted
    std::fs::remove_file(workspace_dir.join("unexpected_secret.rs")).unwrap();

    // Scenario 3: Missing checkpoint artifact -> Corrupt (CHK-02, D-14)
    let art_path = artifact_store.path_for(art_id, "bin");
    let art_backup = std::fs::read(&art_path).unwrap();
    std::fs::remove_file(&art_path).unwrap();

    let res3 = scanner.scan_and_reconcile(mission_id).await.unwrap();
    assert_eq!(res3.classification, CrashRecoveryClassification::Corrupt);
    assert!(!res3.classification.is_safe()); // CHK-03: never auto-promoted
    // Restore artifact file
    std::fs::write(&art_path, &art_backup).unwrap();

    // Scenario 4: Abandoned background job with matching Linux starttime -> killed and spool sealed (D-15)
    let child = tokio::process::Command::new("sleep")
        .arg("60")
        .spawn()
        .expect("Failed to spawn background child process");
    let child_pid = child.id().expect("Child must have PID");
    let starttime = m31a::process::job::read_linux_process_starttime(child_pid)
        .expect("Linux starttime must be readable from /proc/<pid>/stat");

    let spool_file = temp_dir.path().join("job_stdout.spool");
    std::fs::write(&spool_file, b"job executing output...\n").unwrap();

    let job_id_1 = JobId::new();
    let now = chrono::Utc::now().to_rfc3339();
    let rec_json = format!(r#"{{"linux_starttime": {}}}"#, starttime);

    sqlx::query(
        r#"
        INSERT INTO jobs (id, mission_id, task_id, agent_id, command, args_json, working_dir, state, pid, provider, resource_limits_json, stdout_spool_path, recovery_metadata_json, submitted_at, started_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(job_id_1.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(task_id.as_bytes().as_slice())
    .bind(agent_id.as_bytes().as_slice())
    .bind("sleep 60")
    .bind("[]")
    .bind(workspace_dir.to_str().unwrap())
    .bind("running")
    .bind(child_pid as i64)
    .bind("local")
    .bind("{}")
    .bind(spool_file.to_str().unwrap())
    .bind(&rec_json)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let res4 = scanner.scan_and_reconcile(mission_id).await.unwrap();
    assert_eq!(res4.killed_process_groups_count, 1);
    assert_eq!(res4.sealed_spools_count, 1);

    // Verify spool file on disk now contains [INTERRUPTED]
    let spool_content = std::fs::read_to_string(&spool_file).unwrap();
    assert!(spool_content.contains("[INTERRUPTED]"));

    // Verify job in SQLite is marked Lost
    use sqlx::Row;
    let job1_row = sqlx::query("SELECT state, artifact_id FROM jobs WHERE id = ?")
        .bind(job_id_1.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    let job1_state: String = job1_row.get("state");
    let job1_artifact_id: Option<Vec<u8>> = job1_row.get("artifact_id");
    assert_eq!(job1_state, "Lost");
    assert!(job1_artifact_id.is_some());

    // Scenario 5: Recycled PID (starttime mismatch) -> marked Lost without kill signal (D-15)
    let job_id_2 = JobId::new();
    let current_test_pid = std::process::id();
    let fake_rec_json = r#"{"linux_starttime": 999999999999}"#;

    sqlx::query(
        r#"
        INSERT INTO jobs (id, mission_id, task_id, agent_id, command, args_json, working_dir, state, pid, provider, resource_limits_json, recovery_metadata_json, submitted_at, started_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(job_id_2.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(task_id.as_bytes().as_slice())
    .bind(agent_id.as_bytes().as_slice())
    .bind("dummy")
    .bind("[]")
    .bind(workspace_dir.to_str().unwrap())
    .bind("running")
    .bind(current_test_pid as i64)
    .bind("local")
    .bind("{}")
    .bind(fake_rec_json)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let res5 = scanner.scan_and_reconcile(mission_id).await.unwrap();
    // No kill signal issued because starttimes mismatch
    assert_eq!(res5.killed_process_groups_count, 0);

    let job2_state: String = sqlx::query_scalar("SELECT state FROM jobs WHERE id = ?")
        .bind(job_id_2.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(job2_state, "Lost");
}

// ============================================================================
// Task 10-03-03: Safe Resume & Task Completion Preservation (CHK-05, D-16)
// ============================================================================

#[tokio::test]
async fn test_safe_resume_task_preservation() {
    let (temp_dir, pool, artifact_store, mission_id, task_1, _agent_id) = setup_test_env().await;

    // Seed task 2 and task 3
    let task_2 = TaskId::new();
    let task_3 = TaskId::new();
    let now = chrono::Utc::now().to_rfc3339();

    // Mark task 1 as succeeded
    sqlx::query("UPDATE tasks SET status = 'succeeded' WHERE id = ?")
        .bind(task_1.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    // Insert task 2 (succeeded)
    sqlx::query(
        "INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(task_2.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind("Task 2 - Succeeded")
    .bind("succeeded")
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    // Insert task 3 (running)
    sqlx::query(
        "INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(task_3.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind("Task 3 - In Flight")
    .bind("running")
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    // Create workspace output files for Task 1 and Task 2
    let workspace_dir = temp_dir.path().join("workspace_resume");
    std::fs::create_dir_all(&workspace_dir).unwrap();
    std::fs::write(
        workspace_dir.join("task_1_out.txt"),
        b"verified output for task 1\n",
    )
    .unwrap();
    std::fs::write(
        workspace_dir.join("task_2_out.txt"),
        b"verified output for task 2\n",
    )
    .unwrap();

    // Capture and save repository baseline
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
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(check_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind(tid.as_bytes().as_slice())
        .bind(3) // Tier 3 tests
        .bind("passed")
        .bind("cargo test")
        .bind("{}")
        .bind(aid.as_bytes().as_slice())
        .bind("Verification passed")
        .bind(&snapshot_identity)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();
    }

    // Create checkpoint capturing Tasks 1 & 2 Succeeded, Task 3 Running
    let cp_id = CheckpointId::new();
    let mut task_states = BTreeMap::new();
    task_states.insert(task_1, TaskState::Succeeded);
    task_states.insert(task_2, TaskState::Succeeded);
    task_states.insert(task_3, TaskState::Running);

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

    let staging_dir = temp_dir.path().join("staging_resume");
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

    // -------------------------------------------------------------------------
    // Step A: Simulate Crash & Restart -> Verify Task Preservation
    // -------------------------------------------------------------------------
    let resume_engine = SafeResumeEngine::new(pool.clone(), artifact_store.clone(), &workspace_dir)
        .with_task_output_file(task_1, "task_1_out.txt")
        .with_task_output_file(task_2, "task_2_out.txt");

    let report_1 = resume_engine
        .resume_mission(mission_id, &manifest)
        .await
        .expect("Safe resume should succeed");

    // Tasks 1 and 2 must be preserved as Succeeded (CHK-05, D-16)
    assert!(report_1.preserved_tasks.contains(&task_1));
    assert!(report_1.preserved_tasks.contains(&task_2));
    assert_eq!(report_1.preserved_tasks.len(), 2);

    // Task 3 (was Running) must transition to Pending
    assert!(report_1.rescheduled_tasks.contains(&task_3));
    assert_eq!(report_1.rescheduled_tasks.len(), 1);

    // No tasks should be invalidated yet
    assert!(report_1.invalidated_tasks.is_empty());

    // Verify DB states: Tasks 1 & 2 are succeeded, Task 3 is pending
    let t1_status: String = sqlx::query_scalar("SELECT status FROM tasks WHERE id = ?")
        .bind(task_1.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(t1_status, "succeeded");

    let t2_status: String = sqlx::query_scalar("SELECT status FROM tasks WHERE id = ?")
        .bind(task_2.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(t2_status, "succeeded");

    let t3_status: String = sqlx::query_scalar("SELECT status FROM tasks WHERE id = ?")
        .bind(task_3.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(t3_status, "pending");

    // -------------------------------------------------------------------------
    // Step B: Simulate Workspace Drift on Task 2's output file
    // -------------------------------------------------------------------------
    std::fs::write(
        workspace_dir.join("task_2_out.txt"),
        b"unauthorized external modification causing file drift!\n",
    )
    .unwrap();

    let report_2 = resume_engine
        .resume_mission(mission_id, &manifest)
        .await
        .expect("Safe resume with drift evaluation should succeed");

    // Task 1 remains preserved (intact files)
    assert!(report_2.preserved_tasks.contains(&task_1));
    assert_eq!(report_2.preserved_tasks.len(), 1);

    // Task 2 is invalidated due to file drift!
    assert!(report_2.invalidated_tasks.contains(&task_2));
    assert_eq!(report_2.invalidated_tasks.len(), 1);

    // In SQLite, Task 2 must now be transitioned to pending for re-execution
    let t2_drifted_status: String = sqlx::query_scalar("SELECT status FROM tasks WHERE id = ?")
        .bind(task_2.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(t2_drifted_status, "pending");

    // Task 1 remains succeeded in SQLite
    let t1_intact_status: String = sqlx::query_scalar("SELECT status FROM tasks WHERE id = ?")
        .bind(task_1.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(t1_intact_status, "succeeded");
}

#[tokio::test]
async fn test_uncheckpointed_mission_classification() {
    let (temp_dir, pool, artifact_store, mission_id, task_id, _agent_id) = setup_test_env().await;
    let workspace_dir = temp_dir.path().join("workspace_uncheckpointed");
    std::fs::create_dir_all(&workspace_dir).unwrap();

    let scanner =
        StartupCrashRecoveryScanner::new(pool.clone(), artifact_store.clone(), &workspace_dir);

    // Initial state from setup_test_env: mission has 1 task with status 'running', no checkpoints, no jobs, no mutations.
    // Since task is running and will be reconciled to pending (0 completed/failed, 0 jobs, 0 mutations),
    // it is pristine -> SafeToResume.
    let pristine_scan = scanner.scan_and_reconcile(mission_id).await.unwrap();
    assert_eq!(
        pristine_scan.classification,
        CrashRecoveryClassification::SafeToResume,
        "Pristine mission without checkpoints must be SafeToResume: {}",
        pristine_scan.explanation
    );

    // Now mark task as succeeded without creating any checkpoint -> NeedsRepair!
    sqlx::query("UPDATE tasks SET status = 'succeeded' WHERE id = ?")
        .bind(task_id.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    let uncheckpointed_work_scan = scanner.scan_and_reconcile(mission_id).await.unwrap();
    assert_eq!(
        uncheckpointed_work_scan.classification,
        CrashRecoveryClassification::NeedsRepair,
        "Uncheckpointed mission with completed work must be NeedsRepair"
    );
    assert!(
        uncheckpointed_work_scan.explanation.contains("uncheckpointed work"),
        "Explanation must explicitly mention uncheckpointed work"
    );
}

#[tokio::test]
async fn test_corrupt_baseline_row_is_corrupt() {
    let (temp_dir, pool, artifact_store, mission_id, task_id, _agent_id) = setup_test_env().await;
    let workspace_dir = temp_dir.path().join("workspace_corrupt_baseline");
    std::fs::create_dir_all(&workspace_dir).unwrap();
    std::fs::write(workspace_dir.join("main.rs"), b"fn main() {}\n").unwrap();

    let staging_dir = temp_dir.path().join("staging_corrupt_baseline");
    let manager = CheckpointManager::new(pool.clone(), artifact_store.clone(), &staging_dir);

    let baseline = RepositoryBaseline::capture(&workspace_dir, mission_id, None, None).unwrap();
    baseline.save_to_db(&pool).await.unwrap();

    let mut hasher = Sha256::new();
    let fp_json = serde_json::to_string(&baseline.file_hashes).unwrap();
    hasher.update(fp_json.as_bytes());
    let snapshot_identity = format!("{:x}", hasher.finalize());

    let cp_id = CheckpointId::new();
    let mut task_states = BTreeMap::new();
    task_states.insert(task_id, TaskState::Succeeded);
    let manifest = CheckpointManifest::new(
        cp_id,
        mission_id,
        1,
        "Verify",
        1,
        &snapshot_identity,
        task_states,
        BTreeMap::new(),
        "policy_hash",
        vec![],
        vec![],
        "Checkpoint with baseline",
    );

    manager
        .create_checkpoint(&manifest, vec![])
        .await
        .unwrap();

    // Corrupt repository_baselines table entry with invalid JSON
    sqlx::query("UPDATE repository_baselines SET fingerprint_map = 'INVALID_JSON{{{' WHERE mission_id = ?")
        .bind(mission_id.as_bytes().as_slice())
        .execute(&pool)
        .await
        .unwrap();

    let scanner =
        StartupCrashRecoveryScanner::new(pool.clone(), artifact_store.clone(), &workspace_dir);
    let scan_res = scanner.scan_and_reconcile(mission_id).await.unwrap();

    assert_eq!(
        scan_res.classification,
        CrashRecoveryClassification::Corrupt,
        "Corrupted repository baseline row must fail closed as Corrupt instead of falling back"
    );
}

