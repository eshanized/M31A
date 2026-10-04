//! Comprehensive test suite for M31A Workspace TUI V2.
//!
//! Validates:
//! 1. Mode A — Minimal Welcome Mode (centered identity, centered composer, hints, no telemetry, no context rail)
//! 2. First prompt transition from Welcome to Active mode
//! 3. Slash command transitions from Welcome to Active mode and settles to idle
//! 4. Navigation (Ctrl+P, ?, screen switches) preserves Welcome mode
//! 5. Mode B — Active Session Workspace (conversation pane, bottom persistent composer, contextual right rail)
//! 6. Context Rail dynamic sections (SESSION, CURRENT, ACTION, CONTEXT, TODO, AGENTS, TOOLS, GIT, VERIFICATION)
//! 7. Responsive framebuffers (80x24, 96x24, 100x30, 120x30, 120x36, 160x40, 220x50)
//! 8. Conversation scrolling (follow mode, unseen arrivals, Home, End)
//! 9. Autocomplete Enter semantics (exact match vs prefix completion)

use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use m31a::tui::TuiApp;
use m31a::tui::conversation::TuiConversationItem;
use m31a::tui::model::{
    ActivityKind, LiveToolOperation, LiveToolState, SessionViewMode, TuiAgentSnapshot,
    TuiApprovalRequest, TuiTaskSnapshot, TuiVerificationSummary, UiOperationState,
};
use m31a::tui::navigation::ScreenId;

fn buffer_text(app: &mut TuiApp, w: u16, h: u16) -> String {
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

// ─────────────────────────────────────────────────────────────────────────────
// 1. Mode A — Minimal Welcome Mode
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_welcome_mode_rendering_at_120x30() {
    let mut app = TuiApp::new();

    // Invariant: fresh launch is in Welcome mode
    assert!(app.model.is_welcome());
    assert!(!app.model.is_active());
    assert_eq!(app.model.session_view_mode, SessionViewMode::Welcome);
    assert!(app.is_composer_focused);

    let content = buffer_text(&mut app, 120, 30);

    // Centered identity & welcome message
    assert!(
        content.contains("M31A"),
        "Identity must be present:\n{content}"
    );
    assert!(
        content.contains("What are we building today?"),
        "Welcome prompt must be present:\n{content}"
    );
    assert!(
        content.contains("Your autonomous software engineering workspace."),
        "Tagline must be present:\n{content}"
    );

    // Centered composer box & metadata
    assert!(
        content.contains("Ask M31A anything…"),
        "Composer placeholder must be present:\n{content}"
    );
    assert!(
        content.contains("Build ·"),
        "Build metadata must be present:\n{content}"
    );

    // Bottom hint line
    assert!(
        content.contains("/ for commands")
            && content.contains("@ for files")
            && content.contains("? for help"),
        "Hints must be present:\n{content}"
    );

    // Clean, intentionally minimal: no telemetry, no context rail, no task dashboard, no fake "Working"
    assert!(
        !content.contains("SESSION"),
        "Context rail must NOT appear in Welcome mode"
    );
    assert!(
        !content.contains("CONTEXT"),
        "Context rail must NOT appear in Welcome mode"
    );
    assert!(
        !content.contains("TODO"),
        "Task dashboard must NOT appear in Welcome mode"
    );
    assert!(
        !content.contains("Working ·"),
        "Must not fake Working in Welcome mode"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. First Prompt Transitions to Active Mode
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_first_prompt_transitions_to_active() {
    let mut app = TuiApp::new();
    assert!(app.model.is_welcome());

    // Submit user prompt
    for c in "study the codebase".chars() {
        app.handle_key(KeyEvent::from(KeyCode::Char(c)));
    }
    app.handle_key(KeyEvent::from(KeyCode::Enter));

    // First prompt establishes Active session
    assert!(
        app.model.is_active(),
        "Must transition to Active session on first prompt"
    );
    assert!(!app.model.is_welcome());
    assert_eq!(app.model.session_view_mode, SessionViewMode::Active);

    app.model.model_usage.prompt_tokens = 450;

    // Render active session at 120x30
    let content = buffer_text(&mut app, 120, 30);

    // Conversation pane contains user message
    assert!(
        content.contains("study the codebase"),
        "Prompt must appear in conversation:\n{content}"
    );
    assert!(
        content.contains("You"),
        "User role must appear in conversation:\n{content}"
    );

    // Context rail appears on right when >= 120 cols
    assert!(
        content.contains("SESSION"),
        "Context rail SESSION section must appear in Active mode at 120 cols:\n{content}"
    );
    assert!(
        content.contains("CONTEXT"),
        "Context rail CONTEXT section must appear in Active mode at 120 cols:\n{content}"
    );
    assert!(
        content.contains("GIT"),
        "Context rail GIT section must appear in Active mode at 120 cols:\n{content}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Slash Command Transitions to Active Mode and Settles to Idle
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_slash_command_transitions_to_active_and_settles_to_idle() {
    let mut app = TuiApp::new();
    assert!(app.model.is_welcome());

    // Submit slash command
    for c in "/help".chars() {
        app.handle_key(KeyEvent::from(KeyCode::Char(c)));
    }
    app.handle_key(KeyEvent::from(KeyCode::Enter));

    // Must transition to Active
    assert!(
        app.model.is_active(),
        "Slash command must transition to Active mode"
    );
    assert!(!app.model.is_welcome());

    // Slash command must never trigger model Thinking
    assert_ne!(
        app.model.activity_kind,
        ActivityKind::Thinking,
        "Slash commands must never enter Thinking"
    );

    // Clearing conversation must NEVER revert back to Welcome mode
    app.model.conversation.clear();
    app.model.settle_request();
    assert!(
        app.model.is_active(),
        "Clearing conversation must NEVER revert Active mode to Welcome mode"
    );
    assert!(!app.model.is_welcome());

    // Settle to idle: still Active
    assert_eq!(app.model.operation_state(), UiOperationState::Idle);
    assert!(app.model.is_active());
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Navigation Does Not Accidentally Transition to Active Mode
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_navigation_does_not_transition_to_active() {
    let mut app = TuiApp::new();
    assert!(app.model.is_welcome());

    // Open universal command palette (Ctrl+P)
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    assert!(app.palette.is_open);
    assert!(
        app.model.is_welcome(),
        "Ctrl+P must NOT transition to Active"
    );

    // Close palette (Esc)
    app.handle_key(KeyEvent::from(KeyCode::Esc));
    assert!(!app.palette.is_open);
    assert!(
        app.model.is_welcome(),
        "Closing palette must NOT transition to Active"
    );

    // Open help overlay (?)
    app.overlay_manager.open_help();
    assert!(app.overlay_manager.is_help_open);
    assert!(
        app.model.is_welcome(),
        "Opening help must NOT transition to Active"
    );

    // Close help overlay (Esc)
    app.handle_key(KeyEvent::from(KeyCode::Esc));
    assert!(!app.overlay_manager.is_help_open);
    assert!(
        app.model.is_welcome(),
        "Closing help must NOT transition to Active"
    );

    // Switch screen to Git inspection
    app.navigation.navigate_to(ScreenId::Git);
    assert!(
        app.model.is_welcome(),
        "Navigating screens must NOT transition to Active"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Context Rail Dynamic Sections
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_context_rail_dynamic_sections() {
    let mut app = TuiApp::new();
    app.model.enter_active_session();

    // 5a. Pending approval
    app.model.approvals.push(TuiApprovalRequest {
        id: "appr-1".to_string(),
        tool_name: "fs_write".to_string(),
        agent_role: "executor".to_string(),
        justification: "write file".to_string(),
        parameters_summary: "src/main.rs".to_string(),
        risk_tier: "High".to_string(),
        timestamp: Utc::now(),
    });

    // 5b. Live activity
    app.model.live_activity = Some("cargo check".to_string());

    // 5c. Tasks in DAG
    app.model.tasks = vec![
        TuiTaskSnapshot {
            id: "t1".to_string(),
            title: "Inspect repository".to_string(),
            status: "succeeded".to_string(),
            agent_role: Some("scout".to_string()),
            progress_pct: 100,
            dependencies: vec![],
        },
        TuiTaskSnapshot {
            id: "t2".to_string(),
            title: "Run verification".to_string(),
            status: "running".to_string(),
            agent_role: Some("tester".to_string()),
            progress_pct: 50,
            dependencies: vec![],
        },
        TuiTaskSnapshot {
            id: "t3".to_string(),
            title: "Generate report".to_string(),
            status: "pending".to_string(),
            agent_role: None,
            progress_pct: 0,
            dependencies: vec![],
        },
    ];

    // 5d. Active agents
    app.model.agents = vec![TuiAgentSnapshot {
        id: "a1".to_string(),
        role: "Executor".to_string(),
        state: "running".to_string(),
        current_task: Some("run tests".to_string()),
        total_tokens: 1200,
    }];

    // 5e. Live tools
    app.model.live_tools = vec![LiveToolOperation {
        call_id: "c1".to_string(),
        tool_name: "read_file".to_string(),
        parameters: "src/main.rs".to_string(),
        state: LiveToolState::Running,
        started_at: Utc::now(),
        completed_at: None,
        duration_ms: None,
        output_preview: None,
    }];

    // 5f. Git branch
    app.model.git_branch = "feature-auth".to_string();

    // 5g. Verification summary
    app.model.verification_summary = TuiVerificationSummary {
        total_checks: 10,
        passed_count: 10,
        failed_count: 0,
        blocked_count: 0,
        skipped_count: 0,
        not_run_count: 0,
        overall_status: "passed".to_string(),
        last_evaluated: Some(Utc::now()),
    };

    // 5h. Model usage
    app.model.model_usage.prompt_tokens = 1200;
    app.model.model_usage.completion_tokens = 300;

    let content = buffer_text(&mut app, 130, 35);

    // Verify all dynamic sections rendered in Context Rail
    assert!(
        content.contains("ACTION REQUIRED"),
        "Action section missing:\n{content}"
    );
    assert!(
        content.contains("Approve: fs_write"),
        "Tool approval missing:\n{content}"
    );
    assert!(content.contains("High"), "Risk tier missing:\n{content}");

    assert!(
        content.contains("CONTEXT"),
        "Context section missing:\n{content}"
    );
    assert!(content.contains("1.5k"), "Token count missing:\n{content}");

    assert!(
        content.contains("CURRENT"),
        "Current activity missing:\n{content}"
    );
    assert!(
        content.contains("cargo check"),
        "Live activity name missing:\n{content}"
    );

    assert!(
        content.contains("TODO (1/3)"),
        "TODO progress count missing:\n{content}"
    );
    assert!(
        content.contains("Inspect repository"),
        "Task 1 missing:\n{content}"
    );
    assert!(
        content.contains("Run verification"),
        "Task 2 missing:\n{content}"
    );

    assert!(
        content.contains("AGENTS (1)"),
        "Agents section missing:\n{content}"
    );
    assert!(
        content.contains("Executor"),
        "Agent role missing:\n{content}"
    );

    assert!(
        content.contains("TOOLS (1)"),
        "Tools section missing:\n{content}"
    );
    assert!(
        content.contains("read_file"),
        "Tool name missing:\n{content}"
    );

    assert!(content.contains("GIT"), "Git section missing:\n{content}");
    assert!(
        content.contains("feature-auth"),
        "Branch name missing:\n{content}"
    );

    assert!(
        content.contains("VERIFICATION"),
        "Verification section missing:\n{content}"
    );
    assert!(
        content.contains("10/10"),
        "Verification pass count missing:\n{content}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Responsive Framebuffers Across All Tiers
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_responsive_framebuffers_all_sizes() {
    let sizes = [
        (80, 24),  // Compact
        (96, 24),  // Compact wide
        (100, 30), // Standard narrow
        (120, 30), // Standard wide
        (120, 36), // Standard tall
        (160, 40), // Large
        (220, 50), // UltraWide
    ];

    for (w, h) in sizes {
        // Welcome mode render
        let mut app_welcome = TuiApp::new();
        let content_welcome = buffer_text(&mut app_welcome, w, h);
        assert!(
            content_welcome.contains("M31A"),
            "Welcome mode failed at {w}x{h}"
        );
        assert!(
            !content_welcome.contains("SESSION"),
            "Context rail must not appear in welcome mode at {w}x{h}"
        );

        // Active mode render
        let mut app_active = TuiApp::new();
        app_active.model.enter_active_session();
        app_active
            .model
            .add_conversation_item(TuiConversationItem::User {
                id: "u1".to_string(),
                sequence: 1,
                text: "test responsiveness".to_string(),
                mentions: vec![],
                timestamp: Utc::now(),
            });
        let content_active = buffer_text(&mut app_active, w, h);
        assert!(
            content_active.contains("test responsiveness"),
            "Active mode failed at {w}x{h}"
        );

        if w >= 120 {
            assert!(
                content_active.contains("SESSION"),
                "Context rail must be present when width >= 120 (size: {w}x{h})"
            );
        } else {
            assert!(
                !content_active.contains("SESSION"),
                "Context rail must NOT be present when width < 120 (size: {w}x{h})"
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Conversation Scrolling: Follow Mode, Unseen Arrivals, Home, End
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_conversation_scrolling_follow_unseen_home_end() {
    let mut app = TuiApp::new();
    app.model.enter_active_session();

    // Invariant: starts with follow == true and unseen_count == 0
    assert!(app.model.follow);
    assert_eq!(app.model.unseen_count, 0);

    // Add multiple conversation items that overflow the viewport
    for i in 1..=30 {
        app.model.add_conversation_item(TuiConversationItem::User {
            id: format!("u{i}"),
            sequence: i,
            text: format!("Message number {i}"),
            mentions: vec![],
            timestamp: Utc::now(),
        });
    }

    // Render frame to measure viewport geometry
    let _ = buffer_text(&mut app, 80, 20);

    // Still following because no scroll up occurred
    assert!(app.model.follow);
    assert_eq!(app.model.unseen_count, 0);

    // Scroll up (e.g. user inspects older history via PageUp)
    app.model.scroll_up(5);
    assert!(!app.model.follow, "Scrolling up must clear follow mode");

    // Add new items while scrolled up
    app.model
        .add_conversation_item(TuiConversationItem::Assistant {
            id: "a1".to_string(),
            sequence: 11,
            text: "New incoming response".to_string(),
            streaming: false,
            timestamp: Utc::now(),
        });

    // Unseen count increments, follow stays false
    assert!(!app.model.follow);
    assert_eq!(app.model.unseen_count, 1);

    // Scroll to Bottom / End (re-enables follow and resets unseen)
    app.model.scroll_to_bottom();
    assert!(
        app.model.follow,
        "scroll_to_bottom must restore follow mode"
    );
    assert_eq!(
        app.model.unseen_count, 0,
        "scroll_to_bottom must reset unseen_count"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Autocomplete Exact Match vs Prefix Enter Semantics
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_autocomplete_exact_match_vs_prefix_semantics() {
    let mut app = TuiApp::new();

    // Type prefix "/he"
    for c in "/he".chars() {
        app.composer.handle_key(KeyEvent::from(KeyCode::Char(c)));
    }
    assert!(app.composer.is_autocomplete_open());

    // Without user navigating, typing the exact match "/help"
    app.composer.handle_key(KeyEvent::from(KeyCode::Char('l')));
    app.composer.handle_key(KeyEvent::from(KeyCode::Char('p')));
    assert_eq!(app.composer.text(), "/help");

    // Enter on exact command without explicit selection submits directly
    assert!(app.composer.should_submit_on_enter());
}
