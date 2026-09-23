//! TaskGraphId - strongly typed UUIDv7 identifier for task graphs (D-02).

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Strongly typed identifier for a task graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskGraphId(Uuid);

impl TaskGraphId {
    /// Create a new TaskGraphId using UUIDv7 (timestamp-ordered).
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn into_uuid(self) -> Uuid {
        self.0
    }
}

impl Serialize for TaskGraphId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for TaskGraphId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let uuid = Uuid::parse_str(&s).map_err(serde::de::Error::custom)?;
        Ok(TaskGraphId(uuid))
    }
}

impl fmt::Display for TaskGraphId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for TaskGraphId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<TaskGraphId> for Uuid {
    fn from(id: TaskGraphId) -> Uuid {
        id.0
    }
}

impl FromStr for TaskGraphId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(TaskGraphId)
    }
}

impl Default for TaskGraphId {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::TaskId;

    #[test]
    fn test_task_graph_id_construction() {
        let id = TaskGraphId::new();
        assert!(!id.as_bytes().iter().all(|&b| b == 0));
    }

    #[test]
    fn test_task_graph_id_hex_roundtrip() {
        let id = TaskGraphId::new();
        let serialized = serde_json::to_string(&id).unwrap();
        let deserialized: TaskGraphId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_task_graph_id_binary_roundtrip() {
        let id = TaskGraphId::new();
        let bytes = *id.as_bytes();
        let reconstructed = TaskGraphId::from_bytes(bytes);
        assert_eq!(id, reconstructed);
    }

    #[test]
    fn test_task_graph_id_display_and_from_str() {
        let id = TaskGraphId::new();
        let display = format!("{}", id);
        let parsed: TaskGraphId = display.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_task_graph_id_from_and_into_uuid() {
        let uuid = Uuid::now_v7();
        let id = TaskGraphId::from_uuid(uuid);
        assert_eq!(id.as_uuid(), &uuid);
        let back: Uuid = id.into_uuid();
        assert_eq!(back, uuid);
    }

    #[test]
    fn test_task_graph_id_non_interchangeable() {
        let _graph_id = TaskGraphId::new();
        let _task_id = TaskId::new();
        // Compile-time check: distinct newtypes prevent accidental substitution
    }
}
