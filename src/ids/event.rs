//! EventId - strongly typed UUIDv7 identifier for events.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Strongly typed identifier for an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, schemars::JsonSchema)]
#[schemars(transparent)]
pub struct EventId(Uuid);

impl EventId {
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

impl Serialize for EventId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for EventId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let uuid = Uuid::parse_str(&s).map_err(serde::de::Error::custom)?;
        Ok(EventId(uuid))
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for EventId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<EventId> for Uuid {
    fn from(id: EventId) -> Uuid {
        id.0
    }
}

impl FromStr for EventId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(EventId)
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_id_construction() {
        let id = EventId::new();
        assert!(!id.as_bytes().iter().all(|&b| b == 0));
    }

    #[test]
    fn test_event_id_hex_roundtrip() {
        let id = EventId::new();
        let serialized = serde_json::to_string(&id).unwrap();
        let deserialized: EventId = serde_json::from_str(&serialized).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_event_id_binary_roundtrip() {
        let id = EventId::new();
        let bytes = *id.as_bytes();
        let reconstructed = EventId::from_bytes(bytes);
        assert_eq!(id, reconstructed);
    }

    #[test]
    fn test_event_id_display() {
        let id = EventId::new();
        let display = format!("{}", id);
        let parsed: EventId = display.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_event_id_from_uuid() {
        let uuid = Uuid::now_v7();
        let id = EventId::from(uuid);
        assert_eq!(id.as_uuid(), &uuid);
    }

    #[test]
    fn test_event_id_into_uuid() {
        let id = EventId::new();
        let uuid: Uuid = id.into();
        assert_eq!(uuid, *id.as_uuid());
    }
}
