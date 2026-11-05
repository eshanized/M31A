//! Comprehensive All-Keybindings Integration Test Suite (`all_keybindings_test.rs`).
//!
//! Validates the full spectrum of keyboard shortcuts, modal precedence rules,
//! multi-key combinations, function keys, screen navigations, composer controls,
//! and terminal rendering integrity across standard and narrow viewports.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use m31a_core::Config;
use m31a_tui::keybindings::{
    CANONICAL_KEYBINDINGS, canonical_keybindings, keybindings_by_category, render_keybindings_table,
};
use m31a_tui::screens::Screen;
use m31a_tui::transcript::{NoticeLevel, TranscriptItem};
use m31a_tui::{App, ModalState, ThemeVariant, handle_key_event, init_theme};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tempfile::tempdir;

fn create_test_app() -> (tempfile::TempDir, App) {
    init_theme(ThemeVariant::Dark);
    let temp = tempdir().expect("tempdir");
    let config = Config {
        model: "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
        model_provider: Some("nim".to_string()),
        api_key: Some(
            m31a_core::resolve_stored_api_key()
                .unwrap_or_else(|| "nvapi-runtime-integration-token".to_string()),
        ),
        ..Default::default()
    };
    let app = App::new_test(config, temp.path()).expect("failed to create test app");
    (temp, app)
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl_key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::CONTROL)
}

fn alt_key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::ALT)
}

fn shift_key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::SHIFT)
}

// =============================================================================
// 1. Canonical Keybinding Registry & Completeness
// =============================================================================

#[test]
fn test_all_canonical_keybindings_registry_and_coverage() {
    let all = canonical_keybindings();
    assert_eq!(CANONICAL_KEYBINDINGS.len(), all.len());
    assert!(
        all.len() >= 25,
        "Expected at least 25 canonical keybindings, found {}",
        all.len()
    );

    // Verify all canonical shortcuts exist and have non-empty metadata
    for kb in all {
        assert!(!kb.key.is_empty(), "Key string cannot be empty");
        assert!(
            !kb.description.is_empty(),
            "Description cannot be empty for key {}",
            kb.key
        );
        assert!(
            !kb.category.is_empty(),
            "Category cannot be empty for key {}",
            kb.key
        );
    }

    // Verify key categories exist and return expected members
    let categories = [
        "Global",
        "Model",
        "Configuration",
        "Git",
        "Clipboard",
        "Session",
        "View",
        "Diagnostics",
        "Security",
        "Execution",
        "Multi-Agent",
        "Composer",
        "Navigation",
    ];

    for cat in categories {
        let members = keybindings_by_category(cat);
        assert!(
            !members.is_empty(),
            "Category '{}' must contain registered keybindings",
            cat
        );
    }

    // Verify markdown documentation generation
    let table = render_keybindings_table();
    assert!(table.contains("| Shortcut | Category | Description |"));
    assert!(table.contains("| `Ctrl+C` | Global |"));
    assert!(table.contains("| `Ctrl+P` | Global |"));
    assert!(table.contains("| `F2` | Model |"));
    assert!(table.contains("| `Ctrl+S` | Configuration |"));
    assert!(table.contains("| `Tab` | Execution |"));
}

// =============================================================================
// 2. Level 0: Global Emergency & Lifecycle Keybindings
// =============================================================================

#[test]
fn test_ctrl_c_lifecycle_all_states() {
    let (_tmp, mut app) = create_test_app();

    // State A: Active turn execution -> Ctrl+C cancels turn
    app.turn_state.transition_submitted("turn-1").unwrap();
    app.turn_state.transition_streaming("turn-1").unwrap();
    assert!(app.turn_state.is_active());

    handle_key_event(&mut app, ctrl_key(KeyCode::Char('c')));
    assert!(
        !app.turn_state.is_active(),
        "Ctrl+C during streaming must cancel active turn"
    );

    // State B: Composer draft in progress -> Ctrl+C clears draft
    app.composer.set_text("Draft prompt in composer");
    assert!(!app.composer.is_empty());

    handle_key_event(&mut app, ctrl_key(KeyCode::Char('c')));
    assert!(
        app.composer.is_empty(),
        "Ctrl+C with text in composer must clear composer"
    );
    assert!(!app.should_quit);

    // State C: Idle and empty composer -> first Ctrl+C issues 2-second warning
    handle_key_event(&mut app, ctrl_key(KeyCode::Char('c')));
    assert!(!app.should_quit, "First Ctrl+C when idle must not exit");
    assert!(app.last_ctrl_c_press.is_some());

    // Second Ctrl+C immediately -> triggers exit
    handle_key_event(&mut app, ctrl_key(KeyCode::Char('c')));
    assert!(
        app.should_quit,
        "Second Ctrl+C within 2 seconds must set should_quit"
    );
}

#[test]
fn test_ctrl_q_immediate_quit() {
    let (_tmp, mut app) = create_test_app();
    assert!(!app.should_quit);

    handle_key_event(&mut app, ctrl_key(KeyCode::Char('q')));
    assert!(app.should_quit, "Ctrl+Q must immediately set should_quit");
}

#[test]
fn test_ctrl_p_command_palette_workflow() {
    let (_tmp, mut app) = create_test_app();

    // Ctrl+P opens universal fuzzy command palette
    handle_key_event(&mut app, ctrl_key(KeyCode::Char('p')));
    assert!(matches!(
        app.current_modal(),
        Some(ModalState::CommandPalette { .. })
    ));

    // Escape dismisses command palette
    handle_key_event(&mut app, key(KeyCode::Esc));
    assert!(
        !app.has_active_modal(),
        "Escape must dismiss command palette"
    );
}

#[test]
fn test_help_modal_shortcuts() {
    let (_tmp, mut app) = create_test_app();

    // F1 opens help
    handle_key_event(&mut app, key(KeyCode::F(1)));
    assert!(matches!(
        app.current_modal(),
        Some(ModalState::HelpModal { .. })
    ));
    handle_key_event(&mut app, key(KeyCode::Esc));
    assert!(!app.has_active_modal());

    // Ctrl+H opens help
    handle_key_event(&mut app, ctrl_key(KeyCode::Char('h')));
    assert!(matches!(
        app.current_modal(),
        Some(ModalState::HelpModal { .. })
    ));

    // 'q' also dismisses help modal
    handle_key_event(&mut app, key(KeyCode::Char('q')));
    assert!(!app.has_active_modal());

    // Enter also dismisses help modal
    handle_key_event(&mut app, ctrl_key(KeyCode::Char('h')));
    assert!(app.has_active_modal());
    handle_key_event(&mut app, key(KeyCode::Enter));
    assert!(!app.has_active_modal());
}

// =============================================================================
// 3. Level 1: Core Navigation & Dialog Shortcuts
// =============================================================================

#[test]
fn test_model_picker_all_invocations_and_dismissals() {
    let (_tmp, mut app) = create_test_app();

    // 1. Ctrl+M opens model picker
    handle_key_event(&mut app, ctrl_key(KeyCode::Char('m')));
    assert!(matches!(
        app.current_modal(),
        Some(ModalState::ModelPicker { .. })
    ));
    // Ctrl+M again closes model picker
    handle_key_event(&mut app, ctrl_key(KeyCode::Char('m')));
    assert!(!app.has_active_modal());

    // 2. Alt+M opens model picker (legacy terminal fallback)
    handle_key_event(&mut app, alt_key(KeyCode::Char('m')));
    assert!(matches!(
        app.current_modal(),
        Some(ModalState::ModelPicker { .. })
    ));
    // Alt+M again closes model picker
    handle_key_event(&mut app, alt_key(KeyCode::Char('m')));
    assert!(!app.has_active_modal());

    // 3. F2 opens model picker
    handle_key_event(&mut app, key(KeyCode::F(2)));
    assert!(matches!(
        app.current_modal(),
        Some(ModalState::ModelPicker { .. })
    ));
    // F2 again closes model picker
    handle_key_event(&mut app, key(KeyCode::F(2)));
    assert!(!app.has_active_modal());
}

#[test]
fn test_core_modal_shortcuts_open_and_close() {
    let (_tmp, mut app) = create_test_app();

    // 1. Settings: Ctrl+S
    handle_key_event(&mut app, ctrl_key(KeyCode::Char('s')));
    assert!(matches!(
        app.current_modal(),
        Some(ModalState::SettingsModal { .. })
    ));
    handle_key_event(&mut app, key(KeyCode::Esc));
    assert!(!app.has_active_modal());

    // 2. Analytics Dashboard: Ctrl+A
    handle_key_event(&mut app, ctrl_key(KeyCode::Char('a')));
    assert!(matches!(
        app.current_modal(),
        Some(ModalState::AnalyticsDashboard)
    ));
    handle_key_event(&mut app, key(KeyCode::Esc));
    assert!(!app.has_active_modal());

    // 3. Timing / Critical Path: Ctrl+K
    handle_key_event(&mut app, ctrl_key(KeyCode::Char('k')));
    assert!(matches!(
        app.current_modal(),
        Some(ModalState::CriticalPathModal { .. })
    ));
    handle_key_event(&mut app, key(KeyCode::Esc));
    assert!(!app.has_active_modal());

    // 4. Concurrency Locks: Ctrl+E
    handle_key_event(&mut app, ctrl_key(KeyCode::Char('e')));
    assert!(matches!(
        app.current_modal(),
        Some(ModalState::ActiveLocksModal { .. })
    ));
    handle_key_event(&mut app, key(KeyCode::Esc));
    assert!(!app.has_active_modal());

    // 5. Resume Historical Session: Ctrl+R
    handle_key_event(&mut app, ctrl_key(KeyCode::Char('r')));
    assert!(matches!(
        app.current_modal(),
        Some(ModalState::ResumePicker(_))
    ));
    handle_key_event(&mut app, key(KeyCode::Esc));
    assert!(!app.has_active_modal());

    // 6. Worktree Management: Ctrl+W
    handle_key_event(&mut app, ctrl_key(KeyCode::Char('w')));
    if app.has_active_modal() {
        assert!(matches!(
            app.current_modal(),
            Some(ModalState::WorktreeSelector { .. })
        ));
        handle_key_event(&mut app, key(KeyCode::Esc));
        assert!(!app.has_active_modal());
    }
}

#[test]
fn test_session_creation_ctrl_n() {
    let (_tmp, mut app) = create_test_app();
    let initial_session = app.session_id.clone();

    handle_key_event(&mut app, ctrl_key(KeyCode::Char('n')));
    assert_ne!(
        app.session_id, initial_session,
        "Ctrl+N must initialize a new distinct session ID"
    );
}

#[test]
fn test_transcript_clear_ctrl_l() {
    let (_tmp, mut app) = create_test_app();
    app.transcript.push(TranscriptItem::notice(
        "Historical message",
        NoticeLevel::Info,
    ));
    app.scroll_offset = 12;
    assert!(!app.transcript.is_empty());

    handle_key_event(&mut app, ctrl_key(KeyCode::Char('l')));
    assert!(
        app.transcript.is_empty(),
        "Ctrl+L must clear the transcript"
    );
    assert_eq!(app.scroll_offset, 0, "Ctrl+L must reset scroll offset to 0");
}

#[test]
fn test_inspector_panel_toggles_and_navigation() {
    let (_tmp, mut app) = create_test_app();
    let initial_state = app.inspector_open;

    // Toggle via Ctrl+B
    handle_key_event(&mut app, ctrl_key(KeyCode::Char('b')));
    assert_eq!(app.inspector_open, !initial_state);

    // Toggle via F12
    handle_key_event(&mut app, key(KeyCode::F(12)));
    assert_eq!(app.inspector_open, initial_state);

    // Tab cycling in panel: Ctrl+[ and Ctrl+]
    handle_key_event(&mut app, ctrl_key(KeyCode::Char(']')));
    handle_key_event(&mut app, ctrl_key(KeyCode::Char('[')));

    // Panel resizing: Alt+[ and Alt+]
    handle_key_event(&mut app, alt_key(KeyCode::Char('[')));
    handle_key_event(&mut app, alt_key(KeyCode::Char(']')));
}

// =============================================================================
// 4. Function Keys (F3..F10) & Alt+Numbers (Alt+1..Alt+0) Screen Switching
// =============================================================================

#[test]
fn test_all_10_screens_switching_via_function_keys_and_alt_numbers() {
    let (_tmp, mut app) = create_test_app();

    // 1. Function Key screen switches
    let f_mappings = [
        (KeyCode::F(3), Screen::Models),
        (KeyCode::F(4), Screen::Providers),
        (KeyCode::F(5), Screen::Agents),
        (KeyCode::F(6), Screen::Workspace),
        (KeyCode::F(7), Screen::SourceControl),
        (KeyCode::F(8), Screen::ToolsIntegrations),
        (KeyCode::F(9), Screen::Settings),
        (KeyCode::F(10), Screen::Diagnostics),
    ];

    for (k, expected_screen) in f_mappings {
        handle_key_event(&mut app, key(k));
        assert_eq!(
            app.current_screen, expected_screen,
            "Key {:?} must switch to screen {:?}",
            k, expected_screen
        );
    }

    // 2. Alt+Number direct screen switches (all 10 screens)
    let alt_mappings = [
        (KeyCode::Char('1'), Screen::Chat),
        (KeyCode::Char('2'), Screen::Sessions),
        (KeyCode::Char('3'), Screen::Models),
        (KeyCode::Char('4'), Screen::Providers),
        (KeyCode::Char('5'), Screen::Agents),
        (KeyCode::Char('6'), Screen::Workspace),
        (KeyCode::Char('7'), Screen::SourceControl),
        (KeyCode::Char('8'), Screen::ToolsIntegrations),
        (KeyCode::Char('9'), Screen::Settings),
        (KeyCode::Char('0'), Screen::Diagnostics),
    ];

    for (k, expected_screen) in alt_mappings {
        handle_key_event(&mut app, alt_key(k));
        assert_eq!(
            app.current_screen, expected_screen,
            "Alt+{:?} must switch to screen {:?}",
            k, expected_screen
        );
    }

    // Ctrl+D switches directly to Source Control screen
    handle_key_event(&mut app, alt_key(KeyCode::Char('1')));
    assert_eq!(app.current_screen, Screen::Chat);
    handle_key_event(&mut app, ctrl_key(KeyCode::Char('d')));
    assert_eq!(app.current_screen, Screen::SourceControl);
}

// =============================================================================
// 5. Composer Editing, Multi-line, Cursor, and Mode Keybindings
// =============================================================================

#[test]
fn test_composer_typing_cursor_and_multi_line_newlines() {
    let (_tmp, mut app) = create_test_app();
    app.switch_screen(Screen::Chat);

    // Type "Hello"
    for c in "Hello".chars() {
        handle_key_event(&mut app, key(KeyCode::Char(c)));
    }
    assert_eq!(app.composer.buffer, "Hello");

    // Shift+Enter inserts newline without submitting
    handle_key_event(&mut app, shift_key(KeyCode::Enter));
    assert!(app.composer.buffer.contains('\n'));

    // Alt+Enter inserts newline (terminal fallback)
    for c in "World".chars() {
        handle_key_event(&mut app, key(KeyCode::Char(c)));
    }
    handle_key_event(&mut app, alt_key(KeyCode::Enter));

    // Cursor navigation: Home, Left, Right, End
    handle_key_event(&mut app, key(KeyCode::Home));
    handle_key_event(&mut app, key(KeyCode::Right));
    handle_key_event(&mut app, key(KeyCode::Left));
    handle_key_event(&mut app, key(KeyCode::End));

    // Backspace & Delete
    handle_key_event(&mut app, key(KeyCode::Backspace));
    handle_key_event(&mut app, key(KeyCode::Delete));
}

#[test]
fn test_tab_toggles_plan_and_build_collaboration_modes() {
    let (_tmp, mut app) = create_test_app();
    app.switch_screen(Screen::Chat);
    app.composer.clear();

    let initial_mode = app.collaboration_mode;

    // Tab toggles mode
    handle_key_event(&mut app, key(KeyCode::Tab));
    let toggled_mode = app.collaboration_mode;
    assert_ne!(
        initial_mode, toggled_mode,
        "Tab must toggle collaboration mode"
    );

    // BackTab also toggles mode
    handle_key_event(&mut app, key(KeyCode::BackTab));
    assert_eq!(
        app.collaboration_mode, initial_mode,
        "BackTab must toggle mode back"
    );
}

#[test]
fn test_pageup_pagedown_and_home_end_transcript_scrolling() {
    let (_tmp, mut app) = create_test_app();
    app.switch_screen(Screen::Chat);

    // Fill transcript with 50 items
    for i in 0..50 {
        app.transcript
            .push(TranscriptItem::user(format!("Message line {i}")));
    }
    app.sync_virtualizer();

    let init_offset = app.scroll_offset;
    handle_key_event(&mut app, key(KeyCode::PageUp));
    assert!(
        app.scroll_offset > init_offset,
        "PageUp must increase scroll offset"
    );

    handle_key_event(&mut app, key(KeyCode::PageDown));

    // Ctrl+Home / Ctrl+End
    handle_key_event(&mut app, ctrl_key(KeyCode::Home));
    handle_key_event(&mut app, ctrl_key(KeyCode::End));
}

#[test]
fn test_alt_left_right_subagent_tab_switching() {
    let (_tmp, mut app) = create_test_app();
    app.switch_screen(Screen::Chat);

    // Alt+Left and Alt+Right navigate subagents safely even with 0 subagents
    handle_key_event(&mut app, alt_key(KeyCode::Left));
    handle_key_event(&mut app, alt_key(KeyCode::Right));
}

// =============================================================================
// 6. Interactive Modal Keyboard Navigation & Selection
// =============================================================================

#[test]
fn test_interactive_model_picker_navigation_and_selection() {
    let (_tmp, mut app) = create_test_app();

    // Open Model Picker via F2
    handle_key_event(&mut app, key(KeyCode::F(2)));
    assert!(matches!(
        app.current_modal(),
        Some(ModalState::ModelPicker { .. })
    ));

    // Up and Down navigation
    handle_key_event(&mut app, key(KeyCode::Down));
    handle_key_event(&mut app, key(KeyCode::Down));
    handle_key_event(&mut app, key(KeyCode::Up));

    // Enter confirms selection and dismisses modal
    handle_key_event(&mut app, key(KeyCode::Enter));
    assert!(
        !app.has_active_modal(),
        "Enter on model picker must close modal"
    );
}

#[test]
fn test_interactive_command_palette_filtering_and_navigation() {
    let (_tmp, mut app) = create_test_app();

    // Open via Ctrl+P
    handle_key_event(&mut app, ctrl_key(KeyCode::Char('p')));
    assert!(matches!(
        app.current_modal(),
        Some(ModalState::CommandPalette { .. })
    ));

    // Type query "diff"
    for c in "diff".chars() {
        handle_key_event(&mut app, key(KeyCode::Char(c)));
    }

    // Down and Up navigation within filtered results
    handle_key_event(&mut app, key(KeyCode::Down));
    handle_key_event(&mut app, key(KeyCode::Up));

    // Esc dismisses palette
    handle_key_event(&mut app, key(KeyCode::Esc));
    assert!(!app.has_active_modal());
}

#[test]
fn test_interactive_settings_modal_navigation() {
    let (_tmp, mut app) = create_test_app();

    // Open Settings via Ctrl+S
    handle_key_event(&mut app, ctrl_key(KeyCode::Char('s')));
    assert!(matches!(
        app.current_modal(),
        Some(ModalState::SettingsModal { .. })
    ));

    // Navigate with Down / Up / Tab
    handle_key_event(&mut app, key(KeyCode::Down));
    handle_key_event(&mut app, key(KeyCode::Down));
    handle_key_event(&mut app, key(KeyCode::Up));
    handle_key_event(&mut app, key(KeyCode::Tab));

    // Esc dismisses
    handle_key_event(&mut app, key(KeyCode::Esc));
    assert!(!app.has_active_modal());
}

// =============================================================================
// 7. Terminal Rendering Pipeline Under Full Keybinding Traversal
// =============================================================================

#[test]
fn test_all_keybindings_terminal_render_integrity_across_viewports() {
    let (_tmp, mut app) = create_test_app();

    let viewports = [
        (120, 35), // Standard widescreen
        (80, 24),  // Classic VT100
        (50, 15),  // Constrained narrow terminal
    ];

    let test_keys = [
        key(KeyCode::F(1)),
        key(KeyCode::Esc),
        key(KeyCode::F(2)),
        key(KeyCode::Esc),
        key(KeyCode::F(3)),
        key(KeyCode::F(4)),
        key(KeyCode::F(5)),
        key(KeyCode::F(6)),
        key(KeyCode::F(7)),
        key(KeyCode::F(8)),
        key(KeyCode::F(9)),
        key(KeyCode::F(10)),
        alt_key(KeyCode::Char('1')),
        ctrl_key(KeyCode::Char('p')),
        key(KeyCode::Esc),
        ctrl_key(KeyCode::Char('s')),
        key(KeyCode::Esc),
        ctrl_key(KeyCode::Char('a')),
        key(KeyCode::Esc),
        ctrl_key(KeyCode::Char('k')),
        key(KeyCode::Esc),
        ctrl_key(KeyCode::Char('e')),
        key(KeyCode::Esc),
        ctrl_key(KeyCode::Char('b')),
        key(KeyCode::Tab),
        key(KeyCode::BackTab),
        key(KeyCode::PageUp),
        key(KeyCode::PageDown),
    ];

    for &(width, height) in &viewports {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("ratatui terminal creation");

        for &k in &test_keys {
            handle_key_event(&mut app, k);
            terminal
                .draw(|f| {
                    m31a_tui::renderer::render(f, &mut app);
                })
                .unwrap_or_else(|e| {
                    panic!(
                        "Terminal render failed on viewport {}x{} after key {:?}: {}",
                        width, height, k, e
                    )
                });
        }
    }
}
