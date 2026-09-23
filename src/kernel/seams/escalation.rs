use async_trait::async_trait;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;
use uuid::Uuid;

use super::policy::PolicyDecision;
use crate::events::types::EventType;
use crate::ids::{MissionId, TaskId};

/// Strongly typed UUIDv7 identifier for operator escalation requests (D-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EscalationRequestId(Uuid);

impl EscalationRequestId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for EscalationRequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EscalationRequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for EscalationRequestId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let uuid = Uuid::parse_str(s)?;
        Ok(Self(uuid))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EscalationRequest {
    pub id: EscalationRequestId,
    pub mission_id: MissionId,
    pub task_id: Option<TaskId>,
    pub reason: String,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EscalationResponse {
    Approved,
    Denied { reason: String },
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum EscalationError {
    #[error("escalation channel error: {0}")]
    Failed(String),
}

/// Durable representation of an in-flight operator escalation request (D-14, D-15).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PendingEscalation {
    pub request: EscalationRequest,
    pub requested_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl PendingEscalation {
    pub fn new(request: EscalationRequest, requested_at: DateTime<Utc>) -> Self {
        let expires_at = request
            .timeout_seconds
            .map(|secs| requested_at + ChronoDuration::seconds(secs as i64));
        Self {
            request,
            requested_at,
            expires_at,
        }
    }

    /// Evaluates if the pending escalation has exceeded its timeout deadline.
    pub fn evaluate_timeout(&self, now: DateTime<Utc>) -> bool {
        self.expires_at.is_some_and(|exp| now >= exp)
    }

    /// Resolves an expired escalation to an effective DENY and generates EscalationTimedOut event (D-15, Edge 6).
    pub fn resolve_timeout(&self) -> (PolicyDecision, EventType) {
        (
            PolicyDecision::Deny,
            EventType::EscalationTimedOut {
                request_id: self.request.id.to_string(),
                mission_id: self.request.mission_id,
            },
        )
    }
}

#[async_trait]
pub trait EscalationChannel: Send + Sync {
    async fn request_escalation(
        &self,
        req: EscalationRequest,
    ) -> Result<EscalationRequestId, EscalationError>;
    async fn check_response(
        &self,
        id: EscalationRequestId,
    ) -> Result<Option<EscalationResponse>, EscalationError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escalation_request_id_display_and_parse() {
        let id = EscalationRequestId::new();
        let formatted = format!("{id}");
        let parsed: EscalationRequestId = formatted.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_escalation_request_serde() {
        let req = EscalationRequest {
            id: EscalationRequestId::new(),
            mission_id: MissionId::new(),
            task_id: Some(TaskId::new()),
            reason: "Mutating file outside workspace".to_string(),
            timeout_seconds: Some(300),
        };
        let serialized = serde_json::to_string(&req).unwrap();
        let deserialized: EscalationRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(req, deserialized);
    }

    #[test]
    fn test_pending_escalation_timeout_and_resolution() {
        let start = Utc::now();
        let req = EscalationRequest {
            id: EscalationRequestId::new(),
            mission_id: MissionId::new(),
            task_id: Some(TaskId::new()),
            reason: "Elevated privileges required".into(),
            timeout_seconds: Some(60),
        };

        let pending = PendingEscalation::new(req.clone(), start);
        assert!(!pending.evaluate_timeout(start));
        assert!(!pending.evaluate_timeout(start + ChronoDuration::seconds(59)));
        assert!(pending.evaluate_timeout(start + ChronoDuration::seconds(60)));
        assert!(pending.evaluate_timeout(start + ChronoDuration::seconds(100)));

        // Verify fail-safe timeout converts to effective DENY (Edge 6)
        let (decision, event) = pending.resolve_timeout();
        assert_eq!(decision, PolicyDecision::Deny);
        assert_eq!(
            event,
            EventType::EscalationTimedOut {
                request_id: req.id.to_string(),
                mission_id: req.mission_id,
            }
        );
    }

    #[test]
    fn test_pending_escalation_without_timeout_never_expires() {
        let start = Utc::now();
        let req = EscalationRequest {
            id: EscalationRequestId::new(),
            mission_id: MissionId::new(),
            task_id: None,
            reason: "Indefinite escalation".into(),
            timeout_seconds: None,
        };

        let pending = PendingEscalation::new(req, start);
        assert!(!pending.evaluate_timeout(start + ChronoDuration::days(365)));
    }

    #[test]
    fn test_escalation_response_serde() {
        let resp_approved = EscalationResponse::Approved;
        let s = serde_json::to_string(&resp_approved).unwrap();
        assert_eq!(s, "\"approved\"");
        let deserialized: EscalationResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(resp_approved, deserialized);

        let resp_denied = EscalationResponse::Denied {
            reason: "User denied".into(),
        };
        let s2 = serde_json::to_string(&resp_denied).unwrap();
        let deserialized2: EscalationResponse = serde_json::from_str(&s2).unwrap();
        assert_eq!(resp_denied, deserialized2);
    }
}
