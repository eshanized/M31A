//! Error types and result definitions for background process management.

use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum ProcessError {
    #[error("Process '{0}' was not found")]
    NotFound(String),

    #[error("Failed to spawn process '{0}': {1}")]
    SpawnFailed(String, String),

    #[error("Execution policy violation for process '{0}': {1}")]
    PolicyViolation(String, String),

    #[error("Process '{0}' is already terminated")]
    AlreadyTerminated(String),

    #[error("Failed to terminate process '{0}': {1}")]
    TerminationFailed(String, String),

    #[error("I/O error for process '{0}': {1}")]
    Io(String, String),
}
