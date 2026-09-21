//! Capability errors and failure classification (D-02, CTL-01).

use thiserror::Error;

/// Error returned by capability operations.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum CapabilityError {
    /// Workspace path containment violation or traversal escape.
    #[error("path out of bounds: path {path:?} escapes workspace {workspace:?}")]
    PathOutOfBounds { path: String, workspace: String },

    /// Requested resource was not found.
    #[error("resource not found: {0}")]
    NotFound(String),

    /// Precondition for operation not satisfied.
    #[error("precondition failed: {0}")]
    PreconditionFailed(String),

    /// Operation or resource scope not permitted by capability permissions.
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// External execution or command failed.
    #[error("execution failed: exit code {exit_code:?}, message: {message}")]
    ExecutionFailed {
        exit_code: Option<i32>,
        message: String,
    },

    /// Runtime infrastructure fault (spawn failure, broken pipe, missing binary).
    #[error("infrastructure fault: {0}")]
    InfrastructureFault(String),

    /// Underlying I/O failure.
    #[error("i/o error: {0}")]
    Io(String),

    /// Operation exceeded its timeout ceiling.
    #[error("timeout: {0}")]
    Timeout(String),

    /// Capability instance or provider is currently unavailable.
    #[error("capability unavailable: {0}")]
    Unavailable(String),

    /// Invalid argument supplied to capability.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    /// General capability error.
    #[error("capability error: {0}")]
    Other(String),
}

impl CapabilityError {
    /// Returns true if this error represents an infrastructure fault that should
    /// affect capability health.
    pub fn is_infrastructure(&self) -> bool {
        matches!(
            self,
            CapabilityError::InfrastructureFault(_)
                | CapabilityError::Unavailable(_)
                | CapabilityError::Timeout(_)
                | CapabilityError::Io(_)
        )
    }

    /// Returns true if this error represents a semantic task failure (which must
    /// NOT degrade capability health).
    pub fn is_semantic(&self) -> bool {
        !self.is_infrastructure()
    }
}

impl From<std::io::Error> for CapabilityError {
    fn from(err: std::io::Error) -> Self {
        CapabilityError::Io(err.to_string())
    }
}
