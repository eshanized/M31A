//! TUI visual reconstruction regression scenarios (A–F + responsive).
//!
//! Validates the conversation-first rebuild:
//! A. Fresh launch — quiet, clean, obviously interactive.
//! B. User submits task — message + inline working state, composer governed.
//! C. Tool execution — compact rows, no dashboard.
//! D. Approval — dominant modal, bridge-routed resolution restores context.
//! E. Completion — verification prominent, telemetry secondary.
//! F. Failure — obvious, readable, recoverable.
//! Plus responsive integrity across 7 framebuffers.

use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Modifier;

use m31a::tui::TuiApplication;
use m31a::tui::composer::TuiComposer;
use m31a::tui::conversation::TuiConversationItem;
use m31a::tui::layout::classify_terminal_size;
use m31a::tui::model::{TuiApprovalRequest, TuiViewModel};
use m31a::tui::theme::{ThemeMode, ThemeTokens};

fn buffer_text(app: &mut TuiApplication, w: u16, h: u16) -> String {
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();
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

fn quiet_tokens() -> ThemeTokens {
    ThemeTokens::resolve(ThemeMode::Default)
}

// ── Scenario A — Fresh launch ────────────────────────────────────────────

#[test]
fn test_ux_a_fresh_launch_is_quiet_and_obviously_interactive() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    let content = buffer_text(&mut app, 120, 30);

    // Identity + empty conversation + composer ready + minimal hints.
    assert!(content.contains("M31A"));
    assert!(content.contains("Your autonomous software engineering workspace"));
    assert!(content.contains("/ for commands"));
    assert!(content.contains("@ for files"));
    assert!(content.contains("? for help"));

    // No cyber dashboard noise.
    assert!(!content.contains("MISSION COCKPIT OVERVIEW"));
    assert!(!content.contains("M31A Cockpit"));
    assert!(!content.contains("tok"));
    assert!(!content.contains("[COMPOSER]"));
    assert!(!content.contains("[STREAM]"));
    assert!(!content.contains("◆ M31A Autonomous"));
}

// ── Scenario B — User submits task ───────────────────────────────────────

#[test]
fn test_ux_b_user_submit_shows_message_and_working_state() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    // Fail-closed without bridge: submission records the user turn and an
    // honest error (governed runtime unavailable), composer stays governed.
    for c in "build the auth endpoint".chars() {
        app.handle_key(KeyEvent::from(KeyCode::Char(c)));
    }
    app.handle_key(KeyEvent::from(KeyCode::Enter));

    assert!(
        app.model
            .conversation
            .iter()
            .any(|i| matches!(i, TuiConversationItem::User { text, .. } if text.contains("build the auth endpoint"))),
        "user message must appear in stream"
    );
    // Inline working state (model records thinking immediately on submit).
    assert!(app.model.activity_message.is_some() || app.model.live_activity.is_some());

    let content = buffer_text(&mut app, 120, 30);
    assert!(content.contains("build the auth endpoint"));
    assert!(content.contains("You"));
}

// ── Scenario C — Tool execution ──────────────────────────────────────────

#[test]
fn test_ux_c_tool_activity_is_compact_not_dashboard() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    app.model.add_conversation_item(TuiConversationItem::User {
        id: "u1".to_string(),
        sequence: 1,
        text: "run tests".to_string(),
        mentions: vec![],
        timestamp: Utc::now(),
    });
    app.model
        .add_conversation_item(TuiConversationItem::ToolActivity {
            call_id: "c1".to_string(),
            tool_name: "cargo test".to_string(),
            parameters: "--test oauth2".to_string(),
            timestamp: Utc::now(),
        });
    app.model
        .add_conversation_item(TuiConversationItem::ToolResult {
            call_id: "c1".to_string(),
            tool_name: "cargo test".to_string(),
            success: true,
            output_preview: "12 passed".to_string(),
            expanded: false,
            timestamp: Utc::now(),
        });

    let content = buffer_text(&mut app, 120, 30);
    assert!(content.contains("Tool"));
    assert!(content.contains("cargo test"));
    assert!(content.contains("finished") || content.contains("✓"));
    // No giant tool dashboard.
    assert!(!content.contains("MISSION COCKPIT OVERVIEW"));
    assert!(!content.contains("[TOOL:START]"));
    assert!(!content.contains("[TOOL:OK]"));
}

// ── Scenario D — Approval ────────────────────────────────────────────────

#[test]
fn test_ux_d_approval_dominates_and_restores_context() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    app.model.add_conversation_item(TuiConversationItem::User {
        id: "u1".to_string(),
        sequence: 1,
        text: "apply migration".to_string(),
        mentions: vec![],
        timestamp: Utc::now(),
    });
    let before = buffer_text(&mut app, 120, 30);
    assert!(before.contains("apply migration"));

    let req = TuiApprovalRequest {
        id: "req-d".to_string(),
        tool_name: "cargo migrate".to_string(),
        agent_role: "Implementer".to_string(),
        justification: "Apply the generated database migration.".to_string(),
        parameters_summary: "cargo migrate".to_string(),
        risk_tier: "high".to_string(),
        timestamp: Utc::now(),
    };
    app.approval_modal.open(req);
    let during = buffer_text(&mut app, 120, 30);
    assert!(during.contains("needs your approval"));
    assert!(during.contains("cargo migrate"));
    assert!(during.contains("Approve Once") || during.contains("Approve"));

    // Resolution via canonical keys restores prior context.
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    app = app.with_bridge_tx(tx);
    app.handle_key(KeyEvent::from(KeyCode::Char('y')));
    assert!(!app.approval_modal.is_open);
    let after = buffer_text(&mut app, 120, 30);
    assert!(after.contains("apply migration"));
    assert!(!after.contains("needs your approval"));
}

// ── Scenario E — Completion ──────────────────────────────────────────────

#[test]
fn test_ux_e_completion_prominent_telemetry_secondary() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    app.model.mission_status = "completed".to_string();
    app.model
        .add_conversation_item(TuiConversationItem::Verification {
            sequence: 1,
            passed: true,
            summary: "183 tests passed".to_string(),
            timestamp: Utc::now(),
        });
    app.model
        .add_conversation_item(TuiConversationItem::Assistant {
            id: "a1".to_string(),
            sequence: 2,
            text: "Authentication flow implemented and verified.".to_string(),
            streaming: false,
            timestamp: Utc::now(),
        });

    let content = buffer_text(&mut app, 120, 30);
    assert!(content.contains("Verification"));
    assert!(content.contains("183 tests passed"));
    assert!(content.contains("Authentication flow"));
    // Telemetry stays secondary: no token/cost/event noise by default.
    assert!(!content.contains("tok"));
    assert!(!content.contains("cost"));
}

// ── Scenario F — Failure ─────────────────────────────────────────────────

#[test]
fn test_ux_f_failure_obvious_reason_readable_recovery_discoverable() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    app.model
        .add_conversation_item(TuiConversationItem::Verification {
            sequence: 1,
            passed: false,
            summary: "2 tests failed in oauth2 suite".to_string(),
            timestamp: Utc::now(),
        });
    app.model
        .add_conversation_item(TuiConversationItem::Failure {
            context: "cargo test".to_string(),
            reason: "assertion failed: token refresh returns expired".to_string(),
            timestamp: Utc::now(),
        });

    let content = buffer_text(&mut app, 120, 30);
    assert!(content.contains("Failed"));
    assert!(content.contains("2 tests failed") || content.contains("cargo test"));
    assert!(content.contains("token refresh"));
    assert!(content.contains("Press Enter to inspect"));
    // Restrained error color: failure is a quiet elevated row, not a red panel.
    assert!(!content.contains("[VERIFY:FAILED]"));
    assert!(!content.contains("[FAILED]"));
}

// ── Historical records must not force active-work layout ─────────────────

#[test]
fn test_ux_historical_tasks_do_not_force_active_layout() {
    let model = TuiViewModel::new();
    // Fresh model has no semantic activity.
    assert!(!model.is_semantically_active());

    let mut completed = TuiViewModel::new();
    completed.tasks.push(m31a::tui::model::TuiTaskSnapshot {
        id: "t1".to_string(),
        title: "done".to_string(),
        status: "completed".to_string(),
        agent_role: None,
        progress_pct: 100,
        dependencies: vec![],
    });
    completed.agents.push(m31a::tui::model::TuiAgentSnapshot {
        id: "a1".to_string(),
        role: "Coder".to_string(),
        state: "idle".to_string(),
        current_task: None,
        total_tokens: 0,
    });
    // Historical records alone are idle.
    assert!(
        !completed.is_semantically_active(),
        "completed tasks + idle agents must not read as active work"
    );

    // Dashboard renders full-width conversation in both cases.
    let mut idle_app = TuiApplication::new();
    idle_app.model = completed;
    let content = buffer_text(&mut idle_app, 120, 30);
    assert!(!content.contains("MISSION COCKPIT OVERVIEW"));
}

// ── Responsive integrity (7 framebuffers) ────────────────────────────────

#[test]
fn test_ux_responsive_framebuffers_stay_readable() {
    let sizes: [(u16, u16); 7] = [
        (80, 24),
        (96, 24),
        (100, 30),
        (120, 30),
        (120, 36),
        (160, 40),
        (220, 50),
    ];
    for (w, h) in sizes {
        let mut app = TuiApplication::new().with_composer_focused(true);
        app.model.add_conversation_item(TuiConversationItem::User {
            id: "u".to_string(),
            sequence: 1,
            text: "Build OAuth2 with a very long objective that should wrap gracefully across narrow terminals without clipping or overflow artifacts".to_string(),
            mentions: vec![],
            timestamp: Utc::now(),
        });
        app.model
            .add_conversation_item(TuiConversationItem::Assistant {
                id: "a".to_string(),
                sequence: 2,
                text:
                    "Planning implementation across three modules with verification via cargo test."
                        .to_string(),
                streaming: false,
                timestamp: Utc::now(),
            });
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        app.force_redraw = true;
        app.render_frame(&mut terminal).unwrap();
        let buf = terminal.backend().buffer().clone();
        assert_eq!(buf.area.width, w);
        assert_eq!(buf.area.height, h);
        let content: String = (0..h)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(content.contains("M31A"), "{w}x{h} must keep identity");
        assert!(
            content.contains("You") || content.contains("M31A"),
            "{w}x{h} must keep conversation roles"
        );
        // No boxitis: at most the modal borders exist; default cockpit has
        // no ALL-bordered panels. Count box-drawing corners as a proxy.
        let corners = content.chars().filter(|c| *c == '┌' || *c == '┘').count();
        assert!(
            corners == 0,
            "{w}x{h} default cockpit must not use boxed panels (found {corners} corners)"
        );
        // Tier classification sanity.
        let _ = classify_terminal_size(w, h);
    }
}

// ── Theme tokens flow through semantics, not literals ────────────────────

#[test]
fn test_ux_default_theme_is_restrained_and_mono_safe() {
    let tokens = quiet_tokens();
    assert_eq!(tokens.mode, ThemeMode::Default);
    // No bold-shouting statuses in the calm theme.
    assert!(!tokens.status_running.add_modifier.contains(Modifier::BOLD));
    assert!(!tokens.status_ok.add_modifier.contains(Modifier::BOLD));
    // Semantic aliases resolve.
    assert_eq!(tokens.accent, tokens.accent_primary);
    assert_eq!(tokens.success, tokens.status_ok);
    assert_eq!(tokens.error, tokens.status_failed);
}

// ── Composer behavior preserved ──────────────────────────────────────────

#[test]
fn test_ux_composer_behaviors_preserved() {
    // Slash registry remains the authority (not hardcoded in the view).
    let registry = m31a::interaction::commands::SlashCommandRegistry::new_standard();
    assert!(!registry.commands().is_empty());

    let mut composer = TuiComposer::new(std::path::PathBuf::from("."));
    composer.set_text("/sta");
    assert!(composer.is_autocomplete_open());

    // Enter submits, Shift+Enter newlines, history works (covered elsewhere
    // in golden tests; spot-check submit path here).
    let mut c2 = TuiComposer::new(std::path::PathBuf::from("."));
    c2.set_text("hello");
    let action = c2.handle_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(
        action,
        m31a::tui::composer::ComposerAction::Submit("hello".to_string())
    );
}
