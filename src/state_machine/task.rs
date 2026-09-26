//! Task lifecycle state machine

use crate::state_machine::error::TransitionError;
use serde::{Deserialize, Serialize};

/// Task states per REQUIREMENTS.md KRN-02
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskState {
    Pending,
    Blocked,
    Ready,
    Running,
    Succeeded,
    Failed,
    Skipped,
    Cancelled,
    NeedsReview,
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::Blocked => write!(f, "Blocked"),
            Self::Ready => write!(f, "Ready"),
            Self::Running => write!(f, "Running"),
            Self::Succeeded => write!(f, "Succeeded"),
            Self::Failed => write!(f, "Failed"),
            Self::Skipped => write!(f, "Skipped"),
            Self::Cancelled => write!(f, "Cancelled"),
            Self::NeedsReview => write!(f, "NeedsReview"),
        }
    }
}

impl std::str::FromStr for TaskState {
    type Err = TransitionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "blocked" => Ok(Self::Blocked),
            "ready" => Ok(Self::Ready),
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "skipped" => Ok(Self::Skipped),
            "cancelled" => Ok(Self::Cancelled),
            "needsreview" | "needs_review" => Ok(Self::NeedsReview),
            _ => Err(TransitionError::invalid_transition(s, "UnknownState")),
        }
    }
}

/// Events that can trigger task state transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskEvent {
    Block,
    Unblock,
    MarkReady,
    Start,
    Complete,
    Fail,
    Retry,
    Skip,
    Cancel,
    RequestReview,
}

impl TaskState {
    /// Check if this state is terminal (no further transitions allowed)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskState::Succeeded | TaskState::Failed | TaskState::Skipped | TaskState::Cancelled
        )
    }
}

/// Validate and execute a task state transition.
/// Returns the new state on success, or a TransitionError on failure.
pub fn transition_task(current: TaskState, event: TaskEvent) -> Result<TaskState, TransitionError> {
    if current.is_terminal() {
        return Err(TransitionError::terminal_state(format!("{:?}", current)));
    }

    let new_state = match (current, event) {
        // Universal non-terminal Cancel per D-06
        (_, TaskEvent::Cancel) => TaskState::Cancelled,

        // Block from Pending or Ready per D-05
        (TaskState::Pending, TaskEvent::Block) => TaskState::Blocked,
        (TaskState::Ready, TaskEvent::Block) => TaskState::Blocked,

        // Unblock from Pending or Blocked
        (TaskState::Pending, TaskEvent::Unblock) => TaskState::Ready,
        (TaskState::Blocked, TaskEvent::Unblock) => TaskState::Ready,

        // Mark ready from Pending
        (TaskState::Pending, TaskEvent::MarkReady) => TaskState::Ready,

        // Start from Ready
        (TaskState::Ready, TaskEvent::Start) => TaskState::Running,

        // Complete from Running or NeedsReview
        (TaskState::Running, TaskEvent::Complete) => TaskState::Succeeded,
        (TaskState::NeedsReview, TaskEvent::Complete) => TaskState::Succeeded,

        // Fail from Running or NeedsReview
        (TaskState::Running, TaskEvent::Fail) => TaskState::Failed,
        (TaskState::NeedsReview, TaskEvent::Fail) => TaskState::Failed,

        // Retry from Running or NeedsReview per D-07
        (TaskState::Running, TaskEvent::Retry) => TaskState::Ready,
        (TaskState::NeedsReview, TaskEvent::Retry) => TaskState::Ready,

        // Skip from Ready or Pending
        (TaskState::Ready, TaskEvent::Skip) => TaskState::Skipped,
        (TaskState::Pending, TaskEvent::Skip) => TaskState::Skipped,

        // Request review from Running
        (TaskState::Running, TaskEvent::RequestReview) => TaskState::NeedsReview,

        // Invalid transitions
        (from, event) => {
            return Err(TransitionError::invalid_transition(
                format!("{:?}", from),
                format!("{:?}", event),
            ));
        }
    };

    Ok(new_state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        assert_eq!(
            transition_task(TaskState::Pending, TaskEvent::Unblock),
            Ok(TaskState::Ready)
        );
        assert_eq!(
            transition_task(TaskState::Blocked, TaskEvent::Unblock),
            Ok(TaskState::Ready)
        );
        assert_eq!(
            transition_task(TaskState::Pending, TaskEvent::MarkReady),
            Ok(TaskState::Ready)
        );
        assert_eq!(
            transition_task(TaskState::Ready, TaskEvent::Start),
            Ok(TaskState::Running)
        );
        assert_eq!(
            transition_task(TaskState::Running, TaskEvent::Complete),
            Ok(TaskState::Succeeded)
        );
        assert_eq!(
            transition_task(TaskState::Running, TaskEvent::Fail),
            Ok(TaskState::Failed)
        );
        assert_eq!(
            transition_task(TaskState::Running, TaskEvent::Cancel),
            Ok(TaskState::Cancelled)
        );
        assert_eq!(
            transition_task(TaskState::Running, TaskEvent::RequestReview),
            Ok(TaskState::NeedsReview)
        );
        assert_eq!(
            transition_task(TaskState::NeedsReview, TaskEvent::Complete),
            Ok(TaskState::Succeeded)
        );
        assert_eq!(
            transition_task(TaskState::NeedsReview, TaskEvent::Fail),
            Ok(TaskState::Failed)
        );
        assert_eq!(
            transition_task(TaskState::Ready, TaskEvent::Skip),
            Ok(TaskState::Skipped)
        );
        assert_eq!(
            transition_task(TaskState::Pending, TaskEvent::Skip),
            Ok(TaskState::Skipped)
        );
        // Block transitions (D-05)
        assert_eq!(
            transition_task(TaskState::Pending, TaskEvent::Block),
            Ok(TaskState::Blocked)
        );
        assert_eq!(
            transition_task(TaskState::Ready, TaskEvent::Block),
            Ok(TaskState::Blocked)
        );
        // Retry transitions (D-07)
        assert_eq!(
            transition_task(TaskState::Running, TaskEvent::Retry),
            Ok(TaskState::Ready)
        );
        assert_eq!(
            transition_task(TaskState::NeedsReview, TaskEvent::Retry),
            Ok(TaskState::Ready)
        );
    }

    #[test]
    fn test_universal_cancel_from_non_terminal_states() {
        let non_terminal = [
            TaskState::Pending,
            TaskState::Blocked,
            TaskState::Ready,
            TaskState::Running,
            TaskState::NeedsReview,
        ];
        for state in non_terminal {
            assert_eq!(
                transition_task(state, TaskEvent::Cancel),
                Ok(TaskState::Cancelled),
                "Cancel should succeed from {:?}",
                state
            );
        }
    }

    #[test]
    fn test_terminal_states_reject_transitions() {
        let terminal_states = [
            TaskState::Succeeded,
            TaskState::Failed,
            TaskState::Skipped,
            TaskState::Cancelled,
        ];
        let all_events = [
            TaskEvent::Block,
            TaskEvent::Unblock,
            TaskEvent::MarkReady,
            TaskEvent::Start,
            TaskEvent::Complete,
            TaskEvent::Fail,
            TaskEvent::Retry,
            TaskEvent::Skip,
            TaskEvent::Cancel,
            TaskEvent::RequestReview,
        ];
        for state in terminal_states {
            for event in all_events {
                assert!(
                    transition_task(state, event).is_err(),
                    "Terminal state {:?} should reject event {:?}",
                    state,
                    event
                );
            }
        }
    }

    #[test]
    fn test_invalid_transitions_rejected() {
        assert!(transition_task(TaskState::Pending, TaskEvent::Start).is_err());
        assert!(transition_task(TaskState::Pending, TaskEvent::Complete).is_err());
        assert!(transition_task(TaskState::Ready, TaskEvent::Unblock).is_err());
        assert!(transition_task(TaskState::Ready, TaskEvent::Fail).is_err());
        assert!(transition_task(TaskState::Blocked, TaskEvent::Start).is_err());
        assert!(transition_task(TaskState::Running, TaskEvent::Unblock).is_err());
        assert!(transition_task(TaskState::Running, TaskEvent::MarkReady).is_err());
        assert!(transition_task(TaskState::Pending, TaskEvent::Retry).is_err());
        assert!(transition_task(TaskState::Blocked, TaskEvent::Retry).is_err());
        assert!(transition_task(TaskState::Ready, TaskEvent::Retry).is_err());
    }

    #[test]
    fn test_is_terminal() {
        assert!(TaskState::Succeeded.is_terminal());
        assert!(TaskState::Failed.is_terminal());
        assert!(TaskState::Skipped.is_terminal());
        assert!(TaskState::Cancelled.is_terminal());
        assert!(!TaskState::Pending.is_terminal());
        assert!(!TaskState::Blocked.is_terminal());
        assert!(!TaskState::Ready.is_terminal());
        assert!(!TaskState::Running.is_terminal());
        assert!(!TaskState::NeedsReview.is_terminal());
    }
}
