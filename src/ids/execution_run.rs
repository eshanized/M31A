//! ExecutionRunId - strongly typed UUIDv7 identifier for execution runs.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Strongly typed identifier for an execution run.
/// Wraps UUIDv7 for timestamp-ordered, non-interchangeable identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, schemars::JsonSchema)]
#[schemars(transparent)]
pub struct ExecutionRunId(Uuid);

impl ExecutionRunId {
    /// Create a new ExecutionRunId using UUIDv7 (timestamp-ordered).
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

    /// Create an ExecutionRunId from raw bytes (for SQLite retrieval).
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }
}

// Hex string serialization for JSON/CLI/logs
impl Serialize for ExecutionRunId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for ExecutionRunId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let uuid = Uuid::parse_str(&s).map_err(serde::de::Error::custom)?;
        Ok(ExecutionRunId(uuid))
    }
}

impl fmt::Display for ExecutionRunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for ExecutionRunId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<ExecutionRunId> for Uuid {
    fn from(id: ExecutionRunId) -> Uuid {
        id.0
    }
}

impl FromStr for ExecutionRunId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(ExecutionRunId)
    }
}

impl Default for ExecutionRunId {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_run_id_construction() {
        let id = ExecutionRunId::new();
        assert!(!id.as_bytes().iter().all(|&b| b == 0));
    }

    #[test]
    fn test_execution_run_id_hex_roundtrip() {
        let id = ExecutionRunId::new();
        let serialized = serde_json::to_string(&id).unwrap();
        let deserialized: ExecutionRunId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_execution_run_id_bytes_roundtrip() {
        let id = ExecutionRunId::new();
        let bytes = *id.as_bytes();
        let recovered = ExecutionRunId::from_bytes(bytes);
        assert_eq!(id, recovered);
    }

    #[test]
    fn test_execution_run_id_display_and_from_str() {
        let id = ExecutionRunId::new();
        let display = id.to_string();
        let parsed: ExecutionRunId = display.parse().unwrap();
        assert_eq!(id, parsed);
    }
}
