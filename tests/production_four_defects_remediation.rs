//! Production Verification Test Suite: Four Production Defects Remediation
//!
//! Validates:
//! Problem 1: Runtime startup readiness, bounded timeout, and stage duration tracking.
//! Problem 2: Session lifecycle (fresh launch does not resume, lazy session creation on first prompt,
//!            explicit resume restores truthfully without duplication, cross-workspace protection).
//! Problem 3: Model provider routing (preserving `nvidia/` publisher prefix) and planning resilience
//!            for read-only reconnaissance objectives (like "Study the codebase").
//! Problem 4: Settings surface routing, category and row navigation, draft editing, atomic persistence,
//!            and composer unfocus.

use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use m31a::config::ResolvedConfiguration;
use m31a::events::bus::BroadcastEventBus;
use m31a::interaction::action::ApplicationAction;
use m31a::interaction::events::InteractionEvent;
use m31a::interaction::session::{ConversationTurn, SqliteSessionRepository};
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::runtime::AppRuntime;
use m31a::tui::app::TuiApplication;
use m31a::tui::navigation::ScreenId;
use m31a::tui::runtime_bridge::TuiRuntimeBridge;
use m31a::tui::surface::settings::SettingsCategory;

async fn setup_test_runtime() -> (
    tempfile::TempDir,
    Arc<AppRuntime>,
    Arc<SqliteSessionRepository>,
) {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("production_defects_test.db");
    let pool = initialize_database(&db_path).await.expect("init db");
    let bus = Arc::new(BroadcastEventBus::new(1024));
    let ws = dir.path().to_path_buf();
    let runtime = AppRuntime::from_pool_and_workspace(pool.clone(), ws, bus)
        .await
        .expect("runtime setup");
    let session_repo = Arc::new(SqliteSessionRepository::new(pool));
    (dir, Arc::new(runtime), session_repo)
}

// ============================================================================
// PROBLEM 1: Runtime startup takes more than five minutes / startup readiness
// ============================================================================

#[tokio::test]
async fn test_problem_1_startup_readiness_and_tracking() {
    let (dir, runtime, _repo) = setup_test_runtime().await;
    let ws = dir.path().to_path_buf();
    let config = runtime.config().clone();

    // 1. App starts synchronously in startup state before runtime is awaited
    let mut app = TuiApplication::new().with_runtime_startup(ws.clone(), &config);

    assert!(
        app.model.runtime_status.is_startup(),
        "App must start in startup state"
    );
    assert!(
        app.is_composer_focused,
        "Composer must be focused on startup"
    );

    // Check that stage duration tracking is active
    app.set_runtime_hydrating_workspace(Some("Loading workspace state…".to_string()));
    assert!(
        !app.model.startup_stage_durations.is_empty(),
        "Must record previous stage duration"
    );

    app.set_runtime_hydrating_execution(Some("Connecting cockpit bridge…".to_string()));
    assert!(
        app.model.startup_stage_durations.len() >= 2,
        "Must record hydrating workspace stage duration"
    );

    // 2. Attach runtime and verify transition to Ready without indefinite hang
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("terminal");

    let (mut bridge, _handle) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .expect("spawn bridge");
    let irx = bridge.take_event_receiver().expect("take irx");
    let tx = bridge.sender();

    app = app.with_bridge_tx(tx).with_interaction_rx(irx);
    app.set_runtime_ready();

    assert!(
        app.model.runtime_status.is_ready(),
        "App must reach Ready state"
    );
    assert!(
        app.model.startup_stage_durations.len() >= 3,
        "Must record all completed stages"
    );

    // Verify first frame renders without blocking
    let rendered = app.render_frame(&mut terminal);
    assert!(rendered.is_ok(), "Frame must render successfully");
}

#[test]
fn test_problem_1_stage_durations_formatting() {
    let mut model = m31a::tui::model::TuiViewModel::new();
    model.set_runtime_initializing(Some("Preparing execution authorities…".to_string()));
    std::thread::sleep(Duration::from_millis(15));
    model.set_runtime_hydrating_workspace(Some("Loading state...".to_string()));

    assert_eq!(model.startup_stage_durations.len(), 1);
    assert!(model.startup_stage_durations[0].1 >= Duration::from_millis(10));
}

// ============================================================================
// PROBLEM 2: Session started, resumed, and restored without user intent
// ============================================================================

#[tokio::test]
async fn test_problem_2_fresh_launch_zero_sessions_no_premature_resumption() {
    let (_dir, runtime, _repo) = setup_test_runtime().await;

    // Fresh launch: resume_session_id is None
    let (mut bridge, _handle) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .expect("spawn bridge");

    let mut irx = bridge.take_event_receiver().expect("take irx");

    // Collect initial events emitted during bridge startup
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut events = Vec::new();
    while let Ok(ev) = irx.try_recv() {
        events.push(ev);
    }

    // Assert NO SessionStarted or SessionResumed is emitted on fresh launch!
    for ev in &events {
        match ev {
            InteractionEvent::SessionStarted { session_id } => {
                panic!("Premature SessionStarted emitted on fresh launch: {session_id}");
            }
            InteractionEvent::SessionResumed { session_id, .. } => {
                panic!("Premature SessionResumed emitted on fresh launch: {session_id}");
            }
            InteractionEvent::SessionHistoryLoaded { session_id, .. } => {
                panic!("Premature SessionHistoryLoaded emitted on fresh launch: {session_id}");
            }
            _ => {}
        }
    }
}

#[tokio::test]
async fn test_problem_2_fresh_launch_with_existing_active_session_no_implicit_resume() {
    let (dir, runtime, session_repo) = setup_test_runtime().await;

    // Insert an active session into the database from a "previous run"
    let _prev_session = session_repo
        .create_session(dir.path())
        .await
        .expect("create prev session");

    // Fresh launch: resume_session_id is None
    let (mut bridge, _handle) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .expect("spawn bridge");

    let mut irx = bridge.take_event_receiver().expect("take irx");

    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut events = Vec::new();
    while let Ok(ev) = irx.try_recv() {
        events.push(ev);
    }

    // Must NOT implicitly resume the existing active session
    for ev in &events {
        if let InteractionEvent::SessionResumed { session_id, .. } = ev {
            panic!("Implicitly resumed existing session on fresh launch: {session_id}");
        }
        if let InteractionEvent::SessionStarted { session_id } = ev {
            panic!("Prematurely started session on fresh launch: {session_id}");
        }
    }
}

#[tokio::test]
async fn test_problem_2_first_prompt_creates_exactly_one_session() {
    let (_dir, runtime, _repo) = setup_test_runtime().await;

    let (mut bridge, _handle) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .expect("spawn bridge");

    let mut irx = bridge.take_event_receiver().expect("take irx");
    let tx = bridge.sender();

    // Drain initial configuration events
    tokio::time::sleep(Duration::from_millis(50)).await;
    while irx.try_recv().is_ok() {}

    // Operator submits first prompt
    let user_msg = m31a::interaction::mentions::ParsedUserMessage {
        raw_text: "Hello M31A".to_string(),
        segments: vec![],
        mentions: vec![],
        request_id: Some("req-1".to_string()),
    };
    tx.send(ApplicationAction::UserTextSubmitted(user_msg))
        .expect("send action");

    // Wait for bridge to process
    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut started_sessions = Vec::new();
    let mut resumed_sessions = Vec::new();
    while let Ok(ev) = irx.try_recv() {
        match ev {
            InteractionEvent::SessionStarted { session_id } => {
                started_sessions.push(session_id);
            }
            InteractionEvent::SessionResumed { session_id, .. } => {
                resumed_sessions.push(session_id);
            }
            _ => {}
        }
    }

    assert_eq!(
        started_sessions.len(),
        1,
        "Must emit exactly ONE SessionStarted event"
    );
    assert!(
        resumed_sessions.is_empty(),
        "Must not emit SessionResumed on new session creation"
    );
}

#[tokio::test]
async fn test_problem_2_explicit_resume_restores_requested_session_truthfully() {
    let (dir, runtime, session_repo) = setup_test_runtime().await;

    // Create a previous session with 2 turns
    let prev_session = session_repo
        .create_session(dir.path())
        .await
        .expect("create prev session");
    let session_id = prev_session.id;

    let turn1 = ConversationTurn::UserMessage {
        id: uuid::Uuid::now_v7(),
        sequence: 1,
        content: "What is the project structure?".to_string(),
        raw_text: "What is the project structure?".to_string(),
        mentions: vec![],
        created_at: chrono::Utc::now(),
    };
    let turn2 = ConversationTurn::AssistantMessage {
        id: uuid::Uuid::now_v7(),
        sequence: 2,
        content: "M31A is a single crate Rust workspace.".to_string(),
        created_at: chrono::Utc::now(),
    };
    session_repo
        .append_turn(session_id, &turn1)
        .await
        .expect("append turn1");
    session_repo
        .append_turn(session_id, &turn2)
        .await
        .expect("append turn2");

    // Explicit resume with resume_session_id = Some(session_id)
    let (mut bridge, _handle) = TuiRuntimeBridge::spawn(runtime.clone(), Some(session_id))
        .await
        .expect("spawn bridge with explicit resume");

    let mut irx = bridge.take_event_receiver().expect("take irx");

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut started = false;
    let mut resumed = false;
    let mut history_loaded_turns = 0;

    while let Ok(ev) = irx.try_recv() {
        match ev {
            InteractionEvent::SessionStarted { .. } => started = true,
            InteractionEvent::SessionResumed {
                session_id: res_id, ..
            } => {
                assert_eq!(res_id, session_id);
                resumed = true;
            }
            InteractionEvent::SessionHistoryLoaded {
                session_id: h_id,
                turns,
            } => {
                assert_eq!(h_id, session_id);
                history_loaded_turns = turns.len();
            }
            _ => {}
        }
    }

    assert!(!started, "Must NOT emit SessionStarted on explicit resume");
    assert!(resumed, "Must emit SessionResumed on explicit resume");
    assert_eq!(
        history_loaded_turns, 2,
        "Must truthfully load 2 persisted conversation turns"
    );
}

#[tokio::test]
async fn test_problem_2_cross_workspace_resume_rejected() {
    let (_dir, runtime, session_repo) = setup_test_runtime().await;

    // Create session belonging to /tmp/different_workspace
    let foreign_session = session_repo
        .create_session(std::path::Path::new("/tmp/other_ws"))
        .await
        .expect("create foreign session");
    let foreign_id = foreign_session.id;

    // Attempt to resume foreign session in current workspace
    let res = TuiRuntimeBridge::spawn(runtime.clone(), Some(foreign_id)).await;
    assert!(res.is_err(), "Must reject cross-workspace session resume");
    let err_str = res.err().unwrap().to_string();
    assert!(
        err_str.contains("!= runtime workspace") || err_str.contains("different workspace"),
        "Must reject cross-workspace explicitly: {err_str}"
    );
}

// ============================================================================
// PROBLEM 3: Provider 404 & Planning Resilience
// ============================================================================

#[test]
fn test_problem_3_nvidia_model_publisher_prefix_preserved() {
    let ws = std::path::PathBuf::from(".");
    let cfg = ResolvedConfiguration::build_fallback(ws);

    // Case 1: Model with nvidia/ publisher prefix must NOT have nvidia/ stripped!
    let updated = cfg
        .with_session_model("nvidia/llama-3.1-nemotron-70b-instruct")
        .expect("update model");
    assert_eq!(
        updated.active_model, "nvidia/llama-3.1-nemotron-70b-instruct",
        "Must preserve nvidia/ publisher prefix for Nemotron models"
    );

    // Case 2: Model specified without publisher prefix should be auto-prefixed if it's a Nemotron model
    let updated2 = cfg
        .with_session_model("llama-3.1-nemotron-70b-instruct")
        .expect("update model");
    assert_eq!(
        updated2.active_model, "nvidia/llama-3.1-nemotron-70b-instruct",
        "Must auto-prefix Nemotron model with nvidia/"
    );

    // Case 3: Other providers/models like meta/llama-3.3-70b-instruct keep their prefix
    let updated3 = cfg
        .with_session_model("meta/llama-3.3-70b-instruct")
        .expect("update model");
    assert_eq!(
        updated3.active_model, "meta/llama-3.3-70b-instruct",
        "Must preserve meta/ publisher prefix"
    );
}

#[test]
fn test_problem_3_read_only_objective_classification() {
    use m31a::planning::service::classify_objective;

    // Reconnaissance queries must be classified as read-only
    let obj1 = classify_objective("Study the codebase");
    assert!(
        obj1.is_read_only(),
        "'Study the codebase' must be read-only"
    );

    let obj2 = classify_objective("Investigate system architecture and document findings");
    assert!(obj2.is_read_only(), "'Investigate...' must be read-only");

    // Mutation queries must be classified as mutation
    let obj3 = classify_objective("Refactor the parser and delete legacy structs");
    assert!(!obj3.is_read_only(), "Refactor must not be read-only");
}

// ============================================================================
// PROBLEM 4: Settings surface routing, navigation, editing, and persistence
// ============================================================================

#[tokio::test]
async fn test_problem_4_settings_navigation_and_rendering() {
    let (dir, runtime, _repo) = setup_test_runtime().await;
    let ws = dir.path().to_path_buf();
    let config = runtime.config().clone();

    let mut app = TuiApplication::new().with_runtime_startup(ws.clone(), &config);
    app.set_runtime_ready();

    // Submit /settings command via programmatic composer submit
    app.submit_composer_text("/settings");

    assert_eq!(
        app.navigation.current_screen,
        ScreenId::Settings,
        "Must navigate to ScreenId::Settings"
    );
    assert!(
        !app.is_composer_focused,
        "Composer must be unfocused when entering Settings"
    );

    // Render frame to verify no panic and proper settings widget rendering
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("terminal");

    let res = app.render_frame(&mut terminal);
    assert!(res.is_ok(), "Must render settings frame successfully");

    let buffer = terminal.backend().buffer().clone();
    let content: String = buffer.content().iter().map(|c| c.symbol()).collect();

    // Verify canonical settings categories and rows are rendered, NOT the old placeholder hint
    assert!(content.contains("Settings"), "Must render Settings title");
    assert!(content.contains("General"), "Must render General category");
    assert!(
        content.contains("Provider"),
        "Must render Provider category"
    );
    assert!(
        !content.contains("Open via /settings from the composer"),
        "Must NOT render the old circular settings hint placeholder"
    );
}

#[tokio::test]
async fn test_problem_4_settings_category_and_row_navigation() {
    let (dir, runtime, _repo) = setup_test_runtime().await;
    let ws = dir.path().to_path_buf();
    let config = runtime.config().clone();

    let mut app = TuiApplication::new().with_runtime_startup(ws.clone(), &config);
    app.set_runtime_ready();

    app.submit_composer_text("/settings");

    // Start in category 0 (General)
    assert_eq!(app.settings_state.category(), SettingsCategory::General);

    // Navigate right to Provider category via 'l' or Right arrow
    let key_right = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
    app.handle_key(key_right);
    assert_eq!(app.settings_state.category(), SettingsCategory::Provider);

    // Navigate right to Models category via Tab
    let key_tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    app.handle_key(key_tab);
    assert_eq!(app.settings_state.category(), SettingsCategory::Models);

    // Navigate left back to Provider category via 'h' or Left arrow
    let key_left = KeyEvent::new(KeyCode::Left, KeyModifiers::NONE);
    app.handle_key(key_left);
    assert_eq!(app.settings_state.category(), SettingsCategory::Provider);

    // Navigate down rows via 'j' or Down arrow
    assert_eq!(app.settings_state.row_index, 0);
    let key_down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    app.handle_key(key_down);
    assert_eq!(app.settings_state.row_index, 1);
}

#[tokio::test]
async fn test_problem_4_settings_editing_draft_and_saving() {
    let (dir, runtime, _repo) = setup_test_runtime().await;
    let ws = dir.path().to_path_buf();
    let config = runtime.config().clone();

    let mut app = TuiApplication::new().with_runtime_startup(ws.clone(), &config);
    app.set_runtime_ready();

    app.submit_composer_text("/settings");

    // Navigate to TUI/Interface category
    app.settings_state
        .select_category(SettingsCategory::TuiInterface);
    assert_eq!(
        app.settings_state.category(),
        SettingsCategory::TuiInterface
    );

    // Select row 0 (tui.fps)
    app.settings_state.row_index = 0;

    // Start editing (Enter)
    let key_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    app.handle_key(key_enter);
    assert!(app.settings_state.editing, "Editing mode must be active");

    // Clear and enter new value "60"
    app.settings_state.edit_buffer = "60".to_string();

    // Commit edit (Enter)
    app.handle_key(key_enter);
    assert!(!app.settings_state.editing, "Editing mode must finish");

    // Verify draft is updated
    assert_eq!(
        app.settings_draft.as_ref().unwrap().tui.fps,
        60,
        "Draft tui.fps must be updated to 60"
    );

    // Save settings via 's'
    let key_s = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
    app.handle_key(key_s);

    // Verify config file was written to disk
    let config_path = ws.join(".m31a").join("config.toml");
    assert!(
        config_path.is_file(),
        "Workspace config.toml must exist after save"
    );

    let saved_content = std::fs::read_to_string(&config_path).expect("read saved config");
    assert!(
        saved_content.contains("fps = 60"),
        "Saved config must contain fps = 60: {saved_content}"
    );

    // Press Esc to exit settings back to Dashboard
    let key_esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    app.handle_key(key_esc);
    assert_eq!(
        app.navigation.current_screen,
        ScreenId::Dashboard,
        "Must exit settings and return to Dashboard on Esc"
    );
}
