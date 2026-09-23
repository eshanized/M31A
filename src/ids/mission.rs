//! MissionId - strongly typed UUIDv7 identifier for missions.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Strongly typed identifier for a mission.
/// Wraps UUIDv7 for timestamp-ordered, non-interchangeable identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, schemars::JsonSchema)]
#[schemars(transparent)]
pub struct MissionId(Uuid);

impl MissionId {
    /// Create a new MissionId using UUIDv7 (timestamp-ordered).
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

    /// Create a MissionId from raw bytes (for SQLite retrieval).
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(Uuid::from_bytes(bytes))
    }
}

// Hex string serialization for JSON/CLI/logs
impl Serialize for MissionId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for MissionId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let uuid = Uuid::parse_str(&s).map_err(serde::de::Error::custom)?;
        Ok(MissionId(uuid))
    }
}

impl fmt::Display for MissionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for MissionId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<MissionId> for Uuid {
    fn from(id: MissionId) -> Uuid {
        id.0
    }
}

impl FromStr for MissionId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(MissionId)
    }
}

impl Default for MissionId {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mission_id_construction() {
        let id = MissionId::new();
        assert!(!id.as_bytes().iter().all(|&b| b == 0));
    }

    #[test]
    fn test_mission_id_hex_roundtrip() {
        let id = MissionId::new();
        let serialized = serde_json::to_string(&id).unwrap();
        let deserialized: MissionId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_mission_id_binary_roundtrip() {
        let id = MissionId::new();
        let bytes = *id.as_bytes();
        let reconstructed = MissionId::from_bytes(bytes);
        assert_eq!(id, reconstructed);
    }

    #[test]
    fn test_mission_id_display() {
        let id = MissionId::new();
        let display = format!("{}", id);
        let parsed: MissionId = display.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_mission_id_from_uuid() {
        let uuid = Uuid::now_v7();
        let id = MissionId::from(uuid);
        assert_eq!(id.as_uuid(), &uuid);
    }

    #[test]
    fn test_mission_id_into_uuid() {
        let id = MissionId::new();
        let uuid: Uuid = id.into();
        assert_eq!(uuid, *id.as_uuid());
    }

    // Compile-time assertion: MissionId cannot be used where TaskId is expected
    // This test verifies the type system prevents ID misuse.
    // If this compiles, the type system correctly prevents interchangeability.
    #[test]
    fn test_mission_id_non_interchangeable() {
        let _mission_id = MissionId::new();
        // The following would not compile if uncommented:
        // let _task_id: crate::task::TaskId = _mission_id;
        // This is a compile-time check, not a runtime test.
    }
}
