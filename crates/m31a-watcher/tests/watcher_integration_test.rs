//! Integration tests for filesystem watcher and change debounce filters.
//!
//! Tests recursive filesystem monitoring, event batching, and directory exclusion filters.

use m31a_watcher::{
    FileChangeEvent, FileChangeKind, WatcherConfig, WatcherFilter, WorkspaceChangeBatch,
    WorkspaceWatcher,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn test_watcher_filter_custom_extension_and_env() {
    let filter = WatcherFilter::new().with_additional_ignore("custom_build");

    assert!(filter.should_ignore_dir("custom_build"));
    assert!(filter.should_ignore_dir(".git"));
    assert!(filter.should_ignore_dir("target"));

    // .env should NOT be ignored according to filter specification
    assert!(!filter.should_ignore_file(Path::new(".env")));
    // other dotfiles should be ignored
    assert!(filter.should_ignore_file(Path::new(".DS_Store")));
    assert!(filter.should_ignore_file(Path::new("test.tmp")));
    assert!(filter.should_ignore_file(Path::new("file.swp")));
    assert!(filter.should_ignore_file(Path::new("backup.rs~")));
}

#[test]
fn test_workspace_change_batch_helpers() {
    let mut batch = WorkspaceChangeBatch::new();
    assert!(batch.is_empty());
    assert_eq!(batch.len(), 0);

    batch.changes.push(FileChangeEvent {
        path: PathBuf::from("src/lib.rs"),
        kind: FileChangeKind::Created,
        timestamp: chrono::Utc::now(),
    });
    batch.changes.push(FileChangeEvent {
        path: PathBuf::from("src/old.rs"),
        kind: FileChangeKind::Deleted,
        timestamp: chrono::Utc::now(),
    });

    assert!(!batch.is_empty());
    assert_eq!(batch.len(), 2);
    assert!(batch.has_source_changes());
    let exts = batch.affected_extensions();
    assert!(exts.contains("rs"));
}

#[tokio::test]
async fn test_watcher_file_deletion_and_lifecycle() {
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("transient.txt");
    fs::write(&file_path, "hello").unwrap();

    let config = WatcherConfig {
        poll_interval: Duration::from_millis(50),
        debounce_duration: Duration::from_millis(10),
        max_files_limit: 100,
    };

    let watcher = WorkspaceWatcher::new(temp.path().to_path_buf(), Some(config));
    let mut rx = watcher.subscribe();
    assert!(!watcher.is_running());

    watcher.start();
    assert!(watcher.is_running());

    // Allow initial snapshot
    tokio::time::sleep(Duration::from_millis(80)).await;

    // Delete the file
    fs::remove_file(&file_path).unwrap();

    let batch = tokio::time::timeout(Duration::from_millis(1500), rx.recv())
        .await
        .expect("timeout waiting for delete")
        .expect("recv error");

    assert!(
        batch
            .changes
            .iter()
            .any(|c| c.kind == FileChangeKind::Deleted)
    );

    watcher.stop();
    assert!(!watcher.is_running());
}
