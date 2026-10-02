//! Network capability provider for connectivity check and DNS lookup (CTL-02, D-01, D-03).

use crate::capability::error::CapabilityError;
use crate::capability::traits::network::NetworkService;
use async_trait::async_trait;
use std::time::Duration;
use tokio::net::TcpStream;

/// Native provider for network operations.
pub struct LocalNetworkProvider;

impl Default for LocalNetworkProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalNetworkProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NetworkService for LocalNetworkProvider {
    async fn check_connectivity(&self, host: &str, port: u16) -> Result<bool, CapabilityError> {
        let policy = crate::policy::destination::NetworkDestinationPolicy::new();
        policy.validate_host(host).await.map_err(|e| {
            CapabilityError::PermissionDenied(format!("Egress security policy violation: {e}"))
        })?;

        let addr = format!("{host}:{port}");
        let connect_fut = TcpStream::connect(&addr);
        match tokio::time::timeout(Duration::from_secs(5), connect_fut).await {
            Ok(Ok(_)) => Ok(true),
            _ => Ok(false),
        }
    }

    async fn resolve_host(&self, host: &str) -> Result<Vec<String>, CapabilityError> {
        let policy = crate::policy::destination::NetworkDestinationPolicy::new();
        let ips = policy.validate_host(host).await.map_err(|e| {
            CapabilityError::PermissionDenied(format!("Egress security policy violation: {e}"))
        })?;

        Ok(ips.into_iter().map(|ip| ip.to_string()).collect())
    }
}
