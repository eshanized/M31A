//! CheckId - strongly typed UUIDv7 identifier for verification checks.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Strongly typed UUIDv7 identifier for a verification check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, schemars::JsonSchema)]
#[schemars(transparent)]
pub struct CheckId(Uuid);

impl CheckId {
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

impl Serialize for CheckId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for CheckId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let uuid = Uuid::parse_str(&s).map_err(serde::de::Error::custom)?;
        Ok(CheckId(uuid))
    }
}

impl fmt::Display for CheckId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for CheckId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<CheckId> for Uuid {
    fn from(id: CheckId) -> Uuid {
        id.0
    }
}

impl FromStr for CheckId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(CheckId)
    }
}

impl Default for CheckId {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_id_construction() {
        let id = CheckId::new();
        assert!(!id.as_bytes().iter().all(|&b| b == 0));
    }

    #[test]
    fn test_check_id_serde_roundtrip() {
        let id = CheckId::new();
        let serialized = serde_json::to_string(&id).unwrap();
        let deserialized: CheckId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_check_id_binary_roundtrip() {
        let id = CheckId::new();
        let bytes = *id.as_bytes();
        let reconstructed = CheckId::from_bytes(bytes);
        assert_eq!(id, reconstructed);
    }

    #[test]
    fn test_check_id_display() {
        let id = CheckId::new();
        let display = format!("{}", id);
        let parsed: CheckId = display.parse().unwrap();
        assert_eq!(id, parsed);
    }
}
