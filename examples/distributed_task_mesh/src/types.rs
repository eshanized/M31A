//! Common types and message models for Distributed Task Mesh.
//!
//! Defines task specifications, node identifiers, execution states, and error envelopes.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub Uuid);

impl TaskId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDefinition {
    pub id: TaskId,
    pub name: String,
    pub payload: serde_json::Value,
    pub priority: Priority,
    pub max_retries: u32,
    pub timeout_ms: u64,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    Running {
        worker_id: NodeId,
        started_at: u64,
    },
    Completed {
        result: serde_json::Value,
        duration_ms: u64,
    },
    Failed {
        reason: String,
        retry_count: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MeshEvent {
    TaskSubmitted(TaskDefinition),
    TaskAssigned {
        task_id: TaskId,
        worker_id: NodeId,
    },
    TaskCompleted {
        task_id: TaskId,
        result: serde_json::Value,
        duration_ms: u64,
    },
    TaskFailed {
        task_id: TaskId,
        reason: String,
    },
    NodeJoined(NodeId),
    NodeHeartbeat {
        node_id: NodeId,
        load: f32,
    },
    NodeEvicted(NodeId),
    LeaderElected {
        term: u64,
        leader_id: NodeId,
    },
}
