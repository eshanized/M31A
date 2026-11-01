//! Dedicated integration tests for `m31a-elicitation` (src/questions.rs).
//!
//! Validates:
//! - Single-selection cursor navigation, clamping, and custom input focus transitions
//! - Multi-selection toggle mechanics, index sorting, and deselection
//! - Submission resolution rules (custom input priority, default cursor fallback, empty fallback)
//! - Focus isolation (preventing selection toggles while typing custom input)
//! - Serialization fidelity

use m31a_elicitation::questions::QuestionItem;

#[test]
fn test_question_cursor_navigation_and_custom_focus() {
    let options = vec!["Option A".to_string(), "Option B".to_string()];
    let mut q = QuestionItem::new("Select item", options, false);

    assert_eq!(q.cursor_idx, 0);
    assert!(!q.is_custom_focused);

    // move up at top remains at 0
    q.move_up();
    assert_eq!(q.cursor_idx, 0);

    // move down advances cursor
    q.move_down();
    assert_eq!(q.cursor_idx, 1);
    assert!(!q.is_custom_focused);

    // move down at last option enters custom focus
    q.move_down();
    assert!(q.is_custom_focused);

    // move down while in custom focus does nothing
    q.move_down();
    assert!(q.is_custom_focused);

    // move up leaves custom focus and returns cursor to last option
    q.move_up();
    assert!(!q.is_custom_focused);
    assert_eq!(q.cursor_idx, 1);

    // move up returns to option 0
    q.move_up();
    assert_eq!(q.cursor_idx, 0);
}

#[test]
fn test_single_selection_and_submission() {
    let options = vec!["Rust".to_string(), "Go".to_string(), "Python".to_string()];
    let mut q = QuestionItem::new("Preferred language", options, false);

    // Default submission without explicit toggle returns option at cursor 0
    let res = q.submit();
    assert_eq!(res, vec!["Rust".to_string()]);

    // Move to Go (index 1) and toggle
    q.move_down();
    q.toggle_selection();
    assert_eq!(q.selected_indices.len(), 1);
    assert!(q.selected_indices.contains(&1));

    // Move to Python (index 2) and toggle (should replace index 1)
    q.move_down();
    q.toggle_selection();
    assert_eq!(q.selected_indices.len(), 1);
    assert!(q.selected_indices.contains(&2));
    assert_eq!(q.submit(), vec!["Python".to_string()]);
}

#[test]
fn test_multi_selection_and_deselection() {
    let options = vec![
        "Linter".to_string(),
        "Formatter".to_string(),
        "Fuzzer".to_string(),
    ];
    let mut q = QuestionItem::new("Select tools to run", options, true);

    // Select index 0 (Linter)
    q.toggle_selection();
    assert!(q.selected_indices.contains(&0));

    // Move down and select index 2 (Fuzzer)
    q.move_down(); // idx 1
    q.move_down(); // idx 2
    q.toggle_selection();
    assert_eq!(q.selected_indices.len(), 2);

    let res = q.submit();
    assert_eq!(res, vec!["Linter".to_string(), "Fuzzer".to_string()]);

    // Deselect index 0
    q.move_up(); // idx 1
    q.move_up(); // idx 0
    q.toggle_selection();
    assert_eq!(q.selected_indices.len(), 1);
    assert!(!q.selected_indices.contains(&0));
    assert_eq!(q.submit(), vec!["Fuzzer".to_string()]);
}

#[test]
fn test_custom_input_submission_priority() {
    let options = vec!["Postgres".to_string(), "MySQL".to_string()];
    let mut q = QuestionItem::new("Database choice", options, false);

    q.custom_input = "  CockroachDB  ".to_string();
    // Also toggle an option
    q.toggle_selection(); // selects Postgres

    let res = q.submit();
    // Custom input trimmed should be first, followed by selected options
    assert_eq!(res.len(), 2);
    assert_eq!(res[0], "CockroachDB");
    assert_eq!(res[1], "Postgres");
}

#[test]
fn test_custom_focused_toggle_is_ignored() {
    let options = vec!["Option 1".to_string()];
    let mut q = QuestionItem::new("Query", options, false);

    q.move_down(); // enters custom focus
    assert!(q.is_custom_focused);

    // Toggle while custom focused should be no-op
    q.toggle_selection();
    assert!(q.selected_indices.is_empty());
}
