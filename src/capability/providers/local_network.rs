//! Network capability provider for connectivity check and DNS lookup (CTL-02, D-01, D-03).

use crate::capability::error::CapabilityError;
use crate::capability::traits::network::NetworkService;
use async_trait::async_trait;
use std::time::Duration;
use tokio::net::{TcpStream, lookup_host};

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
        let addr = format!("{host}:{port}");
        let connect_fut = TcpStream::connect(&addr);
        match tokio::time::timeout(Duration::from_secs(5), connect_fut).await {
            Ok(Ok(_)) => Ok(true),
            _ => Ok(false),
        }
    }

    async fn resolve_host(&self, host: &str) -> Result<Vec<String>, CapabilityError> {
        let host_port = format!("{host}:80");
        let addrs = lookup_host(&host_port).await.map_err(|e| {
            CapabilityError::InfrastructureFault(format!("DNS resolution failed: {e}"))
        })?;

        Ok(addrs.map(|a| a.ip().to_string()).collect())
    }
}
