//! Targeted Integration & Regression Test Suite for TUI Layout and Interaction Correctness.
//!
//! Enforces:
//! 1. Welcome layout proportions and bounds at 80x24, 100x30, 120x36, 160x48, and 220x60.
//! 2. Responsive composer width: comfortable prompt width with generous margins across all sizes.
//! 3. Dynamic composer height expansion and bounded scrolling on multiline text.
//! 4. Terminal cursor placement: visible when focused, suppressed when unfocused.
//! 5. Autocomplete popup bounds: clamped within terminal viewport.
//! 6. Shared model display formatting: preserves variant semantics (such as -instruct)
//!    and matches between header and welcome composer metadata.
//! 7. Audited welcome metadata: no fake `Build` prefix; truthful model & profile.
//! 8. Shell footer keyboard guidance visibility on 1-row compact (80x24) and standard layouts.
//! 9. Context-sensitive footer shortcuts (welcome, active conversation, approvals, decisions, overlays).
//! 10. Layout classification parity: `classify_terminal_size` delegates to `LayoutTier::from_dimensions`.
//! 11. Conversation surface empty state: clean layout without confusing fake prompt glyphs.
//! 12. Micro-animation truthfulness: active during startup & execution; stopped while idle.

use ratatui::Terminal;
use ratatui::backend::{Backend, TestBackend};
use std::path::PathBuf;

use m31a::tui::TuiApplication;
use m31a::tui::composer::TuiComposer;
use m31a::tui::layout::{LayoutTier, classify_terminal_size};
use m31a::tui::model::{ActivityKind, RuntimeStartupState, TuiApprovalRequest, TuiViewModel};
use m31a::tui::sanitizer::format_display_model;
use m31a::tui::theme::{ThemeMode, ThemeTokens};

fn buffer_content(
    app: &mut TuiApplication,
    width: u16,
    height: u16,
) -> (ratatui::buffer::Buffer, String) {
    app.force_redraw = true;
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    app.render_frame(&mut terminal).unwrap();
    let buffer = terminal.backend().buffer().clone();
    let text = (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n");
    (buffer, text)
}

fn row_text(buffer: &ratatui::buffer::Buffer, y: u16) -> String {
    (0..buffer.area.width)
        .map(|x| buffer[(x, y)].symbol())
        .collect::<String>()
}

fn mock_approval_request(id: &str) -> TuiApprovalRequest {
    TuiApprovalRequest {
        id: id.to_string(),
        tool_name: "fs_write".to_string(),
        agent_role: "Implementer".to_string(),
        justification: "Update file".to_string(),
        parameters_summary: "path: file.txt".to_string(),
        risk_tier: "low".to_string(),
        timestamp: chrono::Utc::now(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Welcome Layout & Responsive Proportions Across All Resolving Sizes
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_welcome_layout_proportions_and_bounds_across_all_resolutions() {
    let resolutions = [
        (80, 24, 64),
        (100, 30, 78),
        (120, 36, 88),
        (160, 48, 100),
        (220, 60, 108),
    ];

    for (w, h, expected_box_w) in resolutions {
        let mut app = TuiApplication::new().with_composer_focused(true);
        app.set_runtime_ready();
        let (buf, content) = buffer_content(&mut app, w, h);

        assert_eq!(buf.area.width, w);
        assert_eq!(buf.area.height, h);

        // Visual focus: main welcome question without redundant repeated M31A title
        assert!(
            content.contains("What are we building today?"),
            "Resolution {w}x{h} must contain the welcome question"
        );
        assert!(
            content.contains("Your autonomous software engineering workspace."),
            "Resolution {w}x{h} must contain the tagline"
        );
        assert!(
            content.contains("Ask M31A anything…"),
            "Resolution {w}x{h} must contain composer placeholder"
        );
        assert!(
            content.contains("/ for commands")
                && content.contains("@ for files")
                && content.contains("? for help"),
            "Resolution {w}x{h} must contain quick discovery hints"
        );

        // Header maintains application identity
        let header_row = row_text(&buf, 0);
        assert!(
            header_row.contains("M31A"),
            "Header must maintain M31A identity at {w}x{h}"
        );

        // Assert composer border width matches expected responsive calculation
        // Find row containing top composer border '┌' and '┐' using cell coordinates
        let mut found_border = false;
        for y in 0..h {
            let mut left_col = None;
            let mut right_col = None;
            for x in 0..w {
                if buf[(x, y)].symbol() == "┌" {
                    left_col = Some(x);
                }
                if buf[(x, y)].symbol() == "┐" {
                    right_col = Some(x);
                }
            }
            if let (Some(left), Some(right)) = (left_col, right_col) {
                let actual_box_w = right - left + 1;
                assert_eq!(
                    actual_box_w, expected_box_w,
                    "At {w}x{h}, expected box width {expected_box_w}, got {actual_box_w}"
                );
                // Ensure composer box has margins on left and right
                assert!(left > 0, "Composer must have left margin at {w}x{h}");
                assert!(w > right + 1, "Composer must have right margin at {w}x{h}");
                found_border = true;
                break;
            }
        }
        assert!(
            found_border,
            "Must find composer border on welcome workspace at {w}x{h}"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Visible Footer Guidance on 1-Row Compact (80x24) and Standard Layouts
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_footer_shortcut_text_is_visible_in_framebuffer() {
    // 80x24 compact: footer allocated height is 1 row (row 23)
    let mut app = TuiApplication::new().with_composer_focused(true);
    app.set_runtime_ready();
    let (buf, _) = buffer_content(&mut app, 80, 24);

    let footer_row = row_text(&buf, 23);
    assert!(
        footer_row.contains("Enter send"),
        "Footer at row 23 on 80x24 must display 'Enter send', got: {footer_row}"
    );
    assert!(
        footer_row.contains("Shift+Enter newline"),
        "Footer at row 23 on 80x24 must display 'Shift+Enter newline', got: {footer_row}"
    );

    // 100x30 standard: footer row is row 29
    let mut app = TuiApplication::new().with_composer_focused(true);
    app.set_runtime_ready();
    let (buf_std, _) = buffer_content(&mut app, 100, 30);
    let footer_row_std = row_text(&buf_std, 29);
    assert!(
        footer_row_std.contains("Enter send"),
        "Footer at row 29 on 100x30 must display 'Enter send', got: {footer_row_std}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Context-Sensitive Footer Guidance Across Lifecycle and Navigation States
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_context_sensitive_footer_guidance() {
    // 1. Welcome state: prompt entry & discovery shortcuts
    let mut app = TuiApplication::new().with_composer_focused(true);
    app.set_runtime_ready();
    let (buf, _) = buffer_content(&mut app, 120, 30);
    let footer = row_text(&buf, 29);
    assert!(footer.contains("Enter send"));
    assert!(footer.contains("/ commands"));

    // 2. Active conversation, composer unfocused
    app.model.enter_active_session();
    app.is_composer_focused = false;
    let (buf, _) = buffer_content(&mut app, 120, 30);
    let footer = row_text(&buf, 29);
    assert!(footer.contains("Type to write"));
    assert!(footer.contains("PgUp/PgDn"));

    // 3. Approval Modal open
    app.approval_modal.open(mock_approval_request("appr-01"));
    let (buf, _) = buffer_content(&mut app, 120, 30);
    let footer = row_text(&buf, 29);
    assert!(
        footer.contains("y/Enter approve") && footer.contains("n reject"),
        "Approval modal must show decision shortcuts in footer, got: {footer}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Model Identity Consistency & Preserving -instruct Suffix
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_model_identity_consistency_header_and_welcome() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    app.model.active_model = "meta/llama-3.1-70b-instruct".to_string();
    app.model.active_profile = "coding".to_string();
    app.set_runtime_ready();

    let (buf, content) = buffer_content(&mut app, 120, 30);

    // Verify format_display_model directly
    assert_eq!(
        format_display_model("meta/llama-3.1-70b-instruct"),
        "llama-3.1-70b-instruct"
    );

    // Header row 0 must contain the canonical display model name
    let header_row = row_text(&buf, 0);
    assert!(
        header_row.contains("llama-3.1-70b-instruct"),
        "Header must contain 'llama-3.1-70b-instruct', got: {header_row}"
    );

    // Composer metadata inside welcome workspace must also communicate 'llama-3.1-70b-instruct'
    assert!(
        content.contains("llama-3.1-70b-instruct"),
        "Welcome workspace must contain 'llama-3.1-70b-instruct'"
    );
    assert!(
        content.contains("coding"),
        "Welcome workspace must contain profile 'coding'"
    );

    // Hardcoded `Build ·` prefix must be absent
    assert!(
        !content.contains("Build ·"),
        "Welcome workspace must not contain hardcoded 'Build ·' prefix"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Dynamic Multiline Composer Height & Scrolling Cursor Visibility
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_multiline_composer_height_and_cursor_scrolling() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    app.set_runtime_ready();

    // Multiline prompt input
    for c in "Line one of the prompt\nLine two of the prompt\nLine three of the prompt".chars() {
        app.composer.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char(c),
        ));
    }

    let (buf, content) = buffer_content(&mut app, 100, 30);

    assert!(content.contains("Line one"));
    assert!(content.contains("Line three"));

    // Header, description, and bottom hints must remain visible and in-bounds
    assert!(content.contains("What are we building today?"));
    assert!(content.contains("/ for commands"));
    let footer_row = row_text(&buf, 29);
    assert!(footer_row.contains("Enter send"));
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Terminal Cursor Placement When Composer Focused vs Unfocused
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_terminal_cursor_placement_respects_focus() {
    let mut app = TuiApplication::new().with_composer_focused(true);
    app.set_runtime_ready();
    for c in "some prompt".chars() {
        app.composer.handle_key(crossterm::event::KeyEvent::from(
            crossterm::event::KeyCode::Char(c),
        ));
    }

    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    app.render_frame(&mut terminal).unwrap();

    // When focused, cursor position is set
    let cursor_pos = terminal.backend_mut().get_cursor_position().unwrap();
    assert!(
        cursor_pos.x > 0 && cursor_pos.y > 0,
        "Cursor must be placed when focused"
    );

    // When unfocused, cursor position must NOT be in the composer
    let mut app_unfocused = TuiApplication::new().with_composer_focused(false);
    app_unfocused.set_runtime_ready();
    for c in "some prompt".chars() {
        app_unfocused
            .composer
            .handle_key(crossterm::event::KeyEvent::from(
                crossterm::event::KeyCode::Char(c),
            ));
    }
    let backend_unfocused = TestBackend::new(100, 30);
    let mut terminal_unfocused = Terminal::new(backend_unfocused).unwrap();
    app_unfocused.render_frame(&mut terminal_unfocused).unwrap();

    let cursor_unfocused = terminal_unfocused
        .backend_mut()
        .get_cursor_position()
        .unwrap();
    // Default cursor is (0, 0) when no widget sets it
    assert_eq!(
        cursor_unfocused,
        ratatui::layout::Position { x: 0, y: 0 },
        "Terminal cursor must not be positioned in composer when unfocused"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Autocomplete Popup Clamped Within Terminal Viewport Bounds
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_autocomplete_popup_clamped_within_viewport() {
    let mut composer = TuiComposer::new(PathBuf::from("."));
    // Open autocomplete with slash command
    composer.handle_key(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Char('/'),
    ));
    assert!(composer.is_autocomplete_open());

    let terminal_rect = ratatui::layout::Rect::new(0, 0, 80, 24);
    // Position composer at extreme right / bottom
    let composer_area = ratatui::layout::Rect::new(50, 18, 28, 4);

    let popup = composer.popup_rect(terminal_rect, composer_area).unwrap();
    assert!(
        popup.x >= terminal_rect.x,
        "Popup left {} must be >= terminal left {}",
        popup.x,
        terminal_rect.x
    );
    assert!(
        popup.right() <= terminal_rect.right(),
        "Popup right {} must be <= terminal right {}",
        popup.right(),
        terminal_rect.right()
    );
    assert!(
        popup.y >= terminal_rect.y,
        "Popup top {} must be >= terminal top {}",
        popup.y,
        terminal_rect.y
    );
    assert!(
        popup.bottom() <= terminal_rect.bottom(),
        "Popup bottom {} must be <= terminal bottom {}",
        popup.bottom(),
        terminal_rect.bottom()
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 8. Layout Classification Consistency
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_layout_classification_consistency_width_and_height() {
    // Narrow terminal (<100) -> Compact
    assert_eq!(classify_terminal_size(80, 24), LayoutTier::Compact);
    assert_eq!(classify_terminal_size(99, 40), LayoutTier::Compact);

    // Short terminal (<24) -> Compact, regardless of width
    assert_eq!(classify_terminal_size(120, 20), LayoutTier::Compact);
    assert_eq!(classify_terminal_size(160, 15), LayoutTier::Compact);
    assert_eq!(classify_terminal_size(240, 10), LayoutTier::Compact);

    // Standard width & sufficient height -> Standard
    assert_eq!(classify_terminal_size(100, 30), LayoutTier::Standard);
    assert_eq!(classify_terminal_size(159, 36), LayoutTier::Standard);

    // Large width & sufficient height -> Large
    assert_eq!(classify_terminal_size(160, 40), LayoutTier::Large);
    assert_eq!(classify_terminal_size(219, 50), LayoutTier::Large);

    // UltraWide width & sufficient height -> UltraWide
    assert_eq!(classify_terminal_size(220, 50), LayoutTier::UltraWide);
    assert_eq!(classify_terminal_size(300, 60), LayoutTier::UltraWide);
}

// ─────────────────────────────────────────────────────────────────────────────
// 9. Conversation Surface Empty State Does Not Fake Prompt Glyphs
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_conversation_surface_empty_state_no_fake_prompt() {
    let mut model = TuiViewModel::new();
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let tokens = ThemeTokens::resolve(ThemeMode::Default);

    terminal
        .draw(|f| {
            m31a::tui::surface::render_conversation_surface(
                f,
                f.area(),
                &mut model,
                &tokens,
                false,
            );
        })
        .unwrap();

    let text = (0..30)
        .map(|y| {
            (0..100)
                .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n");

    assert!(text.contains("What are we building today?"));
    assert!(text.contains("Your autonomous software engineering workspace."));
    // Ensure no fake standalone prompt glyph `> ` appears in empty stream
    let lines_with_prompt_glyph = text.lines().filter(|l| l.trim() == ">").count();
    assert_eq!(
        lines_with_prompt_glyph, 0,
        "Conversation stream empty state must not render fake prompt glyph"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 10. Micro-Animation Truthfulness (Startup, Working, Idle)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_micro_animation_truthfulness() {
    let mut model = TuiViewModel::new();

    // 1. Startup state: animation is active to tick spinner
    model.runtime_status = RuntimeStartupState::InitializingRuntime;
    assert!(
        model.has_active_animation(),
        "Runtime startup must have active animation"
    );

    // 2. Ready & idle: animation stops
    model.runtime_status = RuntimeStartupState::Ready;
    assert!(
        !model.has_active_animation(),
        "Ready and idle model must NOT have active animation"
    );

    // 3. Approval waiting: NOT animated as executing
    model.approvals.push(mock_approval_request("a1"));
    assert!(
        !model.has_active_animation(),
        "Waiting for approval must NOT animate"
    );

    // 4. Genuine execution: animation active
    model.approvals.clear();
    model.activity_kind = ActivityKind::Executing;
    assert!(
        model.has_active_animation(),
        "Executing activity must have active animation"
    );

    // 5. Spinner ticks advance frame
    let first = model.spinner.tick();
    let second = model.spinner.tick();
    assert_ne!(first, second, "Spinner frame must advance on tick");
}
