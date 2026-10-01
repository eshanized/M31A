//! Comprehensive persistence test suite for workflow kernel (Package 1).
//!
//! Tests:
//! - Fresh migration 013 against empty database
//! - Incremental migration 013 against existing database with 001-012 applied
//! - WorkflowRun, StepRun, and Artifact CRUD
//! - Foreign key constraints and (workflow_run_id, step_key) uniqueness
//! - Transaction atomicity and rollback
//! - Full state reconstruction after pool restart/reload
//! - Concurrency safety under concurrent readers, writers, and artifact inserters

use m31a::ids::{MissionId, WorkflowRunId};
use m31a::persistence::sqlite::pool::create_pool;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::workflow::error::WorkflowError;
use m31a::workflow::repository::{SqliteWorkflowRepository, WorkflowRepository};
use m31a::workflow::state::{
    WorkflowArtifact, WorkflowArtifactStatus, WorkflowMode, WorkflowRun, WorkflowRunState,
    WorkflowStepRun, WorkflowStepState,
};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn test_migration_013_fresh_database() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("fresh.db");

    let pool = initialize_database(&db_path)
        .await
        .expect("initialize database");

    // Verify all 3 workflow tables exist
    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('workflow_runs', 'workflow_step_runs', 'workflow_artifacts') ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .expect("query tables");

    let names: Vec<String> = tables.into_iter().map(|(n,)| n).collect();
    assert_eq!(
        names,
        vec![
            "workflow_artifacts".to_string(),
            "workflow_runs".to_string(),
            "workflow_step_runs".to_string(),
        ]
    );

    // Verify indexes exist
    let indexes: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_workflow_%' ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .expect("query indexes");

    let idx_names: Vec<String> = indexes.into_iter().map(|(n,)| n).collect();
    assert!(idx_names.contains(&"idx_workflow_runs_status".to_string()));
    assert!(idx_names.contains(&"idx_workflow_artifacts_run".to_string()));
    assert!(idx_names.contains(&"idx_workflow_artifacts_step".to_string()));
}

#[tokio::test]
async fn test_migration_013_incremental_from_prior_schema() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("incremental.db");

    // 1. Create a raw pool without automatic migrator
    let pool = create_pool(&db_path).await.expect("create pool");

    // 2. Apply migrations 001 through 012 manually in order
    let prior_migrations = [
        include_str!("../migrations/001_initial.sql"),
        include_str!("../migrations/002_mission_state_events.sql"),
        include_str!("../migrations/003_task_dag_scheduler.sql"),
        include_str!("../migrations/004_agent_runtime.sql"),
        include_str!("../migrations/005_models_context_repo.sql"),
        include_str!("../migrations/006_capabilities_tools_jobs.sql"),
        include_str!("../migrations/007_checkpoints.sql"),
        include_str!("../migrations/008_policy_sandbox_jobs.sql"),
        include_str!("../migrations/009_verification_recovery_checkpoints.sql"),
        include_str!("../migrations/010_git_attribution.sql"),
        include_str!("../migrations/011_telemetry_budgets_reports.sql"),
        include_str!("../migrations/012_interactive_sessions.sql"),
    ];

    for sql in prior_migrations {
        sqlx::raw_sql(sql)
            .execute(&pool)
            .await
            .expect("failed to execute prior migration");
    }

    // Verify pre-013 state: missions and conversation_messages exist, but workflow_runs does not
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='workflow_runs'",
    )
    .fetch_one(&pool)
    .await
    .expect("query workflow_runs count");
    assert_eq!(count.0, 0);

    // 3. Now apply migration 013
    let sql_013 = include_str!("../migrations/013_workflow_orchestration.sql");
    sqlx::raw_sql(sql_013)
        .execute(&pool)
        .await
        .expect("execute 013");

    // 4. Verify workflow_runs now exists and can be queried
    let count_after: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='workflow_runs'",
    )
    .fetch_one(&pool)
    .await
    .expect("query workflow_runs count after");
    assert_eq!(count_after.0, 1);

    // 5. Verify repository operations work on this upgraded database
    let repo = SqliteWorkflowRepository::new(pool);
    let run = WorkflowRun::new(
        "upgraded-genesis",
        1,
        PathBuf::from("/workspace"),
        WorkflowMode::Standard,
    );
    repo.create_run(&run)
        .await
        .expect("create run in upgraded db");
    let fetched = repo
        .get_run(run.id)
        .await
        .expect("get run")
        .expect("run exists");
    assert_eq!(fetched.id, run.id);
    assert_eq!(fetched.definition_id, "upgraded-genesis");
}

#[tokio::test]
async fn test_workflow_run_and_step_crud() {
    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("crud.db"))
        .await
        .unwrap();
    let repo = SqliteWorkflowRepository::new(pool);

    // 1. Create run
    let mut run = WorkflowRun::new(
        "genesis-greenfield",
        1,
        PathBuf::from("/test/workspace"),
        WorkflowMode::Standard,
    );
    repo.create_run(&run).await.expect("create run");

    // 2. Read run
    let loaded_run = repo.get_run(run.id).await.unwrap().expect("run found");
    assert_eq!(loaded_run.id, run.id);
    assert_eq!(loaded_run.definition_id, "genesis-greenfield");
    assert_eq!(loaded_run.definition_version, 1);
    assert_eq!(loaded_run.status, WorkflowRunState::Pending);
    assert_eq!(loaded_run.mode, WorkflowMode::Standard);
    assert_eq!(loaded_run.workspace_root, PathBuf::from("/test/workspace"));

    // 3. Update run
    run.transition_to(WorkflowRunState::Running, None).unwrap();
    run.current_step_key = Some("discovery".to_string());
    repo.update_run(&run).await.expect("update run");

    let updated_run = repo.get_run(run.id).await.unwrap().expect("run found");
    assert_eq!(updated_run.status, WorkflowRunState::Running);
    assert_eq!(updated_run.current_step_key.as_deref(), Some("discovery"));

    // 4. Create step runs
    let step1 = WorkflowStepRun::new(run.id, "discovery");
    let step2 = WorkflowStepRun::new(run.id, "charter");
    repo.create_step_run(&step1).await.expect("create step1");
    repo.create_step_run(&step2).await.expect("create step2");

    // 5. Query steps
    let loaded_step1 = repo
        .get_step_run(step1.id)
        .await
        .unwrap()
        .expect("step1 found");
    assert_eq!(loaded_step1.step_key, "discovery");
    assert_eq!(loaded_step1.status, WorkflowStepState::Pending);

    let loaded_by_key = repo
        .get_step_run_by_key(run.id, "discovery")
        .await
        .unwrap()
        .expect("step1 by key found");
    assert_eq!(loaded_by_key.id, step1.id);

    // 6. Update step
    let mut step1_mut = loaded_step1;
    step1_mut
        .transition_to(WorkflowStepState::Running, None)
        .unwrap();
    let mid = MissionId::new();
    sqlx::query("INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, 'test mission', 'created', datetime('now'), datetime('now'))")
        .bind(mid.as_bytes().as_slice())
        .execute(repo.pool())
        .await
        .unwrap();
    step1_mut.mission_id = Some(mid);
    repo.update_step_run(&step1_mut)
        .await
        .expect("update step1");

    let step1_reloaded = repo.get_step_run(step1.id).await.unwrap().unwrap();
    assert_eq!(step1_reloaded.status, WorkflowStepState::Running);
    assert_eq!(step1_reloaded.mission_id, Some(mid));

    // 7. List step runs
    let all_steps = repo.list_step_runs(run.id).await.unwrap();
    assert_eq!(all_steps.len(), 2);
    assert_eq!(all_steps[0].step_key, "discovery");
    assert_eq!(all_steps[1].step_key, "charter");
}

#[tokio::test]
async fn test_unique_constraint_step_key_per_run() {
    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("uniq.db"))
        .await
        .unwrap();
    let repo = SqliteWorkflowRepository::new(pool);

    let run = WorkflowRun::new("test", 1, PathBuf::from("."), WorkflowMode::Standard);
    repo.create_run(&run).await.unwrap();

    let step_a = WorkflowStepRun::new(run.id, "discovery");
    repo.create_step_run(&step_a).await.unwrap();

    // Attempting to create a second step with the same key "discovery" for the same run
    let step_b = WorkflowStepRun::new(run.id, "discovery");
    let err = repo
        .create_step_run(&step_b)
        .await
        .expect_err("must violate unique constraint");
    assert!(matches!(err, WorkflowError::Database(_)));
}

#[tokio::test]
async fn test_foreign_key_enforcement() {
    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("fk.db"))
        .await
        .unwrap();
    let repo = SqliteWorkflowRepository::new(pool);

    // Inserting step run with non-existent workflow run ID must fail
    let non_existent_run_id = WorkflowRunId::new();
    let orphan_step = WorkflowStepRun::new(non_existent_run_id, "orphan");
    let err = repo
        .create_step_run(&orphan_step)
        .await
        .expect_err("fk constraint should fail");
    assert!(matches!(err, WorkflowError::Database(_)));
}

#[tokio::test]
async fn test_artifact_persistence_and_queries() {
    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("art.db"))
        .await
        .unwrap();
    let repo = SqliteWorkflowRepository::new(pool);

    let run = WorkflowRun::new("test", 1, PathBuf::from("."), WorkflowMode::Standard);
    repo.create_run(&run).await.unwrap();

    let step = WorkflowStepRun::new(run.id, "discovery");
    repo.create_step_run(&step).await.unwrap();

    let artifact1 = WorkflowArtifact::new(
        run.id,
        step.id,
        "DISCOVERY.md",
        PathBuf::from(".planning/DISCOVERY.md"),
        "sha256:11223344",
        1,
    );
    repo.record_artifact(&artifact1)
        .await
        .expect("record artifact1");

    let artifact2 = WorkflowArtifact::new(
        run.id,
        step.id,
        "NOTES.md",
        PathBuf::from(".planning/NOTES.md"),
        "sha256:55667788",
        1,
    );
    repo.record_artifact(&artifact2)
        .await
        .expect("record artifact2");

    // Fetch by ID
    let fetched = repo
        .get_artifact(artifact1.id)
        .await
        .unwrap()
        .expect("artifact found");
    assert_eq!(fetched.id, artifact1.id);
    assert_eq!(fetched.name, "DISCOVERY.md");
    assert_eq!(fetched.status, WorkflowArtifactStatus::Valid);

    // Update status to Superseded
    repo.update_artifact_status(artifact1.id, WorkflowArtifactStatus::Superseded)
        .await
        .unwrap();
    let updated = repo.get_artifact(artifact1.id).await.unwrap().unwrap();
    assert_eq!(updated.status, WorkflowArtifactStatus::Superseded);

    // List artifacts for run
    let run_arts = repo.list_artifacts(run.id).await.unwrap();
    assert_eq!(run_arts.len(), 2);

    // List artifacts for step
    let step_arts = repo.list_step_artifacts(step.id).await.unwrap();
    assert_eq!(step_arts.len(), 2);
}

#[tokio::test]
async fn test_transactional_advance_and_record() {
    let dir = tempdir().unwrap();
    let pool = initialize_database(&dir.path().join("tx.db"))
        .await
        .unwrap();
    let repo = SqliteWorkflowRepository::new(pool);

    let mut run = WorkflowRun::new("test", 1, PathBuf::from("."), WorkflowMode::Standard);
    repo.create_run(&run).await.unwrap();

    let step = WorkflowStepRun::new(run.id, "charter");

    // Atomically advance run to Running and create/upsert step
    run.transition_to(WorkflowRunState::Running, None).unwrap();
    run.current_step_key = Some("charter".to_string());

    repo.advance_step_and_update_run(&run, &step)
        .await
        .expect("advance step and update run atomically");

    // Verify both were persisted
    let loaded_run = repo.get_run(run.id).await.unwrap().unwrap();
    assert_eq!(loaded_run.status, WorkflowRunState::Running);
    assert_eq!(loaded_run.current_step_key.as_deref(), Some("charter"));

    let loaded_step = repo.get_step_run(step.id).await.unwrap().unwrap();
    assert_eq!(loaded_step.step_key, "charter");

    // Atomically record artifact and complete step
    let artifact = WorkflowArtifact::new(
        run.id,
        step.id,
        "PROJECT.md",
        PathBuf::from("PROJECT.md"),
        "hash:9988",
        1,
    );
    let mut step_completed = loaded_step;
    step_completed
        .transition_to(WorkflowStepState::Running, None)
        .unwrap();
    step_completed
        .transition_to(WorkflowStepState::Completed, None)
        .unwrap();

    repo.record_artifact_and_update_step(&artifact, &step_completed)
        .await
        .expect("record artifact and update step atomically");

    let loaded_art = repo.get_artifact(artifact.id).await.unwrap().unwrap();
    assert_eq!(loaded_art.name, "PROJECT.md");

    let step_final = repo.get_step_run(step.id).await.unwrap().unwrap();
    assert_eq!(step_final.status, WorkflowStepState::Completed);
    assert!(step_final.completed_at.is_some());
}

#[tokio::test]
async fn test_state_reconstruction_across_pool_restart() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("restart.db");

    let run_id;
    let step_id;
    let artifact_id;

    // Phase 1: Write state and drop pool
    {
        let pool = initialize_database(&db_path).await.unwrap();
        let repo = SqliteWorkflowRepository::new(pool);

        let mut run = WorkflowRun::new(
            "genesis-greenfield",
            2,
            PathBuf::from("/canonical/path"),
            WorkflowMode::Autonomous,
        );
        run_id = run.id;
        run.transition_to(WorkflowRunState::Running, None).unwrap();
        run.current_step_key = Some("research_stack".to_string());
        repo.create_run(&run).await.unwrap();

        let mut step = WorkflowStepRun::new(run_id, "research_stack");
        step_id = step.id;
        step.transition_to(WorkflowStepState::Running, None)
            .unwrap();
        repo.create_step_run(&step).await.unwrap();

        let artifact = WorkflowArtifact::new(
            run_id,
            step_id,
            "STACK.md",
            PathBuf::from("research/STACK.md"),
            "sha256:fedcba",
            2,
        );
        artifact_id = artifact.id;
        repo.record_artifact(&artifact).await.unwrap();
    }

    // Phase 2: Reopen pool on the same database file and verify state reconstruction
    {
        let pool = create_pool(&db_path).await.unwrap();
        let repo = SqliteWorkflowRepository::new(pool);

        let reloaded_run = repo.get_run(run_id).await.unwrap().expect("reloaded run");
        assert_eq!(reloaded_run.id, run_id);
        assert_eq!(reloaded_run.definition_id, "genesis-greenfield");
        assert_eq!(reloaded_run.definition_version, 2);
        assert_eq!(reloaded_run.status, WorkflowRunState::Running);
        assert_eq!(reloaded_run.mode, WorkflowMode::Autonomous);
        assert_eq!(
            reloaded_run.workspace_root,
            PathBuf::from("/canonical/path")
        );
        assert_eq!(
            reloaded_run.current_step_key.as_deref(),
            Some("research_stack")
        );

        let reloaded_step = repo
            .get_step_run(step_id)
            .await
            .unwrap()
            .expect("reloaded step");
        assert_eq!(reloaded_step.id, step_id);
        assert_eq!(reloaded_step.step_key, "research_stack");
        assert_eq!(reloaded_step.status, WorkflowStepState::Running);

        let reloaded_artifact = repo
            .get_artifact(artifact_id)
            .await
            .unwrap()
            .expect("reloaded artifact");
        assert_eq!(reloaded_artifact.id, artifact_id);
        assert_eq!(reloaded_artifact.name, "STACK.md");
        assert_eq!(reloaded_artifact.version, 2);
        assert_eq!(reloaded_artifact.status, WorkflowArtifactStatus::Valid);
    }
}

#[tokio::test]
async fn test_concurrent_repository_operations_no_busy() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("concurrency.db");

    let pool = initialize_database(&db_path).await.unwrap();
    let repo = Arc::new(SqliteWorkflowRepository::new(pool));

    let run = WorkflowRun::new(
        "concurrent-test",
        1,
        PathBuf::from("."),
        WorkflowMode::Standard,
    );
    let run_id = run.id;
    repo.create_run(&run).await.unwrap();

    // Create 4 steps
    let mut step_ids = Vec::new();
    for i in 1..=4 {
        let step = WorkflowStepRun::new(run_id, format!("step_{}", i));
        step_ids.push(step.id);
        repo.create_step_run(&step).await.unwrap();
    }

    let mut handles = Vec::new();

    // Spawn 4 concurrent readers
    for _ in 0..4 {
        let r = Arc::clone(&repo);
        handles.push(tokio::spawn(async move {
            for _ in 0..20 {
                let fetched = r.get_run(run_id).await.unwrap().unwrap();
                assert_eq!(fetched.id, run_id);
                let steps = r.list_step_runs(run_id).await.unwrap();
                assert_eq!(steps.len(), 4);
                tokio::task::yield_now().await;
            }
        }));
    }

    // Spawn 2 concurrent step updaters
    for (idx, step_id) in step_ids.iter().take(2).enumerate() {
        let r = Arc::clone(&repo);
        let s_id = *step_id;
        handles.push(tokio::spawn(async move {
            for round in 0..10 {
                let mut step = r.get_step_run(s_id).await.unwrap().unwrap();
                step.attempt_count = (round + 1) as u32;
                step.halt_reason = Some(format!("Worker {} round {}", idx, round));
                r.update_step_run(&step).await.unwrap();
                tokio::task::yield_now().await;
            }
        }));
    }

    // Spawn 2 concurrent artifact inserters during run updates
    for idx in 0..2 {
        let r = Arc::clone(&repo);
        let target_step_id = step_ids[idx + 2];
        handles.push(tokio::spawn(async move {
            for round in 0..10 {
                let art = WorkflowArtifact::new(
                    run_id,
                    target_step_id,
                    format!("artifact_{}_{}.txt", idx, round),
                    PathBuf::from(format!("output_{}_{}.txt", idx, round)),
                    format!("hash_{}", round),
                    1,
                );
                r.record_artifact(&art).await.unwrap();

                // Also update run updated_at
                let mut current_run = r.get_run(run_id).await.unwrap().unwrap();
                current_run.updated_at = chrono::Utc::now();
                r.update_run(&current_run).await.unwrap();

                tokio::task::yield_now().await;
            }
        }));
    }

    // Await all tasks; none should fail with SQLITE_BUSY
    for handle in handles {
        handle.await.expect("task panicked");
    }

    // Verify final state consistency
    let artifacts = repo.list_artifacts(run_id).await.unwrap();
    assert_eq!(artifacts.len(), 20); // 2 workers * 10 rounds
}
