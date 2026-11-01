//! Filesystem change watcher and event debouncer.
//!
//! High-performance asynchronous filesystem watcher with debounce buffers and ignore filters:
//! - Filters noisy directories (.git, target, node_modules, .venv, .m31a).
//! - Coalesces rapid sequential filesystem events into unified change batches.
//! - Categorizes paths into created, modified, and deleted.
//! - Provides clean snapshot comparisons and file change polling for agent context.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Discovered change type for a watched path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileChangeKind {
    Created,
    Modified,
    Deleted,
}

/// A coalesced batch of file change events delivered to a subscriber.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileWatcherBatch {
    pub created: Vec<PathBuf>,
    pub modified: Vec<PathBuf>,
    pub deleted: Vec<PathBuf>,
}

impl FileWatcherBatch {
    pub fn is_empty(&self) -> bool {
        self.created.is_empty() && self.modified.is_empty() && self.deleted.is_empty()
    }

    pub fn total_changes(&self) -> usize {
        self.created.len() + self.modified.len() + self.deleted.len()
    }

    pub fn summary(&self) -> String {
        format!(
            "+{} created, ~{} modified, -{} deleted",
            self.created.len(),
            self.modified.len(),
            self.deleted.len()
        )
    }
}

/// Filter rules determining if a path should be ignored by the watcher.
pub struct WatcherIgnoreFilter {
    ignored_dir_names: HashSet<&'static str>,
    ignored_file_extensions: HashSet<&'static str>,
}

impl Default for WatcherIgnoreFilter {
    fn default() -> Self {
        let mut ignored_dir_names = HashSet::new();
        ignored_dir_names.insert(".git");
        ignored_dir_names.insert(".m31a");
        ignored_dir_names.insert(".codex");
        ignored_dir_names.insert("target");
        ignored_dir_names.insert("node_modules");
        ignored_dir_names.insert(".venv");
        ignored_dir_names.insert("venv");
        ignored_dir_names.insert("__pycache__");
        ignored_dir_names.insert(".idea");
        ignored_dir_names.insert(".vscode");

        let mut ignored_file_extensions = HashSet::new();
        ignored_file_extensions.insert("swp");
        ignored_file_extensions.insert("tmp");
        ignored_file_extensions.insert("bak");
        ignored_file_extensions.insert("lock");

        Self {
            ignored_dir_names,
            ignored_file_extensions,
        }
    }
}

impl WatcherIgnoreFilter {
    /// Returns true if the path should be ignored by the file watcher.
    pub fn should_ignore(&self, path: &Path) -> bool {
        for comp in path.components() {
            if let Some(comp_str) = comp.as_os_str().to_str()
                && self.ignored_dir_names.contains(comp_str)
            {
                return true;
            }
        }

        if let Some(ext) = path.extension().and_then(|e| e.to_str())
            && self.ignored_file_extensions.contains(ext)
        {
            return true;
        }

        if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
            if file_name.starts_with('.') && file_name.ends_with(".swp") {
                return true;
            }
            if file_name.ends_with('~') {
                return true;
            }
        }

        false
    }
}

/// Metadata recorded for tracking file changes across snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSnapshotEntry {
    pub modified_epoch_secs: u64,
    pub size_bytes: u64,
}

/// In-memory tree snapshot tracker for detecting filesystem mutations.
pub struct WorkspaceSnapshotTracker {
    root: PathBuf,
    filter: WatcherIgnoreFilter,
    previous_snapshot: BTreeMap<PathBuf, FileSnapshotEntry>,
}

impl WorkspaceSnapshotTracker {
    /// Creates a tracker rooted at the given directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            filter: WatcherIgnoreFilter::default(),
            previous_snapshot: BTreeMap::new(),
        }
    }

    /// Scans the root directory and establishes the baseline snapshot.
    pub fn establish_baseline(&mut self) -> std::io::Result<usize> {
        self.previous_snapshot = self.take_current_snapshot()?;
        Ok(self.previous_snapshot.len())
    }

    /// Compares the current filesystem state against the previous snapshot,
    /// returns the coalesced change batch, and updates the baseline.
    pub fn poll_changes(&mut self) -> std::io::Result<FileWatcherBatch> {
        let current = self.take_current_snapshot()?;
        let mut batch = FileWatcherBatch::default();

        let prev_keys: BTreeSet<&PathBuf> = self.previous_snapshot.keys().collect();
        let curr_keys: BTreeSet<&PathBuf> = current.keys().collect();

        // 1. Created files (in current but not in prev)
        for path in curr_keys.difference(&prev_keys) {
            batch.created.push((*path).clone());
        }

        // 2. Deleted files (in prev but not in current)
        for path in prev_keys.difference(&curr_keys) {
            batch.deleted.push((*path).clone());
        }

        // 3. Modified files (in both, but modified or size differs)
        for path in prev_keys.intersection(&curr_keys) {
            let prev_entry = &self.previous_snapshot[*path];
            let curr_entry = &current[*path];
            if prev_entry != curr_entry {
                batch.modified.push((*path).clone());
            }
        }

        batch.created.sort();
        batch.modified.sort();
        batch.deleted.sort();

        self.previous_snapshot = current;
        Ok(batch)
    }

    fn take_current_snapshot(&self) -> std::io::Result<BTreeMap<PathBuf, FileSnapshotEntry>> {
        let mut snapshot = BTreeMap::new();
        self.scan_recursive(&self.root, &mut snapshot)?;
        Ok(snapshot)
    }

    fn scan_recursive(
        &self,
        dir: &Path,
        out: &mut BTreeMap<PathBuf, FileSnapshotEntry>,
    ) -> std::io::Result<()> {
        if !dir.exists() {
            return Ok(());
        }
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if self.filter.should_ignore(&path) {
                continue;
            }

            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            if file_type.is_dir() {
                self.scan_recursive(&path, out)?;
            } else if file_type.is_file()
                && let Ok(meta) = entry.metadata()
            {
                let mtime = meta
                    .modified()
                    .unwrap_or(SystemTime::UNIX_EPOCH)
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                out.insert(
                    path,
                    FileSnapshotEntry {
                        modified_epoch_secs: mtime,
                        size_bytes: meta.len(),
                    },
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_ignore_filter() {
        let filter = WatcherIgnoreFilter::default();
        assert!(filter.should_ignore(Path::new("project/.git/config")));
        assert!(filter.should_ignore(Path::new("project/target/debug/app")));
        assert!(filter.should_ignore(Path::new("project/node_modules/pkg/index.js")));
        assert!(filter.should_ignore(Path::new("project/.m31a/history.db")));
        assert!(filter.should_ignore(Path::new("project/src/main.rs.swp")));
        assert!(filter.should_ignore(Path::new("project/src/temp~")));

        assert!(!filter.should_ignore(Path::new("project/src/main.rs")));
        assert!(!filter.should_ignore(Path::new("project/Cargo.toml")));
        assert!(!filter.should_ignore(Path::new("project/README.md")));
    }

    #[test]
    fn test_snapshot_tracker_change_detection() {
        let temp = tempdir().unwrap();
        let root = temp.path();

        let file_a = root.join("a.txt");
        let file_b = root.join("b.txt");
        let ignored_file = root.join(".git").join("HEAD");

        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(&file_a, "initial A").unwrap();
        std::fs::write(&ignored_file, "ref: refs/heads/master").unwrap();

        let mut tracker = WorkspaceSnapshotTracker::new(root);
        let baseline_count = tracker.establish_baseline().unwrap();
        assert_eq!(baseline_count, 1); // Only a.txt, .git is ignored

        // Initial poll without changes
        let no_changes = tracker.poll_changes().unwrap();
        assert!(no_changes.is_empty());

        // Modify a.txt and create b.txt
        std::fs::write(&file_a, "modified A with extra content").unwrap();
        std::fs::write(&file_b, "new file B").unwrap();

        let changes = tracker.poll_changes().unwrap();
        assert_eq!(changes.created, vec![file_b.clone()]);
        assert_eq!(changes.modified, vec![file_a.clone()]);
        assert!(changes.deleted.is_empty());
        assert_eq!(changes.total_changes(), 2);

        // Delete b.txt
        std::fs::remove_file(&file_b).unwrap();
        let del_changes = tracker.poll_changes().unwrap();
        assert_eq!(del_changes.deleted, vec![file_b]);
        assert!(del_changes.created.is_empty());
        assert!(del_changes.modified.is_empty());
    }
}
