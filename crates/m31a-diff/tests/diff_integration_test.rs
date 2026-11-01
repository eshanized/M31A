//! Integration tests for unified and intra-line word diff engine.
//!
//! Tests diff hunk generation, word-level insertions/deletions, and theme styling.

use m31a_diff::{DiffEngine, DiffLineKind, DiffRenderer, DiffViewerState};

#[test]
fn test_diff_engine_full_lifecycle() {
    let old = "fn main() {\n    println!(\"hello\");\n}\n";
    let new = "fn main() {\n    let val = 42;\n    println!(\"hello {}\", val);\n}\n";

    let (lines, stats) = DiffEngine::compute_diff(old, new);
    assert_eq!(stats.files_changed, 1);
    assert!(stats.lines_added >= 1);
    assert!(stats.lines_deleted >= 1);

    // Verify context lines retain old and new line numbers
    let context_lines: Vec<_> = lines
        .iter()
        .filter(|l| l.kind == DiffLineKind::Context)
        .collect();
    assert!(!context_lines.is_empty());
    assert_eq!(context_lines[0].old_line_num, Some(1));
    assert_eq!(context_lines[0].new_line_num, Some(1));

    // Verify added lines have None for old_line_num
    let added_lines: Vec<_> = lines
        .iter()
        .filter(|l| l.kind == DiffLineKind::Add)
        .collect();
    assert!(!added_lines.is_empty());
    for add in added_lines {
        assert!(add.old_line_num.is_none());
        assert!(add.new_line_num.is_some());
    }
}

#[test]
fn test_intra_line_word_level_highlighting() {
    let old_line = "const TIMEOUT: u64 = 5000;";
    let new_line = "const TIMEOUT: u64 = 10000;";

    let (lines, stats) = DiffEngine::compute_diff(old_line, new_line);
    assert_eq!(stats.lines_deleted, 1);
    assert_eq!(stats.lines_added, 1);

    let del = lines
        .iter()
        .find(|l| l.kind == DiffLineKind::Delete)
        .unwrap();
    assert!(del.word_spans.iter().any(|s| s.is_highlighted));

    let add = lines.iter().find(|l| l.kind == DiffLineKind::Add).unwrap();
    assert!(add.word_spans.iter().any(|s| s.is_highlighted));
}

#[test]
fn test_parse_unified_diff_hunks() {
    let patch = r#"@@ -1,3 +1,4 @@
 fn foo() {
-    let x = 1;
+    let x = 2;
+    let y = 3;
 }
"#;

    let (lines, stats) = DiffEngine::parse_unified_diff(patch);
    assert_eq!(stats.lines_deleted, 1);
    assert_eq!(stats.lines_added, 2);

    assert!(lines.iter().any(|l| l.kind == DiffLineKind::HunkHeader));
    assert!(
        lines
            .iter()
            .any(|l| l.kind == DiffLineKind::Delete && l.content.contains("let x = 1;"))
    );
    assert!(
        lines
            .iter()
            .any(|l| l.kind == DiffLineKind::Add && l.content.contains("let x = 2;"))
    );
    assert!(
        lines
            .iter()
            .any(|l| l.kind == DiffLineKind::Add && l.content.contains("let y = 3;"))
    );
}

#[test]
fn test_diff_renderer_terminal_spans() {
    let old = "foo\nbar\n";
    let new = "foo\nbaz\n";
    let (lines, _) = DiffEngine::compute_diff(old, new);

    let rendered = DiffRenderer::render_lines(&lines, 50);
    assert_eq!(rendered.len(), lines.len());

    // Check that Add lines have '+' in the first span
    let add_rendered = rendered
        .iter()
        .find(|l| l.spans.iter().any(|s| s.content.contains('+')));
    assert!(add_rendered.is_some());

    // Check that Delete lines have '-' in the first span
    let del_rendered = rendered
        .iter()
        .find(|l| l.spans.iter().any(|s| s.content.contains('-')));
    assert!(del_rendered.is_some());
}

#[test]
fn test_diff_viewer_scrolling_and_page_navigation() {
    let old = (1..=50)
        .map(|i| format!("Line {}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let new = (1..=50)
        .map(|i| format!("Modified {}", i))
        .collect::<Vec<_>>()
        .join("\n");

    let mut viewer = DiffViewerState::new("src/large.rs", &old, &new);
    assert_eq!(viewer.scroll_offset, 0);
    assert!(viewer.max_scroll > 50);

    // Scroll down 5
    viewer.scroll_down(5);
    assert_eq!(viewer.scroll_offset, 5);

    // Scroll page down (15)
    viewer.scroll_page_down();
    assert_eq!(viewer.scroll_offset, 20);

    // Scroll page up (15)
    viewer.scroll_page_up();
    assert_eq!(viewer.scroll_offset, 5);

    // Scroll up past 0 -> clamped to 0
    viewer.scroll_up(50);
    assert_eq!(viewer.scroll_offset, 0);

    // Scroll down past max -> clamped to max_scroll
    viewer.scroll_down(1000);
    assert_eq!(viewer.scroll_offset, viewer.max_scroll);

    // Also test viewer from unified patch
    let patch = "@@ -1,2 +1,2 @@\n-old\n+new\n";
    let mut patch_viewer = DiffViewerState::from_unified_patch("patch.diff", patch);
    assert_eq!(patch_viewer.file_path, "patch.diff");
    assert_eq!(patch_viewer.stats.lines_added, 1);
    assert_eq!(patch_viewer.stats.lines_deleted, 1);
    patch_viewer.scroll_down(1);
    assert_eq!(patch_viewer.scroll_offset, 1);
    patch_viewer.scroll_page_up();
    assert_eq!(patch_viewer.scroll_offset, 0);

    // Test render lines with hunk header
    let (parsed_lines, _) = DiffEngine::parse_unified_diff(patch);
    let rendered_patch = DiffRenderer::render_lines(&parsed_lines, 10);
    assert!(
        rendered_patch
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains('@')))
    );
}
