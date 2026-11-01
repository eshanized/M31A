//! Dedicated integration tests for `m31a-composer` (src/textarea.rs).
//!
//! Validates:
//! - Multiline string insertion, line splitting, and cursor placement
//! - Backspace and forward delete operations across line boundaries
//! - Directional cursor navigation (left, right, up, down, home, end)
//! - Emacs-compatible kill-ring (`kill_line`) and yank (`yank`) buffer
//! - Undo and redo history preservation across mutations

use m31a_composer::textarea::TextArea;

#[test]
fn test_textarea_insertion_and_newlines() {
    let mut area = TextArea::new();
    assert_eq!(area.text(), "");
    assert_eq!(area.cursor_row, 0);
    assert_eq!(area.cursor_col, 0);

    area.insert_str("Hello");
    assert_eq!(area.text(), "Hello");
    assert_eq!(area.cursor_col, 5);

    // Insert newline: splits line
    area.insert_char('\n');
    assert_eq!(area.cursor_row, 1);
    assert_eq!(area.cursor_col, 0);

    area.insert_str("World");
    assert_eq!(area.text(), "Hello\nWorld");
    assert_eq!(area.cursor_row, 1);
    assert_eq!(area.cursor_col, 5);
}

#[test]
fn test_textarea_backspace_and_line_merging() {
    let mut area = TextArea::new();
    area.insert_str("First\nSecond");
    assert_eq!(area.cursor_row, 1);
    assert_eq!(area.cursor_col, 6);

    // Backspace single char
    area.backspace();
    assert_eq!(area.text(), "First\nSecon");
    assert_eq!(area.cursor_col, 5);

    // Move to start of line 1
    area.move_cursor_home();
    assert_eq!(area.cursor_col, 0);

    // Backspace at column 0 merges with line 0
    area.backspace();
    assert_eq!(area.text(), "FirstSecon");
    assert_eq!(area.cursor_row, 0);
    assert_eq!(area.cursor_col, 5);
}

#[test]
fn test_textarea_delete_forward_and_merging() {
    let mut area = TextArea::new();
    area.insert_str("ABC\nDEF");
    area.cursor_row = 0;
    area.cursor_col = 1; // Between A and B

    area.delete_forward(); // Deletes B
    assert_eq!(area.text(), "AC\nDEF");

    // Move to end of line 0
    area.move_cursor_end();
    assert_eq!(area.cursor_col, 2);

    // Delete at end of line merges next line
    area.delete_forward();
    assert_eq!(area.text(), "ACDEF");
}

#[test]
fn test_textarea_cursor_navigation_and_bounds() {
    let mut area = TextArea::new();
    area.insert_str("Alpha\nBeta\nGamma");

    // Home / End
    area.move_cursor_home();
    assert_eq!(area.cursor_col, 0);
    area.move_cursor_end();
    assert_eq!(area.cursor_col, 5); // "Gamma".len()

    // Up / Down
    area.move_cursor_up();
    assert_eq!(area.cursor_row, 1);
    assert_eq!(area.cursor_col, 4); // Clamped to "Beta".len() = 4

    area.move_cursor_up();
    assert_eq!(area.cursor_row, 0);

    // Move up at row 0 clamps
    area.move_cursor_up();
    assert_eq!(area.cursor_row, 0);

    area.move_cursor_down();
    assert_eq!(area.cursor_row, 1);
}

#[test]
fn test_textarea_kill_line_and_yank() {
    let mut area = TextArea::new();
    area.insert_str("Prefix - Suffix to kill");
    area.cursor_col = 8; // At '-'

    area.kill_line();
    assert_eq!(area.text(), "Prefix -");

    // Yank back at end
    area.yank();
    assert_eq!(area.text(), "Prefix - Suffix to kill");
}

#[test]
fn test_textarea_undo_redo_cycles() {
    let mut area = TextArea::new();
    area.insert_char('A');
    area.insert_char('B');
    assert_eq!(area.text(), "AB");

    area.undo();
    assert_eq!(area.text(), "A");

    area.redo();
    assert_eq!(area.text(), "AB");
}
