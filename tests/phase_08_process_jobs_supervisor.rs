//! Phase 8: Process Supervisor, Background Jobs & Process Group Cancellation Tests (TL-02, CTL-01, Law 8).

use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

use m31a::ids::{AgentId, TaskId};
use m31a::persistence::artifacts::fs_store::{ArtifactStore, FsArtifactStore};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::process::env::EnvironmentBuilder;
use m31a::process::job::{JobState, JobSupervisor};
use m31a::process::supervisor::{ProcessError, ProcessSupervisor};

// ============================================================================
// 1. Process Group Isolation & Cancellation Tests (Law 8, D-14)
// ============================================================================

#[tokio::test]
async fn test_run_command_cancellation() {
    let temp = tempdir().unwrap();
    let supervisor = ProcessSupervisor::new(Duration::from_secs(10), Duration::from_millis(500));
    let token = CancellationToken::new();

    let token_clone = token.clone();
    tokio::spawn(async move {
        // Sleep briefly and then cancel
        tokio::time::sleep(Duration::from_millis(150)).await;
        token_clone.cancel();
    });

    // Run long-running command (sleep 10)
    let res = supervisor
        .run_command(
            "sleep",
            &["10".to_string()],
            temp.path(),
            None,
            Some(Duration::from_secs(5)),
            Some(&token),
        )
        .await;

    assert!(res.is_err(), "Sleep 10 must be interrupted by cancellation");
    assert_eq!(res.unwrap_err(), ProcessError::Cancelled);
}

#[tokio::test]
async fn test_run_command_timeout() {
    let temp = tempdir().unwrap();
    let supervisor = ProcessSupervisor::new(Duration::from_secs(10), Duration::from_millis(500));

    // Run command exceeding timeout
    let res = supervisor
        .run_command(
            "sleep",
            &["5".to_string()],
            temp.path(),
            None,
            Some(Duration::from_millis(200)),
            None,
        )
        .await;

    assert!(res.is_err(), "Command must time out");
    assert!(matches!(res.unwrap_err(), ProcessError::TimedOut(_)));
}

#[tokio::test]
async fn test_run_command_success() {
    let temp = tempdir().unwrap();
    let supervisor = ProcessSupervisor::default();

    let res = supervisor
        .run_command(
            "echo",
            &["hello process supervisor".to_string()],
            temp.path(),
            None,
            Some(Duration::from_secs(5)),
            None,
        )
        .await
        .unwrap();

    assert_eq!(res.exit_code, 0);
    assert_eq!(res.stdout.trim(), "hello process supervisor");
}

// ============================================================================
// 2. Deny-by-Default Environment Sanitization Tests (D-15)
// ============================================================================

#[test]
fn test_environment_sanitizer_blocks_secrets_and_dangerous_loaders() {
    let temp = tempdir().unwrap();
    let mut builder = EnvironmentBuilder::new(temp.path());

    // 1. Host credentials must be blocked
    assert!(builder.is_forbidden_key("NVIDIA_API_KEY"));
    assert!(builder.is_forbidden_key("OPENAI_API_KEY"));
    assert!(builder.is_forbidden_key("AWS_SECRET_ACCESS_KEY"));
    assert!(builder.is_forbidden_key("GITHUB_TOKEN"));
    assert!(builder.is_forbidden_key("DATABASE_URL"));

    // 2. Setting forbidden credential returns error
    let res = builder.set_var("NVIDIA_API_KEY", "nv-secret-12345");
    assert!(res.is_err());

    // 3. Dangerous loader overrides must be blocked
    assert!(builder.is_forbidden_key("LD_PRELOAD"));
    assert!(builder.is_forbidden_key("LD_LIBRARY_PATH"));
    assert!(builder.is_forbidden_key("PYTHONPATH"));
    assert!(builder.is_forbidden_key("NODE_OPTIONS"));

    let res2 = builder.set_var("LD_PRELOAD", "/tmp/malicious.so");
    assert!(res2.is_err());

    // 4. Authorized benign variables are accepted
    let res3 = builder.set_var("BUILD_PROFILE", "release");
    assert!(res3.is_ok());

    let map = builder.build_map();
    assert_eq!(map.get("BUILD_PROFILE").unwrap(), "release");
    assert!(!map.contains_key("NVIDIA_API_KEY"));
    assert!(!map.contains_key("LD_PRELOAD"));
}

// ============================================================================
// 3. Background Jobs Lifecycle & Dual-Buffer Spooling Tests (CTL-01, TL-02, D-13, D-16)
// ============================================================================

#[tokio::test]
async fn test_background_jobs_lifecycle() {
    let temp = tempdir().unwrap();
    let spool_dir = temp.path().join("spools");
    let artifact_dir = temp.path().join("artifacts");
    tokio::fs::create_dir_all(&spool_dir).await.unwrap();
    tokio::fs::create_dir_all(&artifact_dir).await.unwrap();

    let artifact_store: Arc<dyn ArtifactStore> = Arc::new(FsArtifactStore::new(artifact_dir));
    let supervisor = Arc::new(
        JobSupervisor::new(spool_dir)
            .with_artifact_store(artifact_store)
            .with_default_timeout(Duration::from_secs(30)),
    );

    let task_id = TaskId::new();
    let agent_id = AgentId::new();

    // 1. Start a fast background job that emits stdout
    let desc = supervisor
        .start_job(
            task_id,
            agent_id,
            "sh",
            &[
                "-c".to_string(),
                "echo 'job line 1'; sleep 0.1; echo 'job line 2'".to_string(),
            ],
            temp.path(),
            None,
            None,
        )
        .await
        .unwrap();

    let jid = m31a::ids::JobId::parse(&desc.job_id).unwrap();

    // Wait for job to complete
    let mut state = JobState::Running;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let status = supervisor.job_status(&jid).await.unwrap();
        if status.state != "running" {
            state = if status.state == "completed" {
                JobState::Completed
            } else {
                JobState::Failed
            };
            break;
        }
    }

    assert_eq!(state, JobState::Completed);

    // Read buffered output
    let chunk = supervisor.job_output(&jid, 0, 4096).await.unwrap();
    assert!(chunk.stdout.contains("job line 1"));
    assert!(chunk.stdout.contains("job line 2"));

    // Check artifact promotion
    let records = supervisor.list_jobs().await;
    let rec = records.iter().find(|r| r.job_id == jid).unwrap();
    assert_eq!(rec.state, JobState::Completed);
    assert_eq!(rec.exit_code, Some(0));
    assert!(
        rec.artifact_id.is_some(),
        "Spool should be promoted to ArtifactStore"
    );

    // 2. Test reap_task_jobs
    let desc2 = supervisor
        .start_job(
            task_id,
            agent_id,
            "sleep",
            &["10".to_string()],
            temp.path(),
            None,
            None,
        )
        .await
        .unwrap();

    let jid2 = m31a::ids::JobId::parse(&desc2.job_id).unwrap();

    // Verify it is running
    let status2 = supervisor.job_status(&jid2).await.unwrap();
    assert_eq!(status2.state, "running");

    // Reap all background jobs for this task
    let reaped = supervisor.reap_task_jobs(&task_id).await;
    assert_eq!(reaped, 1);

    let status_reaped = supervisor.job_status(&jid2).await.unwrap();
    assert_eq!(status_reaped.state, "stopped");
}

// ============================================================================
// 4. SQLite Migration 006 Persistence Tests (D-12)
// ============================================================================

#[tokio::test]
async fn test_migration_006_tables_and_indexes() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("test_migration_006.db");

    let pool = initialize_database(&db_path).await.unwrap();

    // Verify all 3 tables exist
    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('tool_executions', 'background_jobs', 'capability_health_records') ORDER BY name"
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let table_names: Vec<String> = tables.into_iter().map(|(t,)| t).collect();
    assert!(table_names.contains(&"tool_executions".to_string()));
    assert!(table_names.contains(&"background_jobs".to_string()));
    assert!(table_names.contains(&"capability_health_records".to_string()));

    // Verify indexes exist
    let indexes: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_tool_executions_%' OR name LIKE 'idx_background_jobs_%'"
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let index_names: Vec<String> = indexes.into_iter().map(|(i,)| i).collect();
    assert!(index_names.contains(&"idx_tool_executions_task_id".to_string()));
    assert!(index_names.contains(&"idx_tool_executions_mission_id".to_string()));
    assert!(index_names.contains(&"idx_tool_executions_tool_id".to_string()));
    assert!(index_names.contains(&"idx_background_jobs_task_id".to_string()));
    assert!(index_names.contains(&"idx_background_jobs_state".to_string()));
}
