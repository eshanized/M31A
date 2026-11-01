//! Dedicated test for `crates/m31a-diff/src/lib.rs`.
//!
//! Validates library re-exports, public API surface, and end-to-end integration
//! between tracker, engine, minimizer, renderer, and viewer.

use m31a_diff::{
    ChurnVerdict, DiffChurnStats, DiffEngine, DiffLine, DiffLineKind, DiffMinimizer, DiffRenderer,
    DiffStats, DiffViewerState, FileMutationKind, TrackedFileEntry, TurnDiffTracker, WordSpan,
};
use std::path::PathBuf;

#[test]
fn test_lib_exports_and_serialization() {
    let word = WordSpan {
        text: "hello".to_string(),
        is_highlighted: true,
    };
    let json_word = serde_json::to_string(&word).expect("serialize WordSpan");
    let deser_word: WordSpan = serde_json::from_str(&json_word).expect("deserialize WordSpan");
    assert_eq!(word, deser_word);

    let line = DiffLine {
        kind: DiffLineKind::Add,
        old_line_num: None,
        new_line_num: Some(42),
        content: "fn test() {}".to_string(),
        word_spans: vec![word],
    };
    let json_line = serde_json::to_string(&line).expect("serialize DiffLine");
    let deser_line: DiffLine = serde_json::from_str(&json_line).expect("deserialize DiffLine");
    assert_eq!(line, deser_line);

    let stats = DiffStats {
        files_changed: 2,
        lines_added: 10,
        lines_deleted: 3,
    };
    let json_stats = serde_json::to_string(&stats).expect("serialize DiffStats");
    let deser_stats: DiffStats = serde_json::from_str(&json_stats).expect("deserialize DiffStats");
    assert_eq!(stats, deser_stats);

    let verdict = ChurnVerdict::Acceptable;
    let json_verdict = serde_json::to_string(&verdict).expect("serialize ChurnVerdict");
    let deser_verdict: ChurnVerdict = serde_json::from_str(&json_verdict).expect("deserialize ChurnVerdict");
    assert_eq!(verdict, deser_verdict);

    let churn_stats = DiffChurnStats {
        total_lines_added: 5,
        total_lines_deleted: 2,
        whitespace_only_changes: 0,
        comment_only_changes: 0,
        semantic_lines_changed: 7,
        cosmetic_churn_pct: 0.0,
        churn_penalty_score: 0.0,
        verdict: ChurnVerdict::Minimal,
        recommendations: vec!["Looks good".to_string()],
    };
    let json_churn = serde_json::to_string(&churn_stats).expect("serialize DiffChurnStats");
    let deser_churn: DiffChurnStats = serde_json::from_str(&json_churn).expect("deserialize DiffChurnStats");
    assert_eq!(deser_churn.verdict, ChurnVerdict::Minimal);
    assert_eq!(deser_churn.total_lines_added, 5);

    let entry = TrackedFileEntry {
        path: PathBuf::from("src/test.rs"),
        original_path: None,
        kind: FileMutationKind::Modified,
        original_content: Some("old".to_string()),
        current_content: Some("new".to_string()),
        revision: 1,
        timestamp: std::time::SystemTime::now(),
    };
    let json_entry = serde_json::to_string(&entry).expect("serialize TrackedFileEntry");
    let deser_entry: TrackedFileEntry = serde_json::from_str(&json_entry).expect("deserialize TrackedFileEntry");
    assert_eq!(deser_entry.path, PathBuf::from("src/test.rs"));
    assert_eq!(deser_entry.kind, FileMutationKind::Modified);
}

#[test]
fn test_lib_end_to_end_pipeline() {
    let (engine_lines, engine_stats) = DiffEngine::compute_diff("old\n", "new\n");
    assert_eq!(engine_stats.lines_added, 1);
    assert_eq!(engine_lines.len(), 2);

    let mut tracker = TurnDiffTracker::new();
    tracker.record_modification(
        "src/calc.rs",
        "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
        "pub fn add(a: i32, b: i32) -> i32 {\n    // compute sum\n    a + b\n}\n",
    );

    let (patch_text, diff_stats) = tracker.compute_turn_diff(None);
    assert_eq!(diff_stats.files_changed, 1);
    assert_eq!(diff_stats.lines_added, 1);
    assert_eq!(diff_stats.lines_deleted, 0);

    let churn_stats = DiffMinimizer::evaluate_diff(&patch_text);
    assert_eq!(churn_stats.total_lines_added, 1);
    assert_eq!(churn_stats.verdict, ChurnVerdict::Minimal);

    let (diff_lines, _) = tracker
        .compute_file_diff(std::path::Path::new("src/calc.rs"))
        .expect("file diff should be present");

    let rendered = DiffRenderer::render_lines(&diff_lines, 50);
    assert!(!rendered.is_empty());

    let mut viewer = DiffViewerState::from_unified_patch("src/calc.rs", &patch_text);
    assert_eq!(viewer.file_path, "src/calc.rs");
    viewer.scroll_down(2);
    assert!(viewer.scroll_offset <= viewer.max_scroll);
}
