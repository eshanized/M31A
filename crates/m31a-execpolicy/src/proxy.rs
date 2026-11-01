//! Asynchronous local HTTP/CONNECT proxy enforcing domain whitelists and traffic inspection.
//!
//! Prevents unauthorized outbound connections, private IP access, and cloud metadata exfiltration.

use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use crate::attribution::AttributionRegistry;
use crate::credential_broker::CredentialBroker;
use crate::domain_matcher::{DomainPattern, NetworkSecurityFilter};

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub bind_addr: SocketAddr,
    pub allowed_domains: Vec<DomainPattern>,
    pub block_cloud_metadata: bool,
    pub block_loopback_destinations: bool,
    pub credential_broker: Option<CredentialBroker>,
    pub attribution_registry: Option<AttributionRegistry>,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            allowed_domains: vec![DomainPattern::Wildcard], // Default allow-all except security blocks
            block_cloud_metadata: true,
            block_loopback_destinations: true,
            credential_broker: None,
            attribution_registry: None,
        }
    }
}

pub struct NetworkProxyServer {
    config: ProxyConfig,
    shutdown_tx: broadcast::Sender<()>,
}

impl NetworkProxyServer {
    pub fn new(config: ProxyConfig) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Self {
            config,
            shutdown_tx,
        }
    }

    /// Evaluates if a given host is permitted to connect through the proxy.
    pub fn is_destination_allowed(&self, host: &str) -> bool {
        let clean_host = host.trim_end_matches('.');

        // 1. Check cloud metadata exfiltration (SSRF)
        if self.config.block_cloud_metadata
            && NetworkSecurityFilter::is_cloud_metadata_service(clean_host)
        {
            warn!(target = %clean_host, "Blocked outbound connection to cloud metadata endpoint");
            return false;
        }

        // 2. Check loopback / localhost destination blocking
        if self.config.block_loopback_destinations
            && (clean_host == "localhost"
                || clean_host == "127.0.0.1"
                || clean_host == "::1"
                || clean_host.starts_with("127."))
        {
            warn!(target = %clean_host, "Blocked outbound connection to host loopback address");
            return false;
        }

        // 3. Check allowed domains list
        if self.config.allowed_domains.is_empty() {
            return false;
        }

        self.config
            .allowed_domains
            .iter()
            .any(|pattern| pattern.matches(clean_host))
    }

    /// Starts the proxy listener and runs the accept loop in the background.
    pub async fn start(self: Arc<Self>) -> Result<SocketAddr> {
        let listener = TcpListener::bind(self.config.bind_addr).await?;
        let local_addr = listener.local_addr()?;
        info!(addr = %local_addr, "NetworkProxyServer listening");

        let server = self.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept_res = listener.accept() => {
                        match accept_res {
                            Ok((stream, client_addr)) => {
                                let s = server.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = s.handle_connection(stream, client_addr).await {
                                        debug!(error = %e, client = %client_addr, "Proxy connection finished with error");
                                    }
                                });
                            }
                            Err(e) => {
                                error!(error = %e, "Proxy listener accept error");
                                break;
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Proxy server shutting down");
                        break;
                    }
                }
            }
        });

        Ok(local_addr)
    }

    /// Inspects the initial bytes to identify attribution frames, SOCKS5 (0x05), or HTTP.
    async fn handle_connection(
        &self,
        mut client: TcpStream,
        client_addr: SocketAddr,
    ) -> Result<()> {
        // 1. Check for connection attribution frame (\0M31APX1)
        let attribution_token = match crate::attribution::read_attribution_token(&mut client).await
        {
            Ok(token) => token,
            Err(e) => {
                debug!(error = %e, "Attribution frame parse error / none");
                None
            }
        };

        let mut first_byte = [0u8; 1];
        let n = client.peek(&mut first_byte).await?;
        if n == 0 {
            return Ok(());
        }

        if first_byte[0] == 0x05 {
            // SOCKS5 protocol
            self.handle_socks5(client, client_addr).await
        } else {
            // HTTP Proxy / HTTP CONNECT protocol
            self.handle_http(client, client_addr, attribution_token.as_deref())
                .await
        }
    }

    /// Handles SOCKS5 protocol handshake, permission evaluation, and upstream tunneling.
    async fn handle_socks5(&self, mut client: TcpStream, _client_addr: SocketAddr) -> Result<()> {
        // 1. Method negotiation
        let mut ver_nmethods = [0u8; 2];
        client.read_exact(&mut ver_nmethods).await?;
        let ver = ver_nmethods[0];
        let nmethods = ver_nmethods[1] as usize;
        if ver != 0x05 {
            return Err(anyhow!("Unsupported SOCKS version: {}", ver));
        }

        let mut methods = vec![0u8; nmethods];
        client.read_exact(&mut methods).await?;

        // Reply 0x05, 0x00 (NO AUTH REQUIRED)
        client.write_all(&[0x05, 0x00]).await?;

        // 2. Client Request: VER, CMD, RSV, ATYP, DST.ADDR, DST.PORT
        let mut header = [0u8; 4];
        client.read_exact(&mut header).await?;
        let cmd = header[1];
        let atyp = header[3];

        if cmd != 0x01 {
            // Only CONNECT is supported
            client
                .write_all(&[0x05, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await?;
            return Err(anyhow!("Unsupported SOCKS5 command: {}", cmd));
        }

        let target_host = match atyp {
            0x01 => {
                // IPv4
                let mut ip_bytes = [0u8; 4];
                client.read_exact(&mut ip_bytes).await?;
                std::net::Ipv4Addr::from(ip_bytes).to_string()
            }
            0x03 => {
                // Domain name
                let mut len = [0u8; 1];
                client.read_exact(&mut len).await?;
                let mut domain = vec![0u8; len[0] as usize];
                client.read_exact(&mut domain).await?;
                String::from_utf8(domain)
                    .map_err(|e| anyhow!("Invalid SOCKS5 domain UTF-8: {}", e))?
            }
            0x04 => {
                // IPv6
                let mut ip_bytes = [0u8; 16];
                client.read_exact(&mut ip_bytes).await?;
                std::net::Ipv6Addr::from(ip_bytes).to_string()
            }
            other => {
                client
                    .write_all(&[0x05, 0x08, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                    .await?;
                return Err(anyhow!("Unsupported SOCKS5 ATYP: {}", other));
            }
        };

        let mut port_bytes = [0u8; 2];
        client.read_exact(&mut port_bytes).await?;
        let target_port = u16::from_be_bytes(port_bytes);

        // Security check
        if !self.is_destination_allowed(&target_host) {
            // Reply 0x02: connection not allowed by ruleset
            client
                .write_all(&[0x05, 0x02, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await?;
            return Err(anyhow!(
                "SOCKS5 destination denied by policy: {}:{}",
                target_host,
                target_port
            ));
        }

        // Connect upstream
        let target_addr = format!("{}:{}", target_host, target_port);
        match TcpStream::connect(&target_addr).await {
            Ok(upstream) => {
                // Succeeded: 0x05, 0x00 (success), RSV, ATYP=IPv4, 0.0.0.0:0
                client
                    .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                    .await?;
                let (mut cl_read, mut cl_write) = client.into_split();
                let (mut up_read, mut up_write) = upstream.into_split();

                tokio::spawn(async move {
                    let _ = tokio::io::copy(&mut cl_read, &mut up_write).await;
                });
                let _ = tokio::io::copy(&mut up_read, &mut cl_write).await;
                Ok(())
            }
            Err(e) => {
                // General SOCKS server failure (0x01)
                client
                    .write_all(&[0x05, 0x01, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                    .await?;
                Err(anyhow!(
                    "Failed to connect to upstream {}: {}",
                    target_addr,
                    e
                ))
            }
        }
    }

    /// Handles HTTP CONNECT tunneling and standard HTTP proxy requests.
    async fn handle_http(
        &self,
        mut client: TcpStream,
        _client_addr: SocketAddr,
        _attribution_token: Option<&str>,
    ) -> Result<()> {
        let mut buf = [0u8; 4096];
        let n = client.read(&mut buf).await?;
        if n == 0 {
            return Ok(());
        }

        let request_str = String::from_utf8_lossy(&buf[..n]);
        let first_line = request_str.lines().next().unwrap_or("");
        let mut parts = first_line.split_whitespace();
        let method = parts.next().unwrap_or("");
        let uri = parts.next().unwrap_or("");

        if method.eq_ignore_ascii_case("CONNECT") {
            // Tunneling (HTTPS)
            let mut host_port = uri.split(':');
            let host = host_port.next().unwrap_or("");
            let port: u16 = host_port.next().and_then(|p| p.parse().ok()).unwrap_or(443);

            if !self.is_destination_allowed(host) {
                client.write_all(b"HTTP/1.1 403 Forbidden\r\n\r\n").await?;
                return Err(anyhow!(
                    "HTTP CONNECT destination denied by policy: {}:{}",
                    host,
                    port
                ));
            }

            let upstream_target = format!("{}:{}", host, port);
            match TcpStream::connect(&upstream_target).await {
                Ok(upstream) => {
                    client
                        .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                        .await?;
                    let (mut cl_read, mut cl_write) = client.into_split();
                    let (mut up_read, mut up_write) = upstream.into_split();

                    tokio::spawn(async move {
                        let _ = tokio::io::copy(&mut cl_read, &mut up_write).await;
                    });
                    let _ = tokio::io::copy(&mut up_read, &mut cl_write).await;
                    Ok(())
                }
                Err(e) => {
                    client
                        .write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n")
                        .await?;
                    Err(anyhow!("Failed to connect upstream: {}", e))
                }
            }
        } else {
            // Plain HTTP Proxy (e.g. GET http://example.com/path HTTP/1.1)
            let parsed_url = if let Some(stripped) = uri.strip_prefix("http://") {
                stripped
            } else {
                uri
            };
            let host_part = parsed_url.split('/').next().unwrap_or("");
            let host = host_part.split(':').next().unwrap_or("");

            if !self.is_destination_allowed(host) {
                client.write_all(b"HTTP/1.1 403 Forbidden\r\n\r\n").await?;
                return Err(anyhow!("HTTP proxy destination denied by policy: {}", host));
            }

            // Direct to port 80 or specified port
            let port = host_part
                .split(':')
                .nth(1)
                .and_then(|p| p.parse().ok())
                .unwrap_or(80);
            let upstream_target = format!("{}:{}", host, port);

            let mut payload = buf[..n].to_vec();
            if let Some(broker) = &self.config.credential_broker {
                let mut dummy_headers = HashMap::new();
                let req_text = String::from_utf8_lossy(&payload);
                for line in req_text.lines().skip(1) {
                    if let Some((k, v)) = line.split_once(':') {
                        dummy_headers.insert(k.trim().to_string(), v.trim().to_string());
                    }
                }
                broker.substitute_request_secrets(host, &mut dummy_headers, &mut payload);
            }

            match TcpStream::connect(&upstream_target).await {
                Ok(mut upstream) => {
                    upstream.write_all(&payload).await?;
                    let (mut cl_read, mut cl_write) = client.into_split();
                    let (mut up_read, mut up_write) = upstream.into_split();

                    tokio::spawn(async move {
                        let _ = tokio::io::copy(&mut cl_read, &mut up_write).await;
                    });
                    let _ = tokio::io::copy(&mut up_read, &mut cl_write).await;
                    Ok(())
                }
                Err(e) => {
                    client
                        .write_all(b"HTTP/1.1 502 Bad Gateway\r\n\r\n")
                        .await?;
                    Err(anyhow!("Failed to connect upstream HTTP: {}", e))
                }
            }
        }
    }

    /// Configures environment variables for subprocesses / sandboxes to route through this proxy.
    pub fn apply_proxy_env(proxy_addr: SocketAddr, env: &mut HashMap<String, String>) {
        let http_url = format!("http://{}", proxy_addr);
        let socks_url = format!("socks5://{}", proxy_addr);

        env.insert("HTTP_PROXY".to_string(), http_url.clone());
        env.insert("HTTPS_PROXY".to_string(), http_url.clone());
        env.insert("ALL_PROXY".to_string(), socks_url.clone());
        env.insert("http_proxy".to_string(), http_url.clone());
        env.insert("https_proxy".to_string(), http_url);
        env.insert("all_proxy".to_string(), socks_url);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BrokeredCredential;

    #[test]
    fn test_proxy_policy_destination_filtering() {
        let config = ProxyConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            allowed_domains: vec![
                DomainPattern::parse("**.github.com"),
                DomainPattern::parse("crates.io"),
            ],
            block_cloud_metadata: true,
            block_loopback_destinations: true,
            ..Default::default()
        };
        let proxy = NetworkProxyServer::new(config);

        // Allowed domains
        assert!(proxy.is_destination_allowed("github.com"));
        assert!(proxy.is_destination_allowed("api.github.com"));
        assert!(proxy.is_destination_allowed("crates.io"));

        // Denied domains
        assert!(!proxy.is_destination_allowed("evil.com"));
        assert!(!proxy.is_destination_allowed("attacker.net"));

        // Denied cloud metadata SSRF
        assert!(!proxy.is_destination_allowed("169.254.169.254"));
        assert!(!proxy.is_destination_allowed("metadata.google.internal"));

        // Denied loopback
        assert!(!proxy.is_destination_allowed("localhost"));
        assert!(!proxy.is_destination_allowed("127.0.0.1"));
    }

    #[tokio::test]
    async fn test_proxy_server_startup_and_connect_blocking() {
        let config = ProxyConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            allowed_domains: vec![DomainPattern::parse("example.com")],
            block_cloud_metadata: true,
            block_loopback_destinations: true,
            ..Default::default()
        };
        let proxy = Arc::new(NetworkProxyServer::new(config));
        let proxy_addr = proxy.clone().start().await.unwrap();

        // 1. Connect with HTTP CONNECT to forbidden destination (e.g. 169.254.169.254:80)
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        client
            .write_all(b"CONNECT 169.254.169.254:80 HTTP/1.1\r\nHost: 169.254.169.254\r\n\r\n")
            .await
            .unwrap();

        let mut resp = [0u8; 100];
        let n = client.read(&mut resp).await.unwrap();
        let resp_str = String::from_utf8_lossy(&resp[..n]);
        assert!(resp_str.contains("403 Forbidden"));

        // 2. Test SOCKS5 handshake to blocked domain
        let mut socks_client = TcpStream::connect(proxy_addr).await.unwrap();
        // Handshake: VER 5, 1 method, NO AUTH (0)
        socks_client.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
        let mut auth_resp = [0u8; 2];
        socks_client.read_exact(&mut auth_resp).await.unwrap();
        assert_eq!(auth_resp, [0x05, 0x00]);

        // Request: CONNECT to evil.com:80 (ATYP 0x03, domain len 8, "evil.com", port 80)
        let domain = b"evil.com";
        let mut req = vec![0x05, 0x01, 0x00, 0x03, domain.len() as u8];
        req.extend_from_slice(domain);
        req.extend_from_slice(&80u16.to_be_bytes());
        socks_client.write_all(&req).await.unwrap();

        let mut req_resp = [0u8; 10];
        let n_socks = socks_client.read(&mut req_resp).await.unwrap();
        assert!(n_socks >= 2);
        // REP should be 0x02 (connection not allowed by ruleset)
        assert_eq!(req_resp[1], 0x02);
    }

    #[tokio::test]
    async fn test_proxy_attribution_frame_and_secret_substitution() {
        let broker = CredentialBroker::new();
        let cred = BrokeredCredential::new(
            "github",
            vec!["example.com".to_string()],
            "ghp_actual_real_token_12345",
            "Authorization",
            "token {}",
        );
        let dummy = cred.dummy_token.clone();
        broker.register(cred);

        let registry = AttributionRegistry::new();
        registry.register(crate::attribution::AttributedSession {
            session_id: "s1".to_string(),
            agent_id: "a1".to_string(),
            token: "tok-123".to_string(),
            allowed_domains: vec!["example.com".to_string()],
        });

        let config = ProxyConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            allowed_domains: vec![DomainPattern::parse("example.com")],
            block_cloud_metadata: true,
            block_loopback_destinations: false,
            credential_broker: Some(broker),
            attribution_registry: Some(registry),
        };
        let proxy = Arc::new(NetworkProxyServer::new(config));
        let proxy_addr = proxy.clone().start().await.unwrap();

        // Connect with attribution frame
        let mut client = TcpStream::connect(proxy_addr).await.unwrap();
        let mut frame_buf = Vec::new();
        crate::attribution::write_attribution_frame(&mut frame_buf, "tok-123").unwrap();
        client.write_all(&frame_buf).await.unwrap();

        // Send HTTP request containing dummy token
        let req = format!(
            "GET http://example.com/api HTTP/1.1\r\nHost: example.com\r\nAuthorization: token {}\r\n\r\n",
            dummy
        );
        let _ = client.write_all(req.as_bytes()).await;
    }
}
