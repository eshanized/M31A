//! CheckpointId - strongly typed UUIDv7 identifier for checkpoints.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Strongly typed identifier for a checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, schemars::JsonSchema)]
#[schemars(transparent)]
pub struct CheckpointId(Uuid);

impl CheckpointId {
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

impl Serialize for CheckpointId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for CheckpointId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let uuid = Uuid::parse_str(&s).map_err(serde::de::Error::custom)?;
        Ok(CheckpointId(uuid))
    }
}

impl fmt::Display for CheckpointId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for CheckpointId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<CheckpointId> for Uuid {
    fn from(id: CheckpointId) -> Uuid {
        id.0
    }
}

impl FromStr for CheckpointId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(CheckpointId)
    }
}

impl Default for CheckpointId {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_id_construction() {
        let id = CheckpointId::new();
        assert!(!id.as_bytes().iter().all(|&b| b == 0));
    }

    #[test]
    fn test_checkpoint_id_hex_roundtrip() {
        let id = CheckpointId::new();
        let serialized = serde_json::to_string(&id).unwrap();
        let deserialized: CheckpointId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_checkpoint_id_binary_roundtrip() {
        let id = CheckpointId::new();
        let bytes = *id.as_bytes();
        let reconstructed = CheckpointId::from_bytes(bytes);
        assert_eq!(id, reconstructed);
    }

    #[test]
    fn test_checkpoint_id_display() {
        let id = CheckpointId::new();
        let display = format!("{}", id);
        let parsed: CheckpointId = display.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_checkpoint_id_from_uuid() {
        let uuid = Uuid::now_v7();
        let id = CheckpointId::from(uuid);
        assert_eq!(id.as_uuid(), &uuid);
    }

    #[test]
    fn test_checkpoint_id_into_uuid() {
        let id = CheckpointId::new();
        let uuid: Uuid = id.into();
        assert_eq!(uuid, *id.as_uuid());
    }
}
