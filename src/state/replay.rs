//! Shared error types for deterministic aggregate event replay.

use thiserror::Error;

use chrono::{DateTime, Utc};

use crate::events::envelope::EventEnvelope;
use crate::events::types::EventType;
use crate::state_machine::ExecutionRunState;

/// Apply the lifecycle effects of a durable event to a shared aggregate state.
///
/// The Mission and ExecutionRun aggregates share the same lifecycle vocabulary;
/// entity-specific fields such as parent/session identity remain in the aggregates.
pub(crate) fn apply_lifecycle_event(
    status: &mut ExecutionRunState,
    started_at: &mut Option<DateTime<Utc>>,
    completed_at: &mut Option<DateTime<Utc>>,
    last_applied_sequence: &mut u64,
    updated_at: &mut DateTime<Utc>,
    envelope: &EventEnvelope,
) {
    match &envelope.event_type {
        EventType::MissionStarted { .. } => {
            *status = ExecutionRunState::Understanding;
            if started_at.is_none() {
                *started_at = Some(envelope.timestamp);
            }
        }
        EventType::MissionCompleted { .. } => {
            *status = ExecutionRunState::Completed;
            *completed_at = Some(envelope.timestamp);
        }
        EventType::MissionFailed { .. } => {
            *status = ExecutionRunState::Failed;
            *completed_at = Some(envelope.timestamp);
        }
        EventType::MissionCancelled { .. } => {
            *status = ExecutionRunState::Cancelled;
            *completed_at = Some(envelope.timestamp);
        }
        EventType::MissionPaused { .. } => {
            *status = ExecutionRunState::Paused;
        }
        EventType::MissionResumed { .. } => {
            *status = ExecutionRunState::Executing;
        }
        EventType::MissionStateChanged { to, .. } => {
            if let Ok(next_status) = to.parse::<ExecutionRunState>() {
                *status = next_status;
                if next_status.is_terminal() {
                    *completed_at = Some(envelope.timestamp);
                }
            }
        }
        _ => {}
    }

    *last_applied_sequence = envelope.sequence;
    *updated_at = envelope.timestamp;
}

/// Failure encountered while applying a durable event to an aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReplayError {
    #[error("Event sequence {event_seq} is out of order for aggregate watermark {current_seq}")]
    OutOfOrderSequence { current_seq: u64, event_seq: u64 },
    #[error("Invalid state transition during replay from {from} via {event}")]
    InvalidReplayTransition { from: String, event: String },
}
