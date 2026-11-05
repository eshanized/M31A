//! HTTP, Server-Sent Events (SSE), and REST Gateway for M31A App Server.
//!
//! Provides a hardened, local-first asynchronous HTTP/1.1 server on top of
//! Tokio TCP streams, enforcing loopback-only network exposure, strict Origin and
//! Host header validation (DNS rebinding protection), bounded resource limits,
//! and per-thread SSE event filtering in a sovereign single-user environment.
//!
//! ## Local-First Security Boundary Threat Model
//! M31A does not implement first-party user authentication tokens (no passwords, accounts,
//! or bearer tokens). Instead, local single-user security is enforced via:
//! - OS user ownership and process isolation.
//! - Private filesystem directories (0700) and socket permissions (0600).
//! - Loopback-only HTTP binding (127.0.0.1 or ::1).
//! - Host header validation to reject DNS rebinding attacks.
//! - Origin header validation to reject unauthorized browser cross-origin requests.
//! - Workspace trust boundaries.
//! - Sandboxed execution policies and host-mediated human approvals.

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

use crate::protocol::{AppServer, JsonRpcRequest, JsonRpcResponse};

/// Configuration options for the HTTP daemon.
#[derive(Debug, Clone)]
pub struct HttpServerConfig {
    pub bind_addr: SocketAddr,
    pub cors_allow_all: bool,
    pub allowed_origins: Vec<String>,
    pub max_body_size: usize,
    pub max_header_bytes: usize,
    pub header_timeout_ms: u64,
    pub body_timeout_ms: u64,
    pub max_concurrent_connections: usize,
}

impl Default for HttpServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:4096".parse().unwrap(),
            cors_allow_all: false,
            allowed_origins: Vec::new(),
            max_body_size: 10 * 1024 * 1024, // 10MB
            max_header_bytes: 32 * 1024,     // 32KB
            header_timeout_ms: 10_000,
            body_timeout_ms: 30_000,
            max_concurrent_connections: 256,
        }
    }
}

impl HttpServerConfig {
    /// Validates security constraints on the configuration.
    ///
    /// The HTTP daemon is strictly local-first; it MUST bind only to loopback
    /// addresses (127.0.0.1 or ::1) to prevent remote unauthenticated network exposure.
    pub fn validate(&self) -> Result<()> {
        if !self.bind_addr.ip().is_loopback() {
            anyhow::bail!(
                "Security policy violation: Network-accessible bind address '{}' is prohibited. Local-first unauthenticated server MUST bind only to loopback addresses.",
                self.bind_addr
            );
        }
        Ok(())
    }
}

/// Runs the HTTP and SSE daemon listening on `config.bind_addr`.
pub async fn run_http_server(server: AppServer, config: HttpServerConfig) -> Result<()> {
    config.validate()?;
    let listener = TcpListener::bind(config.bind_addr)
        .await
        .with_context(|| format!("Failed to bind HTTP server to {}", config.bind_addr))?;
    run_http_server_with_listener(server, config, listener).await
}

/// Runs the HTTP and SSE daemon on an existing pre-bound `TcpListener`.
pub async fn run_http_server_with_listener(
    server: AppServer,
    config: HttpServerConfig,
    listener: TcpListener,
) -> Result<()> {
    config.validate()?;

    info!(
        "M31A HTTP/SSE Daemon listening on http://{} (local-first access)",
        config.bind_addr
    );

    let server = Arc::new(server);
    let config = Arc::new(config);
    let connection_limiter = Arc::new(Semaphore::new(config.max_concurrent_connections));

    loop {
        match listener.accept().await {
            Ok((stream, remote_addr)) => {
                let permit = match connection_limiter.clone().try_acquire_owned() {
                    Ok(p) => p,
                    Err(_) => {
                        warn!(
                            "HTTP connection rejected from {}: connection concurrency limit reached",
                            remote_addr
                        );
                        continue;
                    }
                };

                let s_clone = Arc::clone(&server);
                let c_clone = Arc::clone(&config);
                tokio::spawn(async move {
                    let _permit = permit;
                    if let Err(e) = handle_connection(stream, remote_addr, s_clone, c_clone).await {
                        warn!("HTTP connection error from {}: {}", remote_addr, e);
                    }
                });
            }
            Err(e) => {
                error!("HTTP accept error: {}", e);
            }
        }
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    _remote: SocketAddr,
    server: Arc<AppServer>,
    config: Arc<HttpServerConfig>,
) -> Result<()> {
    let (read_half, mut write_half) = stream.split();
    let mut reader = BufReader::new(read_half);

    // Read request line and headers with timeout
    let header_deadline = Duration::from_millis(config.header_timeout_ms);
    let (method, raw_uri, headers, content_length) = match tokio::time::timeout(
        header_deadline,
        read_request_headers(&mut reader, config.max_header_bytes),
    )
    .await
    {
        Ok(Ok(Some(parsed))) => parsed,
        Ok(Ok(None)) => return Ok(()), // Client disconnected cleanly
        Ok(Err(e)) => {
            let (status, reason, msg) = match e.to_string().as_str() {
                s if s.contains("413") => (413, "Payload Too Large", "Header size limit exceeded"),
                s if s.contains("431") => (
                    431,
                    "Request Header Fields Too Large",
                    "Too many header lines",
                ),
                s if s.contains("414") => (414, "URI Too Long", "Request line exceeds limit"),
                _ => (400, "Bad Request", "Malformed HTTP request"),
            };
            send_response(
                &mut write_half,
                status,
                reason,
                "text/plain",
                msg.as_bytes(),
                None,
            )
            .await?;
            return Ok(());
        }
        Err(_) => {
            send_response(
                &mut write_half,
                408,
                "Request Timeout",
                "text/plain",
                b"Header read timeout",
                None,
            )
            .await?;
            return Ok(());
        }
    };

    // Parse path and query parameters
    let (path, query_params) = parse_uri(&raw_uri);

    // Validate Host header to protect against DNS rebinding and non-loopback exposure
    if let Some(host_val) = headers.get("host") {
        let host_name = host_val.split(':').next().unwrap_or(host_val).trim();
        let is_loopback_host = host_name == "localhost"
            || host_name == "127.0.0.1"
            || host_name == "::1"
            || host_name == "[::1]";
        if !is_loopback_host {
            send_response(
                &mut write_half,
                403,
                "Forbidden",
                "text/plain",
                b"Forbidden: untrusted Host header",
                None,
            )
            .await?;
            return Ok(());
        }
    }

    // Determine allowed CORS origin
    let origin_header = headers.get("origin").map(|s| s.as_str());
    let allowed_origin = resolve_cors_origin(origin_header, &config);

    // Reject untrusted cross-origin requests from browsers
    if origin_header.is_some() && allowed_origin.is_none() {
        send_response(
            &mut write_half,
            403,
            "Forbidden",
            "text/plain",
            b"CORS origin not allowed",
            None,
        )
        .await?;
        return Ok(());
    }

    // Handle OPTIONS preflight
    if method == "OPTIONS" {
        return send_cors_preflight(&mut write_half, allowed_origin.as_deref()).await;
    }

    // Body size validation before allocation
    if content_length > config.max_body_size {
        send_response(
            &mut write_half,
            413,
            "Payload Too Large",
            "text/plain",
            format!(
                "Content-Length {} exceeds maximum permitted body size of {}",
                content_length, config.max_body_size
            )
            .as_bytes(),
            allowed_origin.as_deref(),
        )
        .await?;
        return Ok(());
    }

    // Read body with timeout
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        let body_deadline = Duration::from_millis(config.body_timeout_ms);
        if tokio::time::timeout(body_deadline, reader.read_exact(&mut body))
            .await
            .is_err()
        {
            send_response(
                &mut write_half,
                408,
                "Request Timeout",
                "text/plain",
                b"Body read timeout",
                allowed_origin.as_deref(),
            )
            .await?;
            return Ok(());
        }
    }

    // Unauthenticated public endpoints
    if path == "/health" || path == "/api/health" {
        let health = serde_json::json!({
            "status": "healthy",
            "server": "m31a-app-server",
            "version": "0.1.13",
            "active_turns": server.active_turns.lock().unwrap().len(),
        });
        let bytes = serde_json::to_vec(&health)?;
        return send_response(
            &mut write_half,
            200,
            "OK",
            "application/json",
            &bytes,
            allowed_origin.as_deref(),
        )
        .await;
    }

    if (path == "/" || path == "/index.html" || path == "/app") && method == "GET" {
        return send_response(
            &mut write_half,
            200,
            "OK",
            "text/html; charset=utf-8",
            crate::web_assets::INDEX_HTML.as_bytes(),
            allowed_origin.as_deref(),
        )
        .await;
    }

    // Route dispatch
    if (path == "/events" || path == "/api/events") && method == "GET" {
        let filter_thread_id = query_params.get("thread_id").cloned();
        return stream_sse_events(
            &mut write_half,
            server,
            filter_thread_id,
            allowed_origin.as_deref(),
        )
        .await;
    }

    if (path == "/rpc" || path == "/api/rpc") && method == "POST" {
        let req_json: JsonRpcRequest = match serde_json::from_slice(&body) {
            Ok(r) => r,
            Err(e) => {
                let err_resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: None,
                    result: None,
                    error: Some(crate::protocol::JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                        data: None,
                    }),
                };
                let bytes = serde_json::to_vec(&err_resp)?;
                return send_response(
                    &mut write_half,
                    200,
                    "OK",
                    "application/json",
                    &bytes,
                    allowed_origin.as_deref(),
                )
                .await;
            }
        };

        let resp = server.handle_request(req_json).await;
        let bytes = serde_json::to_vec(&resp)?;
        return send_response(
            &mut write_half,
            200,
            "OK",
            "application/json",
            &bytes,
            allowed_origin.as_deref(),
        )
        .await;
    }
    if path == "/api/models" && method == "GET" {
        let models_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "models/list".to_string(),
            params: None,
        };
        let resp = server.handle_request(models_req).await;
        let bytes = serde_json::to_vec(&resp.result.unwrap_or(Value::Null))?;
        return send_response(
            &mut write_half,
            200,
            "OK",
            "application/json",
            &bytes,
            allowed_origin.as_deref(),
        )
        .await;
    }

    if path == "/api/threads" && method == "GET" {
        let list_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "thread/list".to_string(),
            params: Some(serde_json::json!({ "limit": 50 })),
        };
        let resp = server.handle_request(list_req).await;
        let bytes = serde_json::to_vec(&resp.result.unwrap_or(Value::Null))?;
        return send_response(
            &mut write_half,
            200,
            "OK",
            "application/json",
            &bytes,
            allowed_origin.as_deref(),
        )
        .await;
    }

    if path == "/api/threads" && method == "POST" {
        let mut body_val: Value = serde_json::from_slice(&body).unwrap_or(serde_json::json!({}));
        if let Some(map) = body_val.as_object_mut() {
            map.remove("allow_untrusted");
        }

        let create_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "thread/create".to_string(),
            params: Some(body_val),
        };
        let resp = server.handle_request(create_req).await;

        if let Some(err) = resp.error {
            let bytes = serde_json::to_vec(&err)?;
            return send_response(
                &mut write_half,
                400,
                "Bad Request",
                "application/json",
                &bytes,
                allowed_origin.as_deref(),
            )
            .await;
        }

        let res_val = resp.result.unwrap_or(Value::Null);
        let bytes = serde_json::to_vec(&res_val)?;
        return send_response(
            &mut write_half,
            201,
            "Created",
            "application/json",
            &bytes,
            allowed_origin.as_deref(),
        )
        .await;
    }

    if let Some(thread_sub) = path.strip_prefix("/api/threads/") {
        if let Some((thread_id, sub)) = thread_sub.split_once('/') {
            if sub == "turns" && method == "POST" {
                let mut body_val: Value =
                    serde_json::from_slice(&body).unwrap_or(serde_json::json!({}));
                if let Some(map) = body_val.as_object_mut() {
                    map.remove("allow_untrusted");
                    map.insert(
                        "thread_id".to_string(),
                        Value::String(thread_id.to_string()),
                    );
                }
                let turn_req = JsonRpcRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(serde_json::json!(1)),
                    method: "turn/start".to_string(),
                    params: Some(body_val),
                };
                let resp = server.handle_request(turn_req).await;
                if let Some(err) = resp.error {
                    let bytes = serde_json::to_vec(&err)?;
                    return send_response(
                        &mut write_half,
                        400,
                        "Bad Request",
                        "application/json",
                        &bytes,
                        allowed_origin.as_deref(),
                    )
                    .await;
                }
                let bytes = serde_json::to_vec(&resp.result.unwrap_or(Value::Null))?;
                return send_response(
                    &mut write_half,
                    200,
                    "OK",
                    "application/json",
                    &bytes,
                    allowed_origin.as_deref(),
                )
                .await;
            } else if sub == "goal" && method == "GET" {
                let goal_req = JsonRpcRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(serde_json::json!(1)),
                    method: "goal/get".to_string(),
                    params: Some(serde_json::json!({ "thread_id": thread_id })),
                };
                let resp = server.handle_request(goal_req).await;
                let bytes = serde_json::to_vec(&resp.result.unwrap_or(Value::Null))?;
                return send_response(
                    &mut write_half,
                    200,
                    "OK",
                    "application/json",
                    &bytes,
                    allowed_origin.as_deref(),
                )
                .await;
            }
        } else if method == "GET" {
            let thread_id = thread_sub;
            let read_req = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: Some(serde_json::json!(1)),
                method: "thread/read".to_string(),
                params: Some(serde_json::json!({ "thread_id": thread_id })),
            };
            let resp = server.handle_request(read_req).await;
            let bytes = serde_json::to_vec(&resp.result.unwrap_or(Value::Null))?;
            return send_response(
                &mut write_half,
                200,
                "OK",
                "application/json",
                &bytes,
                allowed_origin.as_deref(),
            )
            .await;
        }
    }

    send_response(
        &mut write_half,
        404,
        "Not Found",
        "text/plain",
        b"Endpoint not found",
        allowed_origin.as_deref(),
    )
    .await
}

fn resolve_cors_origin(origin_header: Option<&str>, config: &HttpServerConfig) -> Option<String> {
    let origin = origin_header?;
    let is_loopback = origin.starts_with("http://localhost:")
        || origin == "http://localhost"
        || origin.starts_with("http://127.0.0.1:")
        || origin == "http://127.0.0.1"
        || origin.starts_with("http://[::1]:")
        || origin == "http://[::1]";

    if config.cors_allow_all || is_loopback || config.allowed_origins.iter().any(|o| o == origin) {
        Some(origin.to_string())
    } else {
        None
    }
}

fn parse_uri(raw: &str) -> (String, HashMap<String, String>) {
    let mut params = HashMap::new();
    let (path, query) = match raw.split_once('?') {
        Some((p, q)) => (p.to_string(), q),
        None => (raw.to_string(), ""),
    };

    if !query.is_empty() {
        for part in query.split('&') {
            if let Some((k, v)) = part.split_once('=') {
                params.insert(k.to_string(), v.to_string());
            } else if !part.is_empty() {
                params.insert(part.to_string(), String::new());
            }
        }
    }

    (path, params)
}

async fn read_request_headers<R: AsyncBufReadExt + Unpin>(
    reader: &mut R,
    max_header_bytes: usize,
) -> Result<Option<(String, String, HashMap<String, String>, usize)>> {
    // Read request line
    let mut req_line = String::new();
    if reader.read_line(&mut req_line).await? == 0 {
        return Ok(None);
    }

    if req_line.len() > 8192 {
        anyhow::bail!("414: Request line exceeds 8192 byte limit");
    }

    let parts: Vec<&str> = req_line.split_whitespace().collect();
    if parts.len() < 2 {
        anyhow::bail!("400: Malformed request line");
    }

    let method = parts[0].to_string();
    let uri = parts[1].to_string();

    let mut headers = HashMap::new();
    let mut total_header_bytes = req_line.len();
    let mut header_count = 0;
    let mut content_length_seen: Option<usize> = None;
    let mut has_chunked_transfer = false;

    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).await? == 0 {
            break;
        }

        total_header_bytes += line.len();
        if total_header_bytes > max_header_bytes {
            anyhow::bail!(
                "413: Headers exceed size limit of {} bytes",
                max_header_bytes
            );
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            break; // End of headers
        }

        header_count += 1;
        if header_count > 128 {
            anyhow::bail!("431: Exceeded 128 header limit");
        }

        if let Some((k, v)) = trimmed.split_once(':') {
            let key = k.trim().to_ascii_lowercase();
            let val = v.trim().to_string();

            if key == "content-length" {
                let parsed: usize = val
                    .parse()
                    .map_err(|_| anyhow::anyhow!("400: Invalid Content-Length"))?;
                if let Some(existing) = content_length_seen
                    && existing != parsed
                {
                    anyhow::bail!("400: Duplicate conflicting Content-Length header");
                }
                content_length_seen = Some(parsed);
            }

            if key == "transfer-encoding" && val.to_ascii_lowercase().contains("chunked") {
                has_chunked_transfer = true;
            }

            headers.insert(key, val);
        }
    }

    // RFC 7230 §3.3.3 request smuggling check
    if has_chunked_transfer && content_length_seen.is_some() {
        anyhow::bail!("400: Request with both Transfer-Encoding and Content-Length rejected");
    }

    let content_length = content_length_seen.unwrap_or(0);
    Ok(Some((method, uri, headers, content_length)))
}

async fn send_response<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    status_code: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
    allowed_origin: Option<&str>,
) -> Result<()> {
    let mut headers = format!(
        "HTTP/1.1 {} {}\r\n\
        Content-Type: {}\r\n\
        Content-Length: {}\r\n\
        Connection: close\r\n",
        status_code,
        reason,
        content_type,
        body.len()
    );

    if let Some(origin) = allowed_origin {
        headers.push_str(&format!(
            "Access-Control-Allow-Origin: {}\r\n\
            Access-Control-Allow-Credentials: true\r\n\
            Vary: Origin\r\n",
            origin
        ));
    }

    headers.push_str("\r\n");

    writer.write_all(headers.as_bytes()).await?;
    writer.write_all(body).await?;
    writer.flush().await?;
    Ok(())
}

#[allow(dead_code)]
async fn send_forbidden<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    message: &str,
    allowed_origin: Option<&str>,
) -> Result<()> {
    let body = serde_json::json!({
        "error": "forbidden",
        "message": message,
    });
    let bytes = serde_json::to_vec(&body)?;

    send_response(
        writer,
        403,
        "Forbidden",
        "application/json",
        &bytes,
        allowed_origin,
    )
    .await
}

async fn send_cors_preflight<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    allowed_origin: Option<&str>,
) -> Result<()> {
    let mut headers = "HTTP/1.1 204 No Content\r\n\
        Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
        Access-Control-Allow-Headers: Content-Type, Authorization\r\n\
        Access-Control-Max-Age: 86400\r\n\
        Connection: close\r\n"
        .to_string();

    if let Some(origin) = allowed_origin {
        headers.push_str(&format!(
            "Access-Control-Allow-Origin: {}\r\n\
            Access-Control-Allow-Credentials: true\r\n\
            Vary: Origin\r\n",
            origin
        ));
    }
    headers.push_str("\r\n");

    writer.write_all(headers.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

async fn stream_sse_events<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    server: Arc<AppServer>,
    filter_thread_id: Option<String>,
    allowed_origin: Option<&str>,
) -> Result<()> {
    let mut initial_headers = "HTTP/1.1 200 OK\r\n\
        Content-Type: text/event-stream\r\n\
        Cache-Control: no-cache\r\n\
        Connection: keep-alive\r\n"
        .to_string();

    if let Some(origin) = allowed_origin {
        initial_headers.push_str(&format!(
            "Access-Control-Allow-Origin: {}\r\n\
            Access-Control-Allow-Credentials: true\r\n\
            Vary: Origin\r\n",
            origin
        ));
    }
    initial_headers.push_str("\r\n");

    writer.write_all(initial_headers.as_bytes()).await?;
    writer.flush().await?;

    let mut rx = server.subscribe();
    while let Ok(notif) = rx.recv().await {
        if let Some(ref params) = notif.params
            && let Some(tid) = params.get("thread_id").and_then(|v| v.as_str())
            && let Some(ref target_tid) = filter_thread_id
            && tid != target_tid
        {
            continue; // Filtered to another thread
        }

        if let Ok(json_str) = serde_json::to_string(&notif) {
            let sse_chunk = format!("event: {}\ndata: {}\n\n", notif.method, json_str);
            if writer.write_all(sse_chunk.as_bytes()).await.is_err() {
                break;
            }
            if writer.flush().await.is_err() {
                break;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::AppServer;
    use m31a_core::Config;
    use m31a_storage::Storage;
    use m31a_tools::ToolRegistry;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn test_http_auth_and_health_endpoints() {
        let config = Arc::new(Config {
            model: "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
            model_provider: Some("nim".to_string()),
            api_key: m31a_core::resolve_stored_api_key()
                .or_else(|| Some("nvapi-runtime-integration-token".to_string())),
            ..Default::default()
        });
        let storage = Storage::open_in_memory().unwrap();
        let tools = Arc::new(ToolRegistry::new());
        let server = AppServer::new(config, storage, tools).unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_addr = listener.local_addr().unwrap();

        let s_clone = Arc::new(server);
        // Validate rejects non-loopback binding
        let non_loopback = HttpServerConfig {
            bind_addr: std::net::SocketAddr::from(([0, 0, 0, 0], 8080)),
            ..Default::default()
        };
        assert!(non_loopback.validate().is_err());

        let c_clone = Arc::new(HttpServerConfig {
            bind_addr: local_addr,
            cors_allow_all: false,
            allowed_origins: vec!["http://localhost:3000".to_string()],
            ..Default::default()
        });
        assert!(c_clone.validate().is_ok());

        let s_srv = Arc::clone(&s_clone);
        let c_srv = Arc::clone(&c_clone);
        tokio::spawn(async move {
            while let Ok((stream, addr)) = listener.accept().await {
                let s = Arc::clone(&s_srv);
                let c = Arc::clone(&c_srv);
                tokio::spawn(async move {
                    let _ = handle_connection(stream, addr, s, c).await;
                });
            }
        });

        // 1. GET /health succeeds without token
        let mut client = TcpStream::connect(local_addr).await.unwrap();
        let req = format!("GET /health HTTP/1.1\r\nHost: {}\r\n\r\n", local_addr);
        client.write_all(req.as_bytes()).await.unwrap();
        let mut resp = String::new();
        client.read_to_string(&mut resp).await.unwrap();
        assert!(resp.contains("HTTP/1.1 200 OK"));
        assert!(resp.contains("\"status\":\"healthy\""));

        // 2. GET /api/threads succeeds in local-first mode with loopback Host
        let mut client = TcpStream::connect(local_addr).await.unwrap();
        let req = format!("GET /api/threads HTTP/1.1\r\nHost: {}\r\n\r\n", local_addr);
        client.write_all(req.as_bytes()).await.unwrap();
        let mut resp = String::new();
        client.read_to_string(&mut resp).await.unwrap();
        assert!(resp.contains("HTTP/1.1 200 OK"));

        // 3. Untrusted Host header is rejected (DNS rebinding protection)
        let mut client = TcpStream::connect(local_addr).await.unwrap();
        let req = "GET /api/threads HTTP/1.1\r\nHost: attacker.com\r\n\r\n";
        client.write_all(req.as_bytes()).await.unwrap();
        let mut resp = String::new();
        client.read_to_string(&mut resp).await.unwrap();
        assert!(resp.contains("HTTP/1.1 403 Forbidden"));

        // 4. Untrusted Origin header is rejected
        let mut client = TcpStream::connect(local_addr).await.unwrap();
        let req = format!(
            "GET /api/threads HTTP/1.1\r\nHost: {}\r\nOrigin: https://evil.com\r\n\r\n",
            local_addr
        );
        client.write_all(req.as_bytes()).await.unwrap();
        let mut resp = String::new();
        client.read_to_string(&mut resp).await.unwrap();
        assert!(resp.contains("HTTP/1.1 403 Forbidden"));
    }

    #[tokio::test]
    async fn test_http_oversized_payload_rejected_before_allocation() {
        let config = Arc::new(Config {
            model: "nvidia/nemotron-3.5-lightning-30b-a3b".to_string(),
            model_provider: Some("nim".to_string()),
            api_key: m31a_core::resolve_stored_api_key()
                .or_else(|| Some("nvapi-runtime-integration-token".to_string())),
            ..Default::default()
        });
        let storage = Storage::open_in_memory().unwrap();
        let tools = Arc::new(ToolRegistry::new());
        let server = AppServer::new(config, storage, tools).unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_addr = listener.local_addr().unwrap();

        let s_clone = Arc::new(server);
        let c_clone = Arc::new(HttpServerConfig {
            bind_addr: local_addr,
            max_body_size: 1024, // 1KB limit
            ..Default::default()
        });

        tokio::spawn(async move {
            while let Ok((stream, addr)) = listener.accept().await {
                let s = Arc::clone(&s_clone);
                let c = Arc::clone(&c_clone);
                tokio::spawn(async move {
                    let _ = handle_connection(stream, addr, s, c).await;
                });
            }
        });

        let mut client = TcpStream::connect(local_addr).await.unwrap();
        let req = format!(
            "POST /rpc HTTP/1.1\r\nHost: {}\r\nContent-Length: 10485760\r\n\r\n",
            local_addr
        );
        client.write_all(req.as_bytes()).await.unwrap();

        let mut resp = String::new();
        client.read_to_string(&mut resp).await.unwrap();
        assert!(resp.contains("HTTP/1.1 413 Payload Too Large"));
    }
}
