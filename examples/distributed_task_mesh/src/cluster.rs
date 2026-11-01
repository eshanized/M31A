//! Node clustering and membership tracking for Distributed Task Mesh.
//!
//! Maintains cluster node heartbeats, health states, and routing tables for distributed task dispatch.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::types::NodeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeHealth {
    Active,
    Suspect,
    Dead,
}

#[derive(Debug, Clone)]
pub struct PeerState {
    pub id: NodeId,
    pub last_seen: Instant,
    pub load: f32,
    pub health: NodeHealth,
}

pub struct ClusterMembership {
    local_node: NodeId,
    peers: Arc<Mutex<HashMap<NodeId, PeerState>>>,
    heartbeat_timeout: Duration,
    dead_timeout: Duration,
    current_term: Arc<Mutex<u64>>,
    current_leader: Arc<Mutex<Option<NodeId>>>,
}

impl ClusterMembership {
    pub fn new(local_node: NodeId, heartbeat_timeout: Duration) -> Self {
        Self {
            local_node,
            peers: Arc::new(Mutex::new(HashMap::new())),
            heartbeat_timeout,
            dead_timeout: heartbeat_timeout * 2,
            current_term: Arc::new(Mutex::new(0)),
            current_leader: Arc::new(Mutex::new(None)),
        }
    }

    pub fn record_heartbeat(&self, node_id: NodeId, load: f32) {
        let mut peers = self.peers.lock().unwrap();
        peers.insert(
            node_id.clone(),
            PeerState {
                id: node_id,
                last_seen: Instant::now(),
                load,
                health: NodeHealth::Active,
            },
        );
    }

    /// Evaluates peer health and returns newly evicted dead nodes.
    pub fn audit_membership(&self) -> (Vec<NodeId>, Vec<NodeId>) {
        let mut peers = self.peers.lock().unwrap();
        let now = Instant::now();
        let mut newly_suspect = Vec::new();
        let mut newly_dead = Vec::new();

        for peer in peers.values_mut() {
            let elapsed = now.duration_since(peer.last_seen);
            if elapsed > self.dead_timeout && peer.health != NodeHealth::Dead {
                peer.health = NodeHealth::Dead;
                newly_dead.push(peer.id.clone());
            } else if elapsed > self.heartbeat_timeout && peer.health == NodeHealth::Active {
                peer.health = NodeHealth::Suspect;
                newly_suspect.push(peer.id.clone());
            }
        }

        (newly_suspect, newly_dead)
    }

    pub fn elect_leader(&self, candidate_id: NodeId) -> (u64, NodeId) {
        let mut term = self.current_term.lock().unwrap();
        *term += 1;
        let mut leader = self.current_leader.lock().unwrap();
        *leader = Some(candidate_id.clone());
        (*term, candidate_id)
    }

    pub fn leader(&self) -> Option<NodeId> {
        self.current_leader.lock().unwrap().clone()
    }

    pub fn local_node(&self) -> &NodeId {
        &self.local_node
    }

    pub fn active_nodes_count(&self) -> usize {
        let peers = self.peers.lock().unwrap();
        peers
            .values()
            .filter(|p| p.health == NodeHealth::Active)
            .count()
            + 1 // +1 for local node
    }
}
