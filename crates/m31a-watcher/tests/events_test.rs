//! Dedicated integration tests for `m31a-watcher` (src/events.rs).
//!
//! Validates:
//! - `FileChangeKind` variant behavior and hash consistency
//! - `FileChangeEvent` instantiation and timestamp integrity
//! - `WorkspaceChangeBatch` empty detection, length, and git HEAD status
//! - `affected_extensions` extraction, lowercase normalization, and deduplication
//! - `has_source_changes` identification across standard software engineering file types

use chrono::Utc;
use m31a_watcher::events::{FileChangeEvent, FileChangeKind, WorkspaceChangeBatch};
use std::collections::HashSet;
use std::path::PathBuf;

#[test]
fn test_file_change_kind_equality_and_hashing() {
    let mut set = HashSet::new();
    set.insert(FileChangeKind::Created);
    set.insert(FileChangeKind::Modified);
    set.insert(FileChangeKind::Deleted);
    set.insert(FileChangeKind::Created); // duplicate

    assert_eq!(set.len(), 3);
    assert_ne!(FileChangeKind::Created, FileChangeKind::Modified);
    assert_ne!(FileChangeKind::Modified, FileChangeKind::Deleted);
}

#[test]
fn test_change_batch_empty_and_len_semantics() {
    let mut batch = WorkspaceChangeBatch::new();
    assert!(batch.is_empty());
    assert_eq!(batch.len(), 0);

    // git_head_changed makes batch non-empty even with 0 file changes
    batch.git_head_changed = true;
    assert!(!batch.is_empty());
    assert_eq!(batch.len(), 0);

    batch.git_head_changed = false;
    batch.changes.push(FileChangeEvent {
        path: PathBuf::from("src/lib.rs"),
        kind: FileChangeKind::Created,
        timestamp: Utc::now(),
    });
    assert!(!batch.is_empty());
    assert_eq!(batch.len(), 1);
}

#[test]
fn test_affected_extensions_normalization() {
    let mut batch = WorkspaceChangeBatch::new();
    batch.changes.push(FileChangeEvent {
        path: PathBuf::from("src/main.RS"),
        kind: FileChangeKind::Created,
        timestamp: Utc::now(),
    });
    batch.changes.push(FileChangeEvent {
        path: PathBuf::from("web/App.TSX"),
        kind: FileChangeKind::Modified,
        timestamp: Utc::now(),
    });
    batch.changes.push(FileChangeEvent {
        path: PathBuf::from("scripts/build.rs"),
        kind: FileChangeKind::Modified,
        timestamp: Utc::now(),
    });
    batch.changes.push(FileChangeEvent {
        path: PathBuf::from("LICENSE"),
        kind: FileChangeKind::Modified,
        timestamp: Utc::now(),
    });

    let exts = batch.affected_extensions();
    assert_eq!(exts.len(), 2);
    assert!(exts.contains("rs"));
    assert!(exts.contains("tsx"));
}

#[test]
fn test_has_source_changes_detection() {
    let supported_source_files = [
        "code.rs",
        "app.ts",
        "index.js",
        "ui.tsx",
        "component.jsx",
        "script.py",
        "server.go",
        "lib.c",
        "engine.cpp",
        "header.h",
        "class.hpp",
        "Main.java",
        "Cargo.toml",
        "package.json",
        "config.yaml",
        "ci.yml",
        "README.md",
    ];

    for file in supported_source_files {
        let mut batch = WorkspaceChangeBatch::new();
        batch.changes.push(FileChangeEvent {
            path: PathBuf::from(file),
            kind: FileChangeKind::Modified,
            timestamp: Utc::now(),
        });
        assert!(
            batch.has_source_changes(),
            "Expected source change for {}",
            file
        );
    }

    // Non-source files
    let non_source_files = [
        "image.png",
        "binary.wasm",
        "archive.tar.gz",
        "video.mp4",
        "blob.bin",
    ];
    for file in non_source_files {
        let mut batch = WorkspaceChangeBatch::new();
        batch.changes.push(FileChangeEvent {
            path: PathBuf::from(file),
            kind: FileChangeKind::Modified,
            timestamp: Utc::now(),
        });
        assert!(
            !batch.has_source_changes(),
            "Expected NO source change for {}",
            file
        );
    }
}
