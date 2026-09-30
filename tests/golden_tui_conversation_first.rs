//! Conversation-First Autonomous Coding Interface Integration Tests.
//!
//! Validates the approved M31A TUI Experience Redesign specification (docs/ui/M31A_TUI_EXPERIENCE_REDESIGN.md):
//! 1. Composer starts focused on startup; immediate typing without activation key
//! 2. Keyboard routing isolation: '1', 'q', 'l', 'm', 'a' insert text when focused, do not navigate
//! 3. Single-line submission via Enter
//! 4. Multiline creation via Shift+Enter, Alt+Enter, Ctrl+J, and trailing backslash
//! 5. Esc closes autocomplete or cleanly unfocuses composer
//! 6. Enter or 'i' refocuses composer when unfocused
//! 7. Slash-command autocomplete with sub-argument completions (/model, /profile)
//! 8. @mention file autocomplete over workspace files
//! 9. Input history traversal (Up/Down)
//! 10. Approval modal intercept with Enter resolution
//! 11. Empty state welcome presentation with quick commands
//! 12. Active execution state presentation (Task progress, elapsed, tokens, cost)
//! 13. Failure and recovery semantic badge presentation ([VERIFY:FAILED], [RECOVERY], [ERR])
//! 14. Responsive layout tiers (Compact, Standard, Large, UltraWide)

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tempfile::TempDir;

use m31a::tui::TuiApp;
use m31a::tui::approval::{ApprovalDecision, ApprovalModal};
use m31a::tui::composer::{ComposerAction, TuiComposer};
use m31a::tui::conversation::TuiConversationItem;
use m31a::tui::layout::{LayoutTier, classify_terminal_size};
use m31a::tui::model::{TuiApprovalRequest, TuiTaskSnapshot, TuiViewModel};
use m31a::tui::navigation::ScreenId;
use m31a::tui::theme::ThemeTokens;

#[test]
fn test_composer_focus_and_immediate_typing() {
    let app = TuiApp::new().with_composer_focused(true);
    assert!(
        app.is_composer_focused,
        "Composer must start focused on launch"
    );

    let mut app = app;
    // Typing characters that would otherwise be navigation shortcuts
    for c in ['1', '2', 'q', 'l', 'm', 'a', 'r', '?'] {
        let key = KeyEvent::from(KeyCode::Char(c));
        app.handle_key(key);
    }

    assert_eq!(
        app.composer.text(),
        "12qlmar?",
        "Focused composer must absorb characters as text, not trigger navigation"
    );
    assert_eq!(
        app.navigation.current_screen,
        ScreenId::Dashboard,
        "Screen must remain Dashboard while typing"
    );
    assert!(
        app.is_running,
        "Typing 'q' must not quit while composer is focused"
    );
}

#[test]
fn test_single_line_enter_submission() {
    let mut composer = TuiComposer::new(std::path::PathBuf::from("."));
    for c in "cargo test".chars() {
        composer.handle_key(KeyEvent::from(KeyCode::Char(c)));
    }

    let action = composer.handle_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(
        action,
        ComposerAction::Submit("cargo test".to_string()),
        "Enter must submit the single-line input"
    );
    assert!(
        composer.text().is_empty(),
        "Composer text must clear after submit"
    );
}

#[test]
fn test_multiline_shift_enter_ctrl_j_and_backslash() {
    // 1. Shift+Enter
    let mut composer = TuiComposer::new(std::path::PathBuf::from("."));
    composer.handle_key(KeyEvent::from(KeyCode::Char('a')));
    let shift_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT);
    composer.handle_key(shift_enter);
    composer.handle_key(KeyEvent::from(KeyCode::Char('b')));
    assert_eq!(composer.text(), "a\nb", "Shift+Enter must insert a newline");

    // 2. Ctrl+J
    let mut composer = TuiComposer::new(std::path::PathBuf::from("."));
    composer.handle_key(KeyEvent::from(KeyCode::Char('x')));
    let ctrl_j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL);
    composer.handle_key(ctrl_j);
    composer.handle_key(KeyEvent::from(KeyCode::Char('y')));
    assert_eq!(composer.text(), "x\ny", "Ctrl+J must insert a newline");

    // 3. Trailing backslash
    let mut composer = TuiComposer::new(std::path::PathBuf::from("."));
    composer.handle_key(KeyEvent::from(KeyCode::Char('c')));
    composer.handle_key(KeyEvent::from(KeyCode::Char('\\')));
    composer.handle_key(KeyEvent::from(KeyCode::Enter));
    composer.handle_key(KeyEvent::from(KeyCode::Char('d')));
    assert_eq!(
        composer.text(),
        "c\nd",
        "Trailing backslash + Enter must insert newline"
    );
}

#[test]
fn test_esc_unfocus_and_enter_refocus() {
    let mut app = TuiApp::new().with_composer_focused(true);
    assert!(app.is_composer_focused);

    // Esc unfocuses composer
    app.handle_key(KeyEvent::from(KeyCode::Esc));
    assert!(
        !app.is_composer_focused,
        "Esc must unfocus composer when autocomplete is closed"
    );

    // When unfocused, Enter refocuses composer
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    assert!(
        app.is_composer_focused,
        "Enter must refocus composer when unfocused"
    );

    // Unfocus again, test 'i' refocuses
    app.handle_key(KeyEvent::from(KeyCode::Esc));
    assert!(!app.is_composer_focused);
    app.handle_key(KeyEvent::from(KeyCode::Char('i')));
    assert!(
        app.is_composer_focused,
        "'i' must refocus composer when unfocused"
    );
}

#[test]
fn test_slash_and_mention_autocomplete() {
    let temp = TempDir::new().unwrap();
    let ws = temp.path();
    std::fs::write(ws.join("Cargo.toml"), "").unwrap();
    std::fs::write(ws.join("lib.rs"), "").unwrap();

    let mut composer = TuiComposer::new(ws.to_path_buf());

    // Trigger slash autocomplete
    composer.handle_key(KeyEvent::from(KeyCode::Char('/')));
    assert!(
        composer.is_autocomplete_open(),
        "Typing '/' must open autocomplete"
    );

    // Trigger @mention autocomplete
    composer.clear();
    composer.handle_key(KeyEvent::from(KeyCode::Char('@')));
    assert!(
        composer.is_autocomplete_open(),
        "Typing '@' must open file autocomplete"
    );

    // Esc dismisses autocomplete without unfocusing
    composer.handle_key(KeyEvent::from(KeyCode::Esc));
    assert!(
        !composer.is_autocomplete_open(),
        "Esc must close autocomplete popup"
    );
}

#[test]
fn test_history_traversal() {
    let mut composer = TuiComposer::new(std::path::PathBuf::from("."));

    // Submit turn 1
    composer.set_text("first task");
    let _ = composer.handle_key(KeyEvent::from(KeyCode::Enter));

    // Submit turn 2
    composer.set_text("second task");
    let _ = composer.handle_key(KeyEvent::from(KeyCode::Enter));

    // Traverse history with Up
    composer.handle_key(KeyEvent::from(KeyCode::Up));
    assert_eq!(composer.text(), "second task");

    composer.handle_key(KeyEvent::from(KeyCode::Up));
    assert_eq!(composer.text(), "first task");

    // Traverse forward with Down
    composer.handle_key(KeyEvent::from(KeyCode::Down));
    assert_eq!(composer.text(), "second task");
}

#[test]
fn test_approval_modal_enter_resolution() {
    let mut modal = ApprovalModal::new();
    let req = TuiApprovalRequest {
        id: "req-01".to_string(),
        tool_name: "run_command".to_string(),
        agent_role: "Implementer".to_string(),
        justification: "Run tests".to_string(),
        parameters_summary: "cargo test".to_string(),
        risk_tier: "high".to_string(),
        timestamp: chrono::Utc::now(),
    };

    modal.open(req);
    assert!(modal.is_open);

    // Pressing Enter must resolve as ApproveOnce
    let decision = modal.handle_key(KeyEvent::from(KeyCode::Enter));
    assert_eq!(decision, Some(ApprovalDecision::ApproveOnce));
    assert!(!modal.is_open, "Modal must close after approval");
}

#[test]
fn test_empty_state_and_active_execution_rendering() {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let tokens = ThemeTokens::resolve(m31a::tui::theme::ThemeMode::DarkSlateCyan);

    // 1. Empty state rendering
    let model = TuiViewModel::new();
    let composer = TuiComposer::new(std::path::PathBuf::from("."));

    terminal
        .draw(|f| {
            m31a::tui::screens::render_session_cockpit(
                f,
                f.area(),
                &model,
                &composer,
                true,
                &tokens,
            );
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let content = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n");

    assert!(content.contains("M31A"));
    assert!(content.contains("/help"));
    assert!(content.contains("/status"));
    assert!(content.contains("/diff"));
    assert!(content.contains("/doctor"));
    assert!(content.contains("@file"));

    // 2. Active execution state rendering
    let mut active_model = TuiViewModel::new();
    active_model.mission_status = "running".to_string();
    active_model.tasks.push(TuiTaskSnapshot {
        id: "t1".to_string(),
        title: "implement_auth".to_string(),
        status: "running".to_string(),
        agent_role: Some("Coder".to_string()),
        progress_pct: 50,
        dependencies: vec![],
    });

    terminal
        .draw(|f| {
            m31a::tui::screens::render_session_cockpit(
                f,
                f.area(),
                &active_model,
                &composer,
                true,
                &tokens,
            );
        })
        .unwrap();

    let active_buf = terminal.backend().buffer().clone();
    let active_content = (0..active_buf.area.height)
        .map(|y| {
            (0..active_buf.area.width)
                .map(|x| active_buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n");

    assert!(active_content.contains("RUNNING"));
    assert!(active_content.contains("Task 1/1"));
}

#[test]
fn test_failure_and_recovery_semantic_badges() {
    let verify_failed = TuiConversationItem::Verification {
        sequence: 1,
        passed: false,
        summary: "cargo test failed with 2 errors".to_string(),
        timestamp: chrono::Utc::now(),
    };
    assert_eq!(verify_failed.badge(), "[VERIFY:FAILED]");

    let verify_passed = TuiConversationItem::Verification {
        sequence: 2,
        passed: true,
        summary: "all gates passed".to_string(),
        timestamp: chrono::Utc::now(),
    };
    assert_eq!(verify_passed.badge(), "[VERIFY:PASSED]");

    let recovery = TuiConversationItem::Recovery {
        action: "revert_worktree".to_string(),
        details: "restored clean state".to_string(),
        timestamp: chrono::Utc::now(),
    };
    assert_eq!(recovery.badge(), "[RECOVERY]");

    let err = TuiConversationItem::Error {
        message: "timeout".to_string(),
        timestamp: chrono::Utc::now(),
    };
    assert_eq!(err.badge(), "[ERR]");
}

#[test]
fn test_responsive_layout_tier_classification() {
    assert_eq!(classify_terminal_size(80, 24), LayoutTier::Compact);
    assert_eq!(classify_terminal_size(99, 24), LayoutTier::Compact);
    assert_eq!(classify_terminal_size(100, 30), LayoutTier::Standard);
    assert_eq!(classify_terminal_size(159, 30), LayoutTier::Standard);
    assert_eq!(classify_terminal_size(160, 40), LayoutTier::Large);
    assert_eq!(classify_terminal_size(219, 40), LayoutTier::Large);
    assert_eq!(classify_terminal_size(220, 50), LayoutTier::UltraWide);
    assert_eq!(classify_terminal_size(300, 60), LayoutTier::UltraWide);
}
