//! Dynamic agent runtime instance registration and composite bootstrap.
//! Registers dynamic subagent instances, tracking composite capabilities and connection endpoints.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Default upstream registration endpoint.
pub const DEFAULT_REGISTRATION_URL: &str = "https://api.m31a.org/agent/register";

/// Maximum request body bytes allowed for registration payloads (64 KiB).
pub const MAX_BODY_BYTES: usize = 64 * 1024;

/// Default timeout for registration network attempts.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Payload submitted to register a running agent instance.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeRegistrationRequest {
    pub instance_id: String,
    pub agent_role: String,
    pub public_key: String,
    pub capabilities: Vec<String>,
    pub metadata: HashMap<String, String>,
}

/// Confirmation and credentials returned upon successful registration.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeRegistrationResponse {
    pub token: String,
    pub expires_at: i64,
    pub assigned_node: String,
}

/// Validates registration URL constraints and executes bounded registration requests.
#[derive(Clone, Debug)]
pub struct RuntimeRegistrationClient {
    endpoint: String,
    timeout: Duration,
}

impl RuntimeRegistrationClient {
    /// Validates endpoint URL according to SSRF security rules:
    /// Must be HTTPS, or HTTP only if loopback. Rejects credentials, queries, and fragments.
    pub fn new(endpoint: impl Into<String>) -> Result<Self> {
        let ep = endpoint.into();
        Self::validate_endpoint(&ep)?;
        Ok(Self {
            endpoint: ep,
            timeout: REQUEST_TIMEOUT,
        })
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    fn validate_endpoint(url_str: &str) -> Result<()> {
        let (scheme, rest) = if let Some(idx) = url_str.find("://") {
            (&url_str[..idx], &url_str[idx + 3..])
        } else {
            anyhow::bail!("Invalid URL scheme");
        };

        if url_str.contains('?') || url_str.contains('#') {
            anyhow::bail!("Registration endpoint must not contain query parameters or fragments");
        }

        let host = if let Some(idx) = rest.find('/') {
            &rest[..idx]
        } else {
            rest
        };

        if host.contains('@') {
            anyhow::bail!("Registration endpoint must not contain credentials");
        }

        let is_loopback = host.starts_with("localhost")
            || host.starts_with("127.0.0.1")
            || host.starts_with("[::1]");

        if scheme == "https" || (scheme == "http" && is_loopback) {
            Ok(())
        } else {
            anyhow::bail!("Non-loopback endpoints must use HTTPS scheme");
        }
    }

    /// Verifies payload size bounds and prepares registration.
    pub fn validate_payload(&self, request: &RuntimeRegistrationRequest) -> Result<Vec<u8>> {
        let bytes =
            serde_json::to_vec(request).context("Failed to serialize registration request")?;
        if bytes.len() > MAX_BODY_BYTES {
            anyhow::bail!(
                "Registration payload exceeds maximum allowed size of {} bytes ({} bytes provided)",
                MAX_BODY_BYTES,
                bytes.len()
            );
        }
        Ok(bytes)
    }

    /// Executes the real instance registration exchange over HTTP POST.
    pub async fn register(
        &self,
        request: &RuntimeRegistrationRequest,
    ) -> Result<RuntimeRegistrationResponse> {
        let payload = self.validate_payload(request)?;

        let client = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .context("Failed to construct HTTP client for agent registration")?;

        let response = client
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .header("User-Agent", "m31a-runtime/1.0")
            .body(payload)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to reach registration endpoint at '{}': {}", self.endpoint, e))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "Upstream registration rejected with HTTP {}: {}",
                status,
                text
            );
        }

        let reg_response: RuntimeRegistrationResponse = response
            .json()
            .await
            .context("Failed to parse registration response JSON from upstream server")?;

        Ok(reg_response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_endpoints() {
        assert!(RuntimeRegistrationClient::new("https://api.m31a.org/agent/register").is_ok());
        assert!(RuntimeRegistrationClient::new("http://127.0.0.1:8080/register").is_ok());
        assert!(RuntimeRegistrationClient::new("http://localhost:3000/register").is_ok());
    }

    #[test]
    fn test_invalid_endpoints_rejected() {
        // Plain HTTP non-loopback
        assert!(RuntimeRegistrationClient::new("http://example.com/register").is_err());
        // Credentials in URL
        assert!(RuntimeRegistrationClient::new("https://user:pass@example.com/register").is_err());
        // Query parameters
        assert!(RuntimeRegistrationClient::new("https://example.com/register?foo=bar").is_err());
        // Fragments
        assert!(RuntimeRegistrationClient::new("https://example.com/register#section").is_err());
    }

    #[tokio::test]
    async fn test_registration_lifecycle() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{}/agent/register", port);

        // Spawn mock upstream server accepting 1 real HTTP POST request
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 4096];
            let n = socket.read(&mut buf).await.unwrap();
            let req_str = String::from_utf8_lossy(&buf[..n]);
            assert!(req_str.starts_with("POST /agent/register HTTP/1.1"));
            assert!(req_str.contains("user-agent: m31a-runtime/1.0"));
            assert!(req_str.contains("content-type: application/json"));

            let response_body = serde_json::json!({
                "token": "inst_tok_real_test_12345",
                "expires_at": 1799999999i64,
                "assigned_node": "node_cluster_alpha"
            });
            let body_str = response_body.to_string();
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body_str.len(),
                body_str
            );
            socket.write_all(resp.as_bytes()).await.unwrap();
        });

        let client = RuntimeRegistrationClient::new(&url).unwrap();

        let req = RuntimeRegistrationRequest {
            instance_id: "agent_runner_1".to_string(),
            agent_role: "coder".to_string(),
            public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIG123".to_string(),
            capabilities: vec!["file_read".to_string(), "bash_exec".to_string()],
            metadata: HashMap::new(),
        };

        let resp = client
            .register(&req)
            .await
            .expect("real HTTP registration must succeed");
        assert_eq!(resp.token, "inst_tok_real_test_12345");
        assert_eq!(resp.expires_at, 1799999999);
        assert_eq!(resp.assigned_node, "node_cluster_alpha");
    }

    #[tokio::test]
    async fn test_registration_network_failure_handling() {
        // Test connection to unreachable port
        let client = RuntimeRegistrationClient::new("http://127.0.0.1:59999/agent/register").unwrap();
        let req = RuntimeRegistrationRequest {
            instance_id: "agent_runner_fail".to_string(),
            agent_role: "coder".to_string(),
            public_key: "key".to_string(),
            capabilities: vec![],
            metadata: HashMap::new(),
        };
        let err = client.register(&req).await.unwrap_err();
        assert!(err.to_string().contains("Failed to reach registration endpoint"));
    }

    #[test]
    fn test_oversized_payload_rejection() {
        let client = RuntimeRegistrationClient::new("https://api.m31a.org/agent/register").unwrap();
        let huge_val = "x".repeat(MAX_BODY_BYTES + 1);

        let mut meta = HashMap::new();
        meta.insert("bloat".to_string(), huge_val);

        let req = RuntimeRegistrationRequest {
            instance_id: "agent_runner_2".to_string(),
            agent_role: "bloated".to_string(),
            public_key: "key".to_string(),
            capabilities: Vec::new(),
            metadata: meta,
        };

        let err = client.validate_payload(&req).unwrap_err();
        assert!(err.to_string().contains("exceeds maximum allowed size"));
    }
}
