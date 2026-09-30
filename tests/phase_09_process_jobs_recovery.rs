//! Phase 9: Supervised Process Execution & Durable Background Job Manager Tests (PRC-01, PRC-02, JOB-01, JOB-02, D-13..D-16, §235).

use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

use m31a::ids::{AgentId, JobId, MissionId, TaskId};
use m31a::persistence::artifacts::fs_store::{ArtifactStore, FsArtifactStore};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::process::admission::JobAdmissionController;
use m31a::process::job::{
    JobError, JobManager, JobState, SubmitJobRequest, read_linux_process_starttime,
    reconcile_jobs_on_startup,
};
use m31a::process::spool::{DualBufferOutput, StreamType};
use m31a::process::supervisor::execute_direct_argv;
use m31a::process::tree::ProcessTreeController;
use m31a::sandbox::ResourceLimits;

// ============================================================================
// Helper: Seed foreign key dependencies for SQLite
// ============================================================================

async fn seed_test_entities(pool: &sqlx::SqlitePool) -> (MissionId, TaskId, AgentId) {
    let mission_id = MissionId::new();
    let task_id = TaskId::new();
    let agent_id = AgentId::new();
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind("Testing process execution & background jobs")
    .bind("planning")
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(task_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind("Test task for background jobs")
    .bind("in_progress")
    .bind(&now)
    .bind(&now)
    .execute(pool)
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
    .execute(pool)
    .await
    .unwrap();

    (mission_id, task_id, agent_id)
}

// ============================================================================
// 1. Direct Argv Execution Default (PRC-02, D-14)
// ============================================================================

#[tokio::test]
async fn test_direct_argv_execution_preserves_special_chars() {
    let temp = tempdir().unwrap();

    // Arguments with spaces, semicolons, shell variables, and quotes
    let args = vec![
        "[%s] ".to_string(),
        "arg with spaces".to_string(),
        "arg;with;semicolons;rm -rf /".to_string(),
        "$UNEXPANDED_VARIABLE".to_string(),
        "\"double quoted\" and 'single quoted'".to_string(),
    ];

    let mut cmd = execute_direct_argv("printf", &args, temp.path());
    let output = cmd.output().await.expect("Failed to execute printf");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Each argument should be printed as a distinct token verbatim
    assert!(stdout.contains("[arg with spaces]"));
    assert!(stdout.contains("[arg;with;semicolons;rm -rf /]"));
    assert!(stdout.contains("[$UNEXPANDED_VARIABLE]"));
    assert!(stdout.contains("[\"double quoted\" and 'single quoted']"));
}

// ============================================================================
// 2. 5-Stage Signal Escalation Process Tree Containment (PRC-01, D-14)
// ============================================================================

#[tokio::test]
async fn test_supervisor_signal_escalation_reaps_descendants() {
    let temp = tempdir().unwrap();

    // Spawn a shell script that launches multiple background sleep child processes
    let shell_script = "sleep 30 & sleep 30 & wait";
    let mut cmd = tokio::process::Command::new("sh");
    cmd.args(["-c", shell_script]);
    cmd.current_dir(temp.path());

    let (mut child, tree) =
        ProcessTreeController::spawn_isolated(cmd).expect("Failed to spawn isolated command");
    let pgid = tree.pid() as libc::pid_t;

    // Verify process group is running
    tokio::time::sleep(Duration::from_millis(100)).await;
    let is_alive = unsafe { libc::kill(-pgid, 0) == 0 };
    assert!(is_alive, "Process group should be running");

    // Terminate with signal escalation
    let status = tree
        .terminate_supervised(&mut child, Duration::from_millis(200))
        .await
        .expect("Failed to terminate process tree");

    // Exit status should not be success
    assert!(!status.success());

    // Give kernel a moment to reap
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify negative process group no longer exists (ESRCH == 3)
    let pg_alive = unsafe { libc::kill(-pgid, 0) == 0 };
    assert!(
        !pg_alive,
        "Process group should be completely eradicated without zombie or orphaned children"
    );
}

// ============================================================================
// 3. Dedicated JobAdmissionController Concurrency Permits (JOB-01, D-15)
// ============================================================================

#[tokio::test]
async fn test_job_admission_controller_hierarchical_limits() {
    let mission_1 = MissionId::new();
    let mission_2 = MissionId::new();

    // Global capacity: 3, Per-mission capacity: 2
    let controller = JobAdmissionController::new(3, 2);

    // 1. Mission 1 acquires Permit 1A (success)
    let permit_1a = controller
        .acquire_permit(mission_1, Duration::from_millis(500))
        .await
        .expect("Permit 1A should succeed");

    // 2. Mission 1 acquires Permit 1B (success)
    let permit_1b = controller
        .acquire_permit(mission_1, Duration::from_millis(500))
        .await
        .expect("Permit 1B should succeed");

    // 3. Mission 1 attempts 3rd permit -> fails with CapacityExceeded (per-mission limit 2 reached)
    let permit_1c_res = controller
        .acquire_permit(mission_1, Duration::from_millis(50))
        .await;
    match permit_1c_res {
        Err(JobError::CapacityExceeded(msg)) => {
            assert!(msg.contains("Mission"));
            assert!(msg.contains("capacity"));
        }
        other => panic!(
            "Expected CapacityExceeded for mission limit, got {:?}",
            other
        ),
    }

    // 4. Mission 2 acquires Permit 2A (success, global total is now 3)
    let permit_2a = controller
        .acquire_permit(mission_2, Duration::from_millis(500))
        .await
        .expect("Permit 2A should succeed");

    // 5. Mission 2 attempts 2nd permit -> fails with CapacityExceeded (global limit 3 reached)
    let permit_2b_res = controller
        .acquire_permit(mission_2, Duration::from_millis(50))
        .await;
    match permit_2b_res {
        Err(JobError::CapacityExceeded(msg)) => {
            assert!(msg.contains("Global"));
            assert!(msg.contains("capacity"));
        }
        other => panic!(
            "Expected CapacityExceeded for global limit, got {:?}",
            other
        ),
    }

    // 6. Drop Permit 1A (RAII release) -> frees global capacity
    drop(permit_1a);

    // Mission 2 can now acquire Permit 2B
    let permit_2b = controller
        .acquire_permit(mission_2, Duration::from_millis(500))
        .await
        .expect("Permit 2B should now succeed after capacity release");

    // Clean up remaining permits
    drop(permit_1b);
    drop(permit_2a);
    drop(permit_2b);

    // Verify all permits returned
    let (g_avail, m1_avail) = controller.available_permits(mission_1).await;
    assert_eq!(g_avail, 3);
    assert_eq!(m1_avail, 2);
}

// ============================================================================
// 4. Dual-Buffer Output Spooling & Atomic ArtifactStore Promotion (JOB-01, D-16)
// ============================================================================

#[tokio::test]
async fn test_dual_buffer_spool_and_artifact_promotion() {
    let temp = tempdir().unwrap();
    let spool_dir = temp.path().join("spools");
    let artifact_dir = temp.path().join("artifacts");

    let store = Arc::new(FsArtifactStore::new(artifact_dir));
    let job_id = JobId::new();
    let mission_id = MissionId::new();
    let task_id = TaskId::new();

    let output = DualBufferOutput::new(job_id, spool_dir.clone(), 100, 10240)
        .expect("Failed to create DualBufferOutput");

    // Append stdout with ANSI escape sequences
    let stdout_data =
        b"\x1b[32m[INFO]\x1b[0m Starting job execution\n\x1b[1mProcessing step 1\x1b[0m\n";
    output
        .append(StreamType::Stdout, stdout_data)
        .expect("Failed to append stdout");

    // Append stderr with ANSI escape sequences
    let stderr_data = b"\x1b[33m[WARN]\x1b[0m Non-fatal warning occurred\n";
    output
        .append(StreamType::Stderr, stderr_data)
        .expect("Failed to append stderr");

    // Verify in-memory ring buffer strips ANSI sequences
    let tail = output.read_tail(StreamType::Stdout, 10);
    assert_eq!(tail.len(), 2);
    assert_eq!(tail[0], "[INFO] Starting job execution");
    assert_eq!(tail[1], "Processing step 1");

    // Verify read_chunk strips ANSI sequences
    let chunk = output.read_chunk(0, 4096).expect("Failed to read chunk");
    assert!(chunk.stdout.contains("[INFO] Starting job execution"));
    assert!(!chunk.stdout.contains("\x1b[32m"));
    assert!(chunk.stderr.contains("[WARN] Non-fatal warning occurred"));
    assert!(!chunk.stderr.contains("\x1b[33m"));

    // Promote to ArtifactStore
    let promo = output
        .promote_to_artifact_store(store.as_ref(), mission_id, task_id, job_id)
        .await
        .expect("Failed to promote to artifact store");

    assert!(promo.total_bytes > 0);
    assert!(promo.stdout_bytes > 0);
    assert!(promo.stderr_bytes > 0);
    assert!(!promo.sha256_hash.is_empty());

    // Verify artifact is retrievable from FsArtifactStore
    let retrieved = store
        .retrieve(promo.artifact_id, "txt")
        .await
        .expect("Failed to retrieve promoted artifact");
    let retrieved_str = String::from_utf8_lossy(&retrieved);
    assert!(retrieved_str.contains("Starting job execution"));
    assert!(retrieved_str.contains("--- STDERR ---"));
    assert!(retrieved_str.contains("Non-fatal warning occurred"));

    // Spool preservation invariant (D-16): Spool files must NOT be deleted on drop!
    let stdout_spool_path = output.stdout_spool_path().clone();
    let stderr_spool_path = output.stderr_spool_path().clone();
    drop(output);

    assert!(
        stdout_spool_path.exists(),
        "Stdout spool file must be preserved on disk across drops"
    );
    assert!(
        stderr_spool_path.exists(),
        "Stderr spool file must be preserved on disk across drops"
    );
}

// ============================================================================
// 5. SQLite-Backed JobManager Full Lifecycle (JOB-01, JOB-02, D-13)
// ============================================================================

#[tokio::test]
async fn test_job_manager_full_lifecycle() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("test_jobs.db");
    let pool = initialize_database(&db_path)
        .await
        .expect("Failed to initialize database");

    let (mission_id, task_id, agent_id) = seed_test_entities(&pool).await;

    let admission = Arc::new(JobAdmissionController::new(4, 2));
    let store = Arc::new(FsArtifactStore::new(temp.path().join("artifacts")));
    let spool_dir = temp.path().join("spools");

    let job_mgr = JobManager::new(pool.clone(), admission, store.clone(), spool_dir);

    let req = SubmitJobRequest {
        mission_id,
        task_id,
        agent_id,
        tool_call_id: Some("tool-call-123".into()),
        command: "sh".into(),
        args: vec![
            "-c".into(),
            "echo 'line one'; echo 'line two'; echo 'stderr line' >&2".into(),
        ],
        working_dir: temp.path().to_path_buf(),
        provider: "local_process".into(),
        resource_limits: ResourceLimits::default(),
        timeout: Some(Duration::from_secs(5)),
    };

    let job_id = job_mgr.submit_job(req).await.expect("Failed to submit job");

    // Poll until completed
    let mut completed_record = None;
    for _ in 0..60 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let rec = job_mgr
            .job_status(&job_id)
            .await
            .expect("Failed to get status");
        if rec.state.is_terminal() {
            completed_record = Some(rec);
            break;
        }
    }

    let record = completed_record.expect("Job timed out waiting to complete");
    assert_eq!(record.state, JobState::Completed);
    assert_eq!(record.exit_code, Some(0));
    assert!(record.started_at.is_some());
    assert!(record.completed_at.is_some());
    assert!(record.artifact_id.is_some());

    // Verify ownership tuple
    assert_eq!(record.ownership(), (mission_id, task_id, agent_id));

    // Verify output chunk via JobManager
    let chunk = job_mgr
        .job_output(&job_id, 0, 4096)
        .await
        .expect("Failed to read job output");
    assert!(chunk.stdout.contains("line one"));
    assert!(chunk.stdout.contains("line two"));
    assert!(chunk.stderr.contains("stderr line"));
    assert!(chunk.is_eof);

    // Verify promoted artifact
    let artifact_id = record.artifact_id.unwrap();
    let artifact_data = store
        .retrieve(artifact_id, "txt")
        .await
        .expect("Failed to retrieve promoted artifact");
    let artifact_str = String::from_utf8_lossy(&artifact_data);
    assert!(artifact_str.contains("line one"));
    assert!(artifact_str.contains("stderr line"));

    // Verify list_jobs filter
    let list = job_mgr
        .list_jobs(Some(mission_id), Some(task_id))
        .await
        .expect("Failed to list jobs");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].job_id, job_id);
}

// ============================================================================
// 6. Hierarchical Task and Mission Job Cancellation (PRC-01, JOB-02)
// ============================================================================

#[tokio::test]
async fn test_job_manager_hierarchical_cancellation() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("test_cancel.db");
    let pool = initialize_database(&db_path)
        .await
        .expect("Failed to initialize database");

    let (mission_id, task_1, agent_id) = seed_test_entities(&pool).await;

    // Create a second task under same mission
    let task_2 = TaskId::new();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(task_2.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind("Second test task")
    .bind("in_progress")
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    let admission = Arc::new(JobAdmissionController::new(8, 4));
    let store = Arc::new(FsArtifactStore::new(temp.path().join("artifacts")));
    let spool_dir = temp.path().join("spools");

    let job_mgr = JobManager::new(pool.clone(), admission, store, spool_dir);

    // Submit 2 jobs under task_1 running sleep 30
    let req1 = SubmitJobRequest {
        mission_id,
        task_id: task_1,
        agent_id,
        tool_call_id: None,
        command: "sleep".into(),
        args: vec!["30".into()],
        working_dir: temp.path().to_path_buf(),
        provider: "local_process".into(),
        resource_limits: ResourceLimits::default(),
        timeout: Some(Duration::from_secs(30)),
    };
    let job1 = job_mgr
        .submit_job(req1.clone())
        .await
        .expect("Job 1 submit failed");
    let job2 = job_mgr.submit_job(req1).await.expect("Job 2 submit failed");

    // Submit 1 job under task_2 running sleep 30
    let req2 = SubmitJobRequest {
        mission_id,
        task_id: task_2,
        agent_id,
        tool_call_id: None,
        command: "sleep".into(),
        args: vec!["30".into()],
        working_dir: temp.path().to_path_buf(),
        provider: "local_process".into(),
        resource_limits: ResourceLimits::default(),
        timeout: Some(Duration::from_secs(30)),
    };
    let job3 = job_mgr.submit_job(req2).await.expect("Job 3 submit failed");

    // Wait until all 3 jobs are Running
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let s1 = job_mgr.job_status(&job1).await.unwrap();
        let s2 = job_mgr.job_status(&job2).await.unwrap();
        let s3 = job_mgr.job_status(&job3).await.unwrap();
        if s1.state == JobState::Running
            && s2.state == JobState::Running
            && s3.state == JobState::Running
        {
            break;
        }
    }

    // Hierarchically cancel task_1
    let cancelled_count = job_mgr
        .cancel_task_jobs(task_1)
        .await
        .expect("Failed to cancel task 1 jobs");
    assert_eq!(cancelled_count, 2);

    // Verify task_1 jobs transition to Cancelled
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let s1 = job_mgr.job_status(&job1).await.unwrap();
        let s2 = job_mgr.job_status(&job2).await.unwrap();
        if s1.state == JobState::Cancelled && s2.state == JobState::Cancelled {
            break;
        }
    }

    let s1_final = job_mgr.job_status(&job1).await.unwrap();
    let s2_final = job_mgr.job_status(&job2).await.unwrap();
    assert_eq!(s1_final.state, JobState::Cancelled);
    assert_eq!(s2_final.state, JobState::Cancelled);

    // Verify task_2 job is STILL running
    let s3_curr = job_mgr.job_status(&job3).await.unwrap();
    assert_eq!(s3_curr.state, JobState::Running);

    // Cancel remaining job under task_2
    job_mgr
        .cancel_job(&job3, Duration::from_millis(500))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    let s3_final = job_mgr.job_status(&job3).await.unwrap();
    assert_eq!(s3_final.state, JobState::Cancelled);
}

// ============================================================================
// 7. Daemon Crash Restart Reconciliation (JOB-02, D-13, §235)
// ============================================================================

#[tokio::test]
async fn test_startup_reconciliation_marks_lost() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("test_reconcile.db");
    let pool = initialize_database(&db_path)
        .await
        .expect("Failed to initialize database");

    let (mission_id, task_id, agent_id) = seed_test_entities(&pool).await;
    let now = chrono::Utc::now().to_rfc3339();

    let job_1 = JobId::new(); // Dead PID
    let job_2 = JobId::new(); // Starting with no PID
    let job_3 = JobId::new(); // Recycled PID simulation

    let current_pid = std::process::id();
    let current_starttime = read_linux_process_starttime(current_pid);

    // Phase 43 hardening: fixtures must carry production-contract
    // resource_limits_json (full ResourceLimits serialization). The writer
    // (JobManager::submit_job) never emits '{}'; degenerate '{}' rows are
    // typed corruption under strict decode.
    let limits_json = serde_json::to_string(&ResourceLimits::default()).unwrap();

    // 1. Insert pre-crash job with non-existent PID (999999)
    sqlx::query(
        r#"
        INSERT INTO jobs (
            id, mission_id, task_id, agent_id, command, args_json, working_dir,
            state, pid, provider, resource_limits_json, submitted_at, started_at
        ) VALUES (
            ?, ?, ?, ?, 'test_cmd', '[]', '/tmp',
            'running', 999999, 'local_process', ?, ?, ?
        )
        "#,
    )
    .bind(job_1.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(task_id.as_bytes().as_slice())
    .bind(agent_id.as_bytes().as_slice())
    .bind(&limits_json)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    // 2. Insert pre-crash job in starting state without PID
    sqlx::query(
        r#"
        INSERT INTO jobs (
            id, mission_id, task_id, agent_id, command, args_json, working_dir,
            state, pid, provider, resource_limits_json, submitted_at
        ) VALUES (
            ?, ?, ?, ?, 'test_cmd2', '[]', '/tmp',
            'starting', NULL, 'local_process', ?, ?
        )
        "#,
    )
    .bind(job_2.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(task_id.as_bytes().as_slice())
    .bind(agent_id.as_bytes().as_slice())
    .bind(&limits_json)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    // 3. Insert pre-crash job using current PID but simulated mismatched starttime (recycled PID)
    let mismatched_starttime = current_starttime.unwrap_or(100).saturating_add(999999);
    let recovery_json = serde_json::json!({
        "linux_starttime": mismatched_starttime,
        "pid": current_pid,
    })
    .to_string();

    sqlx::query(
        r#"
        INSERT INTO jobs (
            id, mission_id, task_id, agent_id, command, args_json, working_dir,
            state, pid, provider, resource_limits_json, recovery_metadata_json,
            submitted_at, started_at
        ) VALUES (
            ?, ?, ?, ?, 'test_cmd3', '[]', '/tmp',
            'running', ?, 'local_process', ?, ?,
            ?, ?
        )
        "#,
    )
    .bind(job_3.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(task_id.as_bytes().as_slice())
    .bind(agent_id.as_bytes().as_slice())
    .bind(current_pid as i64)
    .bind(&limits_json)
    .bind(&recovery_json)
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .unwrap();

    // Run daemon startup recovery reconciliation
    let reconciled_count = reconcile_jobs_on_startup(&pool)
        .await
        .expect("Failed to reconcile jobs on startup");
    assert_eq!(reconciled_count, 3);

    // Verify all 3 jobs are marked Lost in SQLite
    let admission = Arc::new(JobAdmissionController::new(4, 2));
    let store = Arc::new(FsArtifactStore::new(temp.path().join("artifacts")));
    let spool_dir = temp.path().join("spools");
    let job_mgr = JobManager::new(pool, admission, store, spool_dir);

    let rec1 = job_mgr.job_status(&job_1).await.unwrap();
    assert_eq!(rec1.state, JobState::Lost);
    assert!(rec1.failure_reason.unwrap().contains("does not exist"));

    let rec2 = job_mgr.job_status(&job_2).await.unwrap();
    assert_eq!(rec2.state, JobState::Lost);
    assert!(
        rec2.failure_reason
            .unwrap()
            .contains("not started prior to daemon restart")
    );

    let rec3 = job_mgr.job_status(&job_3).await.unwrap();
    assert_eq!(rec3.state, JobState::Lost);

    // CRITICAL SAFETY INVARIANT CHECK (D-13, §235):
    // The current test process (whose PID matched job_3) is still alive and running!
    // No kill signal was issued to recycled PIDs during reconciliation!
    let self_alive = unsafe { libc::kill(current_pid as libc::pid_t, 0) == 0 };
    assert!(
        self_alive,
        "Current test process must NOT have been signaled/killed by startup reconciliation"
    );
}
