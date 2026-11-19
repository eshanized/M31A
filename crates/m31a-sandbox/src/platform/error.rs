//! Structured Errors for Sandboxed Execution.
//!
//! Enforces fail-closed reporting when sandbox isolation boundaries cannot be established.

use m31a_core::config::SandboxPolicy;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SandboxError {
    #[error(
        "Sandbox isolation unavailable for policy '{policy:?}': {reason}. Security policy is fail-closed."
    )]
    IsolationUnavailable {
        policy: SandboxPolicy,
        reason: String,
    },

    #[error("Sandbox backend '{backend}' failed: {details}")]
    BackendFailure { backend: String, details: String },

    #[error("Sandboxed command execution timed out after {duration_ms}ms")]
    Timeout { duration_ms: u64 },

    #[error("Sandboxed execution cancelled by caller")]
    Cancelled,

    #[error("Underlying I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Internal sandbox error: {0}")]
    Other(#[from] anyhow::Error),
}
