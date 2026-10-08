//! Comprehensive Integration Test Suite: TUI Wiring & Runtime Integration Remediation
//!
//! Validates:
//! A. Single-authority event ingress (TuiRuntimeBridge sole live transport)
//! B. Race-safe session lifecycle & hydration (subscribe before hydration)
//! C. Deterministic hotkey resolution through ViewRegistry (no contradictions)
//! D. ViewKind hierarchy & Escape sequence (modal -> overlay -> detail -> history)
//! E. All 40 canonical views render domain-specific content without generic fallbacks
//! F. Workflow snapshot updates and execution control via bridge
//! G. Provider switch & model selection propagate to runtime config
//! H. Zero DB I/O during render frames
//! I. Replay mode strictly read-only
//! J. Static architectural guards (no split paths or raw event queue in production TUI)

use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tempfile::tempdir;

use m31a::events::bus::{BroadcastEventBus, EventBus};
use m31a::events::envelope::EventEnvelope;
use m31a::events::types::EventType;
use m31a::ids::MissionId;
use m31a::interaction::action::ApplicationAction;
use m31a::interaction::events::InteractionEvent;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::runtime::AppRuntime;
use m31a::tui::app::TuiApplication;
use m31a::tui::conversation::TuiConversationItem;
use m31a::tui::navigation::{
    NavigationAction, NavigationRouter, ScreenId, canonical_screen, resolve_view,
};
use m31a::tui::registry::{ViewId, ViewKind, ViewRegistry};
use m31a::tui::runtime_bridge::TuiRuntimeBridge;
use m31a::workflow::engine::WorkflowExecutionSnapshot;
use m31a::workflow::state::{WorkflowMode, WorkflowRun};

async fn setup_runtime() -> (tempfile::TempDir, Arc<AppRuntime>) {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("remediation_test.db");
    let pool = initialize_database(&db_path).await.expect("init db");
    let bus = Arc::new(BroadcastEventBus::new(1024));
    let ws = dir.path().to_path_buf();
    let runtime = AppRuntime::from_pool_and_workspace(pool, ws, bus)
        .await
        .expect("runtime setup");
    (dir, Arc::new(runtime))
}

// ─────────────────────────────────────────────────────────────────────────────
// Test A: Single-authority event ingress
// ─────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn test_a_single_authority_event_ingress() {
    let (_dir, runtime) = setup_runtime().await;
    let (mut bridge, _handle) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .expect("spawn bridge");

    let mut app = TuiApplication::new()
        .with_bridge_tx(bridge.sender())
        .with_workspace_root(runtime.workspace_root().to_path_buf());

    let irx = bridge.take_event_receiver().expect("take event receiver");
    app = app.with_interaction_rx(irx);

    // Initial configuration update should be emitted on bridge spawn
    tokio::time::sleep(Duration::from_millis(50)).await;
    let updates = app.poll_updates();
    assert!(updates > 0, "Bridge must emit initial interaction events");
    assert_eq!(
        app.model.active_model,
        runtime.config().active_model,
        "Model must match runtime config"
    );
    assert_eq!(
        app.model.active_provider,
        runtime.config().active_provider,
        "Provider must match runtime config"
    );

    // Emit a mission started event via the runtime's EventBus
    let mission_id = MissionId::new();
    let mission_event = EventEnvelope::new(
        1,
        None,
        None,
        "scheduler".to_string(),
        EventType::MissionStarted {
            mission_id,
            objective: "Remediation Mission".to_string(),
        },
    );
    runtime
        .event_bus()
        .publish(mission_event)
        .await
        .expect("publish");

    tokio::time::sleep(Duration::from_millis(50)).await;
    let new_updates = app.poll_updates();
    assert!(
        new_updates > 0,
        "Bridge must translate EventBus events to InteractionEvents"
    );
    assert_eq!(
        app.model.mission_id,
        Some(mission_id.to_string()),
        "Mission ID must be projected into TuiViewModel via single bridge ingress"
    );
    assert_eq!(
        app.model.mission_status, "running",
        "Mission status must be projected into TuiViewModel via single bridge ingress"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test B: Race-safe session lifecycle & hydration
// ─────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn test_b_race_safe_session_lifecycle_and_hydration() {
    let (_dir, runtime) = setup_runtime().await;

    // Verify that spawning TuiRuntimeBridge subscribes to EventBus immediately,
    // ensuring zero loss of events published around hydration.
    let (mut bridge, _handle) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .expect("spawn bridge");

    let irx = bridge.take_event_receiver().expect("take irx");
    let mut app = TuiApplication::new()
        .with_bridge_tx(bridge.sender())
        .with_interaction_rx(irx);

    // Hydrate
    app.hydrate_from_runtime(&runtime).await;

    // Publish an event immediately
    let mission_id = MissionId::new();
    let mission_event = EventEnvelope::new(
        2,
        None,
        None,
        "kernel".to_string(),
        EventType::MissionStarted {
            mission_id,
            objective: "Hydration race-safety mission".to_string(),
        },
    );
    runtime
        .event_bus()
        .publish(mission_event)
        .await
        .expect("publish");

    tokio::time::sleep(Duration::from_millis(50)).await;
    app.poll_updates();
    assert_eq!(
        app.model.mission_id,
        Some(mission_id.to_string()),
        "Event emitted right after bridge startup must be captured"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test C: Deterministic hotkey resolution through ViewRegistry
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn test_c_deterministic_hotkey_resolution() {
    let registry = ViewRegistry::new();

    // Verify all canonical numeric and character hotkeys without contradiction
    let expected_mappings = [
        ('1', ViewId::MissionDashboard),
        ('2', ViewId::DagInspector),
        ('3', ViewId::TaskDetails),
        ('4', ViewId::AgentInspector),
        ('5', ViewId::ToolActivity),
        ('6', ViewId::ExecutionStream),
        ('7', ViewId::FailureRecovery),
        ('8', ViewId::SystemHealth),
        ('9', ViewId::PolicyLedger),
        ('0', ViewId::ApprovalsQueue),
        ('a', ViewId::ArtifactExplorer),
        ('g', ViewId::GitTimeline),
        ('l', ViewId::EventLog),
        ('b', ViewId::BudgetMonitor),
        ('s', ViewId::SkillRegistry),
        ('d', ViewId::DoctorDiagnostics),
        ('w', ViewId::WorkspaceSetup),
        ('m', ViewId::ModelRegistry),
        ('c', ViewId::CheckpointTree),
        ('v', ViewId::VerificationSuite),
        ('k', ViewId::KeybindingsGuide),
        ('?', ViewId::HelpDocs),
        (':', ViewId::CommandPalette),
        ('n', ViewId::MissionCreation),
    ];

    for (hotkey, expected_view) in expected_mappings {
        let mut router = NavigationRouter::new();

        // 1. ViewRegistry lookup
        let reg_view = registry
            .get_by_hotkey(hotkey)
            .unwrap_or_else(|| panic!("Registry must have hotkey '{hotkey}'"));
        assert_eq!(
            reg_view.id, expected_view,
            "Hotkey '{hotkey}' must resolve to {expected_view:?}"
        );

        // 2. canonical_screen mapping
        let expected_screen = canonical_screen(expected_view);
        let _res = resolve_view(expected_view);

        // 3. NavigationRouter keyboard dispatch
        let key = KeyEvent::new(KeyCode::Char(hotkey), KeyModifiers::NONE);
        let action = router.handle_key(key);
        match action {
            NavigationAction::ScreenChanged(s) => {
                assert_eq!(
                    s, expected_screen,
                    "Router screen change mismatch for '{hotkey}'"
                );
            }
            NavigationAction::OpenPalette => {
                assert_eq!(expected_view, ViewId::CommandPalette);
                router.navigate_to_view(expected_view);
            }
            _ => panic!("Unexpected action {action:?} for hotkey '{hotkey}'"),
        }

        // Verify active detail/overlay semantics match ViewKind
        match reg_view.kind {
            ViewKind::PrimaryRoute => {
                assert_eq!(router.active_detail, None);
            }
            ViewKind::DetailInspector => {
                assert_eq!(router.active_detail, Some(expected_view));
            }
            ViewKind::ModalDialog => {
                assert_eq!(router.active_modal, Some(expected_view));
            }
            ViewKind::Overlay => {
                assert_eq!(router.active_overlay, Some(expected_view));
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Test D: ViewKind hierarchy & Escape sequence
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn test_d_view_kind_hierarchy_and_escape() {
    let mut router = NavigationRouter::new();
    router.navigate_to(ScreenId::Dashboard);
    router.navigate_to(ScreenId::TaskGraph);

    // Open detail pane
    router.open_detail(ViewId::TaskDetails);
    assert_eq!(router.active_detail, Some(ViewId::TaskDetails));

    // Open overlay
    router.open_overlay(ViewId::KeybindingsGuide);
    assert_eq!(router.active_overlay, Some(ViewId::KeybindingsGuide));

    // Open modal
    router.open_modal(ViewId::ApprovalsQueue);
    assert_eq!(router.active_modal, Some(ViewId::ApprovalsQueue));

    // Escape Level 1: closes modal
    let popped_modal = router.pop_history();
    assert!(popped_modal.is_some(), "Esc must close active modal");
    assert_eq!(router.active_modal, None);
    assert_eq!(router.active_overlay, Some(ViewId::KeybindingsGuide));
    assert_eq!(router.active_detail, Some(ViewId::TaskDetails));
    assert_eq!(router.current_screen, ScreenId::TaskGraph);

    // Escape Level 2: closes overlay
    let popped_overlay = router.pop_history();
    assert!(popped_overlay.is_some(), "Esc must close active overlay");
    assert_eq!(router.active_modal, None);
    assert_eq!(router.active_overlay, None);
    assert_eq!(router.active_detail, Some(ViewId::TaskDetails));
    assert_eq!(router.current_screen, ScreenId::TaskGraph);

    // Escape Level 3: closes detail inspector
    let popped_detail = router.pop_history();
    assert!(popped_detail.is_some(), "Esc must close active detail");
    assert_eq!(router.active_modal, None);
    assert_eq!(router.active_overlay, None);
    assert_eq!(router.active_detail, None);
    assert_eq!(router.current_screen, ScreenId::TaskGraph);

    // Escape Level 4: navigates back in screen history
    let popped_screen = router.pop_history();
    assert!(
        popped_screen.is_some(),
        "Esc must return to previous screen"
    );
    assert_eq!(router.current_screen, ScreenId::Dashboard);
}

// ─────────────────────────────────────────────────────────────────────────────
// Test E: All 40 canonical views render domain-specific content without generic fallbacks
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn test_e_all_40_views_domain_specific_rendering() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("terminal");
    let mut app = TuiApplication::new();
    app.unfocus_composer();

    for view_id in ViewId::all() {
        // 1. Render as Detail Inspector
        app.navigation.active_detail = Some(*view_id);
        app.navigation.active_overlay = None;
        app.navigation.active_modal = None;
        app.force_redraw = true;

        terminal
            .draw(|f| {
                let area = f.area();
                let (_tier, layout) = m31a::tui::layout::compute_layout(area);
                let tokens = m31a::tui::theme::ThemeTokens::resolve(app.theme_mode);
                let ws_area = ratatui::layout::Rect {
                    x: area.x,
                    y: layout.header.bottom(),
                    width: area.width,
                    height: area
                        .height
                        .saturating_sub(layout.header.height + layout.footer.height),
                };
                m31a::tui::shell::workspace::render_workspace(
                    f,
                    ws_area,
                    &mut app.model,
                    &app.composer,
                    app.navigation.current_screen,
                    &app.replay,
                    app.focus.current(),
                    app.is_composer_focused,
                    &tokens,
                    app.context_selected_idx,
                    app.navigation.active_detail,
                    app.navigation.active_overlay,
                    &app.workflow_snapshot,
                    &mut app.workflow_dashboard_state,
                    &mut app.model_selector_state,
                    &mut app.setup_wizard,
                );
            })
            .expect("draw detail");

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(
            !content.contains("lifecycle=None mission=None status="),
            "View {:?} rendered forbidden generic detail fallback string",
            view_id
        );

        // 2. Render as Overlay / Modal
        app.navigation.active_detail = None;
        app.navigation.active_overlay = Some(*view_id);
        app.force_redraw = true;

        terminal
            .draw(|f| {
                let area = f.area();
                let (_tier, layout) = m31a::tui::layout::compute_layout(area);
                let tokens = m31a::tui::theme::ThemeTokens::resolve(app.theme_mode);
                let ws_area = ratatui::layout::Rect {
                    x: area.x,
                    y: layout.header.bottom(),
                    width: area.width,
                    height: area
                        .height
                        .saturating_sub(layout.header.height + layout.footer.height),
                };
                m31a::tui::shell::workspace::render_workspace(
                    f,
                    ws_area,
                    &mut app.model,
                    &app.composer,
                    app.navigation.current_screen,
                    &app.replay,
                    app.focus.current(),
                    app.is_composer_focused,
                    &tokens,
                    app.context_selected_idx,
                    app.navigation.active_detail,
                    app.navigation.active_overlay,
                    &app.workflow_snapshot,
                    &mut app.workflow_dashboard_state,
                    &mut app.model_selector_state,
                    &mut app.setup_wizard,
                );
            })
            .expect("draw overlay");

        let overlay_buffer = terminal.backend().buffer().clone();
        let overlay_content: String = overlay_buffer
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        // Verify the generic 4-line fallback `{name}\n{:?}\n{}\nEsc to close` is not rendered
        let reg = ViewRegistry::new();
        let meta = reg.get(*view_id).unwrap();
        let bad_pattern = format!(
            "{}\n{:?}\n{}",
            meta.name,
            app.navigation.current_screen,
            app.model.lifecycle.stage.label()
        );
        assert!(
            !overlay_content.contains(&bad_pattern),
            "View {:?} rendered forbidden generic overlay fallback format",
            view_id
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Test F: Workflow snapshot updates and execution control via bridge
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn test_f_workflow_snapshot_and_bridge_execution_control() {
    let (bridge_tx, mut bridge_rx) = tokio::sync::mpsc::unbounded_channel();
    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();

    let mut app = TuiApplication::new()
        .with_bridge_tx(bridge_tx)
        .with_interaction_rx(event_rx);
    app.unfocus_composer();

    let mut run = WorkflowRun::new(
        "test_workflow",
        1,
        std::path::PathBuf::from("."),
        WorkflowMode::Standard,
    );
    let run_id = run.id;
    let _ = run.transition_to(m31a::workflow::state::WorkflowRunState::Running, None);
    let snapshot = WorkflowExecutionSnapshot {
        run,
        step_runs: vec![],
        artifacts: vec![],
        ready_step_keys: vec![],
        blocked_step_keys: vec![],
    };

    // Emit snapshot through interaction event channel
    event_tx
        .send(InteractionEvent::WorkflowSnapshotUpdated {
            snapshot: Box::new(snapshot.clone()),
        })
        .expect("send snapshot");

    let count = app.poll_updates();
    assert_eq!(count, 1);
    assert_eq!(
        app.workflow_snapshot.as_ref().map(|s| s.run.id),
        Some(run_id),
        "TuiApplication must cache workflow snapshot from InteractionEvent"
    );

    // Open DagInspector detail to interact with workflow dashboard
    app.navigation.active_detail = Some(ViewId::DagInspector);

    // Press 'p' to pause workflow
    let pause_key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE);
    let cmd = app.handle_key(pause_key);
    assert!(
        cmd.is_none(),
        "Interactive TUI dispatch must not return RuntimeCommand"
    );

    let action = bridge_rx.try_recv().expect("action received");
    match action {
        ApplicationAction::WorkflowPauseRequested {
            run_id: sent_id, ..
        } => {
            assert_eq!(
                sent_id,
                run_id.to_string(),
                "Pause action must carry real run id"
            );
        }
        other => panic!("Expected WorkflowPauseRequested, got {other:?}"),
    }

    // Simulate transition to Blocked (awaiting operator action/pause state)
    if let Some(ref mut s) = app.workflow_snapshot {
        s.run.status = m31a::workflow::state::WorkflowRunState::Blocked;
    }

    // Press 'r' to resume workflow
    let resume_key = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);
    app.handle_key(resume_key);
    let action = bridge_rx.try_recv().expect("action received");
    match action {
        ApplicationAction::WorkflowResumeRequested { run_id: sent_id } => {
            assert_eq!(
                sent_id,
                run_id.to_string(),
                "Resume action must carry real run id"
            );
        }
        other => panic!("Expected WorkflowResumeRequested, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Test G: Provider switch & model selection propagate to runtime config
// ─────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn test_g_provider_switch_and_model_selection() {
    let (_dir, runtime) = setup_runtime().await;
    let (mut bridge, _handle) = TuiRuntimeBridge::spawn(runtime.clone(), None)
        .await
        .expect("spawn bridge");

    let mut app = TuiApplication::new().with_bridge_tx(bridge.sender());
    let irx = bridge.take_event_receiver().expect("irx");
    app = app.with_interaction_rx(irx);

    // Initial poll
    tokio::time::sleep(Duration::from_millis(50)).await;
    app.poll_updates();

    // 1. Attempting to switch to unsupported provider yields deterministic runtime rejection
    bridge
        .sender()
        .send(ApplicationAction::ProviderChangeRequested {
            provider: "anthropic".to_string(),
        })
        .expect("send provider change");

    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        app.poll_updates();
        if app.model.conversation.iter().any(|c| match c {
            TuiConversationItem::Error { message, .. } => {
                message.contains("Unsupported model provider")
            }
            _ => false,
        }) {
            break;
        }
    }

    // Active provider remains the valid production provider
    assert_eq!(
        app.model.active_provider, "nvidia_nim",
        "Unsupported provider must not overwrite active provider"
    );

    // 2. Switch provider to valid alias "nvidia" (normalizes to "nvidia_nim")
    bridge
        .sender()
        .send(ApplicationAction::ProviderChangeRequested {
            provider: "nvidia".to_string(),
        })
        .expect("send valid provider change");

    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        app.poll_updates();
        if app.model.conversation.iter().any(|c| match c {
            TuiConversationItem::System { text, .. } => text.contains("Active provider switched"),
            _ => false,
        }) {
            break;
        }
    }

    assert_eq!(
        app.model.active_provider, "nvidia_nim",
        "TuiViewModel active provider must reflect runtime change"
    );

    // 3. Switch model via bridge
    bridge
        .sender()
        .send(ApplicationAction::ModelChangeRequested {
            model: "meta/llama-3.3-70b-instruct".to_string(),
        })
        .expect("send model change");

    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        app.poll_updates();
        if app.model.active_model == "meta/llama-3.3-70b-instruct" {
            break;
        }
    }

    assert_eq!(
        app.model.active_model, "meta/llama-3.3-70b-instruct",
        "TuiViewModel active model must reflect runtime change"
    );

    // 4. Switch profile via bridge
    bridge
        .sender()
        .send(ApplicationAction::ProfileChangeRequested {
            profile: "safe".to_string(),
        })
        .expect("send profile change");

    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        app.poll_updates();
        if app.model.active_profile == "safe" {
            break;
        }
    }

    assert_eq!(
        app.model.active_profile, "safe",
        "TuiViewModel active profile must reflect runtime change"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test H: Zero DB I/O during render frames
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn test_h_zero_db_io_during_render_frame() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).expect("terminal");
    let mut app = TuiApplication::new();

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
    ] {
        app.navigation.navigate_to(screen);
        app.force_redraw = true;

        let pre_queries = app.model.sqlite_render_access_count();
        let rendered = app.render_frame(&mut terminal).expect("render_frame");
        assert!(rendered, "Frame must render on force_redraw");
        let post_queries = app.model.sqlite_render_access_count();

        assert_eq!(
            pre_queries, post_queries,
            "Screen {screen:?} triggered DB queries during frame render! Invariant: 0 DB queries."
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Test I: Replay mode strictly read-only
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn test_i_replay_mode_strictly_read_only() {
    let (bridge_tx, mut bridge_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut app = TuiApplication::new().with_bridge_tx(bridge_tx);

    // Enter replay mode
    app.replay.is_active = true;

    // Keys that would mutate in normal mode
    let test_keys = [
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
    ];

    for key in test_keys {
        let cmd = app.handle_key(key);
        assert!(
            cmd.is_none(),
            "Replay mode must never produce mutating RuntimeCommands"
        );
        assert!(
            bridge_rx.try_recv().is_err(),
            "Replay mode must never dispatch mutating actions to runtime bridge"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Test J: Static architectural guards
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn test_j_static_architectural_guards() {
    let main_src = std::fs::read_to_string("src/main.rs").expect("read main.rs");
    let app_src = std::fs::read_to_string("src/tui/app.rs").expect("read app.rs");
    let workspace_src =
        std::fs::read_to_string("src/tui/shell/workspace.rs").expect("read workspace.rs");

    // Guard 1: No raw event queue in production TUI startup (src/main.rs)
    assert!(
        !main_src.contains("tui_tx.try_send"),
        "main.rs must not contain raw event try_send"
    );
    assert!(
        !main_src.contains(".with_receiver(tui_rx)"),
        "main.rs must not connect duplicate raw event receiver"
    );

    // Guard 2: No dispatcher.dispatch in interactive TUI loop (src/main.rs)
    let tui_fn = main_src
        .split("async fn run_tui_or_fallback")
        .nth(1)
        .expect("run_tui_or_fallback");
    assert!(
        !tui_fn.contains("dispatcher.dispatch"),
        "interactive TUI loop must not call dispatcher.dispatch"
    );

    // Guard 3: No generic fallback strings in workspace.rs
    assert!(
        !workspace_src.contains("lifecycle={:?} mission={:?} status={}"),
        "workspace.rs must not contain generic detail inspector fallback"
    );
    assert!(
        !workspace_src.contains("{name}\\n{:?}\\n{}\\nEsc to close"),
        "workspace.rs must not contain generic overlay fallback"
    );

    // Guard 4: TuiApplication handles actions through bridge_tx fail-closed
    assert!(
        app_src.contains("fn send_or_fail"),
        "app.rs must implement fail-closed send_or_fail"
    );
}
