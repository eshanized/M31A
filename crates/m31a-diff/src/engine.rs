//! Unified and intra-line word-level diff engine.

use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffLineKind {
    Context,
    Add,
    Delete,
    HunkHeader,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WordSpan {
    pub text: String,
    pub is_highlighted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_line_num: Option<usize>,
    pub new_line_num: Option<usize>,
    pub content: String,
    pub word_spans: Vec<WordSpan>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffStats {
    pub files_changed: usize,
    pub lines_added: usize,
    pub lines_deleted: usize,
}

pub struct DiffEngine;

impl DiffEngine {
    /// Computes a unified diff between old_text and new_text.
    pub fn compute_diff(old_text: &str, new_text: &str) -> (Vec<DiffLine>, DiffStats) {
        let diff = TextDiff::from_lines(old_text, new_text);
        let mut lines = Vec::new();
        let mut stats = DiffStats::default();

        let mut old_num = 1;
        let mut new_num = 1;

        for change in diff.iter_all_changes() {
            let tag = change.tag();
            let value = change.value().trim_end_matches(['\r', '\n']).to_string();

            match tag {
                ChangeTag::Equal => {
                    lines.push(DiffLine {
                        kind: DiffLineKind::Context,
                        old_line_num: Some(old_num),
                        new_line_num: Some(new_num),
                        content: value.clone(),
                        word_spans: vec![WordSpan {
                            text: value,
                            is_highlighted: false,
                        }],
                    });
                    old_num += 1;
                    new_num += 1;
                }
                ChangeTag::Delete => {
                    stats.lines_deleted += 1;
                    lines.push(DiffLine {
                        kind: DiffLineKind::Delete,
                        old_line_num: Some(old_num),
                        new_line_num: None,
                        content: value.clone(),
                        word_spans: vec![WordSpan {
                            text: value,
                            is_highlighted: true,
                        }],
                    });
                    old_num += 1;
                }
                ChangeTag::Insert => {
                    stats.lines_added += 1;
                    lines.push(DiffLine {
                        kind: DiffLineKind::Add,
                        old_line_num: None,
                        new_line_num: Some(new_num),
                        content: value.clone(),
                        word_spans: vec![WordSpan {
                            text: value,
                            is_highlighted: true,
                        }],
                    });
                    new_num += 1;
                }
            }
        }

        if stats.lines_added > 0 || stats.lines_deleted > 0 {
            stats.files_changed = 1;
        }

        (lines, stats)
    }

    /// Parses a standard unified diff patch (such as from `git diff`).
    pub fn parse_unified_diff(patch: &str) -> (Vec<DiffLine>, DiffStats) {
        let mut lines = Vec::new();
        let mut stats = DiffStats::default();

        let mut old_line = 1;
        let mut new_line = 1;

        for line in patch.lines() {
            if line.starts_with("diff --git") {
                stats.files_changed += 1;
                lines.push(DiffLine {
                    kind: DiffLineKind::HunkHeader,
                    old_line_num: None,
                    new_line_num: None,
                    content: line.to_string(),
                    word_spans: vec![WordSpan {
                        text: line.to_string(),
                        is_highlighted: false,
                    }],
                });
            } else if line.starts_with("@@") {
                lines.push(DiffLine {
                    kind: DiffLineKind::HunkHeader,
                    old_line_num: None,
                    new_line_num: None,
                    content: line.to_string(),
                    word_spans: vec![WordSpan {
                        text: line.to_string(),
                        is_highlighted: false,
                    }],
                });
            } else if line.starts_with("+++")
                || line.starts_with("---")
                || line.starts_with("index ")
            {
                lines.push(DiffLine {
                    kind: DiffLineKind::Context,
                    old_line_num: None,
                    new_line_num: None,
                    content: line.to_string(),
                    word_spans: vec![WordSpan {
                        text: line.to_string(),
                        is_highlighted: false,
                    }],
                });
            } else if let Some(stripped) = line.strip_prefix('+') {
                stats.lines_added += 1;
                lines.push(DiffLine {
                    kind: DiffLineKind::Add,
                    old_line_num: None,
                    new_line_num: Some(new_line),
                    content: stripped.to_string(),
                    word_spans: vec![WordSpan {
                        text: stripped.to_string(),
                        is_highlighted: false,
                    }],
                });
                new_line += 1;
            } else if let Some(stripped) = line.strip_prefix('-') {
                stats.lines_deleted += 1;
                lines.push(DiffLine {
                    kind: DiffLineKind::Delete,
                    old_line_num: Some(old_line),
                    new_line_num: None,
                    content: stripped.to_string(),
                    word_spans: vec![WordSpan {
                        text: stripped.to_string(),
                        is_highlighted: false,
                    }],
                });
                old_line += 1;
            } else {
                let text = line.strip_prefix(' ').unwrap_or(line);
                lines.push(DiffLine {
                    kind: DiffLineKind::Context,
                    old_line_num: Some(old_line),
                    new_line_num: Some(new_line),
                    content: text.to_string(),
                    word_spans: vec![WordSpan {
                        text: text.to_string(),
                        is_highlighted: false,
                    }],
                });
                old_line += 1;
                new_line += 1;
            }
        }

        if stats.files_changed == 0 && (stats.lines_added > 0 || stats.lines_deleted > 0) {
            stats.files_changed = 1;
        }

        (lines, stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_diff_stats() {
        let old = "fn main() {\n    println!(\"old\");\n}\n";
        let new = "fn main() {\n    println!(\"new\");\n}\n";
        let (lines, stats) = DiffEngine::compute_diff(old, new);

        assert_eq!(stats.lines_added, 1);
        assert_eq!(stats.lines_deleted, 1);
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn test_parse_unified_diff() {
        let patch = "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,3 @@\n-old line\n+new line\n context";
        let (lines, stats) = DiffEngine::parse_unified_diff(patch);
        assert_eq!(stats.lines_added, 1);
        assert_eq!(stats.lines_deleted, 1);
        assert_eq!(stats.files_changed, 1);
        assert_eq!(lines.len(), 7);
    }
}
