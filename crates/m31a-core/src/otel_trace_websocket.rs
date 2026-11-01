//! Forwards loopback OTLP trace batches to a WebSocket broadcast channel for live streaming observability.

use anyhow::{Context, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

/// Channel capacity for broadcast telemetry batches.
pub const TRACE_CHANNEL_CAPACITY: usize = 1_024;

/// Maximum size allowed for a single trace websocket frame (4 MiB).
pub const MAX_TRACE_WEBSOCKET_FRAME_BYTES: usize = 4 * 1024 * 1024;

/// Owns the loopback OTLP receiver endpoint and trace WebSocket broadcaster.
#[derive(Clone)]
pub struct TraceWebSocket {
    exporter_endpoint: String,
    listen_addr: SocketAddr,
    sender: Arc<broadcast::Sender<Vec<u8>>>,
}

impl TraceWebSocket {
    /// Binds listener sockets on local interfaces.
    pub async fn bind(listen_url: &str) -> Result<Self> {
        let bind_address = if let Some(stripped) = listen_url.strip_prefix("ws://") {
            stripped.parse::<SocketAddr>().with_context(|| {
                format!("invalid --otel-trace-listen URL '{listen_url}'; expected 'ws://IP:PORT'")
            })?
        } else {
            listen_url
                .parse::<SocketAddr>()
                .with_context(|| format!("invalid socket address '{listen_url}'"))?
        };

        let websocket_listener = TcpListener::bind(bind_address)
            .await
            .with_context(|| format!("failed to bind OTEL trace websocket to {bind_address}"))?;
        let listen_addr = websocket_listener
            .local_addr()
            .context("failed to read OTEL trace websocket listen address")?;

        let otlp_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .context("failed to bind loopback OTLP trace receiver")?;
        let otlp_addr = otlp_listener
            .local_addr()
            .context("failed to read OTLP trace receiver listen address")?;

        let exporter_endpoint = format!("http://{otlp_addr}/v1/traces");
        let (sender, _) = broadcast::channel(TRACE_CHANNEL_CAPACITY);

        Ok(Self {
            exporter_endpoint,
            listen_addr,
            sender: Arc::new(sender),
        })
    }

    /// Exporter endpoint URL where local telemetry agents send OTLP POST requests.
    pub fn exporter_endpoint(&self) -> &str {
        &self.exporter_endpoint
    }

    /// WebSocket listen address for external listeners.
    pub fn listen_addr(&self) -> SocketAddr {
        self.listen_addr
    }

    /// Broadcasts a trace payload frame to all active subscribers.
    pub fn publish_trace_batch(&self, batch: Vec<u8>) -> Result<usize> {
        if batch.len() > MAX_TRACE_WEBSOCKET_FRAME_BYTES {
            anyhow::bail!(
                "trace frame exceeds maximum allowed size of {} bytes ({} bytes provided)",
                MAX_TRACE_WEBSOCKET_FRAME_BYTES,
                batch.len()
            );
        }
        let receiver_count = self.sender.send(batch).unwrap_or(0);
        Ok(receiver_count)
    }

    /// Subscribes to the broadcast stream of OTLP trace frames.
    pub fn subscribe(&self) -> broadcast::Receiver<Vec<u8>> {
        self.sender.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_trace_websocket_bind_and_broadcast() {
        let bridge = TraceWebSocket::bind("ws://127.0.0.1:0").await.unwrap();
        assert!(bridge.exporter_endpoint().starts_with("http://127.0.0.1:"));
        assert!(bridge.listen_addr().port() > 0);

        let mut rx = bridge.subscribe();
        let payload = b"{\"resourceSpans\":[{\"scopeSpans\":[]}]}".to_vec();

        let count = bridge.publish_trace_batch(payload.clone()).unwrap();
        assert_eq!(count, 1);

        let received = rx.recv().await.unwrap();
        assert_eq!(received, payload);
    }

    #[tokio::test]
    async fn test_trace_websocket_oversized_rejection() {
        let bridge = TraceWebSocket::bind("ws://127.0.0.1:0").await.unwrap();
        let huge = vec![0u8; MAX_TRACE_WEBSOCKET_FRAME_BYTES + 1];
        let err = bridge.publish_trace_batch(huge).unwrap_err();
        assert!(err.to_string().contains("exceeds maximum allowed size"));
    }
}
