//! Sovereign daemon lifecycle, graceful shutdown, and interrupted thread recovery coordinator.
//!
//! Manages daemon lifecycle states, graceful shutdown sequences, and interrupted thread recovery.
//! Coordinates:
//! - Graceful daemon shutdown with active turn draining and client notifications.
//! - Clean UDS socket unlinking and lock release.
//! - Interrupted thread recovery upon daemon startup, repairing threads left in dangling
//!   `running` states from previous crashes, abrupt SIGKILLs, or machine reboots.

use crate::connection_multiplexer::ConnectionMultiplexer;
use crate::protocol::JsonRpcNotification;
use crate::turn_admission::TurnAdmissionController;
use anyhow::Result;
use chrono::{DateTime, Utc};
use m31a_storage::Storage;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::{Duration, sleep};

/// Interrupted turn marker prefix recorded in thread records when a process termination interrupts an active turn.
pub const INTERRUPTED_TURN_PREFIX: &str = "m31a-interrupted-v1:";

/// Recovery snapshot persisting loaded threads across daemon restarts (§7.7).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct LoadedThreadsSnapshot {
    pub timestamp: DateTime<Utc>,
    pub loaded_thread_ids: Vec<String>,
    pub daemon_pid: u32,
}

impl LoadedThreadsSnapshot {
    pub fn save(&self, daemon_dir: &Path) -> Result<PathBuf> {
        fs::create_dir_all(daemon_dir)?;
        let path = daemon_dir.join("loaded-threads.json");
        let data = serde_json::to_string_pretty(self)?;
        fs::write(&path, data)?;
        Ok(path)
    }

    pub fn load(daemon_dir: &Path) -> Result<Option<Self>> {
        let path = daemon_dir.join("loaded-threads.json");
        if path.exists() {
            let text = fs::read_to_string(&path)?;
            let snapshot: Self = serde_json::from_str(&text)?;
            Ok(Some(snapshot))
        } else {
            Ok(None)
        }
    }
}

/// Sets up the control socket with 0700 permissions, hash-protected physical path,
/// and rendezvous symlink app-server-control.sock (§7.7).
pub fn setup_control_socket_rendezvous(base_dir: &Path) -> Result<(PathBuf, PathBuf)> {
    let control_dir = base_dir.join("app-server-control");
    fs::create_dir_all(&control_dir)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&control_dir, fs::Permissions::from_mode(0o700));
    }

    let dir_str = control_dir.to_string_lossy();
    let mut hash_num: u64 = 0;
    for b in dir_str.as_bytes() {
        hash_num = hash_num.wrapping_mul(31).wrapping_add(*b as u64);
    }
    let hash = format!("{:016x}", hash_num);

    let physical_socket = control_dir.join(format!("sock-{}", &hash[..8]));
    let symlink_path = control_dir.join("app-server-control.sock");

    if symlink_path.exists() || fs::symlink_metadata(&symlink_path).is_ok() {
        let _ = fs::remove_file(&symlink_path);
    }

    #[cfg(unix)]
    {
        let _ = std::os::unix::fs::symlink(&physical_socket, &symlink_path);
    }

    Ok((physical_socket, symlink_path))
}

/// The event that triggered daemon shutdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonShutdownTrigger {
    SigTerm,
    SigInt,
    UserRpcCommand,
    IdleTimeout,
    CrashRecovery,
}

/// Structured summary of actions performed during graceful shutdown.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonShutdownReport {
    pub trigger: DaemonShutdownTrigger,
    pub drained_turns: usize,
    pub active_clients_notified: usize,
    pub socket_unlinked: bool,
    pub clean_shutdown: bool,
    pub timestamp: DateTime<Utc>,
}

/// Structured summary of interrupted threads repaired during daemon startup.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThreadRecoveryReport {
    pub interrupted_threads_recovered: usize,
    pub recovered_thread_ids: Vec<String>,
    pub diagnostic_items_appended: usize,
    pub timestamp: DateTime<Utc>,
}

/// Daemon lifecycle supervisor.
pub struct DaemonLifecycleCoordinator {
    is_shutting_down: AtomicBool,
    admission: Arc<TurnAdmissionController>,
    multiplexer: Arc<ConnectionMultiplexer>,
    socket_path: Option<PathBuf>,
}

impl DaemonLifecycleCoordinator {
    pub fn new(
        admission: Arc<TurnAdmissionController>,
        multiplexer: Arc<ConnectionMultiplexer>,
        socket_path: Option<impl AsRef<Path>>,
    ) -> Self {
        Self {
            is_shutting_down: AtomicBool::new(false),
            admission,
            multiplexer,
            socket_path: socket_path.map(|p| p.as_ref().to_path_buf()),
        }
    }

    pub fn is_shutting_down(&self) -> bool {
        self.is_shutting_down.load(Ordering::SeqCst)
    }

    /// Scan storage for threads left dangling in active or running states from a previous
    /// abrupt daemon exit, and repair them to a clean `idle` state.
    pub fn recover_interrupted_threads(&self, storage: &Storage) -> Result<ThreadRecoveryReport> {
        let threads = storage.list_threads(1000)?;
        let mut report = ThreadRecoveryReport {
            timestamp: Utc::now(),
            ..Default::default()
        };

        for thread in threads {
            // Check if thread was left in an unfinalized state
            let thread_id = &thread.id;
            let turns = storage.list_turns(thread_id)?;

            if let Some(last_turn) = turns.last()
                && (last_turn.status == "running" || last_turn.status == "in_flight")
            {
                // Turn was interrupted in-flight! Finalize it cleanly with §7.7 prefix
                let status = format!("{}interrupted", INTERRUPTED_TURN_PREFIX);
                storage.save_turn(&last_turn.id, thread_id, last_turn.turn_index, &status)?;

                // Append diagnostic recovery notification item to the thread
                let notice_content = serde_json::to_string(&serde_json::json!({
                    "type": "daemon_recovery",
                    "message": "Turn was interrupted by previous daemon shutdown. State recovered cleanly.",
                    "timestamp": Utc::now().to_rfc3339()
                }))?;

                let item_id = uuid::Uuid::new_v4().to_string();
                storage.save_item(
                    &item_id,
                    thread_id,
                    Some(&last_turn.id),
                    "system_notice",
                    &notice_content,
                )?;

                report.interrupted_threads_recovered += 1;
                report.recovered_thread_ids.push(thread_id.clone());
                report.diagnostic_items_appended += 1;
            }
        }

        Ok(report)
    }

    /// Execute graceful daemon shutdown sequence.
    pub async fn initiate_shutdown(
        &self,
        trigger: DaemonShutdownTrigger,
        drain_timeout_ms: u64,
    ) -> DaemonShutdownReport {
        self.is_shutting_down.store(true, Ordering::SeqCst);

        let client_count = self.multiplexer.connected_client_count();

        // 1. Broadcast shutdown notification to all connected clients
        let shutdown_notif = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "notification/daemon_shutting_down".to_string(),
            params: Some(serde_json::json!({
                "trigger": format!("{trigger:?}"),
                "drain_timeout_ms": drain_timeout_ms,
                "timestamp": Utc::now().to_rfc3339()
            })),
        };
        self.multiplexer.broadcast_global(shutdown_notif);

        // 2. Drain active turns with timeout
        let start = tokio::time::Instant::now();
        let timeout_dur = Duration::from_millis(drain_timeout_ms);
        let mut drained_turns = 0;

        while self.admission.active_turn_count() > 0 && start.elapsed() < timeout_dur {
            sleep(Duration::from_millis(50)).await;
            drained_turns = drained_turns.max(self.admission.active_turn_count());
        }

        // 3. Unlink UDS socket file if specified
        let mut socket_unlinked = false;
        if let Some(ref path) = self.socket_path
            && path.exists()
        {
            let _ = std::fs::remove_file(path);
            socket_unlinked = true;
        }

        let clean_shutdown = self.admission.active_turn_count() == 0;

        DaemonShutdownReport {
            trigger,
            drained_turns,
            active_clients_notified: client_count,
            socket_unlinked,
            clean_shutdown,
            timestamp: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection_multiplexer::ClientKind;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_shutdown_lifecycle_and_socket_cleanup() {
        let temp = tempdir().unwrap();
        let socket_file = temp.path().join("daemon.sock");
        std::fs::write(&socket_file, b"dummy socket").unwrap();
        assert!(socket_file.exists());

        let admission = Arc::new(TurnAdmissionController::default());
        let mux = Arc::new(ConnectionMultiplexer::new());

        let (_cid, mut rx) = mux.register_client(ClientKind::Tui, 16);

        let coordinator = DaemonLifecycleCoordinator::new(admission, mux, Some(&socket_file));

        assert!(!coordinator.is_shutting_down());

        let report = coordinator
            .initiate_shutdown(DaemonShutdownTrigger::UserRpcCommand, 500)
            .await;

        assert!(coordinator.is_shutting_down());
        assert_eq!(report.trigger, DaemonShutdownTrigger::UserRpcCommand);
        assert_eq!(report.active_clients_notified, 1);
        assert!(report.socket_unlinked);
        assert!(!socket_file.exists()); // Socket file successfully removed!
        assert!(report.clean_shutdown);

        let notif = rx.recv().await.expect("client receives shutdown notice");
        assert_eq!(notif.method, "notification/daemon_shutting_down");
    }

    #[test]
    fn test_recover_interrupted_threads() {
        let temp = tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        let storage = Storage::open(&db_path).unwrap();

        // Create a thread with an interrupted turn
        let thread_id = "th_test_rec";
        let meta = m31a_core::types::SessionMetadata {
            id: thread_id.to_string(),
            title: "Recovery Thread".to_string(),
            model: "meta/llama-3.3-70b-instruct".to_string(),
            cwd: "/workspace".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            extra: std::collections::HashMap::new(),
        };
        storage
            .save_thread(&meta, "/rollout", "write", "on-request")
            .unwrap();

        // Save a turn in running state (simulating crash during turn execution)
        storage
            .save_turn("turn_crash_1", thread_id, 1, "running")
            .unwrap();

        let admission = Arc::new(TurnAdmissionController::default());
        let mux = Arc::new(ConnectionMultiplexer::new());
        let coordinator = DaemonLifecycleCoordinator::new(admission, mux, None::<&str>);

        let report = coordinator
            .recover_interrupted_threads(&storage)
            .expect("recovery succeeds");

        assert_eq!(report.interrupted_threads_recovered, 1);
        assert_eq!(report.recovered_thread_ids, vec![thread_id.to_string()]);
        assert_eq!(report.diagnostic_items_appended, 1);

        // Verify turn status was repaired in SQLite
        let turns = storage.list_turns(thread_id).unwrap();
        assert_eq!(
            turns[0].status,
            format!("{}interrupted", INTERRUPTED_TURN_PREFIX)
        );
    }

    #[test]
    fn test_loaded_threads_snapshot_and_rendezvous_socket() {
        let temp = tempdir().unwrap();
        let daemon_dir = temp.path().join("daemon");
        let snapshot = LoadedThreadsSnapshot {
            timestamp: Utc::now(),
            loaded_thread_ids: vec!["t1".to_string(), "t2".to_string()],
            daemon_pid: std::process::id(),
        };

        let file = snapshot.save(&daemon_dir).unwrap();
        assert!(file.exists());

        let loaded = LoadedThreadsSnapshot::load(&daemon_dir).unwrap().unwrap();
        assert_eq!(loaded.loaded_thread_ids, vec!["t1", "t2"]);

        let (physical, symlink) = setup_control_socket_rendezvous(temp.path()).unwrap();
        assert!(symlink.is_symlink() || symlink.exists());
        assert!(physical.to_string_lossy().contains("sock-"));
    }
}
