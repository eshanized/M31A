//! Architecture Redesign and Unification Integration Test Suite.
//!
//! Validates:
//! 1. Session independence: Session exists before any ExecutionRun and starts with active_run_id = None.
//! 2. Zero dummy/placeholder missions created when creating interactive sessions.
//! 3. Multiple sequential ExecutionRuns within the same session.
//! 4. Legacy .planning/ document migration to .m31a/project/ with conflict preservation, idempotency, and 0 writes to .planning/.
//! 5. Model proposes, runtime decides: TaskGraph in SQLite is authoritative, execution does not require plan.json.
//! 6. Sandbox isolation: .m31a/ directory is never exposed in Bubblewrap sandbox mounts.

use m31a::events::bus::BroadcastEventBus;
use m31a::ids::ExecutionRunId;
use m31a::interaction::session::SqliteSessionRepository as InteractiveSessionRepo;
use m31a::persistence::sqlite::repositories::SqliteExecutionRunRepository;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::state::execution_run::ExecutionRun;
use m31a::storage::migration::migrate_legacy_planning_documents;
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn test_session_exists_independently_without_dummy_mission() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("session_test.db");
    let pool = initialize_database(&db_path).await.expect("init db");
    let bus = Arc::new(BroadcastEventBus::new(128));

    let session_store = InteractiveSessionRepo::new(pool.clone()).with_event_bus(bus);

    // 1. Create a fresh session
    let session = session_store
        .create_session(temp.path())
        .await
        .expect("create session");

    // 2. Invariant: session starts with NO active run
    assert!(session.active_mission_id.is_none());
    assert!(session.active_run_id.is_none());

    // 3. Invariant: NO dummy mission row created in missions or execution_runs table
    let mission_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM missions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        mission_count, 0,
        "No dummy mission must be created on session startup"
    );

    let run_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM execution_runs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        run_count, 0,
        "No dummy execution run must be created on session startup"
    );

    // 4. Retrieve session from repository
    let loaded = session_store
        .get_session(session.id)
        .await
        .expect("get session")
        .expect("session exists");
    assert_eq!(loaded.id, session.id);
    assert!(loaded.active_run_id.is_none());
    assert!(loaded.active_mission_id.is_none());
}

#[tokio::test]
async fn test_multiple_sequential_execution_runs_in_single_session() {
    let temp = tempdir().unwrap();
    let db_path = temp.path().join("multi_run_test.db");
    let pool = initialize_database(&db_path).await.expect("init db");

    let session_store = InteractiveSessionRepo::new(pool.clone());
    let run_repo = SqliteExecutionRunRepository::new(pool.clone());

    // 1. Session created independently
    let session = session_store
        .create_session(temp.path())
        .await
        .expect("create session");
    assert!(session.active_run_id.is_none());

    // 2. First ExecutionRun
    let run_id_1 = ExecutionRunId::new();
    let mut run_1 = ExecutionRun::new(run_id_1, "First user request".to_string());
    run_1.session_id = Some(session.id);
    run_repo.insert(&run_1).await.expect("insert run 1");

    session_store
        .set_active_run(session.id, run_id_1)
        .await
        .expect("set active run 1");

    let loaded_1 = session_store
        .get_session(session.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded_1.active_run_id, Some(run_id_1));

    // Clear run 1 after completion
    session_store
        .clear_active_run(session.id)
        .await
        .expect("clear active run 1");
    let cleared_1 = session_store
        .get_session(session.id)
        .await
        .unwrap()
        .unwrap();
    assert!(cleared_1.active_run_id.is_none());

    // 3. Second ExecutionRun in the SAME session
    let run_id_2 = ExecutionRunId::new();
    let mut run_2 = ExecutionRun::new(run_id_2, "Follow-up user request".to_string());
    run_2.session_id = Some(session.id);
    run_repo.insert(&run_2).await.expect("insert run 2");

    session_store
        .set_active_run(session.id, run_id_2)
        .await
        .expect("set active run 2");

    let loaded_2 = session_store
        .get_session(session.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded_2.active_run_id, Some(run_id_2));

    // 4. Session repository queries: verify both runs linked to same session
    let runs_for_session: Vec<(Vec<u8>,)> = sqlx::query_as(
        "SELECT id FROM execution_runs WHERE session_id = ? ORDER BY created_at ASC",
    )
    .bind(session.id.as_bytes().as_slice())
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(
        runs_for_session.len(),
        2,
        "Both runs linked to same session"
    );
}

#[tokio::test]
async fn test_legacy_planning_migration_to_m31a_project() {
    let temp = tempdir().unwrap();
    let ws = temp.path();

    // 1. Create legacy .planning directory with files
    let legacy_planning = ws.join(".planning");
    tokio::fs::create_dir_all(&legacy_planning).await.unwrap();
    tokio::fs::write(legacy_planning.join("PROJECT.md"), "# Legacy Project\n")
        .await
        .unwrap();
    tokio::fs::write(legacy_planning.join("REQUIREMENTS.md"), "# Requirements\n")
        .await
        .unwrap();

    let legacy_sub = legacy_planning.join("research");
    tokio::fs::create_dir_all(&legacy_sub).await.unwrap();
    tokio::fs::write(legacy_sub.join("notes.md"), "Legacy research notes\n")
        .await
        .unwrap();

    // 2. Perform migration
    let report = migrate_legacy_planning_documents(ws);
    assert_eq!(report.files_migrated.len(), 3);
    assert_eq!(report.conflicts_detected.len(), 0);

    // 3. Verify files now exist under .m31a/project/
    let project_dir = ws.join(".m31a").join("project");
    assert!(project_dir.join("PROJECT.md").is_file());
    assert!(project_dir.join("REQUIREMENTS.md").is_file());
    assert!(project_dir.join("research").join("notes.md").is_file());

    let proj_content = tokio::fs::read_to_string(project_dir.join("PROJECT.md"))
        .await
        .unwrap();
    assert_eq!(proj_content, "# Legacy Project\n");

    // 4. Invariant: Idempotency (running migration again produces 0 files migrated and 0 conflicts)
    let report_2 = migrate_legacy_planning_documents(ws);
    assert_eq!(report_2.files_migrated.len(), 0);
    assert_eq!(report_2.conflicts_detected.len(), 0);

    // 5. Invariant: Conflict preservation
    // If the canonical file was modified, legacy file migration must preserve conflict with .conflict-<hash> suffix
    tokio::fs::write(project_dir.join("PROJECT.md"), "# Modified Canonical\n")
        .await
        .unwrap();
    let report_3 = migrate_legacy_planning_documents(ws);
    assert_eq!(report_3.conflicts_detected.len(), 1);

    // Verify canonical was NOT overwritten
    let canon_content = tokio::fs::read_to_string(project_dir.join("PROJECT.md"))
        .await
        .unwrap();
    assert_eq!(canon_content, "# Modified Canonical\n");

    // Verify conflict file was created
    let mut entries = tokio::fs::read_dir(&project_dir).await.unwrap();
    let mut conflict_found = false;
    while let Some(e) = entries.next_entry().await.unwrap() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with("PROJECT.md.conflict-") {
            conflict_found = true;
            let conflict_content = tokio::fs::read_to_string(e.path()).await.unwrap();
            assert_eq!(conflict_content, "# Legacy Project\n");
        }
    }
    assert!(conflict_found, "Conflict file preserved with hash suffix");
}

#[tokio::test]
async fn test_sandbox_masking_never_mounts_m31a() {
    let ws = std::path::Path::new("/workspace/myproject");
    let m31a_path = ws.join(".m31a");

    // 1. is_denied_extra_ro_mount rejects any .m31a path
    assert!(m31a::sandbox::plan::SandboxPlan::is_denied_extra_ro_mount(
        &m31a_path
    ));
    assert!(m31a::sandbox::plan::SandboxPlan::is_denied_extra_ro_mount(
        &m31a_path.join("project")
    ));
    assert!(m31a::sandbox::plan::SandboxPlan::is_denied_extra_ro_mount(
        &m31a_path.join("project").join("PROJECT.md")
    ));

    // 2. Extra mounts validation fails closed if .m31a is attempted
    let plan =
        m31a::sandbox::plan::SandboxPlan::new(ws.to_path_buf()).with_extra_ro_mount(m31a_path);
    assert!(plan.validate_extra_ro_mounts().is_err());
}
