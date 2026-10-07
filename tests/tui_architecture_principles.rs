//! TUI architecture principles (M31A visual identity preserved).
//!
//! Principle reference only — no OpenCode UI, theme, layout, branding, or
//! component hierarchy is imported. These tests assert *architectural*
//! properties of M31A's own Ratatui cockpit:
//!
//! 1. Renderer-first startup (first frame before runtime).
//!
//! 2. One composition root (`TuiApplication` + `TuiRuntimeBinding`).
//!
//! 3. Runtime authoritative, TUI is projection (section views borrow).
//!
//! 4. Event to state to render (typed `TuiEvent` reducer, zero render I/O).
//!
//! 5. State split by responsibility (views, no duplicate domain objects).
//!
//! 6. Incremental projection (deltas append, no full reloads).
//!
//! 7. Session composed surface and routes own composition.
//!
//! 8. Transient UI layered (dialog, overlay, composer, route, global).
//!
//! 9. Async phased hydration (every phase renders).
//!
//! 10. Explicit error states (every kind visible, never blank).
//!
//! 11. M31A preserved (all routes and screens).
//!
//! 12. Authority identity (no duplicate production authorities in TUI).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use m31a::tui::navigation::ScreenId;
use m31a::tui::transient::{classify_transient, input_priority};
use m31a::tui::{KeyAction, TransientUi};
use m31a::tui::{
    KeyContext, RuntimeAssemblyOutcome, TuiApp, TuiApplication, TuiError, TuiErrorKind, TuiEvent,
    TuiRuntimeBinding, TuiState, apply_tui_event, resolve_key,
};

fn render(app: &mut TuiApp, w: u16, h: u16) -> String {
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

fn test_config() -> m31a::config::ResolvedConfiguration {
    m31a::config::ResolvedConfiguration::build_fallback(std::path::PathBuf::from("/tmp/ws"))
}

// P1: renderer-first — construction + first frame need no runtime.
#[test]
fn principle_1_renderer_first_frame_without_runtime() {
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
    assert!(content.contains("M31A"), "first frame must show M31A");
    assert!(
        content.contains("Initializing") || content.contains("Booting"),
        "first frame must show startup state"
    );
}

// P2: one composition root — binding starts empty, never builds authorities.
#[test]
fn principle_2_one_binding_starts_empty() {
    let b = TuiRuntimeBinding::new();
    assert!(!b.has_runtime());
    assert!(!b.has_bridge());
    assert!(b.runtime().is_none());
    assert!(b.last_error().is_none());
}

// P3+P5: runtime authoritative — views borrow, no duplicate truth.
#[test]
fn principle_3_and_5_projection_views_borrow_truth() {
    let model = m31a::tui::model::TuiViewModel::new();
    let state = TuiState::new(&model);
    assert_eq!(state.session().workspace_path, ".");
    assert!(state.conversation().items.is_empty());
    assert_eq!(state.mission().mission_status, "idle");
    assert!(state.approvals().pending.is_empty());
    assert_eq!(state.git().branch, "N/A");
    assert!(state.tasks().is_empty());
    assert!(state.artifacts().is_empty());
}

// P4+P6: typed events reduce incrementally; deltas don't rebuild.
#[test]
fn principle_4_and_6_incremental_stream_projection() {
    let mut model = m31a::tui::model::TuiViewModel::new();
    let id = "msg-1".to_string();
    apply_tui_event(
        &mut model,
        &TuiEvent::StreamDelta {
            message_id: id.clone(),
            delta: "a".into(),
        },
    );
    apply_tui_event(
        &mut model,
        &TuiEvent::StreamDelta {
            message_id: id.clone(),
            delta: "b".into(),
        },
    );
    assert_eq!(
        model.conversation.len(),
        1,
        "deltas must append, not reload"
    );
    apply_tui_event(&mut model, &TuiEvent::StreamFinished { message_id: id });
    let mut app = TuiApp::new();
    app.model = model;
    let content = render(&mut app, 120, 40);
    assert!(content.contains("M31A"));
}

// P9: transient layering + centralized key priority.
#[test]
fn principle_9_transient_priority_dialog_overlay_composer_route() {
    let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    // Dialog wins.
    let ctx = KeyContext::new(false, true, true, true);
    assert_eq!(resolve_key(key, ctx), KeyAction::Dialog);
    // Overlay next.
    let ctx = KeyContext::new(false, false, true, true);
    assert_eq!(resolve_key(key, ctx), KeyAction::Overlay);
    // Composer before route.
    let ctx = KeyContext::new(false, false, false, true);
    assert_eq!(resolve_key(key, ctx), KeyAction::Composer);
    // Route otherwise.
    let ctx = KeyContext::new(false, false, false, false);
    assert_eq!(resolve_key(key, ctx), KeyAction::Route);
    // Replay locks everything read-only.
    let ctx = KeyContext::new(true, false, false, true);
    assert_eq!(resolve_key(key, ctx), KeyAction::Replay);

    let t = classify_transient(false, true, false, false, false);
    assert_eq!(input_priority(&t, true), m31a::tui::InputPriority::Dialog);
    let _ = TransientUi::new();
}

// P10: every hydration phase renders visible UI (partial data valid).
#[test]
fn principle_10_every_hydration_phase_renders() {
    let phases = [
        ("boot", "Booting"),
        ("loading", "Initializing"),
        ("session", "Loading session"),
        ("workspace", "Loading workspace"),
        ("execution", "Loading execution"),
    ];
    for (name, needle) in phases {
        let mut app = TuiApp::new().with_composer_focused(true);
        match name {
            "boot" => app.set_runtime_booting(),
            "loading" => app.set_runtime_initializing(None),
            "session" => app.model.set_runtime_hydrating_session(None),
            "workspace" => app.model.set_runtime_hydrating_workspace(None),
            "execution" => app.model.set_runtime_hydrating_execution(None),
            _ => unreachable!(),
        }
        let content = render(&mut app, 120, 40);
        assert!(content.contains("M31A"), "{name} must show M31A");
        assert!(content.contains(needle), "{name} must show '{needle}'");
    }
}

// P11: every error kind becomes visible state, never blank.
#[test]
fn principle_11_all_error_kinds_visible() {
    for kind in [
        TuiErrorKind::Runtime,
        TuiErrorKind::Bridge,
        TuiErrorKind::Hydration,
        TuiErrorKind::Model,
        TuiErrorKind::Workspace,
    ] {
        let mut app = TuiApp::new().with_composer_focused(true);
        let err = TuiError::new(kind, "boom");
        app.model
            .set_runtime_failed_with_kind(err.kind, err.message.clone());
        let content = render(&mut app, 120, 40);
        assert!(content.contains("M31A"), "{kind:?} must keep shell");
        assert!(content.contains("boom"), "{kind:?} reason must be visible");
        assert_eq!(app.model.runtime_error_kind, Some(kind));
    }
}

// P12: M31A preserved — every route renders through the shell.
#[test]
fn principle_12_m31a_routes_preserved() {
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
        app.unfocus_composer();
        let content = render(&mut app, 120, 40);
        assert!(content.contains("M31A"), "{screen:?} must keep M31A shell");
    }
}

// P13: zero render-time I/O in every phase.
#[test]
fn principle_13_zero_render_io_all_phases() {
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

// P14: authority identity — TUI never builds production authorities.
#[test]
fn principle_14_no_duplicate_authorities() {
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
        "src/tui/application.rs",
        "src/tui/binding.rs",
        "src/tui/state.rs",
        "src/tui/routes.rs",
        "src/tui/keymap.rs",
        "src/tui/transient.rs",
        "src/tui/errors.rs",
        "src/tui/runtime_bridge.rs",
        "src/tui/model.rs",
    ];
    for f in files {
        let src = std::fs::read_to_string(f).unwrap();
        for needle in forbidden {
            assert!(!src.contains(needle), "{f} must not construct {needle}");
        }
    }
    let _ = RuntimeAssemblyOutcome::Failed(TuiError::runtime("x"));
}
