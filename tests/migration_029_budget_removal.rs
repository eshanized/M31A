//! Integration test suite for migration 029 (budget system removal).
//!
//! Validates:
//! 1. Migration from legacy 001..028 schema to 029 removes budget tables and column.
//! 2. Unrelated mission data, IDs, and statuses remain completely intact.
//! 3. `budget_grants` and `budget_ledger` tables and indexes no longer exist.
//! 4. `missions` table has no `budget` column in post-migration schema.
//! 5. `SqliteMissionRepository` can fetch pre-existing legacy missions and insert new ones.
//! 6. Migration idempotency and tracking in `_sqlx_migrations`.
//! 7. Fresh initialization via `initialize_database` matches upgraded schema.

use std::path::Path;
use tempfile::tempdir;
use uuid::Uuid;

use m31a::ids::MissionId;
use m31a::persistence::sqlite::pool::create_pool;
use m31a::persistence::sqlite::repositories::mission::SqliteMissionRepository;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::state::mission::Mission;
use sqlx::Row;
use sqlx::migrate::Migrator;

#[tokio::test]
async fn test_migration_029_removes_budget_tables_and_preserves_missions() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("migration_test.db");
    let pool = create_pool(&db_path).await.expect("create sqlite pool");

    // Prepare legacy migrations folder with 001..028 only
    let legacy_migrations = temp.path().join("migrations_legacy");
    tokio::fs::create_dir_all(&legacy_migrations).await.unwrap();

    let project_migrations = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut entries = tokio::fs::read_dir(&project_migrations).await.unwrap();
    while let Some(entry) = entries.next_entry().await.unwrap() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".sql") && !name.starts_with("029_") {
            let content = tokio::fs::read_to_string(entry.path()).await.unwrap();
            tokio::fs::write(legacy_migrations.join(&name), content)
                .await
                .unwrap();
        }
    }

    // Step 1: Run legacy migrations 001..028
    let legacy_migrator = Migrator::new(legacy_migrations.clone())
        .await
        .expect("load legacy migrator");
    legacy_migrator
        .run(&pool)
        .await
        .expect("run legacy migrations");

    // Verify pre-029 schema has budget tables and budget column
    let legacy_grant_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='budget_grants'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        legacy_grant_tables, 1,
        "budget_grants table must exist in legacy schema"
    );

    let legacy_ledger_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='budget_ledger'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        legacy_ledger_tables, 1,
        "budget_ledger table must exist in legacy schema"
    );

    let legacy_cols: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('missions')")
            .fetch_all(&pool)
            .await
            .unwrap();
    let col_names: Vec<String> = legacy_cols.into_iter().map(|(n,)| n).collect();
    assert!(
        col_names.contains(&"budget".to_string()),
        "missions.budget column must exist in legacy schema"
    );

    // Step 2: Seed legacy mission, grant, and ledger records
    let mission_uuid = Uuid::now_v7();
    let mission_id = MissionId::from_bytes(*mission_uuid.as_bytes());

    let policy_context_json =
        serde_json::to_string(&m31a::state::policy_context::PolicyContext::standard()).unwrap();

    sqlx::query(
        r#"
        INSERT INTO missions (
            id, objective, status, workspace_root, mode, policy_context, budget,
            constraints, requirements, success_criteria, started_at, completed_at,
            parent_mission_id, task_graph_id, last_applied_sequence, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, NULL, NULL, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')
        "#
    )
    .bind(mission_id.as_bytes().as_slice())
    .bind("Legacy Mission Objective")
    .bind("Planning")
    .bind("/workspace/legacy")
    .bind("safe")
    .bind(&policy_context_json)
    .bind(r#"{"tokens": 100000, "cost_usd": 10.0}"#)
    .bind("[]")
    .bind("[]")
    .bind("[]")
    .execute(&pool)
    .await
    .expect("seed legacy mission");

    sqlx::query(
        r#"
        INSERT INTO budget_grants (
            grant_id, mission_id, actor, reason, limits_json, granted_at
        ) VALUES ('grant-01', ?, 'operator', 'initial allocation', '{"tokens": 100000}', '2026-01-01T00:00:00Z')
        "#
    )
    .bind(mission_uuid.to_string())
    .execute(&pool)
    .await
    .expect("seed budget grant");

    sqlx::query(
        r#"
        INSERT INTO budget_ledger (
            mission_id, consumed_tokens, consumed_cost_microcents, consumed_artifact_bytes,
            steps_consumed, calls_consumed, updated_at
        ) VALUES (?, 100, 1000, 500, 5, 10, '2026-01-01T00:00:00Z')
        "#,
    )
    .bind(mission_id.as_bytes().as_slice())
    .execute(&pool)
    .await
    .expect("seed budget ledger");

    // Step 3: Copy migration 029 and apply it
    let migration_029_content =
        tokio::fs::read_to_string(project_migrations.join("029_remove_budget_system.sql"))
            .await
            .unwrap();
    tokio::fs::write(
        legacy_migrations.join("029_remove_budget_system.sql"),
        migration_029_content,
    )
    .await
    .unwrap();

    let updated_migrator = Migrator::new(legacy_migrations.clone())
        .await
        .expect("reload migrator with 029");
    updated_migrator
        .run(&pool)
        .await
        .expect("run migration 029");

    // Step 4: Verify budget tables and indexes are completely gone
    let grant_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='budget_grants'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(grant_tables, 0, "budget_grants table must be removed");

    let ledger_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='budget_ledger'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ledger_tables, 0, "budget_ledger table must be removed");

    let indexes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name IN ('idx_budget_grants_mission', 'idx_budget_ledger_mission')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(indexes, 0, "budget indexes must be removed");

    // Step 5: Verify missions.budget column is absent
    let post_cols: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('missions')")
            .fetch_all(&pool)
            .await
            .unwrap();
    let post_col_names: Vec<String> = post_cols.into_iter().map(|(n,)| n).collect();
    assert!(
        !post_col_names.contains(&"budget".to_string()),
        "missions.budget column must be dropped"
    );

    // Step 6: Verify unrelated mission data remains completely intact
    let legacy_row = sqlx::query(
        "SELECT id, objective, status, workspace_root, mode FROM missions WHERE id = ?",
    )
    .bind(mission_id.as_bytes().as_slice())
    .fetch_one(&pool)
    .await
    .expect("fetch legacy mission row");

    let objective: String = legacy_row.get("objective");
    let status: String = legacy_row.get("status");
    let workspace_root: String = legacy_row.get("workspace_root");
    let mode: String = legacy_row.get("mode");

    assert_eq!(objective, "Legacy Mission Objective");
    assert_eq!(status, "Planning");
    assert_eq!(workspace_root, "/workspace/legacy");
    assert_eq!(mode, "safe");

    // Step 7: Verify SqliteMissionRepository works end-to-end with post-removal schema
    let repo = SqliteMissionRepository::new(pool.clone());
    let fetched = repo
        .get(mission_id)
        .await
        .unwrap()
        .expect("fetched legacy mission");
    assert_eq!(fetched.id, mission_id);
    assert_eq!(fetched.objective, "Legacy Mission Objective");

    // Create and insert a new mission post-removal
    let new_mission_id = MissionId::new();
    let new_mission = Mission::new(new_mission_id, "Post-removal mission".to_string());
    repo.insert(&new_mission)
        .await
        .expect("insert new mission post-removal");

    let fetched_new = repo
        .get(new_mission_id)
        .await
        .unwrap()
        .expect("fetched new mission");
    assert_eq!(fetched_new.id, new_mission_id);
    assert_eq!(fetched_new.objective, "Post-removal mission");

    // Step 8: Verify idempotency
    let re_run = updated_migrator.run(&pool).await;
    assert!(re_run.is_ok(), "Re-running migrations must be idempotent");

    // Step 9: Verify fresh database initialization matches upgraded database
    let fresh_db_path = temp.path().join("fresh.db");
    let fresh_pool = initialize_database(&fresh_db_path)
        .await
        .expect("init fresh database");

    let fresh_cols: Vec<(String,)> =
        sqlx::query_as("SELECT name FROM pragma_table_info('missions')")
            .fetch_all(&fresh_pool)
            .await
            .unwrap();
    let fresh_col_names: Vec<String> = fresh_cols.into_iter().map(|(n,)| n).collect();

    assert_eq!(
        post_col_names, fresh_col_names,
        "Upgraded missions schema must match fresh missions schema"
    );
}
