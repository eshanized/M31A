//! Integration tests for TUI composer and text input widgets.
//!
//! Tests multiline editing, paste burst protection, slash command completion, and @ mentions.

use m31a_composer::{MentionsPopup, PasteBurstDetector, SlashPopup, TextArea};

#[test]
fn test_textarea_undo_redo_and_kill_yank() {
    let mut ta = TextArea::new();
    ta.insert_str("Hello World");
    assert_eq!(ta.text(), "Hello World");

    // Move cursor to "World"
    ta.cursor_col = 6;
    ta.kill_line();
    assert_eq!(ta.text(), "Hello ");

    // Yank it back
    ta.yank();
    assert_eq!(ta.text(), "Hello World");

    // Undo/Redo typing test
    let mut ta2 = TextArea::new();
    ta2.insert_str("A");
    ta2.insert_str("B");
    assert_eq!(ta2.text(), "AB");
    ta2.undo();
    assert_eq!(ta2.text(), "A");
    ta2.redo();
    assert_eq!(ta2.text(), "AB");
}

#[test]
fn test_textarea_multiline_cursor_navigation() {
    let mut ta = TextArea::new();
    ta.insert_str("Line 1\nLine 2\nLine 3");
    assert_eq!(ta.cursor_row, 2);

    ta.move_cursor_up();
    assert_eq!(ta.cursor_row, 1);

    ta.move_cursor_home();
    assert_eq!(ta.cursor_col, 0);

    ta.move_cursor_end();
    assert_eq!(ta.cursor_col, 6); // "Line 2".len()

    // Backspace from start of Line 3 joins lines
    ta.move_cursor_down();
    ta.move_cursor_home();
    assert_eq!(ta.cursor_row, 2);
    assert_eq!(ta.cursor_col, 0);

    ta.backspace();
    assert_eq!(ta.cursor_row, 1);
    assert_eq!(ta.lines[1], "Line 2Line 3");
}

#[test]
fn test_paste_burst_bracketed_paste_pill_creation() {
    let mut detector = PasteBurstDetector::new();
    detector.start_bracketed_paste();

    // Large multiline snippet
    let large_paste = "fn main() {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n    println!(\"{}\", a + b + c);\n}\n";
    for ch in large_paste.chars() {
        detector.on_char(ch);
    }

    let pill_opt = detector.end_bracketed_paste();
    assert!(pill_opt.is_some());
    let pill = pill_opt.unwrap();
    assert!(pill.line_count >= 5);
    assert!(pill.full_content.contains("fn main()"));
}

#[test]
fn test_slash_popup_trigger_and_filtering() {
    let mut popup = SlashPopup::new();
    assert!(!popup.is_active);

    // Type "/mo"
    popup.update("/mo");
    assert!(popup.is_active);

    assert!(popup.matches.iter().any(|c| c.name == "/model"));
    assert!(!popup.matches.iter().any(|c| c.name == "/diff"));

    let selected = popup.selected_command();
    assert!(selected.is_some());
    assert_eq!(selected.unwrap().name, "/model");
}

#[test]
fn test_mentions_popup_trigger_and_selection() {
    let mut popup = MentionsPopup::new();
    assert!(!popup.is_active);

    let cwd = std::env::current_dir().unwrap();
    popup.update_from_line("Inspect @Cargo", 14, &cwd);
    assert!(popup.is_active);
    assert_eq!(popup.query, "Cargo");

    let selected = popup.selected_candidate();
    assert!(selected.is_some());

    // Navigation in mentions
    popup.move_down();
    popup.move_up();
}

#[test]
fn test_textarea_delete_forward_and_cursor_movement() {
    let mut ta = TextArea::default();
    ta.insert_str("Hello World");
    ta.cursor_col = 0;

    // Delete 'H'
    ta.delete_forward();
    assert_eq!(ta.text(), "ello World");

    // Move right and left
    ta.move_cursor_right();
    assert_eq!(ta.cursor_col, 1);
    ta.move_cursor_left();
    assert_eq!(ta.cursor_col, 0);

    // Clear
    ta.clear();
    assert_eq!(ta.text(), "");

    // SlashPopup navigation
    let mut slash = SlashPopup::default();
    slash.update("/");
    assert!(slash.is_active);
    slash.move_down();
    assert_eq!(slash.selected_index, 1);
    slash.move_up();
    assert_eq!(slash.selected_index, 0);
}
