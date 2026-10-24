//! Architectural Consolidation Integration Tests
//!
//! Validates:
//! 1. Canonical Workspace Configuration (.m31a/M31A.toml) and forward-migration from legacy config.toml.
//! 2. Headless initialization and repository context auto-detection.
//! 3. Migration 032 execution run consolidation and single execution authority.
//! 4. Request-sensitive orchestration (frictionless routing for bug fixes, greenfield discovery for new apps).
//! 5. /workspace configure and status interaction actions.

use std::fs;
use tempfile::tempdir;

use m31a::config::paths::PlatformPaths;
use m31a::config::resolved::ResolvedConfiguration;
use m31a::ids::ExecutionRunId;
use m31a::interaction::session::SqliteSessionRepository;
use m31a::persistence::sqlite::initialize_database;
use m31a::persistence::sqlite::repositories::{SqliteExecutionRunRepository, SqliteTaskRepository};
use m31a::runtime::PlanningChoice;
use m31a::state::ExecutionRun;
use m31a::tui::screens::workspace_setup::{
    DetectedRepoContext, WorkspaceSetupScreen, initialize_workspace_headless,
};

#[test]
fn test_canonical_workspace_config_path_resolution() {
    let tmp = tempdir().expect("tempdir");
    let ws = tmp.path();

    // Canonical path is .m31a/config.md
    let canonical = PlatformPaths::canonical_workspace_config_file(ws);
    assert_eq!(canonical, ws.join(".m31a").join("config.md"));

    // When no config exists, workspace_config_file returns canonical config.md
    let active = PlatformPaths::workspace_config_file(ws);
    assert_eq!(active, canonical);

    // When legacy config.toml exists and config.md does not, workspace_config_file returns legacy
    let legacy = ws.join(".m31a").join("config.toml");
    fs::create_dir_all(ws.join(".m31a")).expect("mkdir");
    fs::write(&legacy, "[workspace]\nname = \"legacy-project\"\n").expect("write legacy");
    assert_eq!(PlatformPaths::workspace_config_file(ws), legacy);

    // When config.md also exists, canonical config.md takes precedence
    let md_content = "---\nkind: config\nschema_version: 1\ndata:\n  workspace:\n    name: canonical-project\n---\n";
    fs::write(&canonical, md_content).expect("write canonical");
    assert_eq!(PlatformPaths::workspace_config_file(ws), canonical);
}

#[tokio::test]
async fn test_legacy_config_forward_migration() {
    let tmp = tempdir().expect("tempdir");
    let ws = tmp.path();

    // Setup legacy config
    let legacy = ws.join(".m31a").join("config.toml");
    let canonical = ws.join(".m31a").join("config.md");
    fs::create_dir_all(ws.join(".m31a")).expect("mkdir");
    fs::write(
        &legacy,
        "[workspace]\nproject_name = \"migrated-app\"\ndescription = \"legacy app test\"\n",
    )
    .expect("write legacy");

    assert!(legacy.exists());
    assert!(!canonical.exists());

    // Resolving configuration automatically forward-migrates legacy config.toml to config.md
    let resolved = ResolvedConfiguration::for_workspace(ws).expect("build resolved config");
    assert_eq!(
        resolved.app_config.workspace.project_name.as_deref(),
        Some("migrated-app")
    );

    // Verify config.md was written atomically
    assert!(
        canonical.exists(),
        "config.md must be written during forward migration"
    );
    let content = fs::read_to_string(&canonical).expect("read config.md");
    assert!(content.contains("migrated-app"));
    assert!(content.contains("kind: config"));
}

#[test]
fn test_headless_initialization_and_manifest_detection() {
    let tmp = tempdir().expect("tempdir");
    let ws = tmp.path();

    // Create a mock Rust project with Cargo.toml
    fs::write(
        ws.join("Cargo.toml"),
        "[package]\nname = \"test-rust-service\"\nversion = \"0.1.0\"\n",
    )
    .expect("write Cargo.toml");

    // Detect repo context
    let detected = DetectedRepoContext::inspect(ws);
    assert_eq!(
        detected.detected_type,
        Some(m31a::tui::screens::workspace_setup::ProjectTypeChoice::Library)
    );
    assert!(detected.detected_languages.contains(&"rust".to_string()));
    assert_eq!(detected.detected_build_system.as_deref(), Some("cargo"));

    // Run headless initialization
    let config_path = initialize_workspace_headless(
        ws,
        Some("my-custom-service".to_string()),
        Some("web_service".to_string()),
        Some(vec!["Rust".to_string()]),
        Some("cargo test".to_string()),
    )
    .expect("headless init");

    assert_eq!(config_path, ws.join(".m31a").join("config.md"));
    assert!(config_path.exists());

    let content = fs::read_to_string(&config_path).expect("read config");
    assert!(content.contains("my-custom-service"));
    assert!(content.contains("web_service"));
    assert!(content.contains("cargo test"));
}

#[test]
fn test_workspace_setup_screen_keystrokes_and_state() {
    let tmp = tempdir().expect("tempdir");
    let ws = tmp.path();

    let mut screen = WorkspaceSetupScreen::new(ws.to_path_buf());
    assert_eq!(screen.active_field_idx(), 0);

    // Key Down moves down
    screen.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Down,
        crossterm::event::KeyModifiers::empty(),
    ));
    assert_eq!(screen.active_field_idx(), 1);

    // Key Up moves up
    screen.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Up,
        crossterm::event::KeyModifiers::empty(),
    ));
    assert_eq!(screen.active_field_idx(), 0);

    // Single-key confirmation 'Y' accepts configuration and writes M31A.toml
    let outcome = screen.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('Y'),
        crossterm::event::KeyModifiers::empty(),
    ));
    assert!(matches!(
        outcome,
        m31a::tui::screens::workspace_setup::WorkspaceSetupOutcome::AcceptedDefaults
    ));
    assert!(ws.join(".m31a").join("config.md").exists());
}

#[tokio::test]
async fn test_migration_032_execution_run_consolidation() {
    let tmp = tempdir().expect("tempdir");
    let db_path = tmp.path().join("state.db");
    let pool = initialize_database(&db_path).await.expect("init db");

    let run_repo = SqliteExecutionRunRepository::new(pool.clone());
    let task_repo = SqliteTaskRepository::new(pool.clone());

    // Canonical identity is ExecutionRunId
    let run_id = ExecutionRunId::new();
    let exec_run = ExecutionRun::new(run_id, "Consolidated execution test".to_string());
    run_repo
        .insert(&exec_run)
        .await
        .expect("insert execution run");

    // Retrieve via canonical repository
    let retrieved = run_repo.get(run_id).await.expect("get run");
    assert!(retrieved.is_some());
    let found = retrieved.unwrap();
    assert_eq!(found.id, run_id);
    assert_eq!(found.objective, "Consolidated execution test");

    // Task records link to canonical run_id
    let task_id = m31a::ids::TaskId::new();
    let task = m31a::state::Task::new(task_id, run_id, "Test task".to_string());
    task_repo.insert(&task).await.expect("insert task");

    let task_retrieved = task_repo.get(task_id).await.expect("get task");
    assert!(task_retrieved.is_some());
    let t = task_retrieved.unwrap();
    assert_eq!(t.mission_id, run_id);
    assert_eq!(t.run_id, Some(run_id));
}

#[test]
fn test_request_sensitive_orchestration() {
    let tmp = tempdir().expect("tempdir");
    let ws = tmp.path();

    // 1. Bug fix in existing or unspecified workspace proceeds directly to DirectExecution
    let fix_choice =
        PlanningChoice::resolve_for_request("fix the null pointer exception", None, None, false);
    assert_eq!(fix_choice, Some(PlanningChoice::DirectExecution));

    // 2. Refactor in brownfield workspace proceeds to DirectExecution
    fs::write(ws.join("Cargo.toml"), "[package]\nname = \"demo\"\n").expect("write");
    let refactor_choice = PlanningChoice::resolve_for_request_with_workspace(
        "refactor the auth service",
        Some(ws),
        None,
        None,
        false,
    );
    assert_eq!(refactor_choice, Some(PlanningChoice::DirectExecution));

    // 3. Greenfield new app creation in empty workspace routes to ProjectPlanning
    let greenfield_tmp = tempdir().expect("tempdir");
    let greenfield_ws = greenfield_tmp.path();
    let create_choice = PlanningChoice::resolve_for_request_with_workspace(
        "build a distributed key-value store from scratch",
        Some(greenfield_ws),
        None,
        None,
        false,
    );
    assert_eq!(create_choice, Some(PlanningChoice::ProjectPlanning));

    // 4. Explicit commands override auto-detection
    let explicit_noplan = PlanningChoice::resolve_for_request_with_workspace(
        "/no-plan build a compiler",
        Some(greenfield_ws),
        None,
        None,
        false,
    );
    assert_eq!(explicit_noplan, Some(PlanningChoice::DirectExecution));

    let explicit_plan = PlanningChoice::resolve_for_request_with_workspace(
        "/plan fix typo",
        Some(ws),
        None,
        None,
        false,
    );
    assert_eq!(explicit_plan, Some(PlanningChoice::ProjectPlanning));
}

#[tokio::test]
async fn test_session_canonical_execution_binding() {
    let tmp = tempdir().expect("tempdir");
    let db_path = tmp.path().join("state.db");
    let pool = initialize_database(&db_path).await.expect("init db");
    let session_repo = SqliteSessionRepository::new(pool.clone());

    let sess = session_repo
        .create_session(tmp.path())
        .await
        .expect("create session");
    assert_eq!(sess.active_mission_id, None);
    assert_eq!(sess.active_run_id, None);

    let run_id = ExecutionRunId::new();
    let exec_run = ExecutionRun::new(run_id, "Session run binding".to_string());
    let run_repo = SqliteExecutionRunRepository::new(pool.clone());
    run_repo.insert(&exec_run).await.expect("insert run");

    // Setting active execution updates both canonical run and compatibility alias
    session_repo
        .set_active_run(sess.id, run_id)
        .await
        .expect("set active run");

    let loaded = session_repo
        .get_session(sess.id)
        .await
        .expect("get session")
        .expect("session found");
    assert_eq!(loaded.active_run_id, Some(run_id));
    assert_eq!(loaded.active_mission_id, Some(run_id));
}
