//! Session aggregate model (MSN-05, CONTEXT.md §43).

use crate::ids::{MissionId, SessionId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// State of an interaction session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SessionState {
    #[default]
    Active,
    Paused,
    Closed,
    Failed,
}

impl fmt::Display for SessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Paused => write!(f, "paused"),
            Self::Closed => write!(f, "closed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

impl std::str::FromStr for SessionState {
    type Err = crate::error::M31AError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "paused" => Ok(Self::Paused),
            "closed" => Ok(Self::Closed),
            "failed" => Ok(Self::Failed),
            other => Err(crate::error::M31AError::validation(format!(
                "unknown session state: {}",
                other
            ))),
        }
    }
}

/// Interaction/execution stream session associated with a mission.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub id: SessionId,
    pub mission_id: MissionId,
    pub status: SessionState,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

impl Session {
    /// Create a new active session for a mission.
    pub fn new(id: SessionId, mission_id: MissionId) -> Self {
        let now = Utc::now();
        Self {
            id,
            mission_id,
            status: SessionState::Active,
            metadata: serde_json::json!({}),
            created_at: now,
            updated_at: now,
            closed_at: None,
        }
    }

    /// Close the session gracefully.
    pub fn close(&mut self) {
        let now = Utc::now();
        self.status = SessionState::Closed;
        self.closed_at = Some(now);
        self.updated_at = now;
    }

    /// Fail the session due to an error.
    pub fn fail(&mut self) {
        let now = Utc::now();
        self.status = SessionState::Failed;
        self.closed_at = Some(now);
        self.updated_at = now;
    }

    /// Update session metadata JSON.
    pub fn update_metadata(&mut self, metadata: serde_json::Value) {
        self.metadata = metadata;
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_construction() {
        let id = SessionId::new();
        let mission_id = MissionId::new();
        let session = Session::new(id, mission_id);
        assert_eq!(session.id, id);
        assert_eq!(session.mission_id, mission_id);
        assert_eq!(session.status, SessionState::Active);
        assert!(session.closed_at.is_none());
    }

    #[test]
    fn test_session_close() {
        let mut session = Session::new(SessionId::new(), MissionId::new());
        session.close();
        assert_eq!(session.status, SessionState::Closed);
        assert!(session.closed_at.is_some());
    }

    #[test]
    fn test_session_serde_roundtrip() {
        let mut session = Session::new(SessionId::new(), MissionId::new());
        session.update_metadata(serde_json::json!({"client": "tui", "theme": "dark"}));
        let json = serde_json::to_string(&session).unwrap();
        let parsed: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(session, parsed);
    }
}
