//! Event envelope with metadata, causal tracking, and categorization (D-13, EVT-01, EVT-02).

use crate::events::types::EventType;
use crate::ids::{EventId, MissionId, SessionId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Categorization of events across the runtime (EVT-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EventCategory {
    /// Durable events that must be persisted to SQLite and survive restart.
    #[default]
    Durable,
    /// Ephemeral live progress and streaming events.
    Ephemeral,
    /// Diagnostic telemetry and debugging traces.
    Diagnostic,
}

impl std::fmt::Display for EventCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Durable => write!(f, "durable"),
            Self::Ephemeral => write!(f, "ephemeral"),
            Self::Diagnostic => write!(f, "diagnostic"),
        }
    }
}

impl std::str::FromStr for EventCategory {
    type Err = crate::error::M31AError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "durable" => Ok(Self::Durable),
            "ephemeral" => Ok(Self::Ephemeral),
            "diagnostic" => Ok(Self::Diagnostic),
            other => Err(crate::error::M31AError::validation(format!(
                "unknown event category: {}",
                other
            ))),
        }
    }
}

/// Event envelope containing event metadata, payload, and causal provenance.
///
/// Per D-13: Retains EventId, sequence, timestamp, mission_id, session_id, actor,
/// schema_version, with optional correlation_id and causation_id for causal tracing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventEnvelope {
    pub id: EventId,
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
    pub mission_id: Option<MissionId>,
    pub session_id: Option<SessionId>,
    pub actor: String,
    pub event_type: EventType,
    pub category: EventCategory,
    pub correlation_id: Option<String>,
    pub causation_id: Option<EventId>,
    pub schema_version: u32,
}

impl EventEnvelope {
    /// Create a new event envelope with generated ID and current timestamp.
    /// Defaults schema_version to 1 and category to Durable.
    pub fn new(
        sequence: u64,
        mission_id: Option<MissionId>,
        session_id: Option<SessionId>,
        actor: String,
        event_type: EventType,
    ) -> Self {
        Self {
            id: EventId::new(),
            sequence,
            timestamp: Utc::now(),
            mission_id,
            session_id,
            actor,
            event_type,
            category: EventCategory::Durable,
            correlation_id: None,
            causation_id: None,
            schema_version: 1,
        }
    }

    /// Create a new event envelope with a specific schema version.
    pub fn with_schema_version(
        sequence: u64,
        mission_id: Option<MissionId>,
        session_id: Option<SessionId>,
        actor: String,
        event_type: EventType,
        schema_version: u32,
    ) -> Self {
        let mut envelope = Self::new(sequence, mission_id, session_id, actor, event_type);
        envelope.schema_version = schema_version;
        envelope
    }

    /// Builder method to attach causal tracking identifiers (D-13).
    pub fn with_causation(
        mut self,
        correlation_id: Option<String>,
        causation_id: Option<EventId>,
    ) -> Self {
        self.correlation_id = correlation_id;
        self.causation_id = causation_id;
        self
    }

    /// Builder method to set the event category (EVT-01).
    pub fn with_category(mut self, category: EventCategory) -> Self {
        self.category = category;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::types::EventType;
    use crate::ids::{MissionId, SessionId};

    #[test]
    fn test_event_envelope_new() {
        let mission_id = MissionId::new();
        let envelope = EventEnvelope::new(
            1,
            Some(mission_id),
            None,
            "test-actor".to_string(),
            EventType::MissionStarted {
                mission_id,
                objective: "test".to_string(),
            },
        );

        assert_eq!(envelope.sequence, 1);
        assert_eq!(envelope.mission_id, Some(mission_id));
        assert_eq!(envelope.actor, "test-actor");
        assert_eq!(envelope.category, EventCategory::Durable);
        assert!(envelope.correlation_id.is_none());
        assert!(envelope.causation_id.is_none());
        assert_eq!(envelope.schema_version, 1);

        let now = Utc::now();
        let diff = (now - envelope.timestamp).num_seconds().abs();
        assert!(diff < 5, "Timestamp should be within 5 seconds of now");
    }

    #[test]
    fn test_event_envelope_causation_and_category() {
        let mission_id = MissionId::new();
        let causation_id = EventId::new();
        let envelope = EventEnvelope::new(
            10,
            Some(mission_id),
            None,
            "orchestrator".to_string(),
            EventType::MissionCompleted { mission_id },
        )
        .with_causation(Some("corr-123".to_string()), Some(causation_id))
        .with_category(EventCategory::Diagnostic);

        assert_eq!(envelope.correlation_id.as_deref(), Some("corr-123"));
        assert_eq!(envelope.causation_id, Some(causation_id));
        assert_eq!(envelope.category, EventCategory::Diagnostic);
    }

    #[test]
    fn test_event_envelope_serde_roundtrip() {
        let mission_id = MissionId::new();
        let causation_id = EventId::new();
        let envelope = EventEnvelope::new(
            42,
            Some(mission_id),
            Some(SessionId::new()),
            "agent-1".to_string(),
            EventType::TaskCompleted {
                task_id: crate::ids::TaskId::new(),
                mission_id,
                result: "done".to_string(),
            },
        )
        .with_causation(Some("corr-xyz".to_string()), Some(causation_id))
        .with_category(EventCategory::Ephemeral);

        let json = serde_json::to_string(&envelope).unwrap();
        let parsed: EventEnvelope = serde_json::from_str(&json).unwrap();

        assert_eq!(envelope.id, parsed.id);
        assert_eq!(envelope.sequence, parsed.sequence);
        assert_eq!(envelope.mission_id, parsed.mission_id);
        assert_eq!(envelope.session_id, parsed.session_id);
        assert_eq!(envelope.actor, parsed.actor);
        assert_eq!(envelope.category, parsed.category);
        assert_eq!(envelope.correlation_id, parsed.correlation_id);
        assert_eq!(envelope.causation_id, parsed.causation_id);
        assert_eq!(envelope.schema_version, parsed.schema_version);
        assert_eq!(envelope.event_type.name(), parsed.event_type.name());
    }

    #[test]
    fn test_event_envelope_generates_unique_ids() {
        let envelope1 = EventEnvelope::new(
            1,
            None,
            None,
            "a".to_string(),
            EventType::MissionCompleted {
                mission_id: MissionId::new(),
            },
        );
        let envelope2 = EventEnvelope::new(
            2,
            None,
            None,
            "b".to_string(),
            EventType::MissionCompleted {
                mission_id: MissionId::new(),
            },
        );
        assert_ne!(envelope1.id, envelope2.id);
    }
}
