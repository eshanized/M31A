//! Error types for the CUA (Computer Use Agent) engine.

use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum CuaError {
    #[error(
        "State mismatch: expected state_id '{expected}', but active state is '{actual}'. UI may have mutated."
    )]
    StateMismatch { expected: String, actual: String },

    #[error("Action execution failed: {message} (action_sent: {action_sent})")]
    ActionFailed { message: String, action_sent: bool },

    #[error("Application not found: {0}")]
    AppNotFound(String),

    #[error("Accessible element with index {0} not found in active tree")]
    ElementNotFound(usize),

    #[error("Replay blocked: action hash '{0}' was already sent in this state")]
    ReplayBlocked(String),

    #[error("CUA backend error: {0}")]
    BackendError(String),

    #[error("CUA controller lease has expired")]
    LeaseExpired,

    #[error("Unsupported platform or accessibility subsystem unavailable: {0}")]
    Unavailable(String),
}
