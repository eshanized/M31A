//! Network connectivity and resolution service trait (CTL-01, CTL-02).

use crate::capability::error::CapabilityError;
use async_trait::async_trait;

/// Asynchronous service seam for network connectivity and resolution.
#[async_trait]
pub trait NetworkService: Send + Sync + 'static {
    /// Test TCP reachability to a host and port.
    async fn check_connectivity(&self, host: &str, port: u16) -> Result<bool, CapabilityError>;

    /// Resolve host name to IP addresses.
    async fn resolve_host(&self, host: &str) -> Result<Vec<String>, CapabilityError>;
}
