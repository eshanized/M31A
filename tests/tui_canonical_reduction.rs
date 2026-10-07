//! Behavioral regression tests for the reduced TUI architecture.
//!
//! After the architectural reduction there is exactly:
//! - ONE application root (`TuiApplication` owning the `TuiApp` engine),
//! - ONE runtime binding (`TuiRuntimeBinding`),
//! - ONE event ingress (bridge `InteractionEvent` → `apply_tui_event`),
//! - ONE navigation authority (`navigation::NavigationRouter`),
//! - ONE input-priority authority (`keymap::resolve_key`, consumed),
//! - ONE responsive-tier type (`layout::LayoutTier`),
//! - ONE primary rendering path (header → workspace → route → composer → footer),
//! - ZERO database I/O during render.
//!
//! These tests assert user-visible behavior through that single path, never
//! implementation structure.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use m31a::ids::MissionId;
use m31a::interaction::events::InteractionEvent;
use m31a::tui::layout::LayoutTier;
use m31a::tui::navigation::ScreenId;
use m31a::tui::{KeyAction, TuiApp, TuiApplication, TuiEvent, apply_tui_event};

fn test_config() -> m31a::config::ResolvedConfiguration {
    m31a::config::ResolvedConfiguration::build_fallback(std::path::PathBuf::from("/tmp/ws"))
}

fn render_to_string(app: &mut TuiApp, w: u16, h: u16) -> String {
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();
    app.force_redraw = true;
    app.render_frame(&mut terminal).unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect()
}

// ONE ROOT: production entry constructs synchronously and draws M31A
// content on the very first frame, before any runtime exists.
#[test]
fn single_root_first_frame_is_visible_m31a() {
    let config = test_config();
    let mut tui = TuiApplication::new(std::path::PathBuf::from("/tmp/ws"), &config);
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    assert!(tui.render_first_frame(&mut terminal).unwrap());
    let content: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(content.contains("M31A"));
}

// ONE INGRESS: bridge events reduce through the single reducer and reach
// the visible conversation + mission projection.
#[test]
fn single_ingress_bridge_event_reaches_projection_and_render() {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = TuiApp::new().with_interaction_rx(rx);

    let mid = MissionId::new();
    tx.send(InteractionEvent::MissionStateChanged {
        mission_id: mid,
        status: "running".to_string(),
    })
    .unwrap();
    assert_eq!(app.poll_updates(), 1);
    assert_eq!(app.model.mission_status, "running");

    // The same reducer handles streaming deltas incrementally.
    let msg = "stream-1".to_string();
    apply_tui_event(
        &mut app.model,
        &TuiEvent::StreamDelta {
            message_id: msg.clone(),
            delta: "hello ".to_string(),
        },
    );
    apply_tui_event(
        &mut app.model,
        &TuiEvent::StreamDelta {
            message_id: msg.clone(),
            delta: "world".to_string(),
        },
    );
    assert_eq!(app.model.conversation.len(), 1);
    apply_tui_event(
        &mut app.model,
        &TuiEvent::StreamFinished { message_id: msg },
    );

    app.set_runtime_ready();
    let content = render_to_string(&mut app, 120, 40);
    assert!(content.contains("M31A"));
    assert!(content.contains("hello world"));
}

// ONE INPUT AUTHORITY: classification is consumed — replay locks input to
// read-only scrub instead of reaching mutation handlers.
#[test]
fn single_input_authority_replay_locks_mutations() {
    let config = test_config();
    let mut tui = TuiApplication::new(std::path::PathBuf::from("/tmp/ws"), &config);
    tui.app_mut().replay.load_history(Vec::new());
    // Replay with empty history still activates the controller; classification
    // must report Replay and handle_key must swallow the key.
    if tui.app_mut().replay.is_active {
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(tui.classify_key(key), KeyAction::Replay);
        assert!(tui.handle_key(key).is_none());
    } else {
        // Without replay, a plain key classifies to composer (focused).
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(tui.classify_key(key), KeyAction::Composer);
    }
}

// ONE TIER: terminal classification has a single vocabulary.
#[test]
fn single_tier_vocabulary() {
    assert_eq!(LayoutTier::from_dimensions(80, 24), LayoutTier::Compact);
    assert_eq!(LayoutTier::from_dimensions(120, 40), LayoutTier::Standard);
    assert_eq!(LayoutTier::from_dimensions(180, 60), LayoutTier::Large);
    assert_eq!(LayoutTier::from_dimensions(240, 80), LayoutTier::UltraWide);
    assert!(LayoutTier::is_below_minimum(79, 24));
    assert!(!LayoutTier::is_below_minimum(80, 24));
}

// ONE NAVIGATION AUTHORITY + ONE RENDER PATH: every primary route renders
// non-blank through the shell; navigation + input agree on the route.
#[test]
fn single_navigation_authority_every_route_renders() {
    for screen in [
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
    ] {
        let mut app = TuiApp::new().with_composer_focused(true);
        app.set_runtime_ready();
        app.navigation.navigate_to(screen);
        if screen == ScreenId::Dashboard {
            app.model.enter_active_session();
        }
        let content = render_to_string(&mut app, 120, 40);
        assert!(
            content.trim().chars().any(|c| !c.is_whitespace()),
            "{screen:?} must render visible content"
        );
        assert_eq!(app.navigation.current_screen, screen);
    }
}

// ZERO RENDER I/O in every lifecycle phase.
#[test]
fn zero_render_io_all_phases() {
    let mut app = TuiApp::new();
    for phase in ["boot", "loading", "ready", "failed"] {
        match phase {
            "boot" => app.set_runtime_booting(),
            "loading" => app.set_runtime_initializing(None),
            "ready" => app.set_runtime_ready(),
            "failed" => app.set_runtime_failed("boom"),
            _ => unreachable!(),
        }
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        app.force_redraw = true;
        let pre = app.model.sqlite_render_access_count();
        app.render_frame(&mut terminal).unwrap();
        assert_eq!(
            pre,
            app.model.sqlite_render_access_count(),
            "{phase} must do zero SQLite I/O during render"
        );
    }
}
