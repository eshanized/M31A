//! Filesystem event representations and change batching models.
//!
//! Categorizes file creations, modifications, deletions, and renames for watcher subscribers.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FileChangeKind {
    Created,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChangeEvent {
    pub path: PathBuf,
    pub kind: FileChangeKind,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceChangeBatch {
    pub changes: Vec<FileChangeEvent>,
    pub git_head_changed: bool,
    pub timestamp: DateTime<Utc>,
}

impl WorkspaceChangeBatch {
    pub fn new() -> Self {
        Self {
            changes: Vec::new(),
            git_head_changed: false,
            timestamp: Utc::now(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty() && !self.git_head_changed
    }

    pub fn len(&self) -> usize {
        self.changes.len()
    }

    pub fn affected_extensions(&self) -> HashSet<String> {
        self.changes
            .iter()
            .filter_map(|c| {
                c.path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|s| s.to_ascii_lowercase())
            })
            .collect()
    }

    pub fn has_source_changes(&self) -> bool {
        let code_exts: HashSet<&str> = [
            "rs", "ts", "js", "tsx", "jsx", "py", "go", "c", "cpp", "h", "hpp", "java", "toml",
            "json", "yaml", "yml", "md",
        ]
        .into_iter()
        .collect();

        self.changes.iter().any(|c| {
            c.path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| code_exts.contains(ext.to_ascii_lowercase().as_str()))
                .unwrap_or(false)
        })
    }
}
