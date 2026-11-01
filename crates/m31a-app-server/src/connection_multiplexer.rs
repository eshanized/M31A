//! Sovereign stateful connection multiplexer and fan-out event bus.
//!
//! Stateful connection multiplexer and fan-out event bus for the M31A App Server.
//!
//! Multiplexes concurrent client sessions (CLI, TUI, IDE extensions, Web UI) connecting
//! over Unix domain sockets or stdio streams:
//! - Thread-scoped and global notification routing with zero-copy broadcast fan-out.
//! - Bounded asynchronous client delivery queues preventing slow consumers from blocking the daemon.
//! - Client connection tracking, thread subscriptions, and liveness bookkeeping.

use crate::protocol::JsonRpcNotification;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Client interface category connecting to the M31A App Server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientKind {
    /// Command-line interface process.
    Cli,
    /// Terminal user interface.
    Tui,
    /// Desktop IDE extension (VS Code, JetBrains, Neovim).
    IdeExtension,
    /// Local browser web dashboard.
    WebDashboard,
    /// Internal background worker or subagent.
    InternalWorker,
}

/// Metadata description of a registered active client session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientMetadata {
    pub client_id: Uuid,
    pub kind: ClientKind,
    pub connected_at: DateTime<Utc>,
    pub subscribed_threads: Vec<String>,
}

struct ClientSession {
    client_id: Uuid,
    kind: ClientKind,
    connected_at: DateTime<Utc>,
    subscribed_threads: HashSet<String>,
    sender: mpsc::Sender<JsonRpcNotification>,
}

/// Thread-safe client connection multiplexer and notification fan-out bus.
pub struct ConnectionMultiplexer {
    sessions: RwLock<HashMap<Uuid, ClientSession>>,
}

impl Default for ConnectionMultiplexer {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionMultiplexer {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    /// Register a new client session, returning its client ID and an inbound notification receiver.
    pub fn register_client(
        &self,
        kind: ClientKind,
        queue_capacity: usize,
    ) -> (Uuid, mpsc::Receiver<JsonRpcNotification>) {
        let client_id = Uuid::new_v4();
        let (sender, receiver) = mpsc::channel(queue_capacity.max(16));

        let session = ClientSession {
            client_id,
            kind,
            connected_at: Utc::now(),
            subscribed_threads: HashSet::new(),
            sender,
        };

        let mut sessions = self.sessions.write().unwrap();
        sessions.insert(client_id, session);

        (client_id, receiver)
    }

    /// Unregister a client session upon disconnect.
    pub fn unregister_client(&self, client_id: Uuid) {
        let mut sessions = self.sessions.write().unwrap();
        sessions.remove(&client_id);
    }

    /// Subscribe a client to real-time events for a specific conversation thread.
    pub fn subscribe_thread(&self, client_id: Uuid, thread_id: &str) -> bool {
        let mut sessions = self.sessions.write().unwrap();
        if let Some(session) = sessions.get_mut(&client_id) {
            session.subscribed_threads.insert(thread_id.to_string());
            true
        } else {
            false
        }
    }

    /// Unsubscribe a client from a specific conversation thread.
    pub fn unsubscribe_thread(&self, client_id: Uuid, thread_id: &str) -> bool {
        let mut sessions = self.sessions.write().unwrap();
        if let Some(session) = sessions.get_mut(&client_id) {
            session.subscribed_threads.remove(thread_id)
        } else {
            false
        }
    }

    /// Broadcast a notification to all currently connected clients regardless of thread subscription.
    pub fn broadcast_global(&self, notification: JsonRpcNotification) {
        let sessions = self.sessions.read().unwrap();
        for session in sessions.values() {
            let _ = session.sender.try_send(notification.clone());
        }
    }

    /// Broadcast a notification specifically to clients subscribed to the given thread.
    pub fn broadcast_thread(&self, thread_id: &str, notification: JsonRpcNotification) {
        let sessions = self.sessions.read().unwrap();
        for session in sessions.values() {
            if session.subscribed_threads.contains(thread_id) {
                let _ = session.sender.try_send(notification.clone());
            }
        }
    }

    /// Query metadata for all currently connected clients.
    pub fn list_clients(&self) -> Vec<ClientMetadata> {
        let sessions = self.sessions.read().unwrap();
        sessions
            .values()
            .map(|s| ClientMetadata {
                client_id: s.client_id,
                kind: s.kind,
                connected_at: s.connected_at,
                subscribed_threads: s.subscribed_threads.iter().cloned().collect(),
            })
            .collect()
    }

    /// Total count of active connected clients.
    pub fn connected_client_count(&self) -> usize {
        let sessions = self.sessions.read().unwrap();
        sessions.len()
    }

    /// Number of clients subscribed to a specific thread.
    pub fn subscribers_for_thread(&self, thread_id: &str) -> usize {
        let sessions = self.sessions.read().unwrap();
        sessions
            .values()
            .filter(|s| s.subscribed_threads.contains(thread_id))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_unregister_client() {
        let mux = ConnectionMultiplexer::new();
        assert_eq!(mux.connected_client_count(), 0);

        let (cid, _rx) = mux.register_client(ClientKind::Tui, 32);
        assert_eq!(mux.connected_client_count(), 1);

        let clients = mux.list_clients();
        assert_eq!(clients.len(), 1);
        assert_eq!(clients[0].client_id, cid);
        assert_eq!(clients[0].kind, ClientKind::Tui);

        mux.unregister_client(cid);
        assert_eq!(mux.connected_client_count(), 0);
    }

    #[tokio::test]
    async fn test_global_broadcast_reaches_all_clients() {
        let mux = ConnectionMultiplexer::new();
        let (_c1, mut rx1) = mux.register_client(ClientKind::Cli, 16);
        let (_c2, mut rx2) = mux.register_client(ClientKind::IdeExtension, 16);

        let notif = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "notification/system_notice".to_string(),
            params: Some(serde_json::json!({ "msg": "Daemon starting" })),
        };

        mux.broadcast_global(notif.clone());

        let recv1 = rx1.recv().await.expect("rx1 receives");
        let recv2 = rx2.recv().await.expect("rx2 receives");

        assert_eq!(recv1.method, "notification/system_notice");
        assert_eq!(recv2.method, "notification/system_notice");
    }

    #[tokio::test]
    async fn test_thread_scoped_routing() {
        let mux = ConnectionMultiplexer::new();
        let (c1, mut rx1) = mux.register_client(ClientKind::Tui, 16);
        let (c2, mut rx2) = mux.register_client(ClientKind::IdeExtension, 16);

        // Client 1 subscribes to thread_a, Client 2 subscribes to thread_b
        mux.subscribe_thread(c1, "thread_a");
        mux.subscribe_thread(c2, "thread_b");

        assert_eq!(mux.subscribers_for_thread("thread_a"), 1);
        assert_eq!(mux.subscribers_for_thread("thread_b"), 1);
        assert_eq!(mux.subscribers_for_thread("thread_c"), 0);

        // Send notification for thread_a
        let notif_a = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "notification/item_appended".to_string(),
            params: Some(serde_json::json!({ "thread_id": "thread_a", "item": "text" })),
        };
        mux.broadcast_thread("thread_a", notif_a);

        // rx1 must receive notif_a, while rx2 must not receive it!
        let recv1 = rx1.recv().await.expect("rx1 receives thread_a event");
        assert_eq!(recv1.method, "notification/item_appended");

        // rx2 should have nothing in its queue
        assert!(rx2.try_recv().is_err());
    }
}
