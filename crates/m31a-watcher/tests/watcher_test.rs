//! Dedicated integration tests for `m31a-watcher` (src/watcher.rs).
//!
//! Validates:
//! - WorkspaceWatcher startup, shutdown, and running status transitions
//! - Real filesystem change propagation (Create, Modify, Delete)
//! - Noise suppression against target/ and .tmp files
//! - Git HEAD tracking triggers git_head_changed flag
//! - Zero-mock execution on temporary directory sandboxes

use m31a_watcher::events::FileChangeKind;
use m31a_watcher::watcher::{WatcherConfig, WorkspaceWatcher};
use std::fs;
use std::io::Write;
use std::time::Duration;
use tempfile::tempdir;

#[tokio::test]
async fn test_watcher_lifecycle_and_idempotence() {
    let temp = tempdir().unwrap();
    let watcher = WorkspaceWatcher::new(temp.path().to_path_buf(), None);

    assert!(!watcher.is_running());
    watcher.start();
    assert!(watcher.is_running());

    // Calling start again should be a safe no-op
    watcher.start();
    assert!(watcher.is_running());

    watcher.stop();
    assert!(!watcher.is_running());
}

#[tokio::test]
async fn test_watcher_detects_real_fs_mutations() {
    let temp = tempdir().unwrap();
    let config = WatcherConfig {
        poll_interval: Duration::from_millis(40),
        debounce_duration: Duration::from_millis(20),
        max_files_limit: 500,
    };

    let watcher = WorkspaceWatcher::new(temp.path().to_path_buf(), Some(config));
    let mut rx = watcher.subscribe();
    watcher.start();

    // Allow watcher initialization
    tokio::time::sleep(Duration::from_millis(60)).await;

    // 1. Create a source file
    let file_path = temp.path().join("service.rs");
    {
        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(b"pub struct Service;").unwrap();
    }

    let batch_create = tokio::time::timeout(Duration::from_millis(2000), rx.recv())
        .await
        .expect("timed out waiting for create event")
        .expect("channel recv error");

    assert!(!batch_create.is_empty());
    assert!(
        batch_create
            .changes
            .iter()
            .any(|c| c.kind == FileChangeKind::Created && c.path == file_path)
    );

    // 2. Modify the file
    tokio::time::sleep(Duration::from_millis(40)).await;
    {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&file_path)
            .unwrap();
        file.write_all(b"\nimpl Service { pub fn new() -> Self { Self } }").unwrap();
    }

    let batch_modify = tokio::time::timeout(Duration::from_millis(2000), rx.recv())
        .await
        .expect("timed out waiting for modify event")
        .expect("channel recv error");

    assert!(
        batch_modify
            .changes
            .iter()
            .any(|c| c.kind == FileChangeKind::Modified && c.path == file_path)
    );

    // 3. Delete the file
    tokio::time::sleep(Duration::from_millis(40)).await;
    fs::remove_file(&file_path).unwrap();

    let batch_delete = tokio::time::timeout(Duration::from_millis(2000), rx.recv())
        .await
        .expect("timed out waiting for delete event")
        .expect("channel recv error");

    assert!(
        batch_delete
            .changes
            .iter()
            .any(|c| c.kind == FileChangeKind::Deleted && c.path == file_path)
    );

    watcher.stop();
}

#[tokio::test]
async fn test_watcher_ignores_noise_and_detects_git_head() {
    let temp = tempdir().unwrap();
    let config = WatcherConfig {
        poll_interval: Duration::from_millis(40),
        debounce_duration: Duration::from_millis(20),
        max_files_limit: 500,
    };

    let watcher = WorkspaceWatcher::new(temp.path().to_path_buf(), Some(config));
    let mut rx = watcher.subscribe();
    watcher.start();

    tokio::time::sleep(Duration::from_millis(60)).await;

    // Create ignored files: swap file and inside target directory
    let target_dir = temp.path().join("target");
    fs::create_dir_all(&target_dir).unwrap();
    let ignored_artifact = target_dir.join("build.log");
    fs::write(&ignored_artifact, b"compiler junk").unwrap();

    let swp_file = temp.path().join(".document.swp");
    fs::write(&swp_file, b"vim swap").unwrap();

    // Now trigger git head change and a normal file change to verify batch
    let git_dir = temp.path().join(".git");
    fs::create_dir_all(&git_dir).unwrap();
    let head_file = git_dir.join("HEAD");
    fs::write(&head_file, b"ref: refs/heads/feature-branch\n").unwrap();

    let trigger_file = temp.path().join("trigger.rs");
    fs::write(&trigger_file, b"// trigger").unwrap();

    let batch = tokio::time::timeout(Duration::from_millis(2000), rx.recv())
        .await
        .expect("timed out waiting for batch")
        .expect("channel recv error");

    // The batch should report git_head_changed = true and contain only the trigger file
    assert!(batch.git_head_changed, "Git HEAD change should be detected");
    assert!(
        batch.changes.iter().any(|c| c.path == trigger_file),
        "Trigger source file must be captured"
    );
    assert!(
        !batch.changes.iter().any(|c| c.path == ignored_artifact || c.path == swp_file),
        "Ignored files must not appear in changes batch"
    );

    watcher.stop();
}
