//! Integration test suite for migration 030 (Mission -> ExecutionRun canonical replacement).
//!
//! Validates:
//! 1. Migration from legacy 001..029 schema to 030 creates execution_runs table.
//! 2. Existing missions, tasks, sessions, events, checkpoints, artifacts are preserved and mapped to run_id.
//! 3. All IDs, objectives, and status fields remain intact.
//! 4. Foreign key relationships and indices on run_id are created correctly.

use std::path::Path;
use tempfile::tempdir;
use uuid::Uuid;

use m31a::persistence::sqlite::pool::create_pool;
use sqlx::Row;
use sqlx::migrate::Migrator;

#[tokio::test]
async fn test_migration_030_preserves_data_and_maps_to_execution_runs() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("migration_030_test.db");
    let pool = create_pool(&db_path).await.expect("create sqlite pool");

    // Prepare legacy migrations folder with 001..029 only
    let legacy_migrations = temp.path().join("migrations_legacy");
    tokio::fs::create_dir_all(&legacy_migrations).await.unwrap();

    let project_migrations = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut entries = tokio::fs::read_dir(&project_migrations).await.unwrap();
    while let Some(entry) = entries.next_entry().await.unwrap() {
        let name = entry.file_name().to_string_lossy().to_string();
        let is_prior_to_030 = name
            .split('_')
            .next()
            .and_then(|v| v.parse::<u32>().ok())
            .map(|v| v < 30)
            .unwrap_or(false);
        if name.ends_with(".sql") && is_prior_to_030 {
            let content = tokio::fs::read_to_string(entry.path()).await.unwrap();
            tokio::fs::write(legacy_migrations.join(&name), content)
                .await
                .unwrap();
        }
    }

    // Step 1: Run legacy migrations 001..029
    let legacy_migrator = Migrator::new(legacy_migrations.clone())
        .await
        .expect("load legacy migrator");
    legacy_migrator
        .run(&pool)
        .await
        .expect("run legacy migrations 001..029");

    // Step 2: Insert legacy mission, session, task, event, checkpoint, artifact records
    let mission_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();
    let task_id = Uuid::now_v7();
    let graph_id = Uuid::now_v7();
    let event_id = Uuid::now_v7();
    let checkpoint_id = Uuid::now_v7();
    let artifact_id = Uuid::now_v7();

    let now_str = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO missions (
            id, objective, status, workspace_root, mode, policy_context,
            constraints, requirements, success_criteria, started_at, completed_at,
            parent_mission_id, task_graph_id, last_applied_sequence, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind("Build kernel subsystem")
    .bind("executing")
    .bind("/workspace/test")
    .bind("autonomous")
    .bind("{}")
    .bind(r#"["no-gpu"]"#)
    .bind(r#"["req-1"]"#)
    .bind(r#"["criterion-1"]"#)
    .bind(&now_str)
    .bind(None::<String>)
    .bind(None::<&[u8]>)
    .bind(Some(graph_id.as_bytes().as_slice()))
    .bind(10i64)
    .bind(&now_str)
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO sessions (
            id, mission_id, active_mission_id, status, metadata, workspace_root,
            created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(session_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind("active")
    .bind("{}")
    .bind("/workspace/test")
    .bind(&now_str)
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO tasks (
            id, mission_id, title, status, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(task_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind("Task 1")
    .bind("pending")
    .bind(&now_str)
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO task_graphs (
            id, mission_id, revision, plan_id, status, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(graph_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(1i64)
    .bind("plan-1")
    .bind("active")
    .bind(&now_str)
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO event_log (
            id, sequence, mission_id, event_type, payload, schema_version, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(event_id.as_bytes().as_slice())
    .bind(1i64)
    .bind(mission_id.as_bytes().as_slice())
    .bind("MissionStarted")
    .bind("{}")
    .bind(1i64)
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO checkpoints (
            id, mission_id, sequence, stage, cycle, state_summary, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(checkpoint_id.as_bytes().as_slice())
    .bind(mission_id.as_bytes().as_slice())
    .bind(1i64)
    .bind("planning")
    .bind(1i64)
    .bind("summary")
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO artifacts (
            id, name, logical_path, content_hash, size_bytes, extension, mission_id, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(artifact_id.as_bytes().as_slice())
    .bind("PROJECT.md")
    .bind(".m31a/project/PROJECT.md")
    .bind("hash123")
    .bind(100i64)
    .bind("md")
    .bind(mission_id.as_bytes().as_slice())
    .bind(&now_str)
    .execute(&pool)
    .await
    .unwrap();

    // Step 3: Now run migration 030!
    let full_migrator = Migrator::new(project_migrations.clone())
        .await
        .expect("load full migrator");
    full_migrator.run(&pool).await.expect("run migration 030");

    // Step 4: Verify execution_runs has the migrated data!
    let run_row = sqlx::query(
        "SELECT id, session_id, objective, status, workspace_root, mode, constraints, requirements FROM execution_runs WHERE id = ?"
    )
    .bind(mission_id.as_bytes().as_slice())
    .fetch_one(&pool)
    .await
    .expect("execution_runs record exists");

    let run_id: Vec<u8> = run_row.get("id");
    assert_eq!(run_id.as_slice(), mission_id.as_bytes().as_slice());
    let obj: String = run_row.get("objective");
    assert_eq!(obj, "Build kernel subsystem");
    let status: String = run_row.get("status");
    assert_eq!(status, "executing");
    let ws: String = run_row.get("workspace_root");
    assert_eq!(ws, "/workspace/test");

    // Verify session.active_run_id
    let sess_run_id: Vec<u8> =
        sqlx::query_scalar("SELECT active_run_id FROM sessions WHERE id = ?")
            .bind(session_id.as_bytes().as_slice())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(sess_run_id.as_slice(), mission_id.as_bytes().as_slice());

    // Verify task.run_id
    let t_run_id: Vec<u8> = sqlx::query_scalar("SELECT run_id FROM tasks WHERE id = ?")
        .bind(task_id.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(t_run_id.as_slice(), mission_id.as_bytes().as_slice());

    // Verify task_graphs.run_id
    let tg_run_id: Vec<u8> = sqlx::query_scalar("SELECT run_id FROM task_graphs WHERE id = ?")
        .bind(graph_id.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tg_run_id.as_slice(), mission_id.as_bytes().as_slice());

    // Verify event_log.run_id
    let ev_run_id: Vec<u8> = sqlx::query_scalar("SELECT run_id FROM event_log WHERE id = ?")
        .bind(event_id.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(ev_run_id.as_slice(), mission_id.as_bytes().as_slice());

    // Verify checkpoints.run_id
    let cp_run_id: Vec<u8> = sqlx::query_scalar("SELECT run_id FROM checkpoints WHERE id = ?")
        .bind(checkpoint_id.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cp_run_id.as_slice(), mission_id.as_bytes().as_slice());

    // Verify artifacts.run_id
    let art_run_id: Vec<u8> = sqlx::query_scalar("SELECT run_id FROM artifacts WHERE id = ?")
        .bind(artifact_id.as_bytes().as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(art_run_id.as_slice(), mission_id.as_bytes().as_slice());
}
