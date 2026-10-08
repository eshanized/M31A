//! First-frame regression: `m31a tui` must immediately show the cockpit.
//!
//! Reproduces the blank alternate-screen symptom at the framebuffer level:
//! terminal initialized -> first `render_frame` must contain recognizable
//! M31A cockpit content (identity/header, startup state, composer shell,
//! footer hints). No path may silently produce an empty buffer.
//!
//! Covers:
//! A. Fresh TuiApplication renders visible content.
//! B. Fresh TuiApplication remains visible before runtime initialization.
//! C. Runtime init failure produces a visible TUI error, not blank.
//! D. Runtime becomes available and TUI transitions to ready state.
//! E. Bridge startup delivers initial session/configuration events.
//! F. Hydration populates the view model.
//! G. First user key reaches the TUI.
//! H. Composer is visible before runtime.
//! I. Every canonical screen renders identifiable content.
//! J. Modal/overlay never destroys the underlying shell.
//! K. Event loop continues rendering without runtime events.
//! L. Cross-width rendering remains valid.
//! M. Bridge task termination surfaces an error.
//! N. Terminal restoration still works on errors/panic.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::sync::Arc;
use std::time::Duration;

use m31a::events::bus::{BroadcastEventBus, EventBus};
use m31a::events::envelope::EventEnvelope;
use m31a::events::types::EventType;
use m31a::ids::MissionId;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::runtime::AppRuntime;
use m31a::tui::TuiApplication;
use m31a::tui::navigation::ScreenId;
use m31a::tui::runtime_bridge::TuiRuntimeBridge;

fn render_to_string(app: &mut TuiApplication, w: u16, h: u16) -> (String, bool) {
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    app.force_redraw = true;
    let rendered = app.render_frame(&mut terminal).expect("render_frame");
    let buf = terminal.backend().buffer().clone();
    let text: String = buf.content().iter().map(|c| c.symbol()).collect();
    (text, rendered)
}

fn render_lines(app: &mut TuiApplication, w: u16, h: u16) -> String {
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    app.force_redraw = true;
    app.render_frame(&mut terminal).expect("render_frame");
    let buf = terminal.backend().buffer().clone();
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_visible_cockpit(content: &str, ctx: &str) {
    assert!(
        content.contains("M31A"),
        "{ctx}: first frame must contain M31A identity, got:\n{}",
        &content[..content.len().min(2000)]
    );
    // Shell must be present: composer affordance (prompt or placeholder) and
    // footer/help hints. Startup panel also carries explicit state.
    let has_composer = content.contains('>')
        || content.contains("Ask M31A")
        || content.contains("Composer")
        || content.contains("composer");
    assert!(
        has_composer,
        "{ctx}: first frame must contain visible composer shell"
    );
}

async fn setup_runtime() -> (tempfile::TempDir, Arc<AppRuntime>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("first_frame_test.db");
    let pool = initialize_database(&db_path).await.expect("init db");
    let bus = Arc::new(BroadcastEventBus::new(1024));
    let ws = dir.path().to_path_buf();
    let rt = AppRuntime::from_pool_and_workspace(pool, ws, bus)
        .await
        .expect("runtime");
    (dir, Arc::new(rt))
}

// A. Fresh TuiApplication renders visible content.
#[test]
fn test_a_fresh_app_renders_visible_first_frame() {
    let mut app = TuiApplication::new()
        .with_workspace_root(std::path::PathBuf::from("/tmp/ws"))
        .with_composer_focused(true);
    assert!(app.is_running, "initial app must start running");
    assert!(app.force_redraw, "initial app must force first redraw");
    let (content, rendered) = render_to_string(&mut app, 120, 40);
    assert!(rendered, "fresh render_frame must report a draw");
    assert_visible_cockpit(&content, "fresh app");
    assert!(
        content.contains("M31A"),
        "fresh frame must contain M31A identity"
    );
}

// B. Fresh TuiApplication remains visible before runtime initialization.
#[test]
fn test_b_startup_states_never_blank() {
    for (state, needle) in [
        ("booting", "Booting"),
        ("initializing", "Initializing M31A runtime"),
        ("hydrating", "Loading workspace state"),
    ] {
        let mut app = TuiApplication::new().with_composer_focused(true);
        match state {
            "booting" => app.set_runtime_booting(),
            "initializing" => {
                app.set_runtime_initializing(Some("Preparing execution authorities…".into()));
            }
            "hydrating" => {
                app.set_runtime_hydrating_workspace(Some("Loading workspace state…".into()))
            }
            _ => unreachable!(),
        }
        let (content, rendered) = render_to_string(&mut app, 120, 40);
        assert!(rendered, "{state} must draw");
        assert_visible_cockpit(&content, state);
        assert!(
            content.contains(needle),
            "{state} must show explicit startup state ({needle})"
        );
        // Startup steps always visible.
        assert!(
            content.contains("Preparing execution authorities"),
            "{state} must list init steps"
        );
        assert!(
            content.contains("Loading workspace state"),
            "{state} must list init steps"
        );
        assert!(
            content.contains("Connecting cockpit bridge"),
            "{state} must list init steps"
        );
    }
}

// C. Runtime initialization failure produces a visible TUI error, not blank.
#[test]
fn test_c_runtime_failure_is_visible_not_blank() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    app.set_runtime_failed("failed to assemble complete AppRuntime for cockpit: boom");
    let (content, rendered) = render_to_string(&mut app, 120, 40);
    assert!(rendered, "failed state must draw");
    assert_visible_cockpit(&content, "failed");
    assert!(
        content.contains("Startup failed") || content.contains("failed"),
        "failure must be explicit"
    );
    assert!(
        content.contains("boom"),
        "failure reason must be visible, got:\n{content}"
    );
}

// D. Runtime becomes available and TUI transitions to ready state.
#[test]
fn test_d_startup_to_ready_transition() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    app.set_runtime_initializing(None);
    let (boot_content, _) = render_to_string(&mut app, 120, 40);
    assert!(
        boot_content.contains("Initializing"),
        "must show initializing"
    );

    app.set_runtime_hydrating_workspace(None);
    let (hyd_content, _) = render_to_string(&mut app, 120, 40);
    assert!(
        hyd_content.contains("Loading workspace state"),
        "must show hydrating"
    );

    app.set_runtime_ready();
    assert!(app.is_runtime_ready(), "must be ready after transition");
    let (ready_content, _) = render_to_string(&mut app, 120, 40);
    assert_visible_cockpit(&ready_content, "ready");
    // Ready returns to the normal cockpit (welcome for a fresh session).
    assert!(
        ready_content.contains("M31A"),
        "ready frame must keep M31A identity"
    );
}

// E. Bridge startup delivers initial session/configuration events.
#[tokio::test]
async fn test_e_bridge_startup_delivers_initial_events() {
    let (_dir, runtime) = setup_runtime().await;
    let (mut bridge, _handle) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .expect("spawn bridge");
    let mut app = TuiApplication::new()
        .with_bridge_tx(bridge.sender())
        .with_workspace_root(runtime.workspace_root().to_path_buf());
    let irx = bridge.take_event_receiver().expect("event receiver");
    app = app.with_interaction_rx(irx);
    // Startup sequence: runtime ready -> hydrate -> bridge events.
    app.set_runtime_hydrating_workspace(None);
    app.hydrate_from_runtime(&runtime).await;
    app.set_runtime_ready();
    tokio::time::sleep(Duration::from_millis(80)).await;
    let n = app.poll_updates();
    assert!(
        n > 0,
        "bridge must emit initial session/configuration events"
    );
    assert!(
        app.model.session_id.is_some() || !app.model.conversation.is_empty(),
        "session/config hydration must reach the view model"
    );
    let (content, _) = render_to_string(&mut app, 120, 40);
    assert_visible_cockpit(&content, "bridge ready");
}

// F. Hydration populates the view model.
#[tokio::test]
async fn test_f_hydration_populates_view_model() {
    let (_dir, runtime) = setup_runtime().await;
    let mut app = TuiApplication::new().with_workspace_root(runtime.workspace_root().to_path_buf());
    app.set_runtime_hydrating_workspace(None);
    app.hydrate_from_runtime(&runtime).await;
    app.set_runtime_ready();
    // Hydration must not wipe shell invariants.
    assert!(app.is_running);
    let (content, rendered) = render_to_string(&mut app, 120, 40);
    assert!(rendered);
    assert_visible_cockpit(&content, "hydrated");
}

// G. First user key reaches the TUI (composer editable during startup).
#[test]
fn test_g_first_key_reaches_composer_during_startup() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    app.set_runtime_initializing(None);
    for c in "hello".chars() {
        app.handle_key(KeyEvent::from(KeyCode::Char(c)));
    }
    assert!(
        app.composer.text().contains("hello"),
        "composer must stay editable while runtime initializes"
    );
    let content = render_lines(&mut app, 120, 40);
    assert_visible_cockpit(&content, "typing during startup");
    assert!(content.contains("hello"), "typed text must be visible");
}

// H. Composer is visible before runtime (bridge absent is a state, not blank).
#[test]
fn test_h_composer_visible_without_bridge() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    app.set_runtime_initializing(None);
    assert!(
        app.bridge_tx.is_none(),
        "precondition: no bridge before runtime"
    );
    let content = render_lines(&mut app, 120, 40);
    assert_visible_cockpit(&content, "no bridge");
    // Submitting without a bridge must fail closed with a visible error,
    // never blank the cockpit.
    for c in "do work".chars() {
        app.handle_key(KeyEvent::from(KeyCode::Char(c)));
    }
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    let after = render_lines(&mut app, 120, 40);
    assert_visible_cockpit(&after, "submit without bridge");
    assert!(
        after.contains("M31A"),
        "cockpit must survive a gated submit"
    );
}

// I. Every canonical screen renders identifiable content.
#[test]
fn test_i_every_screen_renders_identifiable_content() {
    let screens = [
        ScreenId::Dashboard,
        ScreenId::Mission,
        ScreenId::TaskGraph,
        ScreenId::Agents,
        ScreenId::Tools,
        ScreenId::Jobs,
        ScreenId::Verification,
        ScreenId::Git,
        ScreenId::Approvals,
        ScreenId::Doctor,
        ScreenId::Logs,
        ScreenId::ModelUsage,
        ScreenId::Artifacts,
        ScreenId::Replay,
        ScreenId::Help,
        ScreenId::Settings,
    ];
    for screen in screens {
        let mut app = TuiApplication::new().with_composer_focused(true);
        app.set_runtime_ready();
        app.navigation.navigate_to(screen);
        // Non-dashboard screens render the active workspace (not welcome).
        if screen == ScreenId::Dashboard {
            app.model.enter_active_session();
        }
        app.unfocus_composer();
        let (content, rendered) = render_to_string(&mut app, 120, 40);
        assert!(rendered, "{screen:?} must draw");
        assert!(
            content.contains("M31A"),
            "{screen:?} must keep M31A shell identity"
        );
        assert!(
            content.trim().chars().any(|c| !c.is_whitespace()),
            "{screen:?} must not be blank"
        );
    }
}

// J. Modal/overlay never destroys the underlying shell.
#[test]
fn test_j_modal_never_destroys_shell() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    app.set_runtime_ready();
    let req = m31a::tui::model::TuiApprovalRequest {
        id: "req-j".to_string(),
        tool_name: "cargo test".to_string(),
        agent_role: "Implementer".to_string(),
        justification: "Run tests".to_string(),
        parameters_summary: "cargo test".to_string(),
        risk_tier: "high".to_string(),
        timestamp: chrono::Utc::now(),
    };
    app.approval_modal.open(req);
    let (content, _) = render_to_string(&mut app, 120, 40);
    assert!(content.contains("M31A"), "modal must keep shell");
    assert!(
        content.contains("needs your approval") || content.contains("approval"),
        "modal content must be visible"
    );

    app.approval_modal.close();
    app.palette.open();
    let (pal_content, _) = render_to_string(&mut app, 120, 40);
    assert!(
        pal_content.contains("M31A"),
        "palette must keep shell identity"
    );

    app.palette.close();
    app.overlay_manager.toggle_help();
    let (help_content, _) = render_to_string(&mut app, 120, 40);
    assert!(
        help_content.contains("M31A"),
        "help overlay must keep shell identity"
    );
}

// K. Event loop continues rendering without runtime events.
#[test]
fn test_k_tick_renders_without_runtime_events() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    app.set_runtime_initializing(None);
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("terminal");
    // First tick draws the startup frame.
    let first = app.tick(&mut terminal).expect("tick");
    assert!(first, "first tick must draw");
    // Second tick with no new events and no dirty state may skip drawing,
    // but the framebuffer must RETAIN visible cockpit content (alternate
    // screen persists; no blanking).
    let _ = app.tick(&mut terminal).expect("tick");
    let buf = terminal.backend().buffer().clone();
    let content: String = buf.content().iter().map(|c| c.symbol()).collect();
    assert_visible_cockpit(&content, "steady startup frame");
    // New input dirties the model and must draw again.
    app.handle_key(KeyEvent::from(KeyCode::Char('x')));
    let third = app.tick(&mut terminal).expect("tick");
    assert!(third, "dirty tick must draw");
}

// L. Cross-width rendering remains valid.
#[test]
fn test_l_cross_width_rendering_never_blank() {
    let sizes: [(u16, u16); 9] = [
        (80, 24),
        (100, 30),
        (120, 35),
        (120, 40),
        (140, 40),
        (160, 45),
        (180, 45),
        (220, 60),
        (300, 70),
    ];
    for (w, h) in sizes {
        for state in ["initializing", "hydrating", "ready", "failed"] {
            let mut app = TuiApplication::new().with_composer_focused(true);
            match state {
                "initializing" => app.set_runtime_initializing(None),
                "hydrating" => app.set_runtime_hydrating_workspace(None),
                "ready" => app.set_runtime_ready(),
                "failed" => app.set_runtime_failed("boom"),
                _ => unreachable!(),
            }
            let backend = TestBackend::new(w, h);
            let mut terminal = Terminal::new(backend).expect("terminal");
            app.force_redraw = true;
            app.render_frame(&mut terminal)
                .expect("render must not fail");
            let buf = terminal.backend().buffer().clone();
            assert_eq!(buf.area.width, w, "{w}x{h} width preserved");
            assert_eq!(buf.area.height, h, "{w}x{h} height preserved");
            let content: String = buf.content().iter().map(|c| c.symbol()).collect();
            assert!(
                content.contains("M31A"),
                "{w}x{h}/{state} must contain M31A identity"
            );
            assert!(
                content.trim().chars().any(|c| !c.is_whitespace()),
                "{w}x{h}/{state} must not be blank"
            );
        }
    }
}

// M. Bridge task termination surfaces an error (no silent blank/stale Ready).
#[test]
fn test_m_bridge_termination_surfaces_error() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    app.set_runtime_ready();
    // Simulate the main-loop bridge supervisor observing a finished task.
    app.set_runtime_failed("cockpit bridge terminated unexpectedly");
    assert!(!app.is_runtime_ready());
    let (content, _) = render_to_string(&mut app, 120, 40);
    assert_visible_cockpit(&content, "bridge terminated");
    assert!(
        content.contains("bridge") || content.contains("Startup failed"),
        "bridge termination must be explicit"
    );
}

// N. Terminal restoration still works on errors/panic paths.
#[test]
fn test_n_terminal_guard_restores_idempotently() {
    // Emergency restore must be safe to call repeatedly (panic hook, Drop,
    // explicit restore) so errors never leave the terminal in raw mode.
    assert!(m31a::tui::TerminalGuard::emergency_restore().is_ok());
    assert!(m31a::tui::TerminalGuard::emergency_restore().is_ok());
    m31a::tui::install_panic_hook();
    m31a::tui::install_panic_hook();
}

// Builder invariants: TuiApplication::new + config methods must not clear shell.
#[test]
fn test_o_builder_invariants_preserve_shell() {
    let app = TuiApplication::new()
        .with_workspace_root(std::path::PathBuf::from("/tmp/ws"))
        .with_composer_focused(true);
    assert!(app.is_running, "new app must be running");
    assert!(app.force_redraw, "new app must force first redraw");
    assert!(app.is_composer_focused, "composer focus preserved");
    assert_eq!(
        app.navigation.current_screen,
        ScreenId::Dashboard,
        "valid navigation root"
    );
    assert!(!app.model.workspace_path.is_empty(), "workspace preserved");
}

// Zero DB I/O during render (strengthened): startup + ready frames.
#[test]
fn test_p_zero_db_io_during_startup_and_ready_frames() {
    for state in ["initializing", "ready", "failed"] {
        let mut app = TuiApplication::new();
        match state {
            "initializing" => app.set_runtime_initializing(None),
            "ready" => app.set_runtime_ready(),
            "failed" => app.set_runtime_failed("boom"),
            _ => unreachable!(),
        }
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("terminal");
        app.force_redraw = true;
        let pre = app.model.sqlite_render_access_count();
        app.render_frame(&mut terminal).expect("render");
        let post = app.model.sqlite_render_access_count();
        assert_eq!(
            pre, post,
            "{state} frame must perform zero SQLite I/O during draw"
        );
    }
}

// Static authority guard: TUI production paths must not rebuild authorities.
#[test]
fn test_q_no_duplicate_runtime_authorities_in_tui() {
    let forbidden = [
        "CapabilityRegistry::production",
        "ToolRegistry::new_default",
        "ToolPipelineRunner::new",
        "RoutedModelCaller::new",
        "EffectivePolicy::standard",
        "ProductionContextCompiler::new",
        "AuthorizationAuthority::new",
        "JobSupervisor::new",
        "FsArtifactStore::new",
        "ApprovalCoordinator::new",
    ];
    let files = [
        "src/tui/app.rs",
        "src/tui/binding.rs",
        "src/tui/runtime_bridge.rs",
        "src/tui/model.rs",
        "src/tui/state.rs",
        "src/tui/routes.rs",
        "src/tui/shell/workspace.rs",
        "src/tui/shell/header.rs",
        "src/tui/shell/footer.rs",
        "src/tui/composer.rs",
    ];
    for f in files {
        let src = std::fs::read_to_string(f).unwrap_or_else(|_| panic!("read {f}"));
        for needle in forbidden {
            assert!(
                !src.contains(needle),
                "TUI production file {f} must not construct {needle} (TUI is a projection)"
            );
        }
    }
    // The single canonical composition root: main.rs drives one
    // TuiApplication (first frame before runtime); the ONE runtime assembly
    // lives in main.rs — the TUI binding only attaches the finished runtime.
    let main_src = std::fs::read_to_string("src/main.rs").expect("read main.rs");
    assert!(
        main_src.contains("TuiApplication"),
        "main.rs must drive the single TuiApplication composition root"
    );
    assert!(
        main_src.contains("render_first_frame"),
        "main.rs must render the first frame before runtime assembly completes"
    );
    assert!(
        main_src.contains("AppRuntime::from_pool_workspace_and_config"),
        "the real composition root (main.rs) must be the single place assembling the canonical AppRuntime"
    );
    let binding_src = std::fs::read_to_string("src/tui/binding.rs").expect("read binding.rs");
    assert!(
        !binding_src.contains("AppRuntime::from_pool_workspace_and_config"),
        "TuiRuntimeBinding must attach the runtime, never construct it"
    );
}

// Live runtime event still reaches the projection (single ingress sanity).
#[tokio::test]
async fn test_r_live_event_reaches_projection_after_ready() {
    let (_dir, runtime) = setup_runtime().await;
    let (mut bridge, _handle) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .expect("spawn");
    let mut app = TuiApplication::new()
        .with_bridge_tx(bridge.sender())
        .with_workspace_root(runtime.workspace_root().to_path_buf());
    let irx = bridge.take_event_receiver().expect("irx");
    app = app.with_interaction_rx(irx);
    app.hydrate_from_runtime(&runtime).await;
    app.set_runtime_ready();
    tokio::time::sleep(Duration::from_millis(50)).await;
    app.poll_updates();

    let mission_id = MissionId::new();
    runtime
        .event_bus()
        .publish(EventEnvelope::new(
            99,
            None,
            None,
            "test".to_string(),
            EventType::MissionStarted {
                mission_id,
                objective: "first-frame mission".to_string(),
            },
        ))
        .await
        .expect("publish");
    tokio::time::sleep(Duration::from_millis(80)).await;
    app.poll_updates();
    assert_eq!(
        app.model.mission_id,
        Some(mission_id.to_string()),
        "live event must reach the projection through the single bridge"
    );
}
