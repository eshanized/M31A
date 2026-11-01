//! Hardened Responses API reverse proxy with memory-locked credential relay and trace dumper.
//! Local HTTP proxy recording and replaying model request and response envelopes.

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

/// Server identification payload written to disk upon successful bind.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerInfo {
    pub port: u16,
    pub pid: u32,
}

/// Secure memory-locked credential wrapper.
/// On Unix, pins token memory in physical RAM via `mlock(2)` to prevent swapping to disk.
pub struct LockedCredential {
    bytes: Vec<u8>,
}

impl LockedCredential {
    pub fn new(token: &str) -> Self {
        let bearer_prefix = b"Bearer ";
        let mut bytes = Vec::with_capacity(bearer_prefix.len() + token.len());
        bytes.extend_from_slice(bearer_prefix);
        bytes.extend_from_slice(token.trim().as_bytes());

        #[cfg(unix)]
        unsafe {
            if !bytes.is_empty() {
                libc::mlock(bytes.as_ptr() as *const libc::c_void, bytes.len());
            }
        }

        Self { bytes }
    }

    pub fn as_header_value(&self) -> &str {
        std::str::from_utf8(&self.bytes).unwrap_or("Bearer [INVALID_TOKEN]")
    }
}

impl Drop for LockedCredential {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            if !self.bytes.is_empty() {
                libc::munlock(self.bytes.as_ptr() as *const libc::c_void, self.bytes.len());
            }
        }
        // Zero out memory to eliminate lingering traces
        self.bytes.fill(0);
    }
}

/// Configuration options for the Responses API Reverse Proxy.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResponsesProxyConfig {
    pub listen_addr: String,
    pub listen_port: u16,
    pub upstream_url: String,
    pub dump_dir: Option<PathBuf>,
    pub server_info_path: Option<PathBuf>,
    pub enable_http_shutdown: bool,
    #[serde(skip)]
    pub auth_token: Option<String>,
}

impl Default for ResponsesProxyConfig {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1".to_string(),
            listen_port: 0, // Ephemeral by default
            upstream_url: "https://api.openai.com/v1/responses".to_string(),
            dump_dir: None,
            server_info_path: None,
            enable_http_shutdown: true,
            auth_token: None,
        }
    }
}

/// Active running instance of the responses proxy.
pub struct ResponsesProxyHandle {
    port: u16,
    shutdown_tx: watch::Sender<bool>,
    server_task: tokio::task::JoinHandle<()>,
}

impl ResponsesProxyHandle {
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub async fn shutdown(self) {
        let _ = self.shutdown_tx.send(true);
        let _ = self.server_task.await;
    }
}

pub struct ResponsesProxyServer;

impl ResponsesProxyServer {
    /// Launches the responses proxy server background listener.
    pub async fn start(config: ResponsesProxyConfig) -> Result<ResponsesProxyHandle> {
        let bind_target = format!("{}:{}", config.listen_addr, config.listen_port);
        let listener = TcpListener::bind(&bind_target)
            .await
            .with_context(|| format!("Failed to bind responses proxy on {}", bind_target))?;

        let actual_port = listener.local_addr()?.port();
        let pid = std::process::id();

        // Write server info handshake if configured
        if let Some(ref path) = config.server_info_path {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let info = ServerInfo {
                port: actual_port,
                pid,
            };
            let serialized = serde_json::to_string_pretty(&info)?;
            fs::write(path, serialized)?;
        }

        // Prepare dump directory if configured
        if let Some(ref dump_dir) = config.dump_dir {
            fs::create_dir_all(dump_dir)?;
        }

        let locked_cred = config
            .auth_token
            .as_ref()
            .map(|tok| Arc::new(LockedCredential::new(tok)));
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let seq = Arc::new(AtomicU64::new(1));

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()?;

        let config_arc = Arc::new(config);
        let shutdown_tx_clone = shutdown_tx.clone();

        let server_task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }
                    }
                    res = listener.accept() => {
                        match res {
                            Ok((stream, _peer)) => {
                                let client = client.clone();
                                let config = config_arc.clone();
                                let locked_cred = locked_cred.clone();
                                let seq = seq.clone();
                                let shutdown_tx = shutdown_tx_clone.clone();

                                tokio::spawn(async move {
                                    if let Err(e) = Self::handle_connection(
                                        stream,
                                        client,
                                        config,
                                        locked_cred,
                                        seq,
                                        shutdown_tx,
                                    ).await {
                                        tracing::debug!("Proxy connection error: {:?}", e);
                                    }
                                });
                            }
                            Err(e) => {
                                tracing::debug!("Proxy accept error: {:?}", e);
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(ResponsesProxyHandle {
            port: actual_port,
            shutdown_tx,
            server_task,
        })
    }

    async fn handle_connection(
        mut stream: TcpStream,
        client: reqwest::Client,
        config: Arc<ResponsesProxyConfig>,
        locked_cred: Option<Arc<LockedCredential>>,
        seq: Arc<AtomicU64>,
        shutdown_tx: watch::Sender<bool>,
    ) -> Result<()> {
        let mut buffer = vec![0u8; 65536];
        let bytes_read = stream.read(&mut buffer).await?;
        if bytes_read == 0 {
            return Ok(());
        }

        let request_str = String::from_utf8_lossy(&buffer[..bytes_read]);
        let mut lines = request_str.lines();
        let request_line = lines.next().unwrap_or("");
        let mut parts = request_line.split_whitespace();

        let method = parts.next().unwrap_or("");
        let path = parts.next().unwrap_or("");

        // Optional HTTP shutdown endpoint
        if method == "GET" && path == "/shutdown" {
            if config.enable_http_shutdown {
                let resp = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK";
                stream.write_all(resp.as_bytes()).await?;
                let _ = shutdown_tx.send(true);
                return Ok(());
            } else {
                let resp = "HTTP/1.1 403 Forbidden\r\nContent-Length: 9\r\nConnection: close\r\n\r\nForbidden";
                stream.write_all(resp.as_bytes()).await?;
                return Ok(());
            }
        }

        // Only POST /v1/responses or /v1/chat/completions are accepted
        let is_allowed_endpoint = method == "POST"
            && (path == "/v1/responses"
                || path == "/v1/chat/completions"
                || path.starts_with("/v1/responses?")
                || path.starts_with("/v1/chat/completions?"));

        if !is_allowed_endpoint {
            let resp =
                "HTTP/1.1 403 Forbidden\r\nContent-Length: 9\r\nConnection: close\r\n\r\nForbidden";
            stream.write_all(resp.as_bytes()).await?;
            return Ok(());
        }

        // Parse body boundary
        let body_bytes = if let Some(body_start) = request_str.find("\r\n\r\n") {
            &buffer[body_start + 4..bytes_read]
        } else {
            &[]
        };

        let request_id = seq.fetch_add(1, Ordering::SeqCst);
        let timestamp_ms = Utc::now().timestamp_millis();

        // Write request dump if requested
        if let Some(ref dump_dir) = config.dump_dir {
            Self::dump_request(dump_dir, request_id, timestamp_ms, method, path, body_bytes);
        }

        // Build upstream request
        let mut req = client.post(&config.upstream_url);
        if let Some(ref cred) = locked_cred {
            req = req.header("Authorization", cred.as_header_value());
        }
        req = req.header("Content-Type", "application/json");
        req = req.body(body_bytes.to_vec());

        match req.send().await {
            Ok(upstream_resp) => {
                let status = upstream_resp.status();
                let status_code = status.as_u16();
                let status_text = status.canonical_reason().unwrap_or("OK");
                let resp_bytes = upstream_resp.bytes().await.unwrap_or_default();

                if let Some(ref dump_dir) = config.dump_dir {
                    Self::dump_response(
                        dump_dir,
                        request_id,
                        timestamp_ms,
                        status_code,
                        &resp_bytes,
                    );
                }

                let response_header = format!(
                    "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    status_code,
                    status_text,
                    resp_bytes.len()
                );

                stream.write_all(response_header.as_bytes()).await?;
                stream.write_all(&resp_bytes).await?;
            }
            Err(e) => {
                let err_msg = format!("{{\"error\": \"Upstream proxy failed: {}\"}}", e);
                let response = format!(
                    "HTTP/1.1 502 Bad Gateway\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    err_msg.len(),
                    err_msg
                );
                stream.write_all(response.as_bytes()).await?;
            }
        }

        Ok(())
    }

    fn dump_request(
        dump_dir: &Path,
        seq: u64,
        timestamp_ms: i64,
        method: &str,
        path: &str,
        body: &[u8],
    ) {
        let filename = format!("{:06}-{}-request.json", seq, timestamp_ms);
        let out_path = dump_dir.join(filename);

        let body_json: serde_json::Value = serde_json::from_slice(body).unwrap_or_else(|_| {
            serde_json::Value::String(String::from_utf8_lossy(body).to_string())
        });

        let dump_record = serde_json::json!({
            "sequence": seq,
            "timestamp_ms": timestamp_ms,
            "method": method,
            "path": path,
            "headers": {
                "authorization": "[REDACTED]",
                "content-type": "application/json"
            },
            "body": body_json
        });

        if let Ok(formatted) = serde_json::to_string_pretty(&dump_record) {
            let _ = fs::write(out_path, formatted);
        }
    }

    fn dump_response(dump_dir: &Path, seq: u64, timestamp_ms: i64, status: u16, body: &[u8]) {
        let filename = format!("{:06}-{}-response.json", seq, timestamp_ms);
        let out_path = dump_dir.join(filename);

        let body_json: serde_json::Value = serde_json::from_slice(body).unwrap_or_else(|_| {
            serde_json::Value::String(String::from_utf8_lossy(body).to_string())
        });

        let dump_record = serde_json::json!({
            "sequence": seq,
            "timestamp_ms": timestamp_ms,
            "status": status,
            "headers": {
                "content-type": "application/json"
            },
            "body": body_json
        });

        if let Ok(formatted) = serde_json::to_string_pretty(&dump_record) {
            let _ = fs::write(out_path, formatted);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_locked_credential_lifecycle() {
        let cred = LockedCredential::new("sk-test-secret-key-12345");
        assert_eq!(cred.as_header_value(), "Bearer sk-test-secret-key-12345");
        // Drop safely zeroes buffer
        drop(cred);
    }

    #[tokio::test]
    async fn test_proxy_handshake_and_shutdown() {
        let tmp = tempdir().expect("tempdir");
        let server_info = tmp.path().join("server-info.json");
        let dump_dir = tmp.path().join("dumps");

        let config = ResponsesProxyConfig {
            listen_port: 0,
            server_info_path: Some(server_info.clone()),
            dump_dir: Some(dump_dir.clone()),
            auth_token: Some("sk-test-token".to_string()),
            ..Default::default()
        };

        let handle = ResponsesProxyServer::start(config)
            .await
            .expect("start proxy");
        let port = handle.port();
        assert!(port > 0);

        // Verify server info file was generated
        assert!(server_info.exists());
        let info_str = fs::read_to_string(&server_info).expect("read server info");
        let info: ServerInfo = serde_json::from_str(&info_str).expect("parse server info");
        assert_eq!(info.port, port);

        // Send GET /shutdown
        let base = format!("http://127.0.0.1:{}/shutdown", port);
        let resp = reqwest::get(&base).await.expect("shutdown request");
        assert_eq!(resp.status(), 200);

        handle.shutdown().await;
    }

    #[tokio::test]
    async fn test_proxy_rejects_unauthorized_endpoints() {
        let config = ResponsesProxyConfig {
            listen_port: 0,
            enable_http_shutdown: false,
            ..Default::default()
        };

        let handle = ResponsesProxyServer::start(config)
            .await
            .expect("start proxy");
        let port = handle.port();

        // GET /unauthorized should return 403
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://127.0.0.1:{}/unauthorized", port))
            .send()
            .await
            .expect("send GET");
        assert_eq!(resp.status(), 403);

        handle.shutdown().await;
    }
}
