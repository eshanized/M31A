//! Path-Based Concurrency Lock Manager for M31A Agentic Swarms
//!
//! Provides hierarchical path reader-writer locking across subagents, tools, and background tasks.
//! Ensures mutually exclusive writes on directory trees and files, concurrent reads,
//! deterministic canonical acquisition ordering to prevent deadlocks, and RAII guard cleanup.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Notify;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LockMode {
    Read,
    Write,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockEntry {
    pub id: String,
    pub path: PathBuf,
    pub mode: LockMode,
    pub holder_id: String,
    pub reason: String,
    pub acquired_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Error, Debug)]
pub enum ExecutionLockError {
    #[error(
        "Timed out waiting to acquire lock on '{path:?}' for holder '{holder_id}': conflicting with existing locks {conflicts:?}"
    )]
    Timeout {
        path: PathBuf,
        holder_id: String,
        conflicts: Vec<String>,
    },
    #[error("Lock acquisition cancelled")]
    Cancelled,
}

struct InnerState {
    active_locks: HashMap<String, LockEntry>,
}

impl InnerState {
    fn new() -> Self {
        Self {
            active_locks: HashMap::new(),
        }
    }

    /// Check if a proposed lock conflicts with any currently held lock.
    fn find_conflicts(&self, path: &Path, mode: LockMode) -> Vec<LockEntry> {
        let norm_path = normalize_path(path);
        let mut conflicts = Vec::new();

        for entry in self.active_locks.values() {
            let entry_norm = normalize_path(&entry.path);
            let paths_overlap = entry_norm == norm_path
                || entry_norm.starts_with(&norm_path)
                || norm_path.starts_with(&entry_norm);

            if paths_overlap {
                // If either is Write, it is a conflict.
                if entry.mode == LockMode::Write || mode == LockMode::Write {
                    conflicts.push(entry.clone());
                }
            }
        }

        conflicts
    }

    /// Check if a batch of requested locks can ALL be granted without conflict
    /// with existing locks or each other.
    fn can_grant_batch(&self, requests: &[(PathBuf, LockMode)]) -> Option<Vec<LockEntry>> {
        let mut all_conflicts = Vec::new();

        // 1. Check against active locks
        for (path, mode) in requests {
            let confs = self.find_conflicts(path, *mode);
            for c in confs {
                if !all_conflicts
                    .iter()
                    .any(|existing: &LockEntry| existing.id == c.id)
                {
                    all_conflicts.push(c);
                }
            }
        }

        // 2. Check internal batch consistency (requests within the same batch cannot conflict)
        for i in 0..requests.len() {
            for j in (i + 1)..requests.len() {
                let (path_a, mode_a) = &requests[i];
                let (path_b, mode_b) = &requests[j];
                let norm_a = normalize_path(path_a);
                let norm_b = normalize_path(path_b);

                let overlap =
                    norm_a == norm_b || norm_a.starts_with(&norm_b) || norm_b.starts_with(&norm_a);
                if overlap && (*mode_a == LockMode::Write || *mode_b == LockMode::Write) {
                    // Self-conflict in batch
                    all_conflicts.push(LockEntry {
                        id: "batch-internal-conflict".into(),
                        path: path_b.clone(),
                        mode: *mode_b,
                        holder_id: "batch".into(),
                        reason: "conflicts with earlier item in same batch".into(),
                        acquired_at: chrono::Utc::now(),
                    });
                }
            }
        }

        if all_conflicts.is_empty() {
            None
        } else {
            Some(all_conflicts)
        }
    }
}

#[derive(Clone)]
pub struct ExecutionLockManager {
    state: Arc<Mutex<InnerState>>,
    notify: Arc<Notify>,
}

impl ExecutionLockManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(InnerState::new())),
            notify: Arc::new(Notify::new()),
        }
    }

    pub fn global() -> &'static ExecutionLockManager {
        static INSTANCE: LazyLock<ExecutionLockManager> = LazyLock::new(ExecutionLockManager::new);
        &INSTANCE
    }

    /// Try to acquire an exclusive write lock immediately without blocking.
    pub fn try_acquire_write(
        &self,
        path: impl AsRef<Path>,
        holder_id: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<PathLockGuard, ExecutionLockError> {
        self.try_acquire(
            path.as_ref(),
            LockMode::Write,
            holder_id.into(),
            reason.into(),
        )
    }

    /// Try to acquire a shared read lock immediately without blocking.
    pub fn try_acquire_read(
        &self,
        path: impl AsRef<Path>,
        holder_id: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<PathLockGuard, ExecutionLockError> {
        self.try_acquire(
            path.as_ref(),
            LockMode::Read,
            holder_id.into(),
            reason.into(),
        )
    }

    fn try_acquire(
        &self,
        path: &Path,
        mode: LockMode,
        holder_id: String,
        reason: String,
    ) -> Result<PathLockGuard, ExecutionLockError> {
        let mut state = self.state.lock().unwrap();
        let conflicts = state.find_conflicts(path, mode);
        if !conflicts.is_empty() {
            return Err(ExecutionLockError::Timeout {
                path: path.to_path_buf(),
                holder_id,
                conflicts: conflicts
                    .into_iter()
                    .map(|c| format!("{}: {:?}", c.holder_id, c.path))
                    .collect(),
            });
        }

        let lock_id = Uuid::new_v4().to_string();
        let entry = LockEntry {
            id: lock_id.clone(),
            path: normalize_path(path),
            mode,
            holder_id,
            reason,
            acquired_at: chrono::Utc::now(),
        };
        state.active_locks.insert(lock_id.clone(), entry);

        Ok(PathLockGuard {
            lock_ids: vec![lock_id],
            state: Arc::clone(&self.state),
            notify: Arc::clone(&self.notify),
        })
    }

    /// Asynchronously acquire a lock with a timeout.
    pub async fn acquire(
        &self,
        path: impl AsRef<Path>,
        mode: LockMode,
        holder_id: impl Into<String>,
        reason: impl Into<String>,
        timeout: Duration,
    ) -> Result<PathLockGuard, ExecutionLockError> {
        let path = path.as_ref();
        let holder_id = holder_id.into();
        let reason = reason.into();
        let start = tokio::time::Instant::now();

        loop {
            // Check under synchronous mutex
            {
                let mut state = self.state.lock().unwrap();
                let conflicts = state.find_conflicts(path, mode);
                if conflicts.is_empty() {
                    let lock_id = Uuid::new_v4().to_string();
                    let entry = LockEntry {
                        id: lock_id.clone(),
                        path: normalize_path(path),
                        mode,
                        holder_id,
                        reason,
                        acquired_at: chrono::Utc::now(),
                    };
                    state.active_locks.insert(lock_id.clone(), entry);

                    return Ok(PathLockGuard {
                        lock_ids: vec![lock_id],
                        state: Arc::clone(&self.state),
                        notify: Arc::clone(&self.notify),
                    });
                }
            }

            let elapsed = start.elapsed();
            if elapsed >= timeout {
                let conflicts = {
                    let state = self.state.lock().unwrap();
                    state
                        .find_conflicts(path, mode)
                        .into_iter()
                        .map(|c| format!("{}: {:?}", c.holder_id, c.path))
                        .collect()
                };
                return Err(ExecutionLockError::Timeout {
                    path: path.to_path_buf(),
                    holder_id,
                    conflicts,
                });
            }

            let remaining = timeout - elapsed;
            let notified = self.notify.notified();
            tokio::select! {
                _ = notified => {
                    // Loop around and try acquiring again
                }
                _ = tokio::time::sleep(remaining) => {
                    let conflicts = {
                        let state = self.state.lock().unwrap();
                        state
                            .find_conflicts(path, mode)
                            .into_iter()
                            .map(|c| format!("{}: {:?}", c.holder_id, c.path))
                            .collect()
                    };
                    return Err(ExecutionLockError::Timeout {
                        path: path.to_path_buf(),
                        holder_id,
                        conflicts,
                    });
                }
            }
        }
    }

    /// Acquire a batch of locks atomically with canonical path sorting to prevent deadlocks.
    pub async fn acquire_batch(
        &self,
        mut requests: Vec<(PathBuf, LockMode)>,
        holder_id: impl Into<String>,
        reason: impl Into<String>,
        timeout: Duration,
    ) -> Result<PathLockGuard, ExecutionLockError> {
        let holder_id = holder_id.into();
        let reason = reason.into();

        // Sort requests canonically by normalized path then mode to ensure deterministic ordering
        requests.sort_by(|a, b| {
            let norm_a = normalize_path(&a.0);
            let norm_b = normalize_path(&b.0);
            norm_a.cmp(&norm_b).then_with(|| match (a.1, b.1) {
                (LockMode::Read, LockMode::Write) => std::cmp::Ordering::Less,
                (LockMode::Write, LockMode::Read) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            })
        });

        let start = tokio::time::Instant::now();

        loop {
            {
                let mut state = self.state.lock().unwrap();
                if state.can_grant_batch(&requests).is_none() {
                    let mut lock_ids = Vec::with_capacity(requests.len());
                    for (path, mode) in &requests {
                        let lock_id = Uuid::new_v4().to_string();
                        let entry = LockEntry {
                            id: lock_id.clone(),
                            path: normalize_path(path),
                            mode: *mode,
                            holder_id: holder_id.clone(),
                            reason: reason.clone(),
                            acquired_at: chrono::Utc::now(),
                        };
                        state.active_locks.insert(lock_id.clone(), entry);
                        lock_ids.push(lock_id);
                    }

                    return Ok(PathLockGuard {
                        lock_ids,
                        state: Arc::clone(&self.state),
                        notify: Arc::clone(&self.notify),
                    });
                }
            }

            let elapsed = start.elapsed();
            if elapsed >= timeout {
                let conflicts = {
                    let state = self.state.lock().unwrap();
                    match state.can_grant_batch(&requests) {
                        Some(confs) => confs
                            .into_iter()
                            .map(|c| format!("{}: {:?}", c.holder_id, c.path))
                            .collect(),
                        None => Vec::new(),
                    }
                };
                return Err(ExecutionLockError::Timeout {
                    path: requests.first().map(|r| r.0.clone()).unwrap_or_default(),
                    holder_id,
                    conflicts,
                });
            }

            let remaining = timeout - elapsed;
            let notified = self.notify.notified();
            tokio::select! {
                _ = notified => {}
                _ = tokio::time::sleep(remaining) => {
                    let conflicts = {
                        let state = self.state.lock().unwrap();
                        match state.can_grant_batch(&requests) {
                            Some(confs) => confs.into_iter().map(|c| format!("{}: {:?}", c.holder_id, c.path)).collect(),
                            None => Vec::new(),
                        }
                    };
                    return Err(ExecutionLockError::Timeout {
                        path: requests.first().map(|r| r.0.clone()).unwrap_or_default(),
                        holder_id,
                        conflicts,
                    });
                }
            }
        }
    }

    /// Query if a path is currently locked.
    pub fn is_locked(&self, path: impl AsRef<Path>) -> Option<LockMode> {
        let norm_path = normalize_path(path.as_ref());
        let state = self.state.lock().unwrap();
        let mut has_read = false;

        for entry in state.active_locks.values() {
            let entry_norm = normalize_path(&entry.path);
            let paths_overlap = entry_norm == norm_path
                || entry_norm.starts_with(&norm_path)
                || norm_path.starts_with(&entry_norm);

            if paths_overlap {
                if entry.mode == LockMode::Write {
                    return Some(LockMode::Write);
                }
                has_read = true;
            }
        }

        if has_read { Some(LockMode::Read) } else { None }
    }

    /// Return all active locks.
    pub fn active_locks(&self) -> Vec<LockEntry> {
        let state = self.state.lock().unwrap();
        state.active_locks.values().cloned().collect()
    }

    /// Acquire a leased lock and return its ID (token), which stays active until explicitly released via `release_leased`.
    pub async fn acquire_leased(
        &self,
        path: impl AsRef<Path>,
        mode: LockMode,
        holder_id: impl Into<String>,
        reason: impl Into<String>,
        timeout: Duration,
    ) -> Result<String, ExecutionLockError> {
        let guard = self.acquire(path, mode, holder_id, reason, timeout).await?;
        let lock_id = guard.lock_ids[0].clone();
        std::mem::forget(guard);
        Ok(lock_id)
    }

    /// Explicitly release a previously acquired leased lock by ID.
    pub fn release_leased(&self, lock_id: &str) -> bool {
        let mut state = self.state.lock().unwrap();
        let removed = state.active_locks.remove(lock_id).is_some();
        if removed {
            drop(state);
            self.notify.notify_waiters();
        }
        removed
    }

    /// Release all locks held by a specific holder (e.g., when a subagent terminates).
    pub fn release_holder_locks(&self, holder_id: &str) -> usize {
        let mut state = self.state.lock().unwrap();
        let to_remove: Vec<String> = state
            .active_locks
            .iter()
            .filter(|(_, entry)| entry.holder_id == holder_id)
            .map(|(id, _)| id.clone())
            .collect();
        let count = to_remove.len();
        for id in &to_remove {
            state.active_locks.remove(id);
        }
        if count > 0 {
            drop(state);
            self.notify.notify_waiters();
        }
        count
    }
}

impl Default for ExecutionLockManager {
    fn default() -> Self {
        Self::new()
    }
}

/// RAII Guard that releases the held locks when dropped.
pub struct PathLockGuard {
    lock_ids: Vec<String>,
    state: Arc<Mutex<InnerState>>,
    notify: Arc<Notify>,
}

impl Drop for PathLockGuard {
    fn drop(&mut self) {
        let mut state = self.state.lock().unwrap();
        for id in &self.lock_ids {
            state.active_locks.remove(id);
        }
        drop(state);
        self.notify.notify_waiters();
    }
}

/// Helper function to normalize path components (resolves `.` and `..` lexically).
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !components.is_empty() {
                    components.pop();
                }
            }
            c => components.push(c.as_os_str()),
        }
    }
    components.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_exclusive_write_lock_blocks_other_writers_and_readers() {
        let mgr = ExecutionLockManager::global();
        let path = PathBuf::from("/tmp/test_repo/src/main.rs");

        let guard = mgr
            .acquire(
                &path,
                LockMode::Write,
                "agent-1",
                "editing file",
                Duration::from_millis(50),
            )
            .await
            .expect("should acquire write lock");

        assert_eq!(mgr.is_locked(&path), Some(LockMode::Write));

        // Reader should fail with timeout
        let read_res = mgr
            .acquire(
                &path,
                LockMode::Read,
                "agent-2",
                "reading file",
                Duration::from_millis(20),
            )
            .await;
        assert!(read_res.is_err());

        // Writer on parent dir should fail with timeout
        let parent_res = mgr
            .acquire(
                Path::new("/tmp/test_repo/src"),
                LockMode::Write,
                "agent-3",
                "editing dir",
                Duration::from_millis(20),
            )
            .await;
        assert!(parent_res.is_err());

        // Drop guard, now another can acquire
        drop(guard);
        assert_eq!(mgr.is_locked(&path), None);

        let read_res = mgr
            .acquire(
                &path,
                LockMode::Read,
                "agent-2",
                "reading file",
                Duration::from_millis(50),
            )
            .await;
        assert!(read_res.is_ok());
    }

    #[tokio::test]
    async fn test_multiple_readers_allowed_concurrently() {
        let mgr = ExecutionLockManager::global();
        let path = PathBuf::from("/tmp/concurrent_read_test/config.toml");

        let guard1 = mgr
            .acquire(
                &path,
                LockMode::Read,
                "agent-a",
                "read 1",
                Duration::from_millis(50),
            )
            .await
            .expect("acquire 1");
        let guard2 = mgr
            .acquire(
                &path,
                LockMode::Read,
                "agent-b",
                "read 2",
                Duration::from_millis(50),
            )
            .await
            .expect("acquire 2");

        assert_eq!(mgr.is_locked(&path), Some(LockMode::Read));

        drop(guard1);
        assert_eq!(mgr.is_locked(&path), Some(LockMode::Read));
        drop(guard2);
        assert_eq!(mgr.is_locked(&path), None);
    }

    #[tokio::test]
    async fn test_batch_locking_with_canonical_ordering() {
        let mgr = ExecutionLockManager::global();
        let paths = vec![
            (PathBuf::from("/tmp/batch/b.txt"), LockMode::Write),
            (PathBuf::from("/tmp/batch/a.txt"), LockMode::Write),
        ];

        let guard = mgr
            .acquire_batch(
                paths,
                "agent-batch",
                "refactor pair",
                Duration::from_millis(100),
            )
            .await
            .expect("batch acquire should succeed");

        assert_eq!(
            mgr.is_locked(Path::new("/tmp/batch/a.txt")),
            Some(LockMode::Write)
        );
        assert_eq!(
            mgr.is_locked(Path::new("/tmp/batch/b.txt")),
            Some(LockMode::Write)
        );

        drop(guard);
        assert_eq!(mgr.is_locked(Path::new("/tmp/batch/a.txt")), None);
        assert_eq!(mgr.is_locked(Path::new("/tmp/batch/b.txt")), None);
    }

    #[tokio::test]
    async fn test_leased_locks_and_holder_cleanup() {
        let mgr = ExecutionLockManager::global();
        let path1 = PathBuf::from("/tmp/lease/file1.rs");
        let path2 = PathBuf::from("/tmp/lease/file2.rs");

        let token1 = mgr
            .acquire_leased(
                &path1,
                LockMode::Write,
                "agent-swarm-1",
                "long task 1",
                Duration::from_millis(50),
            )
            .await
            .expect("acquire lease 1");
        let _token2 = mgr
            .acquire_leased(
                &path2,
                LockMode::Write,
                "agent-swarm-1",
                "long task 2",
                Duration::from_millis(50),
            )
            .await
            .expect("acquire lease 2");

        assert_eq!(mgr.is_locked(&path1), Some(LockMode::Write));
        assert_eq!(mgr.is_locked(&path2), Some(LockMode::Write));

        // Explicit release 1
        let released = mgr.release_leased(&token1);
        assert!(released);
        assert_eq!(mgr.is_locked(&path1), None);
        assert_eq!(mgr.is_locked(&path2), Some(LockMode::Write));

        // Release all locks for holder
        let cleaned = mgr.release_holder_locks("agent-swarm-1");
        assert_eq!(cleaned, 1);
        assert_eq!(mgr.is_locked(&path2), None);
    }
}
