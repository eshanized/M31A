//! Top-level error type hierarchy for M31A.
//!
//! Uses `thiserror` for domain/public error classification and `anyhow` for application glue.

use thiserror::Error;

/// Top-level error type for M31A.
///
/// Each variant represents a distinct error category at the domain level.
/// More specific error types live in their respective modules (e.g., state_machine::TransitionError).
#[derive(Error, Debug)]
pub enum M31AError {
    /// Invalid state transition attempted.
    /// Will be populated by Plan 02 with state machine integration.
    #[error("invalid state transition: {0}")]
    TransitionError(String),

    /// Database/storage operation failed.
    /// Will be populated by Plan 05 with persistence integration.
    #[error("persistence error: {0}")]
    PersistenceError(String),

    /// Cancellation operation failed.
    /// Will be populated by Plan 04 with cancellation integration.
    #[error("cancellation error: {0}")]
    CancellationError(String),

    /// Input validation failed.
    #[error("validation error: {0}")]
    ValidationError(String),

    /// Requested entity not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// Event bus subscriber lagged behind broadcast channel.
    #[error("event subscriber lagged, {skipped} events skipped")]
    EventLagged { skipped: u64 },

    /// Underlying database operation failed.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// State reconstruction failed or detected corrupt state.
    #[error("state reconstruction error: {0}")]
    ReconstructionError(String),

    /// Policy compilation or file loading failed.
    #[error("policy error: {0}")]
    Policy(#[from] crate::policy::effective::PolicyLoadError),

    /// Unexpected internal error.
    /// Wraps anyhow::Error for application-level error propagation.
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl M31AError {
    /// Create a TransitionError variant.
    pub fn transition(msg: impl Into<String>) -> Self {
        Self::TransitionError(msg.into())
    }

    /// Create a PersistenceError variant.
    pub fn persistence(msg: impl Into<String>) -> Self {
        Self::PersistenceError(msg.into())
    }

    /// Create a CancellationError variant.
    pub fn cancellation(msg: impl Into<String>) -> Self {
        Self::CancellationError(msg.into())
    }

    /// Create a ValidationError variant.
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::ValidationError(msg.into())
    }

    /// Create a NotFound variant.
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    /// Create a ReconstructionError variant.
    pub fn reconstruction(msg: impl Into<String>) -> Self {
        Self::ReconstructionError(msg.into())
    }

    /// Create an Internal variant from a string.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(anyhow::anyhow!(msg.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn test_transition_error() {
        let err = M31AError::transition("invalid transition from Created to Completed");
        assert_eq!(
            err.to_string(),
            "invalid state transition: invalid transition from Created to Completed"
        );
    }

    #[test]
    fn test_persistence_error() {
        let err = M31AError::persistence("database connection failed");
        assert_eq!(
            err.to_string(),
            "persistence error: database connection failed"
        );
    }

    #[test]
    fn test_cancellation_error() {
        let err = M31AError::cancellation("task already completed");
        assert_eq!(
            err.to_string(),
            "cancellation error: task already completed"
        );
    }

    #[test]
    fn test_validation_error() {
        let err = M31AError::validation("empty mission objective");
        assert_eq!(err.to_string(), "validation error: empty mission objective");
    }

    #[test]
    fn test_not_found() {
        let err = M31AError::not_found("mission abc123");
        assert_eq!(err.to_string(), "not found: mission abc123");
    }

    #[test]
    fn test_internal_error_from_string() {
        let err = M31AError::internal("unexpected null pointer");
        assert_eq!(err.to_string(), "internal error: unexpected null pointer");
    }

    #[test]
    fn test_internal_error_from_anyhow() {
        let anyhow_err = anyhow::anyhow!("original error");
        let err = M31AError::Internal(anyhow_err);
        assert!(err.to_string().contains("internal error: original error"));
    }

    #[test]
    fn test_error_trait_source() {
        let err = M31AError::internal("test");
        // The source() method should be available via std::error::Error
        // For Internal variant wrapping anyhow::Error, source() returns Some
        assert!(err.source().is_some());
    }

    #[test]
    fn test_string_variants_have_no_source() {
        let err = M31AError::validation("test");
        // String variants don't wrap another error, so source() should be None
        assert!(err.source().is_none());
    }

    #[test]
    fn test_error_chain_works() {
        let cause = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let anyhow_err = anyhow::Error::new(cause).context("reading config");
        let err = M31AError::Internal(anyhow_err);
        // Error chain should work through anyhow
        assert!(err.source().is_some());
        assert!(err.source().unwrap().source().is_some());
    }
}
