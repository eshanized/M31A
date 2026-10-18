//! Shared lifecycle behavior for Mission and ExecutionRun aggregates.

use crate::error::M31AError;
use crate::events::types::EventType;
use crate::ids::MissionId;
use crate::state::completion::{CompletionContext, CompletionGate};
use crate::state_machine::ExecutionRunState;
use chrono::{DateTime, Utc};

/// Domain label used to preserve entity-specific lifecycle event messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LifecycleEntity {
    Mission,
    ExecutionRun,
}

impl LifecycleEntity {
    fn label(self) -> &'static str {
        match self {
            Self::Mission => "Mission",
            Self::ExecutionRun => "Execution run",
        }
    }
}

/// Apply the shared timestamp semantics when an aggregate changes state.
pub(crate) fn update_status(
    status: &mut ExecutionRunState,
    started_at: &mut Option<DateTime<Utc>>,
    completed_at: &mut Option<DateTime<Utc>>,
    updated_at: &mut DateTime<Utc>,
    next_status: ExecutionRunState,
) {
    if started_at.is_none() && next_status != ExecutionRunState::Created {
        *started_at = Some(Utc::now());
    }
    if next_status.is_terminal() && completed_at.is_none() {
        *completed_at = Some(Utc::now());
    }
    *status = next_status;
    *updated_at = Utc::now();
}

/// Update the event-replay watermark and refresh the aggregate timestamp.
pub(crate) fn update_watermark(
    last_applied_sequence: &mut u64,
    updated_at: &mut DateTime<Utc>,
    sequence: u64,
) {
    *last_applied_sequence = sequence;
    *updated_at = Utc::now();
}

/// Enforce the completion gate consistently for both lifecycle services.
pub(crate) fn enforce_completion_gate(
    next_state: ExecutionRunState,
    context: Option<&CompletionContext>,
    entity: LifecycleEntity,
) -> Result<(), M31AError> {
    if next_state != ExecutionRunState::Completed {
        return Ok(());
    }

    let entity_name = match entity {
        LifecycleEntity::Mission => "Mission",
        LifecycleEntity::ExecutionRun => "ExecutionRun",
    };
    let context = context.ok_or_else(|| {
        M31AError::validation(format!(
            "{entity_name} completion requires a verified CompletionContext"
        ))
    })?;

    CompletionGate::evaluate(context).map_err(|errors| {
        M31AError::validation(format!(
            "Completion gate verification failed: {:?}",
            errors
        ))
    })
}

/// Build the lifecycle event for a validated state transition.
///
/// Mission and ExecutionRun share the event vocabulary, while failure/reason
/// strings retain their externally visible entity-specific wording.
pub(crate) fn event_for_transition(
    mission_id: MissionId,
    previous_state: ExecutionRunState,
    next_state: ExecutionRunState,
    actor: &str,
    entity: LifecycleEntity,
) -> EventType {
    match next_state {
        ExecutionRunState::Completed => EventType::MissionCompleted { mission_id },
        ExecutionRunState::Failed => EventType::MissionFailed {
            mission_id,
            reason: format!("{} marked failed by actor {}", entity.label(), actor),
        },
        ExecutionRunState::Cancelled => EventType::MissionCancelled {
            mission_id,
            reason: format!("{} cancelled by actor {}", entity.label(), actor),
        },
        ExecutionRunState::Paused => EventType::MissionPaused {
            mission_id,
            reason: format!("{} paused by actor {}", entity.label(), actor),
        },
        ExecutionRunState::Executing if previous_state == ExecutionRunState::Paused => {
            EventType::MissionResumed { mission_id }
        }
        _ => EventType::MissionStateChanged {
            mission_id,
            from: previous_state.to_string(),
            to: next_state.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_for_transition_preserves_entity_specific_failure_reasons() {
        let mission_id = MissionId::new();

        assert_eq!(
            event_for_transition(
                mission_id,
                ExecutionRunState::Executing,
                ExecutionRunState::Failed,
                "operator",
                LifecycleEntity::Mission,
            ),
            EventType::MissionFailed {
                mission_id,
                reason: "Mission marked failed by actor operator".to_owned(),
            }
        );
        assert_eq!(
            event_for_transition(
                mission_id,
                ExecutionRunState::Executing,
                ExecutionRunState::Failed,
                "operator",
                LifecycleEntity::ExecutionRun,
            ),
            EventType::MissionFailed {
                mission_id,
                reason: "Execution run marked failed by actor operator".to_owned(),
            }
        );
    }

    #[test]
    fn paused_to_executing_emits_resume_event() {
        let mission_id = MissionId::new();

        assert_eq!(
            event_for_transition(
                mission_id,
                ExecutionRunState::Paused,
                ExecutionRunState::Executing,
                "operator",
                LifecycleEntity::Mission,
            ),
            EventType::MissionResumed { mission_id }
        );
    }
}
