//! Loopback TCP forwarding and tunneling manager.
//!
//! Manages dynamic loopback ports for proxy forwarding and secure socket bridging.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::copy_bidirectional;
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, warn};

use crate::domain_matcher::NetworkSecurityFilter;

/// Information about an active TCP tunnel forwarder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpTunnelInfo {
    pub tunnel_id: String,
    pub local_addr: String,
    pub remote_target: String,
    pub created_at: u64,
    pub active_connections: usize,
    pub total_connections: u64,
    pub bytes_forwarded: u64,
}

struct TunnelState {
    info: TcpTunnelInfo,
    abort_handle: tokio::task::AbortHandle,
    active_connections: Arc<AtomicUsize>,
    total_connections: Arc<AtomicU64>,
    bytes_forwarded: Arc<AtomicU64>,
}

/// Loopback TCP forwarding and tunneling manager providing secure local port forwarding.
#[derive(Clone, Default)]
pub struct TcpTunnelManager {
    tunnels: Arc<RwLock<HashMap<String, TunnelState>>>,
    next_id: Arc<AtomicUsize>,
}

impl TcpTunnelManager {
    pub fn new() -> Self {
        Self {
            tunnels: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(AtomicUsize::new(1)),
        }
    }

    /// Spawns a TCP listener on `local_bind` and forwards connections to `remote_target`.
    pub async fn start_tunnel(
        &self,
        local_bind: &str,
        remote_target: &str,
    ) -> Result<TcpTunnelInfo> {
        let listener = TcpListener::bind(local_bind)
            .await
            .with_context(|| format!("Failed to bind TCP tunnel to {}", local_bind))?;

        let local_addr = listener.local_addr()?;
        let id_num = self.next_id.fetch_add(1, Ordering::SeqCst);
        let tunnel_id = format!("tunnel-{}", id_num);

        let now_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let active_conns = Arc::new(AtomicUsize::new(0));
        let total_conns = Arc::new(AtomicU64::new(0));
        let bytes_forwarded = Arc::new(AtomicU64::new(0));

        let initial_info = TcpTunnelInfo {
            tunnel_id: tunnel_id.clone(),
            local_addr: local_addr.to_string(),
            remote_target: remote_target.to_string(),
            created_at: now_unix,
            active_connections: 0,
            total_connections: 0,
            bytes_forwarded: 0,
        };

        let target_str = remote_target.to_string();
        let active_clone = Arc::clone(&active_conns);
        let total_clone = Arc::clone(&total_conns);
        let bytes_clone = Arc::clone(&bytes_forwarded);

        let join_handle = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((mut inbound, peer)) => {
                        debug!("Tunnel received connection from {}", peer);
                        total_clone.fetch_add(1, Ordering::Relaxed);
                        active_clone.fetch_add(1, Ordering::Relaxed);

                        let target_host = target_str.clone();
                        let active_inner = Arc::clone(&active_clone);
                        let bytes_inner = Arc::clone(&bytes_clone);

                        tokio::spawn(async move {
                            // Extract host for SSRF check
                            let host_part = target_host.split(':').next().unwrap_or(&target_host);
                            if NetworkSecurityFilter::is_cloud_metadata_service(host_part) {
                                warn!("Tunnel rejected SSRF blocked destination: {}", target_host);
                                active_inner.fetch_sub(1, Ordering::Relaxed);
                                return;
                            }

                            match TcpStream::connect(&target_host).await {
                                Ok(mut outbound) => {
                                    if let Ok((from_client, from_server)) =
                                        copy_bidirectional(&mut inbound, &mut outbound).await
                                    {
                                        bytes_inner.fetch_add(
                                            from_client + from_server,
                                            Ordering::Relaxed,
                                        );
                                    }
                                }
                                Err(e) => {
                                    warn!("Tunnel failed to connect to {}: {}", target_host, e);
                                }
                            }

                            active_inner.fetch_sub(1, Ordering::Relaxed);
                        });
                    }
                    Err(e) => {
                        error!("Tunnel listener accept failed: {}", e);
                        break;
                    }
                }
            }
        });

        let state = TunnelState {
            info: initial_info.clone(),
            abort_handle: join_handle.abort_handle(),
            active_connections: active_conns,
            total_connections: total_conns,
            bytes_forwarded,
        };

        {
            let mut map = self.tunnels.write().unwrap();
            map.insert(tunnel_id, state);
        }

        Ok(initial_info)
    }

    /// Stops and closes an active TCP tunnel.
    pub fn stop_tunnel(&self, tunnel_id: &str) -> Result<bool> {
        let mut map = self.tunnels.write().unwrap();
        if let Some(state) = map.remove(tunnel_id) {
            state.abort_handle.abort();
            debug!("Stopped TCP tunnel {}", tunnel_id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Lists all registered TCP tunnels with real-time connection counters.
    pub fn list_tunnels(&self) -> Vec<TcpTunnelInfo> {
        let map = self.tunnels.read().unwrap();
        map.values()
            .map(|state| {
                let mut info = state.info.clone();
                info.active_connections = state.active_connections.load(Ordering::Relaxed);
                info.total_connections = state.total_connections.load(Ordering::Relaxed);
                info.bytes_forwarded = state.bytes_forwarded.load(Ordering::Relaxed);
                info
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn test_tcp_tunnel_forwarding_lifecycle() {
        // 1. Start target echo server
        let echo_server = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target_addr = echo_server.local_addr().unwrap();

        tokio::spawn(async move {
            if let Ok((mut stream, _)) = echo_server.accept().await {
                let mut buf = [0u8; 4];
                let _ = stream.read_exact(&mut buf).await;
                assert_eq!(&buf, b"PING");
                let _ = stream.write_all(b"PONG").await;
            }
        });

        // 2. Start tunnel manager
        let manager = TcpTunnelManager::new();
        let info = manager
            .start_tunnel("127.0.0.1:0", &target_addr.to_string())
            .await
            .unwrap();
        assert!(info.tunnel_id.starts_with("tunnel-"));

        // 3. Connect to local tunnel address
        let mut client = TcpStream::connect(&info.local_addr).await.unwrap();
        client.write_all(b"PING").await.unwrap();

        let mut resp = [0u8; 4];
        client.read_exact(&mut resp).await.unwrap();
        assert_eq!(&resp, b"PONG");

        // 4. Verify tunnel counters
        let tunnels = manager.list_tunnels();
        assert_eq!(tunnels.len(), 1);
        assert_eq!(tunnels[0].total_connections, 1);

        // 5. Stop tunnel
        assert!(manager.stop_tunnel(&info.tunnel_id).unwrap());
        assert!(manager.list_tunnels().is_empty());
    }
}
