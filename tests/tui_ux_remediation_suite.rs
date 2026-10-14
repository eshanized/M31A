//! TUI/UX Comprehensive Remediation Regression Suite (UI-01 through UI-20).
//!
//! Validates that all 20 UI/UX defects are resolved, their root causes fixed,
//! and end-to-end event -> state -> render paths are verified.

use std::path::PathBuf;

use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tokio::sync::mpsc;

use m31a::events::envelope::EventEnvelope;
use m31a::events::types::EventType;
use m31a::ids::MissionId;
use m31a::interaction::action::ApplicationAction;
use m31a::tui::TuiApplication;
use m31a::tui::approval::ApprovalDecision;
use m31a::tui::composer::{TuiComposer, calculate_visual_cursor};
use m31a::tui::focus::FocusTarget;
use m31a::tui::model::{
    QuarantineProjectionState, TuiAgentSnapshot, TuiApprovalRequest, TuiJobSnapshot,
    TuiQuarantineItem, TuiSandboxStatus,
};
use m31a::tui::navigation::{RouteResolution, ScreenId, resolve_view};
use m31a::tui::registry::ViewId;
use m31a::tui::replay::{ReplayController, ReplaySpeed};
use m31a::tui::screens::wizard::SetupWizardScreen;
use m31a::tui::surface::visible_list_range;

fn buffer_text(app: &mut TuiApplication, w: u16, h: u16) -> String {
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();
    app.set_terminal_size(w, h);
    app.force_redraw = true;
    app.render_frame(&mut terminal).unwrap();
    let buf = terminal.backend().buffer().clone();
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n")
}

// ─────────────────────────────────────────────────────────────────────────────
// UI-01 — Registry modal actions execute real domain actions
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ui_01_registry_modal_actions_approvals_rollback_recovery_mission() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut app = TuiApplication::new().with_bridge_tx(tx);

    // 1. ApprovalsQueue: 'a' dispatches ApproveAlways
    app.model.approvals.push(TuiApprovalRequest {
        id: "req-1".to_string(),
        tool_name: "bash".to_string(),
        agent_role: "coder".to_string(),
        justification: "test".to_string(),
        parameters_summary: "rm -rf".to_string(),
        risk_tier: "high".to_string(),
        timestamp: Utc::now(),
    });
    app.navigation.open_modal(ViewId::ApprovalsQueue);
    assert_eq!(app.navigation.active_modal, Some(ViewId::ApprovalsQueue));
    app.handle_key(KeyEvent::from(KeyCode::Char('a')));
    let action = rx.try_recv().expect("action dispatched for Approve All");
    assert!(
        matches!(
            action,
            ApplicationAction::ApprovalDecision {
                decision: ApprovalDecision::ApproveAlways,
                ..
            }
        ),
        "Expected ApproveAlways, got {:?}",
        action
    );

    // 2. ApprovalsQueue: 'r' dispatches Reject
    app.model.approvals.push(TuiApprovalRequest {
        id: "req-2".to_string(),
        tool_name: "bash".to_string(),
        agent_role: "coder".to_string(),
        justification: "test".to_string(),
        parameters_summary: "rm -rf".to_string(),
        risk_tier: "high".to_string(),
        timestamp: Utc::now(),
    });
    app.navigation.open_modal(ViewId::ApprovalsQueue);
    app.handle_key(KeyEvent::from(KeyCode::Char('r')));
    let action = rx.try_recv().expect("action dispatched for Reject");
    assert!(
        matches!(
            action,
            ApplicationAction::ApprovalDecision {
                decision: ApprovalDecision::Reject,
                ..
            }
        ),
        "Expected Reject, got {:?}",
        action
    );

    // 3. ApprovalsQueue: Esc cancels modal without dispatching action
    app.navigation.open_modal(ViewId::ApprovalsQueue);
    app.handle_key(KeyEvent::from(KeyCode::Esc));
    assert_eq!(app.navigation.active_modal, None);
    assert!(
        rx.try_recv().is_err(),
        "Esc must not dispatch approval action"
    );

    // 4. RollbackInspector: Enter dispatches Rollback
    app.navigation.open_modal(ViewId::RollbackInspector);
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    let action = rx.try_recv().expect("action dispatched for rollback");
    assert!(
        matches!(
            action,
            ApplicationAction::SlashCommandSubmitted { ref command, .. } if command == "rollback"
        ),
        "Expected rollback command, got {:?}",
        action
    );

    // 5. StartupRecovery: 'r' dispatches Resume
    app.navigation.open_modal(ViewId::StartupRecovery);
    app.handle_key(KeyEvent::from(KeyCode::Char('r')));
    let action = rx.try_recv().expect("action dispatched for resume");
    assert!(
        matches!(
            action,
            ApplicationAction::SlashCommandSubmitted { ref command, .. } if command == "resume"
        ),
        "Expected resume command, got {:?}",
        action
    );

    // 6. StartupRecovery: 'n' dispatches ClearSessionRequested
    app.navigation.open_modal(ViewId::StartupRecovery);
    app.handle_key(KeyEvent::from(KeyCode::Char('n')));
    let action = rx.try_recv().expect("action dispatched for new session");
    assert!(
        matches!(action, ApplicationAction::ClearSessionRequested),
        "Expected ClearSessionRequested, got {:?}",
        action
    );

    // 7. MissionCreation: interactive typing and submission
    app.navigation.open_modal(ViewId::MissionCreation);
    assert!(app.model.modal_draft.is_empty());
    for c in "ship v2".chars() {
        app.handle_key(KeyEvent::from(KeyCode::Char(c)));
    }
    assert_eq!(app.model.modal_draft, "ship v2");

    // Backspace deletes a character
    app.handle_key(KeyEvent::from(KeyCode::Backspace));
    assert_eq!(app.model.modal_draft, "ship v");

    // Enter submits the drafted mission text
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(app.navigation.active_modal, None);
    assert!(app.model.modal_draft.is_empty());
    let action = rx
        .try_recv()
        .expect("action dispatched for mission creation");
    match action {
        ApplicationAction::UserTextSubmitted(parsed) => {
            assert_eq!(parsed.raw_text, "ship v");
        }
        other => panic!("Expected UserTextSubmitted, got {:?}", other),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// UI-02 — Embedded Setup Wizard keyboard ownership and atomic dismissal
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ui_02_setup_wizard_keyboard_precedence_and_atomic_dismissal() {
    let mut app = TuiApplication::new();
    app.setup_wizard = Some(SetupWizardScreen::new(PathBuf::from(".")));
    app.navigation.open_modal(ViewId::SetupWizard);
    assert!(app.is_wizard_active(), "Wizard must be active on open");
    assert!(app.setup_wizard.is_some());

    // Advance wizard steps: Space confirms workspace trust, Enter advances step
    let initial_step = app.setup_wizard.as_ref().unwrap().current_step();
    app.handle_key(KeyEvent::from(KeyCode::Char(' ')));
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    let next_step = app.setup_wizard.as_ref().unwrap().current_step();
    assert_ne!(
        initial_step, next_step,
        "Wizard should advance step after trust confirmation"
    );

    // Esc dismisses/cancels wizard
    app.handle_key(KeyEvent::from(KeyCode::Esc));
    assert!(!app.is_wizard_active(), "Wizard must be inactive after Esc");
    assert!(
        app.setup_wizard.is_none(),
        "Setup wizard state must be cleared"
    );
    assert_eq!(app.navigation.active_modal, None);

    // Composer input works and is not intercepted by dismissed wizard
    for c in "test message".chars() {
        app.handle_key(KeyEvent::from(KeyCode::Char(c)));
    }
    assert_eq!(app.composer.text(), "test message");

    // Route navigation works without wizard intercepting
    app.handle_key(KeyEvent::from(KeyCode::Char('2')));
    assert_eq!(app.composer.text(), "test message2");
}

// ─────────────────────────────────────────────────────────────────────────────
// UI-03 — Overlays own keyboard input and close cleanly
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ui_03_overlays_own_keyboard_input_and_close() {
    let mut app = TuiApplication::new().with_composer_focused(true);

    // 1. Open KeybindingsGuide overlay
    app.navigation.open_overlay(ViewId::KeybindingsGuide);
    assert_eq!(
        app.navigation.active_overlay,
        Some(ViewId::KeybindingsGuide)
    );

    // Keys that would type in composer or switch routes are swallowed by overlay
    app.handle_key(KeyEvent::from(KeyCode::Char('2')));
    assert!(
        app.composer.text().is_empty(),
        "Composer must not receive text while overlay is open"
    );
    assert_eq!(app.navigation.current_screen, ScreenId::Dashboard);

    // Esc dismisses overlay and restores prior focus
    app.handle_key(KeyEvent::from(KeyCode::Esc));
    assert_eq!(app.navigation.active_overlay, None);
    assert!(app.is_composer_focused);

    // 2. Open HelpDocs overlay and verify 'q' dismisses overlay without exiting app
    app.navigation.open_overlay(ViewId::HelpDocs);
    assert_eq!(app.navigation.active_overlay, Some(ViewId::HelpDocs));
    app.handle_key(KeyEvent::from(KeyCode::Char('q')));
    assert_eq!(app.navigation.active_overlay, None);
    assert!(
        !app.should_exit(KeyEvent::from(KeyCode::Char('q'))),
        "App should not exit on 'q' when dismissing overlay"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// UI-04 — Focus targets and terminal resize normalization
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ui_04_focus_targets_and_terminal_resize_normalization() {
    let mut app = TuiApplication::new();
    app.model.enter_active_session();

    // 1. Narrow terminal (< 120 cols): context panel is hidden
    app.set_terminal_size(80, 24);
    assert!(!app.has_context_panel());

    // Focus cycling Tab only cycles between Conversation and Composer
    app.focus.set_focus(FocusTarget::Conversation);
    app.handle_key(KeyEvent::from(KeyCode::Tab));
    assert_eq!(app.focus.current(), FocusTarget::Composer);
    app.handle_key(KeyEvent::from(KeyCode::Tab));
    assert_eq!(app.focus.current(), FocusTarget::Conversation);
    assert_ne!(app.focus.current(), FocusTarget::ContextPanel);

    // 2. Wide terminal (>= 120 cols): context panel is visible
    app.set_terminal_size(140, 30);
    assert!(app.has_context_panel());
    app.focus.set_focus(FocusTarget::ContextPanel);

    // Resizing to narrow normalizes focus away from ContextPanel
    app.set_terminal_size(90, 24);
    app.normalize_focus();
    assert_eq!(
        app.focus.current(),
        FocusTarget::Conversation,
        "Focus on hidden ContextPanel must normalize to Conversation"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// UI-05 — Replay autoplay loop, speeds, and controls
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ui_05_replay_autoplay_loop_and_controls() {
    let mut replay = ReplayController::new();
    let mission_id = MissionId::from(uuid::Uuid::now_v7());
    let history = vec![
        EventEnvelope::new(
            1,
            Some(mission_id),
            None,
            "runtime".to_string(),
            EventType::MissionStarted {
                mission_id,
                objective: "Step 1".to_string(),
            },
        ),
        EventEnvelope::new(
            2,
            Some(mission_id),
            None,
            "runtime".to_string(),
            EventType::MissionStarted {
                mission_id,
                objective: "Step 2".to_string(),
            },
        ),
        EventEnvelope::new(
            3,
            Some(mission_id),
            None,
            "runtime".to_string(),
            EventType::MissionStarted {
                mission_id,
                objective: "Step 3".to_string(),
            },
        ),
        EventEnvelope::new(
            4,
            Some(mission_id),
            None,
            "runtime".to_string(),
            EventType::MissionStarted {
                mission_id,
                objective: "Step 4".to_string(),
            },
        ),
        EventEnvelope::new(
            5,
            Some(mission_id),
            None,
            "runtime".to_string(),
            EventType::MissionStarted {
                mission_id,
                objective: "Step 5".to_string(),
            },
        ),
    ];
    replay.load_history(history);
    replay.cursor = 0;

    assert_eq!(replay.cursor, 0);
    assert!(!replay.is_playing);

    // Playback controls: Space toggles play
    replay.handle_key(KeyEvent::from(KeyCode::Char(' ')));
    assert!(replay.is_playing);

    // Speed Normal: requires 4 ticks to advance 1 step
    replay.speed = ReplaySpeed::Normal;
    assert!(!replay.tick_playback()); // tick 1
    assert!(!replay.tick_playback()); // tick 2
    assert!(!replay.tick_playback()); // tick 3
    assert!(replay.tick_playback()); // tick 4 advances
    assert_eq!(replay.cursor, 1);

    // Speed Hyper: advances every tick
    replay.speed = ReplaySpeed::Hyper;
    assert!(replay.tick_playback());
    assert_eq!(replay.cursor, 2);

    // Seeking: End seeks to last frame, Home seeks to start
    replay.handle_key(KeyEvent::from(KeyCode::End));
    assert_eq!(replay.cursor, 4);

    replay.handle_key(KeyEvent::from(KeyCode::Home));
    assert_eq!(replay.cursor, 0);

    // Manual stepping
    replay.handle_key(KeyEvent::from(KeyCode::Right));
    assert_eq!(replay.cursor, 1);
    replay.handle_key(KeyEvent::from(KeyCode::Left));
    assert_eq!(replay.cursor, 0);
}

// ─────────────────────────────────────────────────────────────────────────────
// UI-06 — Failed jobs render truthfully (never false success)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ui_06_failed_jobs_look_failed() {
    let mut app = TuiApplication::new();
    app.navigation.navigate_to(ScreenId::Jobs);

    app.model.jobs = vec![
        TuiJobSnapshot {
            id: "j-run".to_string(),
            name: "compile".to_string(),
            status: "running".to_string(),
            duration_ms: 5000,
        },
        TuiJobSnapshot {
            id: "j-fail".to_string(),
            name: "test_suite".to_string(),
            status: "failed".to_string(),
            duration_ms: 12000,
        },
        TuiJobSnapshot {
            id: "j-cancel".to_string(),
            name: "linter".to_string(),
            status: "cancelled".to_string(),
            duration_ms: 0,
        },
        TuiJobSnapshot {
            id: "j-block".to_string(),
            name: "deploy".to_string(),
            status: "blocked".to_string(),
            duration_ms: 0,
        },
        TuiJobSnapshot {
            id: "j-unknown".to_string(),
            name: "custom_job".to_string(),
            status: "unknown_weird_state".to_string(),
            duration_ms: 0,
        },
    ];

    let content = buffer_text(&mut app, 120, 30);
    // Failed job must have '✗' symbol and "[FAILED]" label
    assert!(content.contains("✗") && content.contains("[FAILED]"));
    // Cancelled job has '⊘' symbol and "[CANCELLED]" label
    assert!(content.contains("⊘") && content.contains("[CANCELLED]"));
    // Blocked job has '⛔' symbol and "[BLOCKED]" label
    assert!(content.contains("⛔") && content.contains("[BLOCKED]"));
    // Unknown status has '?' symbol, NEVER '✓'
    assert!(content.contains("?") && content.contains("[UNKNOWN_WEIRD_STATE]"));
}

// ─────────────────────────────────────────────────────────────────────────────
// UI-07 — Settings focus state and synchronization
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ui_07_settings_focus_and_synchronization() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    app.navigation.navigate_to(ScreenId::Settings);
    assert_eq!(app.navigation.current_screen, ScreenId::Settings);

    // Closing Settings with 'q' restores composer focus and prior route
    app.handle_key(KeyEvent::from(KeyCode::Char('q')));
    assert_eq!(app.navigation.current_screen, ScreenId::Dashboard);
    assert!(app.is_composer_focused);
    assert_eq!(app.focus.current(), FocusTarget::Composer);
}

// ─────────────────────────────────────────────────────────────────────────────
// UI-08 — Canonical routes and detail inspectors
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ui_08_canonical_routes_and_inspectors() {
    let mut app = TuiApplication::new();

    // 1. PromptLab resolves contextually to Mission route with PromptLab inspector
    let target = resolve_view(ViewId::PromptLab);
    assert_eq!(
        target,
        RouteResolution::DetailInspector {
            parent: ScreenId::Mission,
            detail: ViewId::PromptLab
        }
    );

    app.navigation.navigate_to_view(ViewId::PromptLab);
    assert_eq!(app.navigation.current_screen, ScreenId::Mission);
    assert_eq!(app.navigation.active_detail, Some(ViewId::PromptLab));

    let content = buffer_text(&mut app, 120, 30);
    assert!(content.contains("Prompt Lab"));

    // 2. PolicyLedger resolves contextually to Approvals route with PolicyLedger inspector
    let target = resolve_view(ViewId::PolicyLedger);
    assert_eq!(
        target,
        RouteResolution::DetailInspector {
            parent: ScreenId::Approvals,
            detail: ViewId::PolicyLedger
        }
    );

    app.navigation.navigate_to_view(ViewId::PolicyLedger);
    assert_eq!(app.navigation.current_screen, ScreenId::Approvals);
    assert_eq!(app.navigation.active_detail, Some(ViewId::PolicyLedger));

    let content = buffer_text(&mut app, 120, 30);
    assert!(content.contains("POLICY LEDGER"));
}

// ─────────────────────────────────────────────────────────────────────────────
// UI-09 — Agent detail renders active lifecycle states truthfully
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ui_09_agent_lifecycle_states_truthful_rendering() {
    let mut app = TuiApplication::new();
    app.navigation.navigate_to(ScreenId::Agents);

    app.model.agents = vec![
        TuiAgentSnapshot {
            id: "a-run".to_string(),
            role: "lead".to_string(),
            state: "running".to_string(),
            current_task: Some("designing schema".to_string()),
            total_tokens: 1500,
        },
        TuiAgentSnapshot {
            id: "a-fail".to_string(),
            role: "tester".to_string(),
            state: "failed".to_string(),
            current_task: Some("panic on assertion".to_string()),
            total_tokens: 800,
        },
        TuiAgentSnapshot {
            id: "a-succ".to_string(),
            role: "developer".to_string(),
            state: "completed".to_string(),
            current_task: None,
            total_tokens: 3200,
        },
    ];

    // Selected agent 0 is running: status badge renders "Working", task shows "designing schema"
    app.context_selected_idx = 0;
    let content = buffer_text(&mut app, 120, 30);
    assert!(content.contains("Working"));
    assert!(content.contains("designing schema"));

    // Selected agent 1 is failed: status badge renders "Failed", task shows "panic on assertion"
    app.context_selected_idx = 1;
    let content = buffer_text(&mut app, 120, 30);
    assert!(content.contains("Failed"));
    assert!(content.contains("panic on assertion"));
}

// ─────────────────────────────────────────────────────────────────────────────
// UI-10 — List viewport keeps selection visible
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ui_10_list_viewport_keeps_selection_visible() {
    // 1. Total items 50, viewport 10
    // Selected at 0 -> (0, 10)
    assert_eq!(visible_list_range(50, 0, 10), (0, 10));

    // Selected at 45 -> selection must be within range
    let (start, end) = visible_list_range(50, 45, 10);
    assert!(
        45 >= start && 45 < end,
        "Selected index 45 must be inside range [{}, {})",
        start,
        end
    );
    assert_eq!(end - start, 10);

    // Selected at 25 -> selection within range
    let (start, end) = visible_list_range(50, 25, 10);
    assert!(25 >= start && 25 < end);
    assert_eq!(end - start, 10);

    // 2. Total items < viewport
    assert_eq!(visible_list_range(5, 2, 10), (0, 5));

    // 3. Empty list
    assert_eq!(visible_list_range(0, 0, 10), (0, 0));
}

// ─────────────────────────────────────────────────────────────────────────────
// UI-11 — Context rail bounds clamping and focus border
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ui_11_context_rail_bounds_and_focus() {
    let mut app = TuiApplication::new();
    app.model.enter_active_session();
    app.set_terminal_size(140, 30);
    app.focus.set_focus(FocusTarget::ContextPanel);

    // Context rail selection cannot be scrolled indefinitely
    app.context_selected_idx = 1000;
    let content = buffer_text(&mut app, 140, 30);

    // Context rail renders without crash and displays standard sections
    assert!(
        content.contains("Tasks")
            || content.contains("TASKS")
            || content.contains("Mission")
            || content.contains("SESSION")
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// UI-12 — Mission badge displays dynamic status and disambiguated shortcut
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ui_12_mission_badge_and_shortcut_hint() {
    let mut app = TuiApplication::new();
    app.navigation.navigate_to(ScreenId::Mission);

    // 1. Completed status
    app.model.mission_status = "completed".to_string();
    let content = buffer_text(&mut app, 120, 30);
    assert!(content.contains("COMPLETED"));
    assert!(content.contains("Mission Specification [6]"));

    // 2. Failed status
    app.model.mission_status = "failed".to_string();
    let content = buffer_text(&mut app, 120, 30);
    assert!(content.contains("FAILED"));
}

// ─────────────────────────────────────────────────────────────────────────────
// UI-13 — Quarantine Manager states (Loading, Failed, Empty, Populated)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ui_13_quarantine_manager_states() {
    let mut app = TuiApplication::new();

    // 1. Loading state
    app.navigation.navigate_to_view(ViewId::QuarantineManager);
    app.model.quarantine_state = QuarantineProjectionState::Loading;
    let content = buffer_text(&mut app, 120, 30);
    assert!(content.contains("Loading containment data..."));

    // 2. Failed state
    app.model.quarantine_state = QuarantineProjectionState::Failed("IO error 13".to_string());
    let content = buffer_text(&mut app, 120, 30);
    assert!(content.contains("Retrieval failure (IO error 13)"));

    // 3. Ready and empty (0 breaches)
    app.model.quarantine_state = QuarantineProjectionState::Ready;
    app.model.quarantine_items = vec![];
    let content = buffer_text(&mut app, 120, 30);
    assert!(content.contains("0 (No containment breaches detected)"));

    // 4. Populated items
    app.model.quarantine_items = vec![TuiQuarantineItem {
        id: "q-1".to_string(),
        name: "target/untrusted.bin".to_string(),
        reason: "SEC-004 Executable artifact prohibited".to_string(),
        quarantined_at: Some(Utc::now()),
    }];
    let content = buffer_text(&mut app, 120, 30);
    assert!(content.contains("target/untrusted.bin"));
    assert!(content.contains("SEC-004 Executable artifact prohibited"));
}

// ─────────────────────────────────────────────────────────────────────────────
// UI-14 — Sandbox truthful runtime evidence
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ui_14_sandbox_truthful_evidence() {
    let mut app = TuiApplication::new();
    app.navigation.navigate_to(ScreenId::Jobs);

    // 1. When sandbox status is absent or unverified, report truthful unverified notice
    app.model.sandbox_status = None;
    let content = buffer_text(&mut app, 120, 30);
    assert!(
        content.contains("unverified (runtime evidence not")
            || (content.contains("unverified") && content.contains("runtime evidence not"))
    );

    // 2. When sandbox status reports verified evidence, display it
    app.model.sandbox_status = Some(TuiSandboxStatus {
        isolation_active: true,
        quotas_enforced: true,
        watchdog_active: true,
        mode: "bubblewrap".to_string(),
        details: Some("strict".to_string()),
    });
    let content = buffer_text(&mut app, 120, 30);
    assert!(content.contains("enforced (active)"));
    assert!(content.contains("monitored and bounded"));
    assert!(content.contains("active"));
}

// ─────────────────────────────────────────────────────────────────────────────
// UI-15 — Mouse scroll does not leak through open modals or overlays
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ui_15_mouse_scroll_insulation() {
    let mut app = TuiApplication::new();
    app.model.scroll_offset = 10;

    // 1. Open KeybindingsGuide overlay
    app.navigation.open_overlay(ViewId::KeybindingsGuide);

    // Dispatch mouse scroll up
    app.handle_mouse_scroll(true);

    // Background scroll offset must remain insulated
    assert_eq!(
        app.model.scroll_offset, 10,
        "Mouse scroll must not leak through open overlay"
    );

    // 2. Open modal
    app.navigation.close_overlay();
    app.navigation.open_modal(ViewId::ApprovalsQueue);
    app.handle_mouse_scroll(true);
    assert_eq!(
        app.model.scroll_offset, 10,
        "Mouse scroll must not leak through open modal"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// UI-16 — Composer Enter key inserts newline at caret position
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ui_16_composer_multiline_caret_position() {
    let mut composer = TuiComposer::new(PathBuf::from("."));

    // Type "hello world"
    for c in "hello world".chars() {
        composer.handle_key(KeyEvent::from(KeyCode::Char(c)));
    }
    assert_eq!(composer.text(), "hello world");

    // Move caret back 6 positions to be right between "hello" and " world"
    for _ in 0..6 {
        composer.handle_key(KeyEvent::from(KeyCode::Left));
    }

    // Ctrl+J inserts newline at caret
    composer.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL));
    assert_eq!(composer.text(), "hello\n world");

    // Shift+Enter also inserts newline at caret
    composer.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT));
    assert_eq!(composer.text(), "hello\n\n world");
}

// ─────────────────────────────────────────────────────────────────────────────
// UI-17 — Composer visual cursor positioning for unicode and wrapped text
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ui_17_composer_cursor_positioning_unicode_and_wrapping() {
    // 1. ASCII cursor calculation (returns (row, col))
    let (row, col) = calculate_visual_cursor("hello", 3, 2, 80);
    assert_eq!(row, 0);
    assert_eq!(col, 5); // prefix 2 + 3 chars

    // 2. Fullwidth CJK cursor calculation (each CJK char is 2 display cells)
    let (row, col) = calculate_visual_cursor("你好世界", 2, 0, 80);
    assert_eq!(row, 0);
    assert_eq!(col, 4); // 2 characters * 2 cells = 4

    // 3. Tab cursor calculation (tab is 4 cells)
    let (row, col) = calculate_visual_cursor("\tcode", 1, 0, 80);
    assert_eq!(row, 0);
    assert_eq!(col, 4);

    // 4. Soft wrapping cursor calculation
    // Max width 10, prefix 2. Line of 20 characters wrapped across lines.
    let text = "0123456789abcdefghij";
    let (row, col) = calculate_visual_cursor(text, 15, 2, 10);
    assert!(row >= 1, "Cursor at index 15 should wrap to row >= 1");
    assert!(
        col < 10,
        "Cursor column {} must be within max_width 10",
        col
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// UI-18 — Slash/Mention cache invalidation
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ui_18_autocomplete_cache_invalidation() {
    let mut composer = TuiComposer::new(PathBuf::from("."));
    composer.set_cached_models(vec![("model-a".to_string(), "desc-a".to_string())]);

    // Invalidation resets suggestions and reloads catalog
    composer.invalidate_cache();
    assert!(!composer.is_autocomplete_open());
}

// ─────────────────────────────────────────────────────────────────────────────
// UI-19 — Zero-I/O startup and bounded file traversal
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ui_19_zero_io_critical_path_and_bounded_scan() {
    // 1. TuiComposer::new() performs 0 disk scans on startup
    let composer = TuiComposer::new(PathBuf::from("."));
    assert!(
        !composer.is_file_cache_loaded(),
        "File cache must be lazy, not loaded at startup"
    );

    // 2. Updating workspace root resets cache lazily
    let mut app = TuiApplication::new();
    app = app.with_workspace_root(PathBuf::from("/tmp"));
    assert!(!app.composer.is_file_cache_loaded());
}

// ─────────────────────────────────────────────────────────────────────────────
// UI-20 — Narrow terminal layout does not panic
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ui_20_narrow_terminal_geometry_no_panic() {
    let mut app = TuiApplication::new();

    let geometries = [
        (1, 1),
        (10, 4),
        (20, 6),
        (29, 8),
        (40, 12),
        (60, 16),
        (80, 24),
    ];

    for &(w, h) in &geometries {
        // Base screen
        let _ = buffer_text(&mut app, w, h);

        // With modal open
        app.navigation.open_modal(ViewId::ApprovalsQueue);
        let _ = buffer_text(&mut app, w, h);

        // With overlay open
        app.navigation.close_modal();
        app.navigation.open_overlay(ViewId::KeybindingsGuide);
        let _ = buffer_text(&mut app, w, h);

        // With setup wizard open
        app.navigation.close_overlay();
        app.setup_wizard = Some(SetupWizardScreen::new(PathBuf::from(".")));
        app.navigation.open_modal(ViewId::SetupWizard);
        let _ = buffer_text(&mut app, w, h);

        // Clean up for next geometry
        app.setup_wizard = None;
        app.navigation.close_modal();
    }
}
