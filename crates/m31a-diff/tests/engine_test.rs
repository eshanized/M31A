//! Dedicated test for `crates/m31a-diff/src/engine.rs`.
//!
//! Validates `DiffEngine::compute_diff` and `DiffEngine::parse_unified_diff`.

use m31a_diff::engine::{DiffEngine, DiffLineKind};

#[test]
fn test_engine_identical_content() {
    let text = "line 1\nline 2\nline 3\n";
    let (lines, stats) = DiffEngine::compute_diff(text, text);

    assert_eq!(stats.files_changed, 0);
    assert_eq!(stats.lines_added, 0);
    assert_eq!(stats.lines_deleted, 0);
    assert_eq!(lines.len(), 3);

    for (idx, line) in lines.iter().enumerate() {
        assert_eq!(line.kind, DiffLineKind::Context);
        assert_eq!(line.old_line_num, Some(idx + 1));
        assert_eq!(line.new_line_num, Some(idx + 1));
        assert!(!line.word_spans[0].is_highlighted);
    }
}

#[test]
fn test_engine_additions_and_deletions() {
    let old = "apple\nbanana\ncherry\n";
    let new = "apple\nblueberry\ncherry\ndate\n";

    let (lines, stats) = DiffEngine::compute_diff(old, new);
    assert_eq!(stats.files_changed, 1);
    assert_eq!(stats.lines_deleted, 1);
    assert_eq!(stats.lines_added, 2);

    // Verify 'apple' context
    assert_eq!(lines[0].kind, DiffLineKind::Context);
    assert_eq!(lines[0].old_line_num, Some(1));
    assert_eq!(lines[0].new_line_num, Some(1));

    // Verify 'banana' delete
    let del = lines
        .iter()
        .find(|l| l.content == "banana")
        .expect("deleted banana");
    assert_eq!(del.kind, DiffLineKind::Delete);
    assert_eq!(del.old_line_num, Some(2));
    assert_eq!(del.new_line_num, None);
    assert!(del.word_spans[0].is_highlighted);

    // Verify 'blueberry' add
    let add_blue = lines
        .iter()
        .find(|l| l.content == "blueberry")
        .expect("added blueberry");
    assert_eq!(add_blue.kind, DiffLineKind::Add);
    assert_eq!(add_blue.old_line_num, None);
    assert_eq!(add_blue.new_line_num, Some(2));
    assert!(add_blue.word_spans[0].is_highlighted);

    // Verify 'date' add
    let add_date = lines
        .iter()
        .find(|l| l.content == "date")
        .expect("added date");
    assert_eq!(add_date.kind, DiffLineKind::Add);
    assert_eq!(add_date.old_line_num, None);
    assert_eq!(add_date.new_line_num, Some(4));
}

#[test]
fn test_engine_empty_input_extremes() {
    // Empty to non-empty
    let (lines, stats) = DiffEngine::compute_diff("", "single line\n");
    assert_eq!(stats.lines_added, 1);
    assert_eq!(stats.lines_deleted, 0);
    assert_eq!(stats.files_changed, 1);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].kind, DiffLineKind::Add);

    // Non-empty to empty
    let (lines, stats) = DiffEngine::compute_diff("single line\n", "");
    assert_eq!(stats.lines_added, 0);
    assert_eq!(stats.lines_deleted, 1);
    assert_eq!(stats.files_changed, 1);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].kind, DiffLineKind::Delete);
}

#[test]
fn test_engine_parse_multi_file_unified_diff() {
    let patch = r#"diff --git a/foo.rs b/foo.rs
index 1111..2222 100644
--- a/foo.rs
+++ b/foo.rs
@@ -1,2 +1,2 @@
-let a = 1;
+let a = 10;
 common line
diff --git a/bar.rs b/bar.rs
index 3333..4444 100644
--- a/bar.rs
+++ b/bar.rs
@@ -1,1 +1,2 @@
+new bar
 unchanged
"#;

    let (lines, stats) = DiffEngine::parse_unified_diff(patch);
    assert_eq!(stats.files_changed, 2);
    assert_eq!(stats.lines_added, 2);
    assert_eq!(stats.lines_deleted, 1);

    let hunk_headers: Vec<_> = lines
        .iter()
        .filter(|l| l.kind == DiffLineKind::HunkHeader)
        .collect();
    assert_eq!(hunk_headers.len(), 4); // 2 "diff --git" + 2 "@@"

    let adds: Vec<_> = lines
        .iter()
        .filter(|l| l.kind == DiffLineKind::Add)
        .collect();
    assert_eq!(adds.len(), 2);
    assert_eq!(adds[0].content, "let a = 10;");
    assert_eq!(adds[1].content, "new bar");

    let dels: Vec<_> = lines
        .iter()
        .filter(|l| l.kind == DiffLineKind::Delete)
        .collect();
    assert_eq!(dels.len(), 1);
    assert_eq!(dels[0].content, "let a = 1;");
}
