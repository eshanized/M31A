//! Terminal session service trait (CTL-01, CTL-02).

use crate::capability::error::CapabilityError;
use async_trait::async_trait;

/// Asynchronous service seam for interactive terminal stream operations.
#[async_trait]
pub trait TerminalService: Send + Sync + 'static {
    /// Send input bytes to a terminal session stream.
    async fn write_input(&self, session_id: &str, input: &[u8]) -> Result<(), CapabilityError>;

    /// Read available output bytes from a terminal session stream up to timeout.
    async fn read_stream(
        &self,
        session_id: &str,
        timeout_ms: u64,
    ) -> Result<Vec<u8>, CapabilityError>;
}
