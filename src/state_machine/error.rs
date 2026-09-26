//! Transition error types for state machines

use thiserror::Error;

/// Error returned when a state transition is invalid.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum TransitionError {
    /// Attempted an invalid state transition
    #[error("invalid transition from {from:?} to {to:?}")]
    InvalidTransition { from: String, to: String },

    /// Attempted to transition from a terminal state
    #[error("terminal state {state:?} cannot transition")]
    TerminalState { state: String },
}

impl TransitionError {
    pub fn invalid_transition(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self::InvalidTransition {
            from: from.into(),
            to: to.into(),
        }
    }

    pub fn terminal_state(state: impl Into<String>) -> Self {
        Self::TerminalState {
            state: state.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_transition_display() {
        let err = TransitionError::invalid_transition("Created", "Completed");
        assert_eq!(
            err.to_string(),
            "invalid transition from \"Created\" to \"Completed\""
        );
    }

    #[test]
    fn test_terminal_state_display() {
        let err = TransitionError::terminal_state("Completed");
        assert_eq!(
            err.to_string(),
            "terminal state \"Completed\" cannot transition"
        );
    }
}
