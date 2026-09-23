//! Workflow domain identifiers (D-13, D-15).
//!
//! Provides strongly typed UUIDv7 identifiers for workflow runs and workflow step runs.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Strongly typed identifier for a workflow run.
/// Wraps UUIDv7 for timestamp-ordered, non-interchangeable identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, schemars::JsonSchema)]
#[schemars(transparent)]
pub struct WorkflowRunId(Uuid);

impl WorkflowRunId {
    /// Create a new WorkflowRunId using UUIDv7 (timestamp-ordered).
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Get the inner UUID.
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Get the raw bytes for SQLite storage.
    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }

    /// Create a WorkflowRunId from raw bytes (for SQLite retrieval).
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }
}

impl Serialize for WorkflowRunId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for WorkflowRunId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let uuid = Uuid::parse_str(&s).map_err(serde::de::Error::custom)?;
        Ok(WorkflowRunId(uuid))
    }
}

impl fmt::Display for WorkflowRunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for WorkflowRunId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<WorkflowRunId> for Uuid {
    fn from(id: WorkflowRunId) -> Uuid {
        id.0
    }
}

impl FromStr for WorkflowRunId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(WorkflowRunId)
    }
}

impl Default for WorkflowRunId {
    fn default() -> Self {
        Self::new()
    }
}

/// Strongly typed identifier for a workflow step run.
/// Wraps UUIDv7 for timestamp-ordered, non-interchangeable identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, schemars::JsonSchema)]
#[schemars(transparent)]
pub struct WorkflowStepRunId(Uuid);

impl WorkflowStepRunId {
    /// Create a new WorkflowStepRunId using UUIDv7 (timestamp-ordered).
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Get the inner UUID.
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Get the raw bytes for SQLite storage.
    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }

    /// Create a WorkflowStepRunId from raw bytes (for SQLite retrieval).
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }
}

impl Serialize for WorkflowStepRunId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for WorkflowStepRunId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let uuid = Uuid::parse_str(&s).map_err(serde::de::Error::custom)?;
        Ok(WorkflowStepRunId(uuid))
    }
}

impl fmt::Display for WorkflowStepRunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for WorkflowStepRunId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<WorkflowStepRunId> for Uuid {
    fn from(id: WorkflowStepRunId) -> Uuid {
        id.0
    }
}

impl FromStr for WorkflowStepRunId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(WorkflowStepRunId)
    }
}

impl Default for WorkflowStepRunId {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_run_id_construction() {
        let id = WorkflowRunId::new();
        assert!(!id.as_bytes().iter().all(|&b| b == 0));
        assert_eq!(id.to_string(), id.as_uuid().to_string());
    }

    #[test]
    fn test_workflow_run_id_serde() {
        let id = WorkflowRunId::new();
        let json = serde_json::to_string(&id).unwrap();
        let parsed: WorkflowRunId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_workflow_run_id_bytes_roundtrip() {
        let id = WorkflowRunId::new();
        let bytes = *id.as_bytes();
        let reconstructed = WorkflowRunId::from_bytes(bytes);
        assert_eq!(id, reconstructed);
    }

    #[test]
    fn test_workflow_step_run_id_construction() {
        let id = WorkflowStepRunId::new();
        assert!(!id.as_bytes().iter().all(|&b| b == 0));
        assert_eq!(id.to_string(), id.as_uuid().to_string());
    }

    #[test]
    fn test_workflow_step_run_id_serde() {
        let id = WorkflowStepRunId::new();
        let json = serde_json::to_string(&id).unwrap();
        let parsed: WorkflowStepRunId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_workflow_step_run_id_bytes_roundtrip() {
        let id = WorkflowStepRunId::new();
        let bytes = *id.as_bytes();
        let reconstructed = WorkflowStepRunId::from_bytes(bytes);
        assert_eq!(id, reconstructed);
    }
}
