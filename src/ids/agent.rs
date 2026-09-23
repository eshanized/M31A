//! AgentId - strongly typed UUIDv7 identifier for agents.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Strongly typed identifier for an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, schemars::JsonSchema)]
#[schemars(transparent)]
pub struct AgentId(Uuid);

impl AgentId {
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

impl Serialize for AgentId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for AgentId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let uuid = Uuid::parse_str(&s).map_err(serde::de::Error::custom)?;
        Ok(AgentId(uuid))
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for AgentId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<AgentId> for Uuid {
    fn from(id: AgentId) -> Uuid {
        id.0
    }
}

impl FromStr for AgentId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(AgentId)
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_id_construction() {
        let id = AgentId::new();
        assert!(!id.as_bytes().iter().all(|&b| b == 0));
    }

    #[test]
    fn test_agent_id_hex_roundtrip() {
        let id = AgentId::new();
        let serialized = serde_json::to_string(&id).unwrap();
        let deserialized: AgentId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_agent_id_binary_roundtrip() {
        let id = AgentId::new();
        let bytes = *id.as_bytes();
        let reconstructed = AgentId::from_bytes(bytes);
        assert_eq!(id, reconstructed);
    }

    #[test]
    fn test_agent_id_display() {
        let id = AgentId::new();
        let display = format!("{}", id);
        let parsed: AgentId = display.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_agent_id_from_uuid() {
        let uuid = Uuid::now_v7();
        let id = AgentId::from(uuid);
        assert_eq!(id.as_uuid(), &uuid);
    }

    #[test]
    fn test_agent_id_into_uuid() {
        let id = AgentId::new();
        let uuid: Uuid = id.into();
        assert_eq!(uuid, *id.as_uuid());
    }
}
