//! Network capability provider for connectivity check and DNS lookup (CTL-02, D-01, D-03).

use crate::capability::error::CapabilityError;
use crate::capability::traits::network::NetworkService;
use async_trait::async_trait;
use std::time::Duration;

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
        // P1-01: resolve → validate every address → connect to the VALIDATED
        // SocketAddr. The hostname is never resolved a second time, so DNS
        // cannot change the destination between validation and connection.
        let policy = crate::policy::destination::NetworkDestinationPolicy::new();
        let bound = policy.resolve_socket_addrs(host, port).await.map_err(|e| {
            CapabilityError::PermissionDenied(format!("Egress security policy violation: {e}"))
        })?;
        for candidate in bound {
            if policy
                .connect_validated(
                    &candidate,
                    Duration::from_secs(
                        crate::config::canonical::DEFAULT_METADATA_CONNECT_TIMEOUT_SECS,
                    ),
                )
                .await
                .is_ok()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn resolve_host(&self, host: &str) -> Result<Vec<String>, CapabilityError> {
        let policy = crate::policy::destination::NetworkDestinationPolicy::new();
        let ips = policy.validate_host(host).await.map_err(|e| {
            CapabilityError::PermissionDenied(format!("Egress security policy violation: {e}"))
        })?;

        Ok(ips.into_iter().map(|ip| ip.to_string()).collect())
    }
}
