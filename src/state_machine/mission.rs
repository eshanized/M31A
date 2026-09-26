//! Mission lifecycle state machine

use crate::state_machine::error::TransitionError;
use serde::{Deserialize, Serialize};

/// Mission lifecycle states representing stages from inception to completion or failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MissionState {
    Created,
    Understanding,
    Researching,
    Planning,
    Scheduled,
    Executing,
    Verifying,
    Reviewing,
    Integrating,
    Shipping,
    Completed,
    Paused,
    Blocked,
    Failed,
    Cancelled,
}

impl std::fmt::Display for MissionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Created => write!(f, "Created"),
            Self::Understanding => write!(f, "Understanding"),
            Self::Researching => write!(f, "Researching"),
            Self::Planning => write!(f, "Planning"),
            Self::Scheduled => write!(f, "Scheduled"),
            Self::Executing => write!(f, "Executing"),
            Self::Verifying => write!(f, "Verifying"),
            Self::Reviewing => write!(f, "Reviewing"),
            Self::Integrating => write!(f, "Integrating"),
            Self::Shipping => write!(f, "Shipping"),
            Self::Completed => write!(f, "Completed"),
            Self::Paused => write!(f, "Paused"),
            Self::Blocked => write!(f, "Blocked"),
            Self::Failed => write!(f, "Failed"),
            Self::Cancelled => write!(f, "Cancelled"),
        }
    }
}

impl std::str::FromStr for MissionState {
    type Err = TransitionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "created" => Ok(Self::Created),
            "understanding" => Ok(Self::Understanding),
            "researching" => Ok(Self::Researching),
            "planning" => Ok(Self::Planning),
            "scheduled" => Ok(Self::Scheduled),
            "executing" => Ok(Self::Executing),
            "verifying" => Ok(Self::Verifying),
            "reviewing" => Ok(Self::Reviewing),
            "integrating" => Ok(Self::Integrating),
            "shipping" => Ok(Self::Shipping),
            "completed" => Ok(Self::Completed),
            "paused" => Ok(Self::Paused),
            "blocked" => Ok(Self::Blocked),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(TransitionError::invalid_transition(s, "UnknownState")),
        }
    }
}

/// Events that can trigger mission state transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MissionEvent {
    Start,
    UnderstandComplete,
    ResearchComplete,
    PlanComplete,
    Schedule,
    Execute,
    Verify,
    Review,
    Integrate,
    Ship,
    Complete,
    Pause,
    Resume,
    Block,
    Unblock,
    Fail,
    Cancel,
}

impl MissionState {
    /// Check if this state is terminal (no further transitions allowed)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            MissionState::Completed | MissionState::Failed | MissionState::Cancelled
        )
    }

    /// Get the target state for an event (used for error reporting)
    pub fn event_target_state(event: MissionEvent) -> Option<MissionState> {
        match event {
            MissionEvent::Start => Some(MissionState::Understanding),
            MissionEvent::UnderstandComplete => Some(MissionState::Researching),
            MissionEvent::ResearchComplete => Some(MissionState::Planning),
            MissionEvent::PlanComplete => Some(MissionState::Scheduled),
            MissionEvent::Schedule => Some(MissionState::Scheduled),
            MissionEvent::Execute => Some(MissionState::Executing),
            MissionEvent::Verify => Some(MissionState::Verifying),
            MissionEvent::Review => Some(MissionState::Reviewing),
            MissionEvent::Integrate => Some(MissionState::Integrating),
            MissionEvent::Ship => Some(MissionState::Shipping),
            MissionEvent::Complete => Some(MissionState::Completed),
            MissionEvent::Pause => Some(MissionState::Paused),
            MissionEvent::Resume => Some(MissionState::Executing),
            MissionEvent::Block => Some(MissionState::Blocked),
            MissionEvent::Unblock => None, // Returns to previous state
            MissionEvent::Fail => Some(MissionState::Failed),
            MissionEvent::Cancel => Some(MissionState::Cancelled),
        }
    }
}

/// Validate and execute a mission state transition.
/// Returns the new state on success, or a TransitionError on failure.
pub fn transition_mission(
    current: MissionState,
    event: MissionEvent,
) -> Result<MissionState, TransitionError> {
    // Terminal states cannot transition
    if current.is_terminal() {
        return Err(TransitionError::terminal_state(format!("{:?}", current)));
    }

    let new_state = match (current, event) {
        // Forward progression
        (MissionState::Created, MissionEvent::Start) => MissionState::Understanding,
        (MissionState::Understanding, MissionEvent::UnderstandComplete) => {
            MissionState::Researching
        }
        (MissionState::Researching, MissionEvent::ResearchComplete) => MissionState::Planning,
        (MissionState::Planning, MissionEvent::PlanComplete) => MissionState::Scheduled,
        (MissionState::Scheduled, MissionEvent::Execute) => MissionState::Executing,
        (MissionState::Executing, MissionEvent::Verify) => MissionState::Verifying,
        (MissionState::Verifying, MissionEvent::Review) => MissionState::Reviewing,
        (MissionState::Reviewing, MissionEvent::Integrate) => MissionState::Integrating,
        (MissionState::Integrating, MissionEvent::Ship) => MissionState::Shipping,
        (MissionState::Shipping, MissionEvent::Complete) => MissionState::Completed,

        // Pause/Resume
        (MissionState::Executing, MissionEvent::Pause) => MissionState::Paused,
        (MissionState::Paused, MissionEvent::Resume) => MissionState::Executing,

        // Block/Unblock (Unblock returns to previous non-blocked state)
        (state, MissionEvent::Block) if !state.is_terminal() && state != MissionState::Blocked => {
            MissionState::Blocked
        }
        (MissionState::Blocked, MissionEvent::Unblock) => MissionState::Executing, // Simplified: return to Executing

        // Failure/Cancellation from any non-terminal state
        (state, MissionEvent::Fail) if !state.is_terminal() => MissionState::Failed,
        (state, MissionEvent::Cancel) if !state.is_terminal() => MissionState::Cancelled,

        // Invalid transitions
        (from, event) => {
            let target = MissionState::event_target_state(event).unwrap_or(from);
            return Err(TransitionError::invalid_transition(
                format!("{:?}", from),
                format!("{:?}", target),
            ));
        }
    };

    Ok(new_state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_forward_transitions() {
        assert_eq!(
            transition_mission(MissionState::Created, MissionEvent::Start),
            Ok(MissionState::Understanding)
        );
        assert_eq!(
            transition_mission(
                MissionState::Understanding,
                MissionEvent::UnderstandComplete
            ),
            Ok(MissionState::Researching)
        );
        assert_eq!(
            transition_mission(MissionState::Researching, MissionEvent::ResearchComplete),
            Ok(MissionState::Planning)
        );
        assert_eq!(
            transition_mission(MissionState::Planning, MissionEvent::PlanComplete),
            Ok(MissionState::Scheduled)
        );
        assert_eq!(
            transition_mission(MissionState::Scheduled, MissionEvent::Execute),
            Ok(MissionState::Executing)
        );
        assert_eq!(
            transition_mission(MissionState::Executing, MissionEvent::Verify),
            Ok(MissionState::Verifying)
        );
        assert_eq!(
            transition_mission(MissionState::Verifying, MissionEvent::Review),
            Ok(MissionState::Reviewing)
        );
        assert_eq!(
            transition_mission(MissionState::Reviewing, MissionEvent::Integrate),
            Ok(MissionState::Integrating)
        );
        assert_eq!(
            transition_mission(MissionState::Integrating, MissionEvent::Ship),
            Ok(MissionState::Shipping)
        );
        assert_eq!(
            transition_mission(MissionState::Shipping, MissionEvent::Complete),
            Ok(MissionState::Completed)
        );
    }

    #[test]
    fn test_pause_resume() {
        assert_eq!(
            transition_mission(MissionState::Executing, MissionEvent::Pause),
            Ok(MissionState::Paused)
        );
        assert_eq!(
            transition_mission(MissionState::Paused, MissionEvent::Resume),
            Ok(MissionState::Executing)
        );
    }

    #[test]
    fn test_block_unblock() {
        assert_eq!(
            transition_mission(MissionState::Executing, MissionEvent::Block),
            Ok(MissionState::Blocked)
        );
        assert_eq!(
            transition_mission(MissionState::Planning, MissionEvent::Block),
            Ok(MissionState::Blocked)
        );
        assert_eq!(
            transition_mission(MissionState::Blocked, MissionEvent::Unblock),
            Ok(MissionState::Executing)
        );
    }

    #[test]
    fn test_fail_from_non_terminal() {
        assert_eq!(
            transition_mission(MissionState::Executing, MissionEvent::Fail),
            Ok(MissionState::Failed)
        );
        assert_eq!(
            transition_mission(MissionState::Planning, MissionEvent::Fail),
            Ok(MissionState::Failed)
        );
        assert_eq!(
            transition_mission(MissionState::Created, MissionEvent::Fail),
            Ok(MissionState::Failed)
        );
    }

    #[test]
    fn test_cancel_from_non_terminal() {
        assert_eq!(
            transition_mission(MissionState::Executing, MissionEvent::Cancel),
            Ok(MissionState::Cancelled)
        );
        assert_eq!(
            transition_mission(MissionState::Planning, MissionEvent::Cancel),
            Ok(MissionState::Cancelled)
        );
        assert_eq!(
            transition_mission(MissionState::Created, MissionEvent::Cancel),
            Ok(MissionState::Cancelled)
        );
    }

    #[test]
    fn test_terminal_states_reject_transitions() {
        let terminal_states = [
            MissionState::Completed,
            MissionState::Failed,
            MissionState::Cancelled,
        ];
        for state in terminal_states {
            assert!(transition_mission(state, MissionEvent::Start).is_err());
            assert!(transition_mission(state, MissionEvent::Fail).is_err());
            assert!(transition_mission(state, MissionEvent::Cancel).is_err());
        }
    }

    #[test]
    fn test_invalid_transitions_rejected() {
        // Can't skip steps
        assert!(
            transition_mission(MissionState::Created, MissionEvent::UnderstandComplete).is_err()
        );
        assert!(transition_mission(MissionState::Created, MissionEvent::Execute).is_err());
        assert!(transition_mission(MissionState::Understanding, MissionEvent::Execute).is_err());

        // Can't go backwards (except Unblock)
        assert!(
            transition_mission(MissionState::Executing, MissionEvent::UnderstandComplete).is_err()
        );
    }

    #[test]
    fn test_is_terminal() {
        assert!(MissionState::Completed.is_terminal());
        assert!(MissionState::Failed.is_terminal());
        assert!(MissionState::Cancelled.is_terminal());
        assert!(!MissionState::Executing.is_terminal());
        assert!(!MissionState::Paused.is_terminal());
        assert!(!MissionState::Blocked.is_terminal());
    }
}
