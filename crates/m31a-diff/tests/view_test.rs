//! Dedicated test for `crates/m31a-diff/src/view.rs`.
//!
//! Validates `DiffViewerState` construction, scroll offsets, paging, and clamping.

use m31a_diff::view::DiffViewerState;

#[test]
fn test_view_empty_diff() {
    let mut state = DiffViewerState::new("empty.rs", "", "");
    assert_eq!(state.file_path, "empty.rs");
    assert_eq!(state.scroll_offset, 0);
    assert_eq!(state.max_scroll, 0);

    state.scroll_down(5);
    assert_eq!(state.scroll_offset, 0);

    state.scroll_up(5);
    assert_eq!(state.scroll_offset, 0);

    state.scroll_page_down();
    assert_eq!(state.scroll_offset, 0);

    state.scroll_page_up();
    assert_eq!(state.scroll_offset, 0);
}

#[test]
fn test_view_from_texts_scrolling() {
    let old_text = (0..50)
        .map(|i| format!("line {}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let new_text = (0..50)
        .map(|i| format!("line {}", i * 2))
        .collect::<Vec<_>>()
        .join("\n");

    let mut state = DiffViewerState::new("diff_test.rs", &old_text, &new_text);
    assert_eq!(state.scroll_offset, 0);
    assert!(state.max_scroll >= 50);

    // Step down
    state.scroll_down(10);
    assert_eq!(state.scroll_offset, 10);

    // Page down (15)
    state.scroll_page_down();
    assert_eq!(state.scroll_offset, 25);

    // Page up (15)
    state.scroll_page_up();
    assert_eq!(state.scroll_offset, 10);

    // Step up
    state.scroll_up(4);
    assert_eq!(state.scroll_offset, 6);

    // Underflow clamp
    state.scroll_up(100);
    assert_eq!(state.scroll_offset, 0);

    // Overflow clamp
    state.scroll_down(10000);
    assert_eq!(state.scroll_offset, state.max_scroll);
}

#[test]
fn test_view_from_unified_patch() {
    let patch = r#"@@ -1,3 +1,3 @@
-alpha
+beta
 gamma
"#;
    let mut state = DiffViewerState::from_unified_patch("patch_test.rs", patch);
    assert_eq!(state.file_path, "patch_test.rs");
    assert_eq!(state.stats.lines_added, 1);
    assert_eq!(state.stats.lines_deleted, 1);
    assert_eq!(state.lines.len(), 4);
    assert_eq!(state.max_scroll, 3);

    state.scroll_down(2);
    assert_eq!(state.scroll_offset, 2);

    state.scroll_down(10);
    assert_eq!(state.scroll_offset, 3);
}
