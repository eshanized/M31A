//! Interactive prompt and session lifecycle state machine (PRD §01, CLI-01, CLI-04).
//!
//! Enforces explicit, legal state transitions during interactive developer coding sessions:
//!
//! ```text
//!              ┌───────────────────────────────┐
//!              │             Idle              │◄─────────────────┐
//!              └───────┬───────────────▲───────┘                  │
//!                      │ (user types)  │                          │
//!                      ▼               │                          │
//!              ┌───────────────┐       │ (cancel/complete)        │
//!              │    Editing    │       │                          │
//!              └───────┬───────┘       │                          │
//!                      │ (submit)      │                          │
//!                      ▼               │                          │
//!              ┌───────────────┐       │                          │
//!              │  Submitting   │───────┘ (command/info)           │
//!              └───────┬───────┘                                  │
//!                      │ (run mission)                            │
//!                      ▼                                          │
//!              ┌───────────────┐       (policy ask) ┌────────────────────┐
//!              │   Executing   │───────────────────►│  AwaitingApproval  │
//!              └───────┬───────┘◄───────────────────└────────────────────┘
//!                      │                (resolved)
//!                      ▼
//!              ┌───────────────┐
//!              │  Completing   │──────────────────────────────────┘
//!              └───────────────┘
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Lifecycle state of the interactive developer prompt/session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SessionPromptState {
    #[default]
    Idle,
    Editing,
    Submitting,
    Executing,
    AwaitingApproval,
    WaitingForUser,
    ShowingDiff,
    Completing,
    Error,
    Exiting,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("Illegal prompt state transition from {from:?} to {attempted:?}")]
pub struct StateTransitionError {
    pub from: SessionPromptState,
    pub attempted: SessionPromptState,
}

impl SessionPromptState {
    /// Validates whether transitioning to `next` is permitted.
    pub fn can_transition_to(&self, next: SessionPromptState) -> bool {
        match (self, next) {
            // Self-transitions are permitted
            (a, b) if a == &b => true,

            // Idle transitions
            (SessionPromptState::Idle, SessionPromptState::Editing) => true,
            (SessionPromptState::Idle, SessionPromptState::Submitting) => true,
            (SessionPromptState::Idle, SessionPromptState::ShowingDiff) => true,
            (SessionPromptState::Idle, SessionPromptState::Executing) => true,
            (SessionPromptState::Idle, SessionPromptState::WaitingForUser) => true,
            (SessionPromptState::Idle, SessionPromptState::Exiting) => true,

            // Editing transitions
            (SessionPromptState::Editing, SessionPromptState::Submitting) => true,
            (SessionPromptState::Editing, SessionPromptState::Idle) => true, // e.g. Ctrl-C clears input
            (SessionPromptState::Editing, SessionPromptState::Exiting) => true,

            // Submitting transitions
            (SessionPromptState::Submitting, SessionPromptState::Executing) => true,
            (SessionPromptState::Submitting, SessionPromptState::ShowingDiff) => true,
            (SessionPromptState::Submitting, SessionPromptState::Idle) => true, // info command completed
            (SessionPromptState::Submitting, SessionPromptState::Error) => true,

            // Executing transitions
            (SessionPromptState::Executing, SessionPromptState::AwaitingApproval) => true,
            (SessionPromptState::Executing, SessionPromptState::WaitingForUser) => true,
            (SessionPromptState::Executing, SessionPromptState::Completing) => true,
            (SessionPromptState::Executing, SessionPromptState::Error) => true,
            (SessionPromptState::Executing, SessionPromptState::Idle) => true, // cancelled

            // AwaitingApproval transitions
            (SessionPromptState::AwaitingApproval, SessionPromptState::Executing) => true,
            (SessionPromptState::AwaitingApproval, SessionPromptState::Idle) => true, // operator cancelled
            (SessionPromptState::AwaitingApproval, SessionPromptState::Error) => true,

            // WaitingForUser transitions
            (SessionPromptState::WaitingForUser, SessionPromptState::Executing) => true,
            (SessionPromptState::WaitingForUser, SessionPromptState::Idle) => true, // operator cancelled
            (SessionPromptState::WaitingForUser, SessionPromptState::Error) => true,
            (SessionPromptState::WaitingForUser, SessionPromptState::Exiting) => true,

            // ShowingDiff transitions
            (SessionPromptState::ShowingDiff, SessionPromptState::Idle) => true,

            // Completing transitions
            (SessionPromptState::Completing, SessionPromptState::Idle) => true,

            // Error transitions
            (SessionPromptState::Error, SessionPromptState::Idle) => true,
            (SessionPromptState::Error, SessionPromptState::Exiting) => true,

            _ => false,
        }
    }

    /// Transition to next state, returning error if illegal.
    pub fn transition_to(&mut self, next: SessionPromptState) -> Result<(), StateTransitionError> {
        if self.can_transition_to(next) {
            *self = next;
            Ok(())
        } else {
            Err(StateTransitionError {
                from: *self,
                attempted: next,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        let mut state = SessionPromptState::Idle;
        assert!(state.transition_to(SessionPromptState::Editing).is_ok());
        assert!(state.transition_to(SessionPromptState::Submitting).is_ok());
        assert!(state.transition_to(SessionPromptState::Executing).is_ok());
        assert!(
            state
                .transition_to(SessionPromptState::AwaitingApproval)
                .is_ok()
        );
        assert!(state.transition_to(SessionPromptState::Executing).is_ok());
        assert!(
            state
                .transition_to(SessionPromptState::WaitingForUser)
                .is_ok()
        );
        assert!(state.transition_to(SessionPromptState::Executing).is_ok());
        assert!(state.transition_to(SessionPromptState::Completing).is_ok());
        assert!(state.transition_to(SessionPromptState::Idle).is_ok());
    }

    #[test]
    fn test_invalid_transitions() {
        let mut state = SessionPromptState::AwaitingApproval;
        assert!(state.transition_to(SessionPromptState::Editing).is_err());
    }
}
