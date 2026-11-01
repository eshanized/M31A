//! Dedicated integration tests for `m31a-watcher` (src/lib.rs).
//!
//! Validates:
//! - Root re-exports: `FileChangeEvent`, `FileChangeKind`, `WorkspaceChangeBatch`, `WatcherFilter`, `WatcherConfig`, `WorkspaceWatcher`
//! - Cohesive system lifecycle from root exports

use chrono::Utc;
use m31a_watcher::{
    FileChangeEvent, FileChangeKind, WatcherConfig, WatcherFilter, WorkspaceChangeBatch,
    WorkspaceWatcher,
};
use std::fs;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn test_root_reexports() {
    let filter = WatcherFilter::default();
    assert!(filter.should_ignore_dir("target"));

    let config = WatcherConfig::default();
    assert_eq!(config.debounce_duration, Duration::from_millis(150));

    let batch = WorkspaceChangeBatch::new();
    assert!(batch.is_empty());

    let event = FileChangeEvent {
        path: "src/main.rs".into(),
        kind: FileChangeKind::Created,
        timestamp: Utc::now(),
    };
    assert_eq!(event.kind, FileChangeKind::Created);
}

#[tokio::test]
async fn test_root_level_watcher_pipeline() {
    let temp = tempdir().unwrap();
    let config = WatcherConfig {
        poll_interval: Duration::from_millis(30),
        debounce_duration: Duration::from_millis(15),
        max_files_limit: 100,
    };

    let watcher = WorkspaceWatcher::new(temp.path().to_path_buf(), Some(config));
    let mut receiver = watcher.subscribe();
    watcher.start();

    tokio::time::sleep(Duration::from_millis(50)).await;

    let target_file = temp.path().join("pipeline.rs");
    fs::write(&target_file, b"pub fn run() {}").unwrap();

    let batch = tokio::time::timeout(Duration::from_millis(2000), receiver.recv())
        .await
        .expect("timed out waiting for broadcast")
        .expect("broadcast receive error");

    assert!(!batch.is_empty());
    assert!(batch.has_source_changes());
    assert_eq!(batch.affected_extensions().into_iter().collect::<Vec<_>>(), vec!["rs"]);

    watcher.stop();
}
