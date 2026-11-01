//! In-Memory Turn Diff Tracker
//!
//! Inspired by OpenAI Codex's `TurnDiffTracker`. Captures file mutation deltas
//! directly from patch application and workspace operations without querying
//! the filesystem or spawning high-overhead `git diff` subprocesses.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};

use crate::engine::{DiffEngine, DiffLine, DiffStats};

/// Operation kind applied to a file during the turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileMutationKind {
    Added,
    Modified,
    Deleted,
    Renamed,
}

/// Recorded mutation entry for a single tracked file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedFileEntry {
    pub path: PathBuf,
    pub original_path: Option<PathBuf>,
    pub kind: FileMutationKind,
    pub original_content: Option<String>,
    pub current_content: Option<String>,
    pub revision: u64,
    pub timestamp: std::time::SystemTime,
}

/// Tracks turn-level file diffs and provides high-speed in-memory unified diff generation.
#[derive(Debug, Clone, Default)]
pub struct TurnDiffTracker {
    entries: BTreeMap<PathBuf, TrackedFileEntry>,
    global_revision: u64,
}

impl TurnDiffTracker {
    /// Creates a new, empty `TurnDiffTracker`.
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            global_revision: 0,
        }
    }

    /// Records a file creation or addition.
    pub fn record_addition(&mut self, path: impl Into<PathBuf>, content: impl Into<String>) {
        let path = path.into();
        let content = content.into();
        self.global_revision += 1;

        let entry = self
            .entries
            .entry(path.clone())
            .or_insert_with(|| TrackedFileEntry {
                path: path.clone(),
                original_path: None,
                kind: FileMutationKind::Added,
                original_content: None,
                current_content: Some(content.clone()),
                revision: self.global_revision,
                timestamp: std::time::SystemTime::now(),
            });

        entry.current_content = Some(content);
        entry.revision = self.global_revision;
    }

    /// Records a file modification. Preserves the original baseline if already tracked.
    pub fn record_modification(
        &mut self,
        path: impl Into<PathBuf>,
        old_content: impl Into<String>,
        new_content: impl Into<String>,
    ) {
        let path = path.into();
        let old_content = old_content.into();
        let new_content = new_content.into();
        self.global_revision += 1;

        if let Some(entry) = self.entries.get_mut(&path) {
            entry.current_content = Some(new_content);
            entry.revision = self.global_revision;
            if entry.kind != FileMutationKind::Added {
                entry.kind = FileMutationKind::Modified;
            }
        } else {
            self.entries.insert(
                path.clone(),
                TrackedFileEntry {
                    path,
                    original_path: None,
                    kind: FileMutationKind::Modified,
                    original_content: Some(old_content),
                    current_content: Some(new_content),
                    revision: self.global_revision,
                    timestamp: std::time::SystemTime::now(),
                },
            );
        }
    }

    /// Records a file deletion.
    pub fn record_deletion(&mut self, path: impl Into<PathBuf>, old_content: Option<String>) {
        let path = path.into();
        self.global_revision += 1;

        if let Some(entry) = self.entries.get_mut(&path) {
            if entry.kind == FileMutationKind::Added {
                // If it was added in this turn and then deleted, simply remove from tracker
                self.entries.remove(&path);
                return;
            }
            entry.current_content = None;
            entry.kind = FileMutationKind::Deleted;
            entry.revision = self.global_revision;
        } else {
            self.entries.insert(
                path.clone(),
                TrackedFileEntry {
                    path,
                    original_path: None,
                    kind: FileMutationKind::Deleted,
                    original_content: old_content,
                    current_content: None,
                    revision: self.global_revision,
                    timestamp: std::time::SystemTime::now(),
                },
            );
        }
    }

    /// Returns all paths modified during the current turn.
    pub fn changed_paths(&self) -> Vec<PathBuf> {
        self.entries.keys().cloned().collect()
    }

    /// Returns true if no mutations have been tracked this turn.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the number of distinct files tracked this turn.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Clears all tracked mutations (called upon turn completion or checkpoint reset).
    pub fn clear(&mut self) {
        self.entries.clear();
        self.global_revision = 0;
    }

    /// Computes full unified diff for the turn with timeout bound.
    pub fn compute_turn_diff(&self, timeout: Option<Duration>) -> (String, DiffStats) {
        let start = Instant::now();
        let deadline = timeout.unwrap_or(Duration::from_millis(150));
        let mut diff_str = String::new();
        let mut cumulative_stats = DiffStats::default();

        for (path, entry) in &self.entries {
            if start.elapsed() > deadline {
                diff_str.push_str(&format!(
                    "\n# Diff computation timed out after {:?}; remaining files truncated.\n",
                    deadline
                ));
                break;
            }

            let path_str = path.to_string_lossy();
            let old_text = entry.original_content.as_deref().unwrap_or("");
            let new_text = entry.current_content.as_deref().unwrap_or("");

            match entry.kind {
                FileMutationKind::Added => {
                    diff_str.push_str(&format!("--- /dev/null\n+++ b/{}\n", path_str));
                }
                FileMutationKind::Deleted => {
                    diff_str.push_str(&format!("--- a/{}\n+++ /dev/null\n", path_str));
                }
                FileMutationKind::Modified | FileMutationKind::Renamed => {
                    diff_str.push_str(&format!("--- a/{}\n+++ b/{}\n", path_str, path_str));
                }
            }

            let text_diff = TextDiff::from_lines(old_text, new_text);
            let mut file_added = 0;
            let mut file_deleted = 0;

            for hunk in text_diff.unified_diff().context_radius(3).iter_hunks() {
                diff_str.push_str(&hunk.header().to_string());
                for change in hunk.iter_changes() {
                    match change.tag() {
                        ChangeTag::Equal => {
                            diff_str.push(' ');
                            diff_str.push_str(change.value());
                        }
                        ChangeTag::Delete => {
                            diff_str.push('-');
                            diff_str.push_str(change.value());
                            file_deleted += 1;
                        }
                        ChangeTag::Insert => {
                            diff_str.push('+');
                            diff_str.push_str(change.value());
                            file_added += 1;
                        }
                    }
                }
            }

            if file_added > 0 || file_deleted > 0 {
                cumulative_stats.files_changed += 1;
                cumulative_stats.lines_added += file_added;
                cumulative_stats.lines_deleted += file_deleted;
            }
        }

        (diff_str, cumulative_stats)
    }

    /// Computes rich line-by-line DiffLines for a specific file.
    pub fn compute_file_diff(&self, path: &Path) -> Option<(Vec<DiffLine>, DiffStats)> {
        let entry = self.entries.get(path)?;
        let old_text = entry.original_content.as_deref().unwrap_or("");
        let new_text = entry.current_content.as_deref().unwrap_or("");
        Some(DiffEngine::compute_diff(old_text, new_text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_addition() {
        let mut tracker = TurnDiffTracker::new();
        tracker.record_addition("src/main.rs", "fn main() {}\n");

        assert_eq!(tracker.len(), 1);
        assert_eq!(tracker.changed_paths(), vec![PathBuf::from("src/main.rs")]);

        let (diff, stats) = tracker.compute_turn_diff(None);
        assert!(diff.contains("--- /dev/null"));
        assert!(diff.contains("+++ b/src/main.rs"));
        assert!(diff.contains("+fn main() {}"));
        assert_eq!(stats.files_changed, 1);
        assert_eq!(stats.lines_added, 1);
        assert_eq!(stats.lines_deleted, 0);
    }

    #[test]
    fn test_record_modification() {
        let mut tracker = TurnDiffTracker::new();
        tracker.record_modification("src/lib.rs", "let x = 1;\n", "let x = 2;\n");

        assert_eq!(tracker.len(), 1);
        let (diff, stats) = tracker.compute_turn_diff(None);
        assert!(diff.contains("--- a/src/lib.rs"));
        assert!(diff.contains("+++ b/src/lib.rs"));
        assert!(diff.contains("-let x = 1;"));
        assert!(diff.contains("+let x = 2;"));
        assert_eq!(stats.files_changed, 1);
        assert_eq!(stats.lines_added, 1);
        assert_eq!(stats.lines_deleted, 1);
    }

    #[test]
    fn test_record_deletion() {
        let mut tracker = TurnDiffTracker::new();
        tracker.record_deletion("temp.txt", Some("temporary content\n".to_string()));

        let (diff, stats) = tracker.compute_turn_diff(None);
        assert!(diff.contains("--- a/temp.txt"));
        assert!(diff.contains("+++ /dev/null"));
        assert!(diff.contains("-temporary content"));
        assert_eq!(stats.files_changed, 1);
        assert_eq!(stats.lines_deleted, 1);
    }

    #[test]
    fn test_sequential_edits_preserve_original_baseline() {
        let mut tracker = TurnDiffTracker::new();
        tracker.record_modification("app.rs", "base\n", "mid\n");
        tracker.record_modification("app.rs", "mid\n", "final\n");

        let (diff, stats) = tracker.compute_turn_diff(None);
        assert!(diff.contains("-base"));
        assert!(diff.contains("+final"));
        assert!(!diff.contains("mid"));
        assert_eq!(stats.lines_added, 1);
        assert_eq!(stats.lines_deleted, 1);
    }
}
