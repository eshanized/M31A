//! TaskId and RequirementId - strongly typed UUIDv7 identifiers for tasks and requirements.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Strongly typed identifier for a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, schemars::JsonSchema)]
#[schemars(transparent)]
pub struct TaskId(Uuid);

impl TaskId {
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
}

impl Serialize for TaskId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for TaskId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let uuid = Uuid::parse_str(&s).map_err(serde::de::Error::custom)?;
        Ok(TaskId(uuid))
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for TaskId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<TaskId> for Uuid {
    fn from(id: TaskId) -> Uuid {
        id.0
    }
}

impl FromStr for TaskId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(TaskId)
    }
}

impl FromStr for RequirementId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(RequirementId)
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for RequirementId {
    fn default() -> Self {
        Self::new()
    }
}

/// Strongly typed identifier for a requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, schemars::JsonSchema)]
#[schemars(transparent)]
pub struct RequirementId(Uuid);

impl RequirementId {
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
}

impl Serialize for RequirementId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for RequirementId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let uuid = Uuid::parse_str(&s).map_err(serde::de::Error::custom)?;
        Ok(RequirementId(uuid))
    }
}

impl fmt::Display for RequirementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for RequirementId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<RequirementId> for Uuid {
    fn from(id: RequirementId) -> Uuid {
        id.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_id_construction() {
        let id = TaskId::new();
        assert!(!id.as_bytes().iter().all(|&b| b == 0));
    }

    #[test]
    fn test_task_id_hex_roundtrip() {
        let id = TaskId::new();
        let serialized = serde_json::to_string(&id).unwrap();
        let deserialized: TaskId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_task_id_binary_roundtrip() {
        let id = TaskId::new();
        let bytes = *id.as_bytes();
        let reconstructed = TaskId::from_bytes(bytes);
        assert_eq!(id, reconstructed);
    }

    #[test]
    fn test_task_id_display() {
        let id = TaskId::new();
        let display = format!("{}", id);
        let parsed: TaskId = display.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_task_id_from_uuid() {
        let uuid = Uuid::now_v7();
        let id = TaskId::from(uuid);
        assert_eq!(id.as_uuid(), &uuid);
    }

    #[test]
    fn test_task_id_into_uuid() {
        let id = TaskId::new();
        let uuid: Uuid = id.into();
        assert_eq!(uuid, *id.as_uuid());
    }

    #[test]
    fn test_requirement_id_construction() {
        let id = RequirementId::new();
        assert!(!id.as_bytes().iter().all(|&b| b == 0));
    }

    #[test]
    fn test_requirement_id_hex_roundtrip() {
        let id = RequirementId::new();
        let serialized = serde_json::to_string(&id).unwrap();
        let deserialized: RequirementId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_requirement_id_binary_roundtrip() {
        let id = RequirementId::new();
        let bytes = *id.as_bytes();
        let reconstructed = RequirementId::from_bytes(bytes);
        assert_eq!(id, reconstructed);
    }

    #[test]
    fn test_requirement_id_display() {
        let id = RequirementId::new();
        let display = format!("{}", id);
        let parsed: RequirementId = display.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_requirement_id_from_uuid() {
        let uuid = Uuid::now_v7();
        let id = RequirementId::from(uuid);
        assert_eq!(id.as_uuid(), &uuid);
    }

    #[test]
    fn test_requirement_id_into_uuid() {
        let id = RequirementId::new();
        let uuid: Uuid = id.into();
        assert_eq!(uuid, *id.as_uuid());
    }

    #[test]
    fn test_task_and_requirement_id_non_interchangeable() {
        use std::any::TypeId;
        use std::collections::HashMap;

        let task_id = TaskId::new();
        let req_id = RequirementId::new();

        // 1. Type identity differentiation
        assert_ne!(
            TypeId::of::<TaskId>(),
            TypeId::of::<RequirementId>(),
            "TaskId and RequirementId must have distinct TypeIds"
        );

        // 2. Map isolation: a map keyed on TaskId cannot accept or collide with RequirementId
        let mut task_map = HashMap::<TaskId, &'static str>::new();
        task_map.insert(task_id, "task");
        assert_eq!(task_map.get(&task_id), Some(&"task"));

        let mut req_map = HashMap::<RequirementId, &'static str>::new();
        req_map.insert(req_id, "req");
        assert_eq!(req_map.get(&req_id), Some(&"req"));

        // 3. String representation and JSON differentiation
        let task_json = serde_json::to_string(&task_id).expect("serialize task_id");
        let req_json = serde_json::to_string(&req_id).expect("serialize req_id");
        assert_ne!(task_json, req_json);
    }
}
