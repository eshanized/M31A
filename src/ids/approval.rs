//! ApprovalRequestId - strongly typed UUIDv7 identifier for interactive approval requests.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// Strongly typed identifier for an interactive approval request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ApprovalRequestId(Uuid);

impl ApprovalRequestId {
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

    pub fn parse(s: &str) -> Result<Self, uuid::Error> {
        s.parse()
    }
}

impl Serialize for ApprovalRequestId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for ApprovalRequestId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let uuid = Uuid::parse_str(&s).map_err(serde::de::Error::custom)?;
        Ok(ApprovalRequestId(uuid))
    }
}

impl fmt::Display for ApprovalRequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for ApprovalRequestId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<ApprovalRequestId> for Uuid {
    fn from(id: ApprovalRequestId) -> Uuid {
        id.0
    }
}

impl FromStr for ApprovalRequestId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(ApprovalRequestId)
    }
}

impl Default for ApprovalRequestId {
    fn default() -> Self {
        Self::new()
    }
}
