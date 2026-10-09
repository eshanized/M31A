//! Asynchronous approval notification and response channel trait (POL-01, D-09).

use async_trait::async_trait;
use thiserror::Error;

use crate::ids::ApprovalRequestId;
use crate::policy::approval::{ApprovalAction, ApprovalExplanationPacket};

/// Strongly typed errors encountered during interactive approval resolution.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ApprovalError {
    #[error("approval request timed out: {0}")]
    Timeout(String),

    #[error("approval request cancelled: {0}")]
    Cancelled(String),

    #[error("database error during approval coordination: {0}")]
    Database(String),

    #[error("approval communication channel closed: {0}")]
    ChannelClosed(String),

    #[error("approval request not found: {0}")]
    NotFound(String),

    #[error("approval request already resolved: {0}")]
    AlreadyResolved(String),

    #[error("invalid approval request state: {0}")]
    InvalidState(String),
}

/// Asynchronous channel trait implemented by TUI, CLI, or headless notification surfaces.
#[async_trait]
pub trait ApprovalChannel: Send + Sync + 'static {
    /// Notify the operator of a pending approval request with explanation details.
    async fn notify_request(&self, packet: &ApprovalExplanationPacket)
    -> Result<(), ApprovalError>;

    /// Poll or await an asynchronous operator response for a pending request.
    async fn poll_response(
        &self,
        id: ApprovalRequestId,
    ) -> Result<Option<ApprovalAction>, ApprovalError>;
}
