//! High-level asynchronous filesystem watcher implementation.
//!
//! Leverages OS notification APIs (inotify, kqueue, FSEvents) to detect changes with configurable debouncing.

use crate::events::{FileChangeEvent, FileChangeKind, WorkspaceChangeBatch};
use crate::filter::WatcherFilter;
use chrono::Utc;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

pub struct WatcherConfig {
    pub poll_interval: Duration,
    pub debounce_duration: Duration,
    pub max_files_limit: usize,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(500),
            debounce_duration: Duration::from_millis(150),
            max_files_limit: 50_000,
        }
    }
}

pub struct WorkspaceWatcher {
    root_dir: PathBuf,
    filter: WatcherFilter,
    config: WatcherConfig,
    sender: broadcast::Sender<WorkspaceChangeBatch>,
    running: Arc<AtomicBool>,
    last_git_head: Arc<RwLock<Option<String>>>,
}

impl WorkspaceWatcher {
    pub fn new(root_dir: PathBuf, config: Option<WatcherConfig>) -> Self {
        let (sender, _) = broadcast::channel(128);
        Self {
            root_dir,
            filter: WatcherFilter::default(),
            config: config.unwrap_or_default(),
            sender,
            running: Arc::new(AtomicBool::new(false)),
            last_git_head: Arc::new(RwLock::new(None)),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WorkspaceChangeBatch> {
        self.sender.subscribe()
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Starts the background watcher loop.
    pub fn start(&self) {
        let handle = match tokio::runtime::Handle::try_current() {
            Ok(h) => h,
            Err(_) => return, // No tokio reactor running, ignore gracefully
        };

        if self.running.swap(true, Ordering::SeqCst) {
            return; // Already running
        }

        let root = self.root_dir.clone();
        let filter = self.filter.clone();
        let config = WatcherConfig {
            poll_interval: self.config.poll_interval,
            debounce_duration: self.config.debounce_duration,
            max_files_limit: self.config.max_files_limit,
        };
        let sender = self.sender.clone();
        let running = self.running.clone();
        let git_head_lock = self.last_git_head.clone();

        handle.spawn(async move {
            info!(root = ?root, "WorkspaceWatcher background loop started");

            // Read initial git HEAD
            let mut initial_head = read_git_head(&root);
            {
                let mut guard = git_head_lock.write().await;
                *guard = initial_head.clone();
            }

            // Set up native OS watcher via notify
            let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
            let mut notify_watcher = match RecommendedWatcher::new(
                move |res| {
                    let _ = event_tx.send(res);
                },
                Config::default(),
            ) {
                Ok(w) => Some(w),
                Err(e) => {
                    warn!("Failed to initialize OS watcher: {}. Using polling fallback.", e);
                    None
                }
            };

            if let Some(ref mut w) = notify_watcher
                && let Err(e) = w.watch(&root, RecursiveMode::Recursive) {
                    warn!("Failed to watch root directory recursively: {}. Using polling fallback.", e);
                    notify_watcher = None;
                }

            if notify_watcher.is_some() {
                debug!("Using native OS file notifications (inotify/kqueue/FSEvents)");
                let mut pending_changes: HashMap<PathBuf, FileChangeKind> = HashMap::new();
                let mut git_head_changed = false;
                let mut last_event_time: Option<Instant> = None;

                while running.load(Ordering::SeqCst) {
                    let sleep_dur = match last_event_time {
                        Some(t) => {
                            let elapsed = t.elapsed();
                            if elapsed >= config.debounce_duration {
                                Duration::from_millis(0)
                            } else {
                                config.debounce_duration - elapsed
                            }
                        }
                        None => Duration::from_millis(200),
                    };

                    tokio::select! {
                        res_opt = event_rx.recv() => {
                            match res_opt {
                                Some(Ok(event)) => {
                                    let mut has_relevant_change = false;
                                    for path in event.paths {
                                        // Check if git HEAD changed
                                        if path.ends_with(".git/HEAD") || path.ends_with(".git/refs/heads") {
                                            git_head_changed = true;
                                            has_relevant_change = true;
                                            continue;
                                        }

                                        // Apply filters
                                        if should_ignore_path(&path, &filter) {
                                            continue;
                                        }

                                        let kind = match event.kind {
                                            EventKind::Create(_) => FileChangeKind::Created,
                                            EventKind::Modify(_) => {
                                                if pending_changes.get(&path) == Some(&FileChangeKind::Created) {
                                                    FileChangeKind::Created
                                                } else {
                                                    FileChangeKind::Modified
                                                }
                                            }
                                            EventKind::Remove(_) => FileChangeKind::Deleted,
                                            _ => continue,
                                        };

                                        pending_changes.insert(path, kind);
                                        has_relevant_change = true;
                                    }

                                    if has_relevant_change {
                                        last_event_time = Some(Instant::now());
                                    }
                                }
                                Some(Err(e)) => {
                                    debug!("Watcher event error: {}", e);
                                }
                                None => break, // Channel closed
                            }
                        }
                        _ = tokio::time::sleep(sleep_dur) => {
                            if let Some(t) = last_event_time
                                && t.elapsed() >= config.debounce_duration {
                                    // Flush debounced batch
                                    let mut batch = WorkspaceChangeBatch::new();
                                    for (path, kind) in pending_changes.drain() {
                                        batch.changes.push(FileChangeEvent {
                                            path,
                                            kind,
                                            timestamp: Utc::now(),
                                        });
                                    }

                                    let current_head = read_git_head(&root);
                                    if git_head_changed || current_head != initial_head {
                                        batch.git_head_changed = true;
                                        initial_head = current_head.clone();
                                        let mut guard = git_head_lock.write().await;
                                        *guard = current_head;
                                        git_head_changed = false;
                                    }

                                    last_event_time = None;

                                    if !batch.is_empty() {
                                        debug!(
                                            total_changes = batch.len(),
                                            git_changed = batch.git_head_changed,
                                            "Broadcasting debounced workspace changes"
                                        );
                                        let _ = sender.send(batch);
                                    }
                                }
                        }
                    }
                }
            } else {
                // Polling fallback if OS notifications unavailable
                debug!("Running polling fallback loop");
                let mut snapshot: HashMap<PathBuf, (SystemTime, u64)> = HashMap::new();
                scan_directory(&root, &filter, config.max_files_limit, &mut snapshot);

                while running.load(Ordering::SeqCst) {
                    tokio::time::sleep(config.poll_interval).await;
                    if !running.load(Ordering::SeqCst) {
                        break;
                    }

                    let mut current_snapshot = HashMap::new();
                    scan_directory(&root, &filter, config.max_files_limit, &mut current_snapshot);

                    let mut batch = WorkspaceChangeBatch::new();

                    for (path, (mtime, size)) in &current_snapshot {
                        match snapshot.get(path) {
                            None => {
                                batch.changes.push(FileChangeEvent {
                                    path: path.clone(),
                                    kind: FileChangeKind::Created,
                                    timestamp: Utc::now(),
                                });
                            }
                            Some((old_mtime, old_size)) => {
                                if mtime != old_mtime || size != old_size {
                                    batch.changes.push(FileChangeEvent {
                                        path: path.clone(),
                                        kind: FileChangeKind::Modified,
                                        timestamp: Utc::now(),
                                    });
                                }
                            }
                        }
                    }

                    for old_path in snapshot.keys() {
                        if !current_snapshot.contains_key(old_path) {
                            batch.changes.push(FileChangeEvent {
                                path: old_path.clone(),
                                kind: FileChangeKind::Deleted,
                                timestamp: Utc::now(),
                            });
                        }
                    }

                    let current_head = read_git_head(&root);
                    if current_head != initial_head {
                        batch.git_head_changed = true;
                        initial_head = current_head.clone();
                        let mut guard = git_head_lock.write().await;
                        *guard = current_head;
                    }

                    snapshot = current_snapshot;

                    if !batch.is_empty() {
                        let _ = sender.send(batch);
                    }
                }
            }

            info!(root = ?root, "WorkspaceWatcher background loop stopped");
        });
    }
}

fn should_ignore_path(path: &Path, filter: &WatcherFilter) -> bool {
    for comp in path.components() {
        if let std::path::Component::Normal(os_str) = comp
            && let Some(name) = os_str.to_str()
            && filter.should_ignore_dir(name)
        {
            return true;
        }
    }
    filter.should_ignore_file(path)
}

fn scan_directory(
    dir: &Path,
    filter: &WatcherFilter,
    limit: usize,
    acc: &mut HashMap<PathBuf, (SystemTime, u64)>,
) {
    if acc.len() >= limit {
        return;
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        if acc.len() >= limit {
            break;
        }

        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        if file_type.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && !filter.should_ignore_dir(name)
            {
                scan_directory(&path, filter, limit, acc);
            }
        } else if file_type.is_file()
            && !filter.should_ignore_file(&path)
            && let Ok(meta) = entry.metadata()
        {
            let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let size = meta.len();
            acc.insert(path, (mtime, size));
        }
    }
}

fn read_git_head(root: &Path) -> Option<String> {
    let head_file = root.join(".git").join("HEAD");
    if head_file.is_file() {
        fs::read_to_string(head_file)
            .ok()
            .map(|s| s.trim().to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_workspace_watcher_detects_changes() {
        let temp = tempdir().unwrap();
        let config = WatcherConfig {
            poll_interval: Duration::from_millis(50),
            debounce_duration: Duration::from_millis(10),
            max_files_limit: 1000,
        };

        let watcher = WorkspaceWatcher::new(temp.path().to_path_buf(), Some(config));
        let mut rx = watcher.subscribe();
        watcher.start();

        // Let watcher start
        tokio::time::sleep(Duration::from_millis(80)).await;

        // Create a file
        let test_file = temp.path().join("code.rs");
        {
            let mut f = fs::File::create(&test_file).unwrap();
            f.write_all(b"fn main() {}").unwrap();
        }

        // Wait for created event
        let batch = tokio::time::timeout(Duration::from_millis(1500), rx.recv())
            .await
            .expect("timed out waiting for event")
            .expect("channel closed");

        assert!(!batch.is_empty());
        assert!(
            batch
                .changes
                .iter()
                .any(|c| c.kind == FileChangeKind::Created)
        );

        // Modify the file
        tokio::time::sleep(Duration::from_millis(50)).await;
        {
            let mut f = fs::OpenOptions::new()
                .append(true)
                .open(&test_file)
                .unwrap();
            f.write_all(b"\n// modified").unwrap();
        }

        let batch2 = tokio::time::timeout(Duration::from_millis(1000), rx.recv())
            .await
            .expect("timed out waiting for modify event")
            .expect("channel closed");

        assert!(
            batch2
                .changes
                .iter()
                .any(|c| c.kind == FileChangeKind::Modified)
        );

        watcher.stop();
    }
}
