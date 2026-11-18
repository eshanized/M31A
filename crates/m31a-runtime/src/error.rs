//! Structured error types for the M31A application runtime.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Provider initialization or invocation error: {0}")]
    Provider(String),

    #[error("Storage initialization or database error: {0}")]
    Storage(String),

    #[error("Tool registry error: {0}")]
    Tool(String),

    #[error("Security or trust boundary violation: {0}")]
    Security(String),

    #[error("Engine execution error: {0}")]
    Engine(String),

    #[error("Lifecycle state error: {message}")]
    InvalidState {
        message: String,
        current: crate::lifecycle::RuntimeState,
    },

    #[error("Turn execution error: {0}")]
    Turn(String),

    #[error("Turn not found: {0}")]
    TurnNotFound(String),

    #[error("Approval request not found: {0}")]
    ApprovalNotFound(String),

    #[error("Question request not found: {0}")]
    QuestionNotFound(String),

    #[error("Runtime shutdown error: {0}")]
    Shutdown(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Internal runtime error: {0}")]
    Internal(#[from] anyhow::Error),
}
