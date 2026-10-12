//! Session aggregate model (MSN-05, CONTEXT.md §43).

use crate::ids::{ExecutionRunId, MissionId, SessionId};
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
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

/// Interaction/execution stream session.
///
/// Sessions exist independently of any individual execution run,
/// providing durable multi-turn context and command routing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub id: SessionId,
    #[serde(default)]
    pub active_run_id: Option<ExecutionRunId>,
    #[serde(default)]
    pub mission_id: Option<MissionId>,
    pub status: SessionState,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
}

impl Session {
    /// Create a new standalone active session (independent of any run/mission).
    pub fn new(id: SessionId) -> Self {
        let now = Utc::now();
        Self {
            id,
            active_run_id: None,
            mission_id: None,
            status: SessionState::Active,
            metadata: serde_json::json!({}),
            created_at: now,
            updated_at: now,
            closed_at: None,
        }
    }

    /// Create an active session with an associated execution run.
    pub fn for_run(id: SessionId, run_id: ExecutionRunId) -> Self {
        let mut s = Self::new(id);
        s.active_run_id = Some(run_id);
        s.mission_id = Some(MissionId::from(*run_id.as_uuid()));
        s
    }

    /// Create an active session for a mission (backward compatibility).
    pub fn for_mission(id: SessionId, mission_id: MissionId) -> Self {
        let mut s = Self::new(id);
        s.mission_id = Some(mission_id);
        s.active_run_id = Some(ExecutionRunId::from(*mission_id.as_uuid()));
        s
    }

    /// Backward-compatible constructor accepting both session_id and mission_id.
    pub fn new_with_mission(id: SessionId, mission_id: MissionId) -> Self {
        Self::for_mission(id, mission_id)
    }

    /// Attach an execution run to this session.
    pub fn attach_run(&mut self, run_id: ExecutionRunId) {
        self.active_run_id = Some(run_id);
        self.mission_id = Some(MissionId::from(*run_id.as_uuid()));
        self.updated_at = Utc::now();
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
    fn test_standalone_session_construction() {
        let id = SessionId::new();
        let session = Session::new(id);
        assert_eq!(session.id, id);
        assert!(session.active_run_id.is_none());
        assert!(session.mission_id.is_none());
        assert_eq!(session.status, SessionState::Active);
        assert!(session.closed_at.is_none());
    }

    #[test]
    fn test_session_for_run_and_mission() {
        let id = SessionId::new();
        let run_id = ExecutionRunId::new();
        let mut session = Session::for_run(id, run_id);
        assert_eq!(session.active_run_id, Some(run_id));
        assert!(session.mission_id.is_some());

        let new_run = ExecutionRunId::new();
        session.attach_run(new_run);
        assert_eq!(session.active_run_id, Some(new_run));
    }

    #[test]
    fn test_session_close() {
        let mut session = Session::new(SessionId::new());
        session.close();
        assert_eq!(session.status, SessionState::Closed);
        assert!(session.closed_at.is_some());
    }

    #[test]
    fn test_session_serde_roundtrip() {
        let mut session = Session::new(SessionId::new());
        session.update_metadata(serde_json::json!({"client": "tui", "theme": "dark"}));
        let json = serde_json::to_string(&session).unwrap();
        let parsed: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(session, parsed);
    }
}
