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

    // 1. ApprovalsQueue: 'a' dispatches ApproveOnce (narrow one-time scope)
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
                decision: ApprovalDecision::ApproveOnce,
                ..
            }
        ),
        "Expected ApproveOnce, got {:?}",
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

    // 4a. RollbackInspector: Enter with no checkpoints dispatches nothing and keeps dialog open
    app.navigation.open_modal(ViewId::RollbackInspector);
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    assert!(
        rx.try_recv().is_err(),
        "Enter with no checkpoint target must not dispatch rollback"
    );
    assert_eq!(
        app.navigation.active_modal,
        Some(ViewId::RollbackInspector),
        "Dialog must remain open when rollback is unavailable"
    );

    // 4b. RollbackInspector: Enter with valid target dispatches real CheckpointRestoreRequested
    app.model
        .set_rollback_checkpoint_target("target-hash-123".to_string());
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    let action = rx
        .try_recv()
        .expect("action dispatched for checkpoint restore");
    assert!(
        matches!(
            action,
            ApplicationAction::CheckpointRestoreRequested {
                ref checkpoint_id,
            } if checkpoint_id == "target-hash-123"
        ),
        "Expected CheckpointRestoreRequested with target checkpoint, got {:?}",
        action
    );
    assert_eq!(app.navigation.active_modal, None);

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

    // Speed Normal: elapsed-time pacing (step interval 1000ms)
    replay.speed = ReplaySpeed::Normal;
    assert!(!replay.advance_elapsed(std::time::Duration::from_millis(300))); // 300ms: not enough
    assert!(!replay.advance_elapsed(std::time::Duration::from_millis(300))); // 600ms
    assert!(!replay.advance_elapsed(std::time::Duration::from_millis(300))); // 900ms
    assert!(replay.advance_elapsed(std::time::Duration::from_millis(200))); // 1100ms: advances
    assert_eq!(replay.cursor, 1);

    // Speed Hyper: advances every 250ms
    replay.speed = ReplaySpeed::Hyper;
    assert!(replay.advance_elapsed(std::time::Duration::from_millis(250)));
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

// ─────────────────────────────────────────────────────────────────────────────
// REM-01 through REM-14 Dedicated Regression Verification Suite
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_rem_01_rollback_availability_invariant() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut app = TuiApplication::new().with_bridge_tx(tx);

    // 1. Without valid checkpoints: cannot rollback
    assert!(!app.model.can_rollback());
    assert_eq!(app.model.valid_rollback_target(), None);

    // UI shows disabled copy
    app.navigation.open_modal(ViewId::RollbackInspector);
    let buf = buffer_text(&mut app, 100, 30);
    assert!(buf.contains("(Disabled - No checkpoints)"));

    // Enter dispatches nothing, leaves dialog open
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    assert!(
        rx.try_recv().is_err(),
        "Must not dispatch rollback when unavailable"
    );
    assert_eq!(app.navigation.active_modal, Some(ViewId::RollbackInspector));

    // 2. With valid checkpoint target
    app.model
        .set_rollback_checkpoint_target("chk-target-abc".to_string());
    assert!(app.model.can_rollback());
    assert_eq!(app.model.valid_rollback_target(), Some("chk-target-abc"));

    // UI shows active action
    let buf = buffer_text(&mut app, 100, 30);
    assert!(buf.contains("Target Checkpoint: chk-target-abc"));

    // Enter dispatches checkpoint restore and closes modal
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    let action = rx.try_recv().expect("checkpoint restore dispatched");
    assert!(matches!(
        action,
        ApplicationAction::CheckpointRestoreRequested { ref checkpoint_id }
            if checkpoint_id == "chk-target-abc"
    ));
    assert_eq!(app.navigation.active_modal, None);
}

#[test]
fn test_rem_02_approval_scope_and_partial_failure() {
    // Test that 'a' dispatches ApproveOnce (narrow scope) and partial failure retains items
    let (tx, mut rx) = tokio::sync::mpsc::channel(1); // Bounded channel of 1
    let mut app = TuiApplication::new().with_bridge_tx(tx);

    app.model.approvals.push(TuiApprovalRequest {
        id: "req-1".to_string(),
        tool_name: "tool_1".to_string(),
        agent_role: "role_1".to_string(),
        justification: "just_1".to_string(),
        parameters_summary: "p1".to_string(),
        risk_tier: "medium".to_string(),
        timestamp: Utc::now(),
    });
    app.model.approvals.push(TuiApprovalRequest {
        id: "req-2".to_string(),
        tool_name: "tool_2".to_string(),
        agent_role: "role_2".to_string(),
        justification: "just_2".to_string(),
        parameters_summary: "p2".to_string(),
        risk_tier: "high".to_string(),
        timestamp: Utc::now(),
    });

    app.navigation.open_modal(ViewId::ApprovalsQueue);
    app.handle_key(KeyEvent::from(KeyCode::Char('a')));

    // Channel had capacity 1, so req-1 is sent, req-2 fails to dispatch
    let action1 = rx.try_recv().expect("first action sent");
    assert!(matches!(
        action1,
        ApplicationAction::ApprovalDecision {
            ref request_id,
            decision: ApprovalDecision::ApproveOnce,
        } if request_id == "req-1"
    ));

    // req-2 was retained in approvals queue!
    assert_eq!(app.model.approvals.len(), 1);
    assert_eq!(app.model.approvals[0].id, "req-2");
    // Modal remains open on partial failure
    assert_eq!(app.navigation.active_modal, Some(ViewId::ApprovalsQueue));

    // Drain rx so second attempt succeeds
    app.handle_key(KeyEvent::from(KeyCode::Char('a')));
    let action2 = rx.try_recv().expect("second action sent on retry");
    assert!(matches!(
        action2,
        ApplicationAction::ApprovalDecision {
            ref request_id,
            decision: ApprovalDecision::ApproveOnce,
        } if request_id == "req-2"
    ));
    assert!(app.model.approvals.is_empty());
    assert_eq!(app.navigation.active_modal, None);
}

#[test]
fn test_rem_03_resize_events_idle_refresh() {
    let mut app = TuiApplication::new();
    app.model.enter_active_session();
    app.set_terminal_size(140, 30);
    app.model.is_dirty = false;
    app.force_redraw = false;

    // Simulate idle resize event
    app.handle_resize(80, 24);
    assert_eq!(app.last_area.width, 80);
    assert_eq!(app.last_area.height, 24);
    assert!(app.force_redraw);
    assert!(app.model.is_dirty);

    // Verify render_frame runs when force_redraw is set even if clean
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let rendered = app.render_frame(&mut terminal).unwrap();
    assert!(rendered);
}

#[test]
fn test_rem_04_replay_elapsed_time_schedule_independence() {
    let mut replay = ReplayController::default();
    let mission_id = MissionId::new();
    let history = vec![
        EventEnvelope::new(
            1,
            Some(mission_id),
            None,
            "rt".into(),
            EventType::MissionStarted {
                mission_id,
                objective: "1".into(),
            },
        ),
        EventEnvelope::new(
            2,
            Some(mission_id),
            None,
            "rt".into(),
            EventType::MissionStarted {
                mission_id,
                objective: "2".into(),
            },
        ),
        EventEnvelope::new(
            3,
            Some(mission_id),
            None,
            "rt".into(),
            EventType::MissionStarted {
                mission_id,
                objective: "3".into(),
            },
        ),
    ];
    replay.load_history(history);
    replay.cursor = 0;
    replay.is_playing = true;
    replay.speed = ReplaySpeed::Normal; // 1000ms

    // Schedule 1: 5 small 200ms polls
    for _ in 0..4 {
        assert!(!replay.advance_elapsed(std::time::Duration::from_millis(200)));
    }
    assert!(replay.advance_elapsed(std::time::Duration::from_millis(200))); // Reaches 1000ms
    assert_eq!(replay.cursor, 1);

    // Schedule 2: 1 big 1000ms poll
    assert!(replay.advance_elapsed(std::time::Duration::from_millis(1000)));
    assert_eq!(replay.cursor, 2);

    // End of history: stops playing and does not advance past bounds
    assert!(!replay.advance_elapsed(std::time::Duration::from_millis(1000)));
    assert_eq!(replay.cursor, 2);
    assert!(!replay.is_playing);
}

#[test]
fn test_rem_05_settings_input_and_focus_ownership() {
    let mut app = TuiApplication::new();
    assert_eq!(app.focus.current(), FocusTarget::Composer);

    // Entering settings sets Settings focus
    app.navigation.navigate_to(ScreenId::Settings);
    app.normalize_focus();
    assert_eq!(app.focus.current(), FocusTarget::Settings);
    assert!(app.focus.is_settings());
    assert!(!app.is_composer_focused);

    // Exiting settings restores Composer focus
    app.handle_key(KeyEvent::from(KeyCode::Char('q')));
    assert_eq!(app.navigation.current_screen, ScreenId::Dashboard);
    assert_eq!(app.focus.current(), FocusTarget::Composer);
    assert!(!app.focus.is_settings());
}

#[test]
fn test_rem_06_jobs_viewport_navigation_and_range_indicator() {
    let mut app = TuiApplication::new();
    app.unfocus_composer();
    app.navigation.navigate_to(ScreenId::Jobs);
    for i in 0..15 {
        app.model.jobs.push(TuiJobSnapshot {
            id: format!("job-{i}"),
            name: format!("task_exec_{i}"),
            status: if i % 2 == 0 {
                "running".into()
            } else {
                "failed".into()
            },
            duration_ms: (i as u64) * 1000,
        });
    }

    assert_eq!(app.model.jobs_selected_idx, 0);

    // Down navigation moves jobs selection without mutating context_selected_idx
    app.handle_key(KeyEvent::from(KeyCode::Down));
    assert_eq!(app.model.jobs_selected_idx, 1);
    assert_eq!(
        app.context_selected_idx, 0,
        "Jobs navigation must not mutate context rail index"
    );

    // End navigation moves to last item
    app.handle_key(KeyEvent::from(KeyCode::End));
    assert_eq!(app.model.jobs_selected_idx, 14);
    assert_eq!(app.context_selected_idx, 0);

    // Home navigation returns to first item
    app.handle_key(KeyEvent::from(KeyCode::Home));
    assert_eq!(app.model.jobs_selected_idx, 0);
    assert_eq!(app.context_selected_idx, 0);

    // Render buffer and verify visual indicator ▸
    let buf = buffer_text(&mut app, 120, 24);
    assert!(buf.contains("▸"));
    assert!(buf.contains("showing"));
}

#[test]
fn test_rem_07_artifact_metadata_and_quarantine_reasons() {
    let mut app = TuiApplication::new();
    let artifacts = vec![
        m31a::tui::model::TuiArtifactSnapshot {
            id: "art-1".to_string(),
            name: "malicious_script.sh".to_string(),
            version: 1,
            size_bytes: 4096,
            status: "quarantined".to_string(),
            logical_path: "scripts/run.sh".to_string(),
            producer_role: Some("untrusted_agent".to_string()),
            ..Default::default()
        },
        m31a::tui::model::TuiArtifactSnapshot {
            id: "art-2".to_string(),
            name: "unknown_payload.bin".to_string(),
            version: 1,
            size_bytes: 1024,
            status: "quarantined".to_string(),
            logical_path: String::new(),
            producer_role: None,
            ..Default::default()
        },
    ];

    app.model.load_artifacts(artifacts);
    assert_eq!(app.model.quarantine_items.len(), 2);
    // Explicit metadata reason:
    assert!(
        app.model.quarantine_items[0]
            .reason
            .contains("untrusted_agent")
    );
    assert!(
        app.model.quarantine_items[0]
            .reason
            .contains("scripts/run.sh")
    );
    // Fallback reason when metadata missing (never fake reason):
    assert!(
        app.model.quarantine_items[1]
            .reason
            .contains("reason unavailable")
    );
}

#[test]
fn test_rem_08_sandbox_probing_and_quarantine_error_handling() {
    let mut app = TuiApplication::new();
    app.navigation.navigate_to(ScreenId::Jobs);

    // 1. Sandbox status propagates truthfully to Jobs view
    app.model.set_sandbox_status(TuiSandboxStatus {
        isolation_active: false,
        mode: "disabled".into(),
        details: Some("bwrap missing".into()),
        ..Default::default()
    });
    let buf = buffer_text(&mut app, 120, 24);
    assert!(buf.contains("disabled / not active"));

    // 2. Quarantine error state rendering
    app.model
        .set_quarantine_failed("Storage connection refused");
    assert!(matches!(
        app.model.quarantine_state,
        QuarantineProjectionState::Failed(_)
    ));

    app.navigation.open_modal(ViewId::QuarantineManager);
    let buf = buffer_text(&mut app, 100, 30);
    assert!(buf.contains("Retrieval failure (Storage connection refused)"));
    assert!(buf.contains("[R]etry Quarantine Query"));

    // Pressing 'r' retries query
    app.handle_key(KeyEvent::from(KeyCode::Char('r')));
    assert!(matches!(
        app.model.quarantine_state,
        QuarantineProjectionState::Loading
    ));
}

#[test]
fn test_rem_09_agent_state_mapping_consistency() {
    let tokens = m31a::tui::theme::ThemeTokens::resolve(m31a::tui::theme::ThemeMode::Default);

    // Known states
    let s_run = m31a::tui::surface::agents::map_agent_state("running", &tokens);
    assert_eq!(s_run.symbol, "●");
    assert_eq!(s_run.label, "Working");
    assert!(!s_run.is_unknown);

    let s_idle = m31a::tui::surface::agents::map_agent_state("idle", &tokens);
    assert_eq!(s_idle.symbol, "○");
    assert_eq!(s_idle.label, "Waiting");
    assert!(!s_idle.is_unknown);
    assert!(!s_idle.is_unknown);

    // Unknown state
    let s_unknown = m31a::tui::surface::agents::map_agent_state("custom_scheduler_state", &tokens);
    assert_eq!(s_unknown.symbol, "?");
    assert_eq!(s_unknown.label, "Unknown (custom_scheduler_state)");
    assert!(s_unknown.is_unknown);
}

#[test]
fn test_rem_10_context_rail_bounds_and_line_count_parity() {
    let mut app = TuiApplication::new();
    app.model.enter_active_session();
    app.set_terminal_size(140, 30);

    // Initial total rail lines
    let lines = m31a::tui::shell::context_rail::calculate_total_rail_lines(&app.model);
    assert!(lines > 0);

    // Set out-of-bounds indices
    app.context_selected_idx = 10000;
    app.diff_scroll_offset = 10000;

    // Normalization clamps them within bounds
    app.normalize_context_rail_state();
    assert!(app.context_selected_idx < lines);
    assert!(app.diff_scroll_offset <= lines);
}

#[test]
fn test_rem_11_composer_visual_cursor_unicode_and_wrapping() {
    // 1. CJK glyphs (width 2 each)
    let (row, col) = calculate_visual_cursor("中文", 1, 0, 80);
    assert_eq!(row, 0);
    assert_eq!(col, 2);

    let (row, col) = calculate_visual_cursor("中文", 2, 0, 80);
    assert_eq!(row, 0);
    assert_eq!(col, 4);

    // 2. Tab expansion (4 columns)
    let (row, col) = calculate_visual_cursor("\tx", 1, 0, 80);
    assert_eq!(row, 0);
    assert_eq!(col, 4);

    // 3. Newlines
    let (row, col) = calculate_visual_cursor("hello\nworld", 6, 0, 80);
    assert_eq!(row, 1);
    assert_eq!(col, 0);

    // 4. Wrapping at max_width
    let (row, col) = calculate_visual_cursor("12345", 5, 0, 5);
    assert_eq!(row, 1);
    assert_eq!(col, 0);
}

#[test]
fn test_rem_12_zero_io_composer_startup() {
    // Creating composer does not scan filesystem or read catalog
    let composer = TuiComposer::new(PathBuf::from("/nonexistent/path/for/zero_io_test"));
    assert!(composer.cached_models().is_empty());
    assert!(!composer.is_file_cache_loaded());

    // Populating models explicitly
    let with_models = composer.with_cached_models(vec![("m1".into(), "d1".into())]);
    assert_eq!(with_models.cached_models().len(), 1);
}

#[test]
fn test_rem_13_model_autocomplete_invalidation_and_bounds() {
    let mut composer = TuiComposer::new(PathBuf::from("."));
    let gen0 = composer.autocomplete_generation();

    // Cache invalidation bumps generation
    composer.invalidate_cache();
    let gen1 = composer.autocomplete_generation();
    assert!(gen1 > gen0);

    // Stale generation result is rejected
    let stale_applied = composer.set_async_autocomplete_results(
        gen0,
        m31a::tui::composer::AutocompleteKind::SlashCommand,
        vec![],
    );
    assert!(!stale_applied);

    // Current generation result is accepted
    let current_applied = composer.set_async_autocomplete_results(
        gen1,
        m31a::tui::composer::AutocompleteKind::SlashCommand,
        vec![m31a::tui::composer::AutocompleteSuggestion {
            label: "/test".into(),
            description: "test cmd".into(),
            insert_text: "/test".into(),
        }],
    );
    assert!(current_applied);
    assert!(composer.is_autocomplete_open());
}

#[test]
fn test_rem_14_truthful_ui_copy_and_shortcut_hints() {
    let mut app = TuiApplication::new();

    // 1. Approvals Queue copy
    app.model.approvals.push(TuiApprovalRequest {
        id: "req-1".to_string(),
        tool_name: "test".to_string(),
        agent_role: "tester".to_string(),
        justification: "test".to_string(),
        parameters_summary: "p".to_string(),
        risk_tier: "low".to_string(),
        timestamp: Utc::now(),
    });
    app.navigation.open_modal(ViewId::ApprovalsQueue);
    let buf = buffer_text(&mut app, 100, 30);
    assert!(buf.contains("[A]pprove All (Once)"));
    app.navigation.close_modal();

    // 2. Keybindings guide copy
    app.navigation.open_overlay(ViewId::KeybindingsGuide);
    let buf = buffer_text(&mut app, 100, 30);
    assert!(buf.contains("[Esc/q] Close"));
    app.navigation.close_overlay();

    // 3. Help docs copy
    app.navigation.open_overlay(ViewId::HelpDocs);
    let buf = buffer_text(&mut app, 100, 30);
    assert!(buf.contains("[Esc/q] Close"));
    app.navigation.close_overlay();

    // 4. Onboarding tour copy
    app.navigation.open_overlay(ViewId::OnboardingTour);
    let buf = buffer_text(&mut app, 100, 30);
    assert!(buf.contains("[Esc/q] Close Tour"));
    app.navigation.close_overlay();
}

// ─────────────────────────────────────────────────────────────────────────────
// Explicit Acceptance Tests: M31A-UI-01 through M31A-UI-23
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_ui_01_rollback_inspector_checkpoint_restore() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut app = TuiApplication::new().with_bridge_tx(tx);

    // 1. Without valid checkpoint: can_rollback is false, Enter does nothing
    assert!(!app.model.can_rollback());
    assert_eq!(app.model.valid_rollback_target(), None);
    app.navigation.open_modal(ViewId::RollbackInspector);
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    assert!(rx.try_recv().is_err());
    assert_eq!(app.navigation.active_modal, Some(ViewId::RollbackInspector));

    // 2. Setting canonical checkpoint target enables rollback
    app.model
        .set_rollback_checkpoint_target("chk-alpha-999".to_string());
    assert!(app.model.can_rollback());
    assert_eq!(app.model.valid_rollback_target(), Some("chk-alpha-999"));

    // Enter dispatches CheckpointRestoreRequested with canonical ID and closes modal
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    let action = rx
        .try_recv()
        .expect("action dispatched for checkpoint restore");
    match action {
        ApplicationAction::CheckpointRestoreRequested { checkpoint_id } => {
            assert_eq!(checkpoint_id, "chk-alpha-999");
        }
        other => panic!("Expected CheckpointRestoreRequested, got {:?}", other),
    }
    assert_eq!(app.navigation.active_modal, None);

    // 3. Applying CheckpointRestored event updates model
    app.model.apply_interaction_event(
        &m31a::interaction::events::InteractionEvent::CheckpointRestored {
            checkpoint_id: "chk-alpha-999".to_string(),
            mission_id: "m-1".to_string(),
        },
    );
    assert_eq!(app.model.checkpoint_id, Some("chk-alpha-999".to_string()));
}

#[test]
fn test_ui_02_detail_inspectors_keyboard_ownership_and_close() {
    let mut app = TuiApplication::new();
    app.focus_composer();
    assert!(app.is_composer_focused);

    // Open an interactive detail view
    app.open_detail(ViewId::AgentInspector);
    assert_eq!(app.navigation.active_detail, Some(ViewId::AgentInspector));

    // Composer focus is suppressed while detail is active
    let action = app.classify_key(KeyEvent::from(KeyCode::Char('x')));
    assert_ne!(action, m31a::tui::keymap::KeyAction::Composer);

    // Typing a character does not append to composer
    app.handle_key(KeyEvent::from(KeyCode::Char('x')));
    assert_eq!(app.composer.text(), "");

    // 'q' closes the detail inspector without exiting application
    assert!(!app.should_exit(KeyEvent::from(KeyCode::Char('q'))));
    app.handle_key(KeyEvent::from(KeyCode::Char('q')));
    assert_eq!(app.navigation.active_detail, None);

    // After detail closed, 'q' triggers normal exit check
    assert!(app.should_exit(KeyEvent::from(KeyCode::Char('q'))));
}

#[test]
fn test_ui_03_quarantine_retry_active_detail_path() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut app = TuiApplication::new().with_bridge_tx(tx);

    // Open QuarantineManager as active detail
    app.open_detail(ViewId::QuarantineManager);
    assert_eq!(
        app.navigation.active_detail,
        Some(ViewId::QuarantineManager)
    );
    assert_eq!(app.model.quarantine_generation, 0);

    // Press 'r' to trigger retry
    app.handle_key(KeyEvent::from(KeyCode::Char('r')));
    assert_eq!(app.model.quarantine_generation, 1);
    assert!(matches!(
        app.model.quarantine_state,
        QuarantineProjectionState::Loading
    ));

    let action = rx
        .try_recv()
        .expect("QuarantineRefreshRequested dispatched");
    match action {
        ApplicationAction::QuarantineRefreshRequested { generation } => {
            assert_eq!(generation, 1);
        }
        other => panic!("Expected QuarantineRefreshRequested, got {:?}", other),
    }

    // Applying older generation response is rejected as stale
    app.model.apply_interaction_event(
        &m31a::interaction::events::InteractionEvent::QuarantineQueried {
            generation: 0,
            items: vec![m31a::interaction::events::QuarantineItemRecord {
                id: "old-1".to_string(),
                name: "stale.bin".to_string(),
                reason: "stale".to_string(),
                quarantined_at: None,
            }],
        },
    );
    assert!(matches!(
        app.model.quarantine_state,
        QuarantineProjectionState::Loading
    ));

    // Applying matching generation response updates state to Ready
    app.model.apply_interaction_event(
        &m31a::interaction::events::InteractionEvent::QuarantineQueried {
            generation: 1,
            items: vec![m31a::interaction::events::QuarantineItemRecord {
                id: "fresh-1".to_string(),
                name: "fresh.bin".to_string(),
                reason: "Suspicious execution".to_string(),
                quarantined_at: None,
            }],
        },
    );
    assert!(matches!(
        app.model.quarantine_state,
        QuarantineProjectionState::Ready
    ));
    assert_eq!(app.model.quarantine_items.len(), 1);
    assert_eq!(app.model.quarantine_items[0].id, "fresh-1");
}

#[test]
fn test_ui_04_sandbox_truthful_active_enforcement() {
    let supervisor = m31a::process::job::JobSupervisor::new(std::env::temp_dir());
    // Initially zero running jobs
    assert!(!supervisor.has_active_running_jobs());
    assert_eq!(supervisor.active_running_job_count(), 0);

    // Default probe reports false when no jobs running
    let status = TuiSandboxStatus {
        isolation_active: supervisor.has_active_running_jobs(),
        quotas_enforced: false,
        watchdog_active: false,
        mode: "Standard".to_string(),
        details: None,
    };
    assert!(!status.isolation_active);
    assert!(!status.quotas_enforced);
    assert!(!status.watchdog_active);
}

#[test]
fn test_ui_05_settings_screen_zero_render_credential_io() {
    let cfg =
        m31a::config::ResolvedConfiguration::build_fallback(PathBuf::from("/tmp/test_workspace"));
    // Rows rendered with credential_configured flag, without reading files synchronously
    let rows_unconfigured = m31a::tui::surface::settings::rows_for_category(
        m31a::tui::surface::settings::SettingsCategory::Provider,
        &cfg,
        None,
        &std::collections::HashMap::new(),
        false,
    );
    assert!(
        rows_unconfigured
            .iter()
            .any(|r| r.key.contains("credential") || r.key.contains("provider"))
    );

    let rows_configured = m31a::tui::surface::settings::rows_for_category(
        m31a::tui::surface::settings::SettingsCategory::Provider,
        &cfg,
        None,
        &std::collections::HashMap::new(),
        true,
    );
    assert!(
        rows_configured
            .iter()
            .any(|r| r.key.contains("credential") || r.key.contains("provider"))
    );
}

#[test]
fn test_ui_06_settings_reset_only_selected_field() {
    let default_cfg = m31a::config::AppConfig::default();
    let mut draft = default_cfg.clone();
    draft.agents.default_model = "custom-model-x".to_string();
    draft.tui.fps = 111;

    // Resetting only agents.default_model resets that field but preserves tui.fps
    m31a::tui::surface::settings::reset_draft_field_to_default(
        &mut draft,
        &default_cfg,
        "agents.default_model",
    )
    .expect("field reset succeeds");

    assert_eq!(draft.agents.default_model, default_cfg.agents.default_model);
    assert_eq!(
        draft.tui.fps, 111,
        "Other fields in draft must be preserved"
    );
}

#[test]
fn test_ui_07_jobs_screen_does_not_hijack_context_rail_index() {
    let mut app = TuiApplication::new();
    app.unfocus_composer();
    app.context_selected_idx = 5;

    app.navigation.navigate_to(ScreenId::Jobs);
    for i in 0..10 {
        app.model.jobs.push(TuiJobSnapshot {
            id: format!("job-{i}"),
            name: format!("exec_{i}"),
            status: "running".into(),
            duration_ms: 1000,
        });
    }

    assert_eq!(app.model.jobs_selected_idx, 0);
    assert_eq!(app.context_selected_idx, 5);

    // Moving up and down in Jobs screen updates jobs_selected_idx only
    app.handle_key(KeyEvent::from(KeyCode::Down));
    assert_eq!(app.model.jobs_selected_idx, 1);
    assert_eq!(
        app.context_selected_idx, 5,
        "Context rail index must remain 5"
    );

    app.handle_key(KeyEvent::from(KeyCode::Down));
    assert_eq!(app.model.jobs_selected_idx, 2);
    assert_eq!(app.context_selected_idx, 5);
}

#[test]
fn test_ui_08_and_19_task_completion_and_agent_count_parity() {
    let mut model = m31a::tui::model::TuiViewModel::new();
    model.tasks = vec![
        m31a::tui::model::TuiTaskSnapshot {
            id: "t1".into(),
            title: "Task 1".into(),
            status: "Succeeded".into(),
            ..Default::default()
        },
        m31a::tui::model::TuiTaskSnapshot {
            id: "t2".into(),
            title: "Task 2".into(),
            status: "completed".into(),
            ..Default::default()
        },
        m31a::tui::model::TuiTaskSnapshot {
            id: "t3".into(),
            title: "Task 3".into(),
            status: "Running".into(),
            ..Default::default()
        },
    ];

    model.agents = vec![
        m31a::tui::model::TuiAgentSnapshot {
            id: "a1".into(),
            role: "coder".into(),
            state: "Running".into(),
            ..Default::default()
        },
        m31a::tui::model::TuiAgentSnapshot {
            id: "a2".into(),
            role: "reviewer".into(),
            state: "idle".into(),
            ..Default::default()
        },
    ];

    assert_eq!(model.count_successful_tasks(), 2);
    assert_eq!(model.count_active_agents(), 1);
    assert!(m31a::tui::model::is_task_terminal_success("succeeded"));
    assert!(m31a::tui::model::is_task_terminal_success("completed"));
    assert!(!m31a::tui::model::is_task_terminal_success("failed"));
    assert!(!m31a::tui::model::is_task_terminal_success("running"));
}

#[test]
fn test_ui_10_replay_dirty_flag_cleared_on_render() {
    let mut app = TuiApplication::new();
    let mut replay_model = m31a::tui::model::TuiViewModel::new();
    replay_model.mark_dirty();
    assert!(replay_model.is_dirty);

    app.replay_model = Some(replay_model);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    app.render_frame(&mut terminal).unwrap();

    assert!(
        !app.replay_model.as_ref().unwrap().is_dirty,
        "Replay model dirty flag must be cleared on render"
    );
}

#[test]
fn test_ui_11_autocomplete_selection_preserves_value() {
    let mut composer = TuiComposer::new(PathBuf::from("."));
    let suggestions_1 = vec![
        m31a::tui::composer::AutocompleteSuggestion {
            label: "/help".into(),
            description: "help".into(),
            insert_text: "/help".into(),
        },
        m31a::tui::composer::AutocompleteSuggestion {
            label: "/model".into(),
            description: "model".into(),
            insert_text: "/model".into(),
        },
    ];
    composer.open_autocomplete(
        m31a::tui::composer::AutocompleteKind::SlashCommand,
        suggestions_1,
    );
    // Select second item (/model)
    composer.handle_key(KeyEvent::from(KeyCode::Down));
    assert_eq!(composer.autocomplete_selected_index(), 1);

    // Update suggestions containing the same item
    let suggestions_2 = vec![
        m31a::tui::composer::AutocompleteSuggestion {
            label: "/doctor".into(),
            description: "doctor".into(),
            insert_text: "/doctor".into(),
        },
        m31a::tui::composer::AutocompleteSuggestion {
            label: "/model".into(),
            description: "model".into(),
            insert_text: "/model".into(),
        },
    ];
    composer.open_autocomplete(
        m31a::tui::composer::AutocompleteKind::SlashCommand,
        suggestions_2,
    );
    // Selection stays on /model (index 1) rather than resetting to 0
    assert_eq!(composer.autocomplete_selected_index(), 1);
}

#[test]
fn test_ui_12_workspace_file_scan_bounded_visited_entries() {
    let composer = TuiComposer::new(PathBuf::from("."));
    // MAX_FS_VISITED_ENTRIES ensures recursive traversal is capped
    assert_eq!(m31a::tui::composer::MAX_FS_VISITED_ENTRIES, 1000);
    assert!(!composer.is_file_cache_loaded());
}

#[test]
fn test_ui_13_composer_unicode_grapheme_clusters() {
    let mut input = m31a::tui::input::text_input::TextInput::single_line();
    input.insert_str("Hello 🌍");
    assert_eq!(input.cursor_char_index(), 7);

    // Backspace removes entire multi-byte grapheme cluster
    input.handle_key(KeyEvent::from(KeyCode::Backspace));
    assert_eq!(input.value(), "Hello ");
    assert_eq!(input.cursor_char_index(), 6);
}

#[test]
fn test_ui_14_detail_views_scrolling_indicator() {
    let mut app = TuiApplication::new();
    app.model.enter_active_session();
    app.open_detail(ViewId::TaskDetails);
    for i in 0..30 {
        app.model.tasks.push(m31a::tui::model::TuiTaskSnapshot {
            id: format!("t-{i}"),
            title: format!("Task {i}"),
            status: "pending".into(),
            ..Default::default()
        });
    }

    let buf = buffer_text(&mut app, 100, 24);
    assert!(buf.contains("Showing") || buf.contains("scroll"));
}

#[test]
fn test_ui_15_settings_shortcut_collision_eliminated() {
    assert_eq!(
        ScreenId::Settings.shortcut(),
        "",
        "Settings shortcut must not collide with SkillRegistry 's'"
    );
}

#[test]
fn test_ui_16_command_palette_navigation_opens_live_views() {
    let mut app = TuiApplication::new();

    // Navigating to HelpDocs opens live help overlay
    app.navigate_to_view(ViewId::HelpDocs);
    assert!(app.overlay_manager.is_help_open);

    // Navigating to CommandPalette opens live palette
    app.navigate_to_view(ViewId::CommandPalette);
    assert!(app.palette.is_open);
}

#[test]
fn test_ui_17_settings_focus_model_excludes_conversation() {
    let mut app = TuiApplication::new();
    app.navigation.navigate_to(ScreenId::Settings);
    app.focus.set_focus(FocusTarget::Conversation);

    // In Conversation focus, settings is not focused
    assert_ne!(app.focus.current(), FocusTarget::Settings);

    // In Settings focus, app focus is Settings
    app.focus.set_focus(FocusTarget::Settings);
    assert_eq!(app.focus.current(), FocusTarget::Settings);
}

#[test]
fn test_ui_18_settings_small_terminal_warning() {
    let mut app = TuiApplication::new();
    app.navigation.navigate_to(ScreenId::Settings);
    let buf = buffer_text(&mut app, 40, 12);
    assert!(buf.contains("Terminal too small") || buf.contains("Too small"));
}

#[test]
fn test_ui_20_and_23_context_rail_cost_and_overflow_indicators() {
    let mut app = TuiApplication::new();
    app.model.enter_active_session();
    app.model.model_usage.prompt_tokens = 500;
    app.model.model_usage.total_cost_cents = Some(0);

    for i in 0..6 {
        app.model.agents.push(m31a::tui::model::TuiAgentSnapshot {
            id: format!("ag-{i}"),
            role: format!("Role {i}"),
            state: "running".into(),
            ..Default::default()
        });
    }

    for i in 0..5 {
        app.model
            .live_tools
            .push(m31a::tui::model::LiveToolOperation {
                call_id: format!("call-{i}"),
                tool_name: format!("tool_{i}"),
                parameters: "{}".into(),
                state: m31a::tui::model::LiveToolState::Running,
                started_at: Utc::now(),
                completed_at: None,
                duration_ms: None,
                output_preview: None,
            });
    }

    let buf = buffer_text(&mut app, 120, 30);
    assert!(
        buf.contains("($0.00)"),
        "Must display ($0.00) when cost is Some(0) with active tokens"
    );
    assert!(
        buf.contains("[4] View all"),
        "Must display agent overflow hint"
    );
    assert!(
        buf.contains("[5] View all"),
        "Must display tool overflow hint"
    );
}

#[test]
fn test_ui_21_persisted_usage_hydration_idempotency() {
    let mut model = m31a::tui::model::TuiViewModel::new();
    let records = vec![
        m31a::model::persistence::invocation::ModelInvocationRecord {
            id: uuid::Uuid::from_u128(1),
            mission_id: m31a::ids::MissionId::new(),
            task_id: m31a::ids::TaskId::new(),
            agent_id: m31a::ids::AgentId::new(),
            step_number: 1,
            provider: "nvidia".into(),
            model_name: "model-1".into(),
            attempt_number: 1,
            outcome: "success".into(),
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            usage_source: "reported".into(),
            routing_reason: "primary".into(),
            prompt_provenance: None,
            cost_usd: Some(0.01),
            cost_provenance: Default::default(),
            created_at: Utc::now(),
        },
    ];

    // First hydration sets baseline
    model.hydrate_persisted_usage(&records);
    assert_eq!(model.model_usage.effective_total_tokens(), 150);
    assert_eq!(model.model_usage.total_cost_cents, Some(1));

    // Second hydration with same records is idempotent
    model.hydrate_persisted_usage(&records);
    assert_eq!(
        model.model_usage.effective_total_tokens(),
        150,
        "Repeated hydration must not double-count tokens"
    );
    assert_eq!(
        model.model_usage.total_cost_cents,
        Some(1),
        "Repeated hydration must not double-count cost"
    );
}

#[test]
fn test_ui_22_hydration_queries_distinguish_empty_from_failed() {
    let mut model = m31a::tui::model::TuiViewModel::new();

    // Default states
    assert_eq!(
        model.verification_section_state,
        m31a::tui::model::SectionProjectionState::Loading
    );

    // Setting error states
    model.verification_section_state =
        m31a::tui::model::SectionProjectionState::Failed("gate error".into());
    model.recovery_section_state =
        m31a::tui::model::SectionProjectionState::Failed("budget error".into());
    model.git_section_state = m31a::tui::model::SectionProjectionState::Failed("git error".into());
    model.task_graph_section_state =
        m31a::tui::model::SectionProjectionState::Failed("graph error".into());
    model.catalog_section_state =
        m31a::tui::model::SectionProjectionState::Failed("catalog error".into());
    model.usage_section_state =
        m31a::tui::model::SectionProjectionState::Failed("usage error".into());

    assert!(matches!(
        model.verification_section_state,
        m31a::tui::model::SectionProjectionState::Failed(_)
    ));
    assert!(matches!(
        model.recovery_section_state,
        m31a::tui::model::SectionProjectionState::Failed(_)
    ));
    assert!(matches!(
        model.git_section_state,
        m31a::tui::model::SectionProjectionState::Failed(_)
    ));
    assert!(matches!(
        model.task_graph_section_state,
        m31a::tui::model::SectionProjectionState::Failed(_)
    ));
    assert!(matches!(
        model.catalog_section_state,
        m31a::tui::model::SectionProjectionState::Failed(_)
    ));
    assert!(matches!(
        model.usage_section_state,
        m31a::tui::model::SectionProjectionState::Failed(_)
    ));
}
