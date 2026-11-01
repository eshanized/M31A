//! Filesystem event listening and path notification channels.
//!
//! Provides channel abstractions for subscribing to workspace file modification events.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{Mutex, broadcast};
use tracing::{debug, info};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileWatcherEvent {
    pub paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WatchPath {
    pub path: PathBuf,
    pub recursive: bool,
}

#[derive(Debug, Clone)]
struct FileMeta {
    modified: SystemTime,
    len: u64,
}

pub struct WorkspaceWatcher {
    watched_paths: Arc<Mutex<Vec<WatchPath>>>,
    known_files: Arc<Mutex<HashMap<PathBuf, FileMeta>>>,
    event_tx: broadcast::Sender<FileWatcherEvent>,
    shutdown_tx: broadcast::Sender<()>,
}

impl WorkspaceWatcher {
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(128);
        let (shutdown_tx, _) = broadcast::channel(1);
        Self {
            watched_paths: Arc::new(Mutex::new(Vec::new())),
            known_files: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
            shutdown_tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<FileWatcherEvent> {
        self.event_tx.subscribe()
    }

    pub async fn add_watch(&self, path: PathBuf, recursive: bool) {
        let mut watched = self.watched_paths.lock().await;
        if !watched.iter().any(|w| w.path == path) {
            watched.push(WatchPath { path, recursive });
        }
    }

    pub async fn remove_watch(&self, path: &Path) {
        let mut watched = self.watched_paths.lock().await;
        watched.retain(|w| w.path != path);
    }

    /// Scans watched roots and returns any changed, added, or deleted files since the last scan.
    pub async fn scan_now(&self) -> Option<FileWatcherEvent> {
        let watched = self.watched_paths.lock().await.clone();
        let mut current_snapshot = HashMap::new();

        for wp in &watched {
            if wp.path.is_file() {
                if let Ok(meta) = fs::metadata(&wp.path)
                    && let Ok(mtime) = meta.modified()
                {
                    current_snapshot.insert(
                        wp.path.clone(),
                        FileMeta {
                            modified: mtime,
                            len: meta.len(),
                        },
                    );
                }
            } else if wp.path.is_dir() {
                Self::scan_directory(&wp.path, wp.recursive, &mut current_snapshot);
            }
        }

        let mut known = self.known_files.lock().await;
        let mut changed_paths = BTreeSet::new();

        // 1. Detect additions and modifications
        for (path, meta) in &current_snapshot {
            match known.get(path) {
                Some(prev_meta) => {
                    if prev_meta.modified != meta.modified || prev_meta.len != meta.len {
                        changed_paths.insert(path.clone());
                    }
                }
                None => {
                    // New file added
                    changed_paths.insert(path.clone());
                }
            }
        }

        // 2. Detect deletions
        for path in known.keys() {
            if !current_snapshot.contains_key(path) {
                // If it belonged to one of the watched paths
                let is_under_watched = watched.iter().any(|w| {
                    if w.recursive {
                        path.starts_with(&w.path)
                    } else {
                        path == &w.path || path.parent() == Some(&w.path)
                    }
                });
                if is_under_watched {
                    changed_paths.insert(path.clone());
                }
            }
        }

        // Update baseline
        *known = current_snapshot;

        if changed_paths.is_empty() {
            None
        } else {
            let event = FileWatcherEvent {
                paths: changed_paths.into_iter().collect(),
            };
            let _ = self.event_tx.send(event.clone());
            Some(event)
        }
    }

    fn scan_directory(dir: &Path, recursive: bool, out: &mut HashMap<PathBuf, FileMeta>) {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Skip heavy build / version control trees
            if file_name == ".git" || file_name == "target" || file_name == "node_modules" {
                continue;
            }

            if path.is_file() {
                if let Ok(meta) = entry.metadata()
                    && let Ok(mtime) = meta.modified()
                {
                    out.insert(
                        path,
                        FileMeta {
                            modified: mtime,
                            len: meta.len(),
                        },
                    );
                }
            } else if path.is_dir() && recursive {
                Self::scan_directory(&path, true, out);
            }
        }
    }

    /// Spawns a background task polling watched workspace paths with coalescing interval.
    pub fn start_background_loop(self: Arc<Self>, interval: Duration) {
        let watcher = self.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            info!(
                "FileWatcher background loop started with interval {:?}",
                interval
            );
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(interval) => {
                        let _ = watcher.scan_now().await;
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("FileWatcher background loop stopping");
                        break;
                    }
                }
            }
        });
    }

    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }
}

impl Default for WorkspaceWatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_workspace_watcher_lifecycle_and_diff_detection() {
        let temp = tempdir().unwrap();
        let file1 = temp.path().join("a.txt");
        fs::write(&file1, "initial").unwrap();

        let watcher = WorkspaceWatcher::new();
        watcher.add_watch(temp.path().to_path_buf(), true).await;

        // Baseline scan
        let first_scan = watcher.scan_now().await;
        assert!(first_scan.is_some());
        assert_eq!(first_scan.unwrap().paths, vec![file1.clone()]);

        // Second scan with no changes should return None
        let second_scan = watcher.scan_now().await;
        assert!(second_scan.is_none());

        // Modify file1
        tokio::time::sleep(Duration::from_millis(50)).await;
        fs::write(&file1, "modified content").unwrap();
        let mod_scan = watcher.scan_now().await;
        assert!(mod_scan.is_some());
        assert_eq!(mod_scan.unwrap().paths, vec![file1.clone()]);

        // Add file2
        let file2 = temp.path().join("b.txt");
        fs::write(&file2, "second").unwrap();
        let add_scan = watcher.scan_now().await;
        assert!(add_scan.is_some());
        assert_eq!(add_scan.unwrap().paths, vec![file2.clone()]);

        // Delete file1
        fs::remove_file(&file1).unwrap();
        let del_scan = watcher.scan_now().await;
        assert!(del_scan.is_some());
        assert_eq!(del_scan.unwrap().paths, vec![file1]);
    }
}
