//! Shared error types for deterministic aggregate event replay.

use thiserror::Error;

/// Failure encountered while applying a durable event to an aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReplayError {
    #[error("Event sequence {event_seq} is out of order for aggregate watermark {current_seq}")]
    OutOfOrderSequence { current_seq: u64, event_seq: u64 },
    #[error("Invalid state transition during replay from {from} via {event}")]
    InvalidReplayTransition { from: String, event: String },
}
