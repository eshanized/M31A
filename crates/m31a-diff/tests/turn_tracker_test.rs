//! Dedicated test for `crates/m31a-diff/src/turn_tracker.rs`.
//!
//! Validates `TurnDiffTracker` state transitions, baseline preservation,
//! turn diff generation, file-level querying, and timeout bounds.

use std::path::{Path, PathBuf};
use std::time::Duration;

use m31a_diff::turn_tracker::TurnDiffTracker;

#[test]
fn test_turn_tracker_initial_and_clear() {
    let mut tracker = TurnDiffTracker::new();
    assert!(tracker.is_empty());
    assert_eq!(tracker.len(), 0);
    assert!(tracker.changed_paths().is_empty());

    tracker.record_addition("a.rs", "let a = 1;\n");
    assert!(!tracker.is_empty());
    assert_eq!(tracker.len(), 1);

    tracker.clear();
    assert!(tracker.is_empty());
    assert_eq!(tracker.len(), 0);
}

#[test]
fn test_turn_tracker_add_then_delete_removes_entry() {
    let mut tracker = TurnDiffTracker::new();
    tracker.record_addition("temp.rs", "ephemeral code\n");
    assert_eq!(tracker.len(), 1);

    // Deleting an ephemeral file added during the current turn cleans it up entirely
    tracker.record_deletion("temp.rs", None);
    assert_eq!(tracker.len(), 0);
    assert!(tracker.is_empty());
    assert!(tracker.compute_file_diff(Path::new("temp.rs")).is_none());
}

#[test]
fn test_turn_tracker_baseline_preservation_across_multiple_mutations() {
    let mut tracker = TurnDiffTracker::new();
    tracker.record_modification("config.toml", "port = 8080\n", "port = 8081\n");
    tracker.record_modification("config.toml", "port = 8081\n", "port = 9000\n");

    let (patch, stats) = tracker.compute_turn_diff(None);
    assert_eq!(stats.files_changed, 1);
    assert_eq!(stats.lines_deleted, 1);
    assert_eq!(stats.lines_added, 1);

    assert!(patch.contains("-port = 8080"));
    assert!(patch.contains("+port = 9000"));
    assert!(!patch.contains("8081"));

    let file_diff = tracker.compute_file_diff(Path::new("config.toml"));
    assert!(file_diff.is_some());
    let (lines, f_stats) = file_diff.unwrap();
    assert_eq!(f_stats.lines_deleted, 1);
    assert_eq!(f_stats.lines_added, 1);
    assert!(lines.iter().any(|l| l.content == "port = 8080"));
    assert!(lines.iter().any(|l| l.content == "port = 9000"));
}

#[test]
fn test_turn_tracker_combined_operations() {
    let mut tracker = TurnDiffTracker::new();
    tracker.record_addition("added.txt", "hello world\n");
    tracker.record_modification("mod.txt", "old line\n", "new line\n");
    tracker.record_deletion("del.txt", Some("goodbye\n".to_string()));

    let paths = tracker.changed_paths();
    assert_eq!(paths.len(), 3);
    assert_eq!(
        paths,
        vec![
            PathBuf::from("added.txt"),
            PathBuf::from("del.txt"),
            PathBuf::from("mod.txt"),
        ]
    );

    let (patch, stats) = tracker.compute_turn_diff(None);
    assert_eq!(stats.files_changed, 3);
    assert_eq!(stats.lines_added, 2);
    assert_eq!(stats.lines_deleted, 2);

    assert!(patch.contains("--- /dev/null\n+++ b/added.txt"));
    assert!(patch.contains("--- a/del.txt\n+++ /dev/null"));
    assert!(patch.contains("--- a/mod.txt\n+++ b/mod.txt"));
}

#[test]
fn test_turn_tracker_timeout_truncation() {
    let mut tracker = TurnDiffTracker::new();
    for i in 0..100 {
        tracker.record_addition(format!("file_{}.txt", i), format!("contents of file {}\n", i));
    }

    // Specifying an impossibly small timeout (e.g. 0 nanoseconds) triggers timeout break
    let (patch, _) = tracker.compute_turn_diff(Some(Duration::ZERO));
    assert!(patch.contains("Diff computation timed out after"));
}
