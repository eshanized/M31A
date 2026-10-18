//! ExecutionRun lifecycle state machine
//!
//! Provides validated transitions for the canonical top-level execution lifecycle.

use crate::state_machine::error::TransitionError;
use serde::{Deserialize, Serialize};

/// ExecutionRun lifecycle states representing stages from inception to completion or failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExecutionRunState {
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

impl std::fmt::Display for ExecutionRunState {
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

impl std::str::FromStr for ExecutionRunState {
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

/// Events that can trigger execution run state transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExecutionRunEvent {
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

impl ExecutionRunState {
    /// Check if this state is terminal (no further transitions allowed)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ExecutionRunState::Completed | ExecutionRunState::Failed | ExecutionRunState::Cancelled
        )
    }

    /// Get the target state for an event (used for error reporting)
    pub fn event_target_state(event: ExecutionRunEvent) -> Option<ExecutionRunState> {
        match event {
            ExecutionRunEvent::Start => Some(ExecutionRunState::Understanding),
            ExecutionRunEvent::UnderstandComplete => Some(ExecutionRunState::Researching),
            ExecutionRunEvent::ResearchComplete => Some(ExecutionRunState::Planning),
            ExecutionRunEvent::PlanComplete => Some(ExecutionRunState::Scheduled),
            ExecutionRunEvent::Schedule => Some(ExecutionRunState::Scheduled),
            ExecutionRunEvent::Execute => Some(ExecutionRunState::Executing),
            ExecutionRunEvent::Verify => Some(ExecutionRunState::Verifying),
            ExecutionRunEvent::Review => Some(ExecutionRunState::Reviewing),
            ExecutionRunEvent::Integrate => Some(ExecutionRunState::Integrating),
            ExecutionRunEvent::Ship => Some(ExecutionRunState::Shipping),
            ExecutionRunEvent::Complete => Some(ExecutionRunState::Completed),
            ExecutionRunEvent::Pause => Some(ExecutionRunState::Paused),
            ExecutionRunEvent::Resume => Some(ExecutionRunState::Executing),
            ExecutionRunEvent::Block => Some(ExecutionRunState::Blocked),
            ExecutionRunEvent::Unblock => None,
            ExecutionRunEvent::Fail => Some(ExecutionRunState::Failed),
            ExecutionRunEvent::Cancel => Some(ExecutionRunState::Cancelled),
        }
    }

    /// Map a requested target state from current state to the corresponding transition event.
    pub fn target_state_to_event(
        current: Self,
        target: Self,
    ) -> Result<ExecutionRunEvent, TransitionError> {
        match target {
            Self::Understanding => Ok(ExecutionRunEvent::Start),
            Self::Researching => Ok(ExecutionRunEvent::UnderstandComplete),
            Self::Planning => Ok(ExecutionRunEvent::ResearchComplete),
            Self::Scheduled => {
                if current == Self::Created {
                    Ok(ExecutionRunEvent::Schedule)
                } else {
                    Ok(ExecutionRunEvent::PlanComplete)
                }
            }
            Self::Executing => {
                if current == Self::Paused {
                    Ok(ExecutionRunEvent::Resume)
                } else if current == Self::Blocked {
                    Ok(ExecutionRunEvent::Unblock)
                } else {
                    Ok(ExecutionRunEvent::Execute)
                }
            }
            Self::Verifying => Ok(ExecutionRunEvent::Verify),
            Self::Reviewing => Ok(ExecutionRunEvent::Review),
            Self::Integrating => Ok(ExecutionRunEvent::Integrate),
            Self::Shipping => Ok(ExecutionRunEvent::Ship),
            Self::Completed => Ok(ExecutionRunEvent::Complete),
            Self::Paused => Ok(ExecutionRunEvent::Pause),
            Self::Blocked => Ok(ExecutionRunEvent::Block),
            Self::Failed => Ok(ExecutionRunEvent::Fail),
            Self::Cancelled => Ok(ExecutionRunEvent::Cancel),
            Self::Created => Err(TransitionError::invalid_transition(
                format!("{:?}", current),
                format!("{:?}", target),
            )),
        }
    }

    /// Validate whether a transition from `current` to `target` is legal.
    ///
    /// Rejects transitions from terminal states, illegal leaps across lifecycle stages,
    /// and invalid target states.
    pub fn validate_transition(current: Self, target: Self) -> Result<Self, TransitionError> {
        if current == target {
            return Ok(target);
        }
        if current.is_terminal() {
            return Err(TransitionError::terminal_state(format!("{:?}", current)));
        }
        let event = Self::target_state_to_event(current, target)?;
        transition_execution_run(current, event)
    }
}

/// Validate and execute an execution run state transition.
/// Returns the new state on success, or a TransitionError on failure.
pub fn transition_execution_run(
    current: ExecutionRunState,
    event: ExecutionRunEvent,
) -> Result<ExecutionRunState, TransitionError> {
    transition_execution_run_with_shortcuts(current, event, true)
}

/// Shared transition graph with an explicit policy switch for direct execution.
///
/// Mission workflows use strict staged progression; ExecutionRun additionally
/// permits a small, explicit set of direct-execution shortcuts.
pub(crate) fn transition_execution_run_with_shortcuts(
    current: ExecutionRunState,
    event: ExecutionRunEvent,
    allow_direct_execution: bool,
) -> Result<ExecutionRunState, TransitionError> {
    // Terminal states cannot transition
    if current.is_terminal() {
        return Err(TransitionError::terminal_state(format!("{:?}", current)));
    }

    let new_state = match (current, event) {
        // Full workflow progression
        (ExecutionRunState::Created, ExecutionRunEvent::Start) => ExecutionRunState::Understanding,
        (ExecutionRunState::Understanding, ExecutionRunEvent::UnderstandComplete) => {
            ExecutionRunState::Researching
        }
        (ExecutionRunState::Researching, ExecutionRunEvent::ResearchComplete) => {
            ExecutionRunState::Planning
        }
        (ExecutionRunState::Planning, ExecutionRunEvent::PlanComplete) => {
            ExecutionRunState::Scheduled
        }
        (ExecutionRunState::Planning, ExecutionRunEvent::Schedule)
            if allow_direct_execution =>
        {
            ExecutionRunState::Scheduled
        }
        (ExecutionRunState::Scheduled, ExecutionRunEvent::Execute) => ExecutionRunState::Executing,
        (ExecutionRunState::Executing, ExecutionRunEvent::Verify) => ExecutionRunState::Verifying,
        (ExecutionRunState::Verifying, ExecutionRunEvent::Review) => ExecutionRunState::Reviewing,
        (ExecutionRunState::Reviewing, ExecutionRunEvent::Integrate) => {
            ExecutionRunState::Integrating
        }
        (ExecutionRunState::Integrating, ExecutionRunEvent::Ship) => ExecutionRunState::Shipping,
        (ExecutionRunState::Shipping, ExecutionRunEvent::Complete) => ExecutionRunState::Completed,

        // Direct Execution shortcuts (bypassing full Genesis workflow)
        (ExecutionRunState::Created, ExecutionRunEvent::Schedule)
            if allow_direct_execution =>
        {
            ExecutionRunState::Scheduled
        }
        (ExecutionRunState::Created, ExecutionRunEvent::Execute)
            if allow_direct_execution =>
        {
            ExecutionRunState::Executing
        }
        (ExecutionRunState::Executing, ExecutionRunEvent::Complete)
            if allow_direct_execution =>
        {
            ExecutionRunState::Completed
        }

        // Pause/Resume
        (ExecutionRunState::Executing, ExecutionRunEvent::Pause) => ExecutionRunState::Paused,
        (ExecutionRunState::Paused, ExecutionRunEvent::Resume) => ExecutionRunState::Executing,

        // Block/Unblock
        (state, ExecutionRunEvent::Block)
            if !state.is_terminal() && state != ExecutionRunState::Blocked =>
        {
            ExecutionRunState::Blocked
        }
        (ExecutionRunState::Blocked, ExecutionRunEvent::Unblock) => ExecutionRunState::Executing,

        // Failure/Cancellation from any non-terminal state
        (state, ExecutionRunEvent::Fail) if !state.is_terminal() => ExecutionRunState::Failed,
        (state, ExecutionRunEvent::Cancel) if !state.is_terminal() => ExecutionRunState::Cancelled,

        // Invalid transitions
        (from, event) => {
            let target = ExecutionRunState::event_target_state(event).unwrap_or(from);
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
            transition_execution_run(ExecutionRunState::Created, ExecutionRunEvent::Start),
            Ok(ExecutionRunState::Understanding)
        );
        assert_eq!(
            transition_execution_run(
                ExecutionRunState::Understanding,
                ExecutionRunEvent::UnderstandComplete
            ),
            Ok(ExecutionRunState::Researching)
        );
        assert_eq!(
            transition_execution_run(
                ExecutionRunState::Researching,
                ExecutionRunEvent::ResearchComplete
            ),
            Ok(ExecutionRunState::Planning)
        );
        assert_eq!(
            transition_execution_run(ExecutionRunState::Planning, ExecutionRunEvent::PlanComplete),
            Ok(ExecutionRunState::Scheduled)
        );
        assert_eq!(
            transition_execution_run(ExecutionRunState::Scheduled, ExecutionRunEvent::Execute),
            Ok(ExecutionRunState::Executing)
        );
        assert_eq!(
            transition_execution_run(ExecutionRunState::Executing, ExecutionRunEvent::Verify),
            Ok(ExecutionRunState::Verifying)
        );
        assert_eq!(
            transition_execution_run(ExecutionRunState::Verifying, ExecutionRunEvent::Review),
            Ok(ExecutionRunState::Reviewing)
        );
        assert_eq!(
            transition_execution_run(ExecutionRunState::Reviewing, ExecutionRunEvent::Integrate),
            Ok(ExecutionRunState::Integrating)
        );
        assert_eq!(
            transition_execution_run(ExecutionRunState::Integrating, ExecutionRunEvent::Ship),
            Ok(ExecutionRunState::Shipping)
        );
        assert_eq!(
            transition_execution_run(ExecutionRunState::Shipping, ExecutionRunEvent::Complete),
            Ok(ExecutionRunState::Completed)
        );
    }

    #[test]
    fn test_direct_execution_transitions() {
        assert_eq!(
            transition_execution_run(ExecutionRunState::Created, ExecutionRunEvent::Schedule),
            Ok(ExecutionRunState::Scheduled)
        );
        assert_eq!(
            transition_execution_run(ExecutionRunState::Created, ExecutionRunEvent::Execute),
            Ok(ExecutionRunState::Executing)
        );
        assert_eq!(
            transition_execution_run(ExecutionRunState::Executing, ExecutionRunEvent::Complete),
            Ok(ExecutionRunState::Completed)
        );
    }

    #[test]
    fn test_pause_resume() {
        assert_eq!(
            transition_execution_run(ExecutionRunState::Executing, ExecutionRunEvent::Pause),
            Ok(ExecutionRunState::Paused)
        );
        assert_eq!(
            transition_execution_run(ExecutionRunState::Paused, ExecutionRunEvent::Resume),
            Ok(ExecutionRunState::Executing)
        );
    }

    #[test]
    fn test_block_unblock() {
        assert_eq!(
            transition_execution_run(ExecutionRunState::Executing, ExecutionRunEvent::Block),
            Ok(ExecutionRunState::Blocked)
        );
        assert_eq!(
            transition_execution_run(ExecutionRunState::Blocked, ExecutionRunEvent::Unblock),
            Ok(ExecutionRunState::Executing)
        );
    }

    #[test]
    fn test_fail_from_any_state() {
        assert_eq!(
            transition_execution_run(ExecutionRunState::Created, ExecutionRunEvent::Fail),
            Ok(ExecutionRunState::Failed)
        );
        assert_eq!(
            transition_execution_run(ExecutionRunState::Executing, ExecutionRunEvent::Fail),
            Ok(ExecutionRunState::Failed)
        );
        assert_eq!(
            transition_execution_run(ExecutionRunState::Blocked, ExecutionRunEvent::Fail),
            Ok(ExecutionRunState::Failed)
        );
    }

    #[test]
    fn test_cancel_from_any_state() {
        assert_eq!(
            transition_execution_run(ExecutionRunState::Created, ExecutionRunEvent::Cancel),
            Ok(ExecutionRunState::Cancelled)
        );
        assert_eq!(
            transition_execution_run(ExecutionRunState::Executing, ExecutionRunEvent::Cancel),
            Ok(ExecutionRunState::Cancelled)
        );
    }

    #[test]
    fn test_terminal_states_reject_transitions() {
        assert!(
            transition_execution_run(ExecutionRunState::Completed, ExecutionRunEvent::Execute)
                .is_err()
        );
        assert!(
            transition_execution_run(ExecutionRunState::Failed, ExecutionRunEvent::Resume).is_err()
        );
        assert!(
            transition_execution_run(ExecutionRunState::Cancelled, ExecutionRunEvent::Start)
                .is_err()
        );
    }

    #[test]
    fn test_from_str_and_display() {
        let state = ExecutionRunState::Executing;
        assert_eq!(state.to_string(), "Executing");
        assert_eq!("executing".parse::<ExecutionRunState>().unwrap(), state);
    }
}
