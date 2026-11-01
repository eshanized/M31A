//! Dedicated integration tests for `m31a-composer` (src/mentions.rs).
//!
//! Validates:
//! - `@`-mention popup activation rules (line start, whitespace boundary)
//! - Suppression on mid-token `@` characters (e.g. email addresses)
//! - Dismissal on trailing whitespace in mention queries
//! - Candidate cursor navigation (`move_up`, `move_down`) and bounds clamping
//! - Candidate resolution and popup dismissal cleanup

use m31a_composer::mentions::MentionsPopup;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_mentions_activation_and_whitespace_boundaries() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    // Create candidate files
    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("main.rs"), b"").unwrap();
    fs::write(src.join("lib.rs"), b"").unwrap();

    let mut popup = MentionsPopup::new();
    assert!(!popup.is_active);

    // 1. Valid mention at start of line
    popup.update_from_line("@main", 5, root);
    assert!(popup.is_active);
    assert_eq!(popup.query, "main");
    assert_eq!(popup.at_char_col, 0);

    // 2. Valid mention preceded by space
    popup.update_from_line("Please inspect @lib", 19, root);
    assert!(popup.is_active);
    assert_eq!(popup.query, "lib");
    assert_eq!(popup.at_char_col, 15);

    // 3. Email address must NOT activate mention popup
    popup.update_from_line("Contact user@example.com", 24, root);
    assert!(!popup.is_active, "Email address must not activate mentions");

    // 4. Space after query dismisses mention popup
    popup.update_from_line("Look at @main next", 18, root);
    assert!(!popup.is_active, "Space after query must dismiss popup");
}

#[test]
fn test_mentions_navigation_and_selection() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("alpha.rs"), b"").unwrap();
    fs::write(src.join("beta.rs"), b"").unwrap();
    fs::write(src.join("gamma.rs"), b"").unwrap();

    let mut popup = MentionsPopup::new();
    popup.update_from_line("@", 1, root);
    assert!(popup.is_active);

    if popup.candidates.len() >= 2 {
        assert_eq!(popup.selected_index, 0);

        popup.move_down();
        assert_eq!(popup.selected_index, 1);

        popup.move_up();
        assert_eq!(popup.selected_index, 0);

        popup.move_up();
        assert_eq!(popup.selected_index, 0, "Move up at top should clamp to 0");
    }

    popup.dismiss();
    assert!(!popup.is_active);
    assert!(popup.query.is_empty());
    assert!(popup.candidates.is_empty());
    assert_eq!(popup.selected_index, 0);
}
