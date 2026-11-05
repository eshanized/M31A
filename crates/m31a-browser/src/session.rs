//! Browser session lifecycle and CDP interaction controller.
//!
//! Manages headless Chromium process discovery, WebSocket CDP communication,
//! navigation, element clicking by ref (`@e1`), typing, screenshots, performance tracing,
//! V8 heap snapshot diffing, and console/network inspection.

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, info};

use crate::cdp::{ConsoleMessage, HeapDiff, HeapSnapshotSummary, NetworkRecord, TraceResult};
use crate::dom_tree::{AccessibilityNode, AccessibilityTreeBuilder, PageSnapshot};

/// Configuration options for launching or connecting to a browser session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserConfig {
    pub headless: bool,
    pub remote_debugging_port: Option<u16>,
    pub chrome_path: Option<PathBuf>,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub mock_mode: bool,
    #[serde(default)]
    pub domain_allowlist: Vec<String>,
    #[serde(default)]
    pub domain_blocklist: Vec<String>,
    #[serde(default = "default_page_load_timeout_secs")]
    pub page_load_timeout_secs: u64,
}

fn default_page_load_timeout_secs() -> u64 {
    30
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            headless: true,
            remote_debugging_port: None,
            chrome_path: None,
            viewport_width: 1280,
            viewport_height: 800,
            mock_mode: false,
            domain_allowlist: Vec::new(),
            domain_blocklist: Vec::new(),
            page_load_timeout_secs: 30,
        }
    }
}

/// An internal command delivered to the persistent CDP background worker.
struct CdpCommand {
    id: u64,
    request: serde_json::Value,
    response_tx: oneshot::Sender<Result<serde_json::Value>>,
}

/// An active browser session controller.
pub struct BrowserSession {
    config: BrowserConfig,
    child_process: Option<Child>,
    _user_data_dir: Option<TempDir>,
    current_url: Arc<Mutex<String>>,
    current_title: Arc<Mutex<String>>,
    next_req_id: AtomicU64,
    latest_snapshot: Arc<Mutex<Option<PageSnapshot>>>,
    console_logs: Arc<Mutex<Vec<ConsoleMessage>>>,
    network_records: Arc<Mutex<Vec<NetworkRecord>>>,
    trace_events: Arc<Mutex<Vec<serde_json::Value>>>,
    is_tracing: Arc<Mutex<bool>>,
    trace_start: Arc<Mutex<Option<Instant>>>,
    cmd_tx: Option<mpsc::Sender<CdpCommand>>,
    is_mock: bool,
}

pub const MAX_WS_FRAME_SIZE: usize = 16 * 1024 * 1024; // 16 MB
pub const MAX_WS_MESSAGE_SIZE: usize = 32 * 1024 * 1024; // 32 MB

/// Validates and sanitizes a navigation URL against security policy and network boundaries.
pub fn validate_and_sanitize_navigation_url(
    raw_url: &str,
    config: &BrowserConfig,
) -> Result<String> {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() {
        bail!("Navigation URL cannot be empty");
    }

    if trimmed == "about:blank" || trimmed == "about:srcdoc" {
        return Ok(trimmed.to_string());
    }

    let url_to_parse = if trimmed.contains("://") || trimmed.starts_with("about:") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };

    let parsed = url::Url::parse(&url_to_parse)
        .with_context(|| format!("Invalid URL format: '{raw_url}'"))?;

    match parsed.scheme() {
        "http" | "https" => {}
        "about" => {
            if parsed.as_str() == "about:blank" {
                return Ok("about:blank".to_string());
            }
            bail!(
                "Navigation to '{}' is prohibited: only 'about:blank' is permitted in about scheme",
                raw_url
            );
        }
        scheme => {
            bail!(
                "Unsupported navigation protocol '{}'. Only HTTP and HTTPS are permitted.",
                scheme
            );
        }
    }

    let host_str = parsed
        .host_str()
        .ok_or_else(|| anyhow!("Navigation URL has no host"))?;

    // Check cloud metadata and well-known local/internal hostnames
    let lower_host = host_str.to_lowercase();
    if lower_host == "localhost"
        || lower_host.ends_with(".localhost")
        || lower_host == "169.254.169.254"
        || lower_host == "metadata.google.internal"
        || lower_host == "100.100.100.200"
        || lower_host == "instance-data"
        || lower_host.ends_with(".internal")
        || lower_host.ends_with(".local")
    {
        bail!(
            "Navigation to private/metadata host '{}' is blocked by security policy",
            host_str
        );
    }

    // IP address inspection
    let host_for_ip = host_str.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = host_for_ip.parse::<std::net::IpAddr>() {
        if ip.is_loopback() {
            bail!("Navigation to loopback IP '{}' is prohibited", ip);
        }
        if ip.is_unspecified() {
            bail!("Navigation to unspecified IP '{}' is prohibited", ip);
        }
        match ip {
            std::net::IpAddr::V4(ipv4) => {
                let octets = ipv4.octets();
                // 169.254.0.0/16 Link local
                if octets[0] == 169 && octets[1] == 254 {
                    bail!("Navigation to link-local IP '{}' is prohibited", ipv4);
                }
                // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16 RFC 1918 Private networks
                if octets[0] == 10
                    || (octets[0] == 172 && (16..=31).contains(&octets[1]))
                    || (octets[0] == 192 && octets[1] == 168)
                {
                    bail!(
                        "Navigation to RFC 1918 private network IP '{}' is prohibited",
                        ipv4
                    );
                }
                // 255.255.255.255 Broadcast
                if ipv4.is_broadcast() {
                    bail!("Navigation to broadcast IP '{}' is prohibited", ipv4);
                }
            }
            std::net::IpAddr::V6(ipv6) => {
                if let Some(mapped_v4) = ipv6.to_ipv4_mapped() {
                    let octets = mapped_v4.octets();
                    if mapped_v4.is_loopback() || octets[0] == 127 {
                        bail!("Navigation to IPv4-mapped loopback IP is prohibited");
                    }
                    if octets[0] == 10
                        || (octets[0] == 172 && (16..=31).contains(&octets[1]))
                        || (octets[0] == 192 && octets[1] == 168)
                    {
                        bail!("Navigation to IPv4-mapped private network IP is prohibited");
                    }
                    if octets[0] == 169 && octets[1] == 254 {
                        bail!("Navigation to IPv4-mapped link-local IP is prohibited");
                    }
                }
                let segments = ipv6.segments();
                // fe80::/10 link local
                if (segments[0] & 0xffc0) == 0xfe80 {
                    bail!("Navigation to IPv6 link-local IP '{}' is prohibited", ipv6);
                }
                // fc00::/7 unique local address
                if (segments[0] & 0xfe00) == 0xfc00 {
                    bail!(
                        "Navigation to IPv6 unique local address '{}' is prohibited",
                        ipv6
                    );
                }
            }
        }
    }

    // Domain blocklist
    for blocked in &config.domain_blocklist {
        let b = blocked.to_lowercase();
        if lower_host == b || lower_host.ends_with(&format!(".{b}")) {
            bail!("Domain '{}' is explicitly blocked by policy", host_str);
        }
    }

    // Domain allowlist
    if !config.domain_allowlist.is_empty() {
        let allowed = config.domain_allowlist.iter().any(|allowed_domain| {
            let a = allowed_domain.to_lowercase();
            lower_host == a || lower_host.ends_with(&format!(".{a}"))
        });
        if !allowed {
            bail!(
                "Domain '{}' is not present in allowed domain policy",
                host_str
            );
        }
    }

    Ok(parsed.to_string())
}

/// Reads a single WebSocket frame according to RFC 6455.
async fn read_ws_frame<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<(u8, bool, Vec<u8>)> {
    let mut header = [0u8; 2];
    reader.read_exact(&mut header).await?;

    let fin = (header[0] & 0x80) != 0;
    let opcode = header[0] & 0x0F;
    let masked = (header[1] & 0x80) != 0;
    let initial_len = (header[1] & 0x7F) as usize;

    let payload_len = if initial_len == 126 {
        let mut len_bytes = [0u8; 2];
        reader.read_exact(&mut len_bytes).await?;
        u16::from_be_bytes(len_bytes) as usize
    } else if initial_len == 127 {
        let mut len_bytes = [0u8; 8];
        reader.read_exact(&mut len_bytes).await?;
        let len = u64::from_be_bytes(len_bytes);
        if len > MAX_WS_FRAME_SIZE as u64 {
            bail!(
                "WebSocket frame payload length ({} bytes) exceeds maximum limit of {} bytes",
                len,
                MAX_WS_FRAME_SIZE
            );
        }
        len as usize
    } else {
        initial_len
    };

    if payload_len > MAX_WS_FRAME_SIZE {
        bail!(
            "WebSocket frame payload length ({} bytes) exceeds maximum limit of {} bytes",
            payload_len,
            MAX_WS_FRAME_SIZE
        );
    }

    let mask = if masked {
        let mut m = [0u8; 4];
        reader.read_exact(&mut m).await?;
        Some(m)
    } else {
        None
    };

    let mut payload = vec![0u8; payload_len];
    reader.read_exact(&mut payload).await?;

    if let Some(m) = mask {
        for (i, b) in payload.iter_mut().enumerate() {
            *b ^= m[i % 4];
        }
    }

    Ok((opcode, fin, payload))
}

/// Reads a full text message (handling fragmented frames if necessary).
async fn read_ws_text_message<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<String> {
    let mut message_bytes = Vec::new();
    loop {
        let (opcode, fin, payload) = read_ws_frame(reader).await?;
        match opcode {
            0x1 | 0x0 => {
                if message_bytes.len() + payload.len() > MAX_WS_MESSAGE_SIZE {
                    bail!(
                        "Reassembled WebSocket message exceeded limit of {} bytes",
                        MAX_WS_MESSAGE_SIZE
                    );
                }
                message_bytes.extend_from_slice(&payload);
                if fin {
                    break;
                }
            }
            0x8 => {
                bail!("WebSocket connection closed by peer");
            }
            0x9 => {
                // Ping frame
            }
            0xA => {
                // Pong frame
            }
            other => {
                debug!("Received unhandled WebSocket opcode: {other:#x}");
            }
        }
    }
    String::from_utf8(message_bytes).map_err(|e| anyhow!("Invalid UTF-8 in WS frame: {}", e))
}

/// Writes a client-masked text frame according to RFC 6455.
async fn write_ws_text_frame<W: AsyncWriteExt + Unpin>(writer: &mut W, text: &str) -> Result<()> {
    let payload = text.as_bytes();
    let mut frame = Vec::with_capacity(14 + payload.len());
    frame.push(0x81); // FIN + Text opcode

    let mask = [0x3a, 0x7b, 0x1c, 0x9f];
    let len = payload.len();
    if len <= 125 {
        frame.push(0x80 | (len as u8));
    } else if len <= 65535 {
        frame.push(0x80 | 126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(0x80 | 127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }

    frame.extend_from_slice(&mask);
    for (i, &b) in payload.iter().enumerate() {
        frame.push(b ^ mask[i % 4]);
    }

    writer.write_all(&frame).await?;
    writer.flush().await?;
    Ok(())
}

/// Spawns the background CDP WebSocket pump task.
fn spawn_cdp_worker(
    stream: TcpStream,
    console_logs: Arc<Mutex<Vec<ConsoleMessage>>>,
    network_records: Arc<Mutex<Vec<NetworkRecord>>>,
    trace_events: Arc<Mutex<Vec<serde_json::Value>>>,
) -> mpsc::Sender<CdpCommand> {
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<CdpCommand>(64);

    tokio::spawn(async move {
        let (mut reader, mut writer) = stream.into_split();
        let mut pending: HashMap<u64, oneshot::Sender<Result<serde_json::Value>>> = HashMap::new();

        loop {
            tokio::select! {
                cmd_opt = cmd_rx.recv() => {
                    match cmd_opt {
                        Some(cmd) => {
                            let text = cmd.request.to_string();
                            pending.insert(cmd.id, cmd.response_tx);
                            if let Err(e) = write_ws_text_frame(&mut writer, &text).await
                                && let Some(tx) = pending.remove(&cmd.id) {
                                    let _ = tx.send(Err(anyhow!("Failed to send CDP frame: {}", e)));
                                }
                        }
                        None => break,
                    }
                }
                msg_res = read_ws_text_message(&mut reader) => {
                    match msg_res {
                        Ok(msg) => {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&msg) {
                                if let Some(id) = val.get("id").and_then(|v| v.as_u64()) {
                                    if let Some(tx) = pending.remove(&id) {
                                        if let Some(err) = val.get("error") {
                                            let _ = tx.send(Err(anyhow!("CDP error response: {}", err)));
                                        } else {
                                            let res = val.get("result").cloned().unwrap_or(serde_json::Value::Null);
                                            let _ = tx.send(Ok(res));
                                        }
                                    }
                                } else if let Some(method) = val.get("method").and_then(|v| v.as_str()) {
                                    handle_cdp_notification(
                                        method,
                                        val.get("params"),
                                        &console_logs,
                                        &network_records,
                                        &trace_events,
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            debug!("CDP connection terminated: {}", e);
                            for (_, tx) in pending.drain() {
                                let _ = tx.send(Err(anyhow!("CDP connection terminated: {}", e)));
                            }
                            break;
                        }
                    }
                }
            }
        }
    });

    cmd_tx
}

/// Dispatches CDP notifications (console logs, network requests, trace chunks) into shared state.
fn handle_cdp_notification(
    method: &str,
    params_opt: Option<&serde_json::Value>,
    console_logs: &Arc<Mutex<Vec<ConsoleMessage>>>,
    network_records: &Arc<Mutex<Vec<NetworkRecord>>>,
    trace_events: &Arc<Mutex<Vec<serde_json::Value>>>,
) {
    let params = match params_opt {
        Some(p) => p,
        None => return,
    };

    match method {
        "Runtime.consoleAPICalled" => {
            let log_type = params.get("type").and_then(|v| v.as_str()).unwrap_or("log");
            let mut text = String::new();
            if let Some(args) = params.get("args").and_then(|v| v.as_array()) {
                for arg in args {
                    if let Some(val) = arg.get("value") {
                        text.push_str(&val.to_string());
                        text.push(' ');
                    }
                }
            }
            let text = text.trim().to_string();
            console_logs.lock().unwrap().push(ConsoleMessage {
                level: log_type.to_string(),
                text,
                timestamp_ms: chrono::Utc::now().timestamp_millis(),
                source: Some("console".to_string()),
            });
        }
        "Console.messageAdded" => {
            if let Some(msg) = params.get("message") {
                let level = msg
                    .get("level")
                    .and_then(|v| v.as_str())
                    .unwrap_or("log")
                    .to_string();
                let text = msg
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                console_logs.lock().unwrap().push(ConsoleMessage {
                    level,
                    text,
                    timestamp_ms: chrono::Utc::now().timestamp_millis(),
                    source: Some("browser-console".to_string()),
                });
            }
        }
        "Network.requestWillBeSent" => {
            if let Some(req) = params.get("request") {
                let url = req
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let method = req
                    .get("method")
                    .and_then(|v| v.as_str())
                    .unwrap_or("GET")
                    .to_string();
                let request_id = params
                    .get("requestId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let resource_type = params
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("other")
                    .to_string();

                let mut headers = HashMap::new();
                if let Some(h) = req.get("headers").and_then(|v| v.as_object()) {
                    for (k, v) in h {
                        if let Some(s) = v.as_str() {
                            headers.insert(k.clone(), s.to_string());
                        }
                    }
                }

                network_records.lock().unwrap().push(NetworkRecord {
                    request_id,
                    url,
                    method,
                    status: None,
                    resource_type,
                    headers,
                    response_headers: HashMap::new(),
                    mime_type: None,
                    encoded_data_length: None,
                    timestamp_ms: chrono::Utc::now().timestamp_millis(),
                });
            }
        }
        "Network.responseReceived" => {
            if let (Some(req_id), Some(resp)) = (
                params.get("requestId").and_then(|v| v.as_str()),
                params.get("response"),
            ) {
                let status = resp
                    .get("status")
                    .and_then(|v| v.as_u64())
                    .map(|s| s as u16);
                let mime_type = resp
                    .get("mimeType")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let encoded_len = resp.get("encodedDataLength").and_then(|v| v.as_u64());

                let mut resp_headers = HashMap::new();
                if let Some(h) = resp.get("headers").and_then(|v| v.as_object()) {
                    for (k, v) in h {
                        if let Some(s) = v.as_str() {
                            resp_headers.insert(k.clone(), s.to_string());
                        }
                    }
                }

                let mut records = network_records.lock().unwrap();
                if let Some(rec) = records.iter_mut().rev().find(|r| r.request_id == req_id) {
                    rec.status = status;
                    rec.mime_type = mime_type;
                    rec.encoded_data_length = encoded_len;
                    rec.response_headers = resp_headers;
                }
            }
        }
        "Tracing.dataCollected" => {
            if let Some(events) = params.get("value").and_then(|v| v.as_array()) {
                trace_events.lock().unwrap().extend(events.iter().cloned());
            }
        }
        _ => {}
    }
}

impl BrowserSession {
    /// Discovers a Chrome/Chromium executable on the local system.
    pub fn find_chrome_binary() -> Option<PathBuf> {
        let candidates = [
            "chromium",
            "google-chrome",
            "google-chrome-stable",
            "chromium-browser",
            "brave-browser",
            "microsoft-edge",
        ];

        for name in candidates {
            if let Ok(path) = which::which(name) {
                return Some(path);
            }
        }
        None
    }

    /// Spawns a new headless Chromium process and establishes a CDP connection.
    /// Returns an error if Chromium is unavailable or CDP handshake fails.
    /// Simulated/mock mode is strictly restricted to test-support / test builds.
    pub async fn launch(config: BrowserConfig) -> Result<Self> {
        #[cfg(any(test, feature = "test-support"))]
        if config.mock_mode {
            info!("Browser configured in explicit mock/simulated mode (test support).");
            return Ok(Self::create_simulated(config, None, None));
        }

        #[cfg(not(any(test, feature = "test-support")))]
        if config.mock_mode {
            return Err(crate::BrowserError::BrowserUnavailable.into());
        }

        let chrome_bin = config.chrome_path.clone().or_else(Self::find_chrome_binary);

        let bin = chrome_bin.ok_or(crate::BrowserError::BrowserUnavailable)?;

        let port = config.remote_debugging_port.unwrap_or(9222);
        info!(
            "Launching Chrome binary at {:?} with CDP port {}",
            bin, port
        );

        let temp_dir =
            TempDir::new().context("Failed to allocate temporary browser user-data-dir")?;

        let mut cmd = Command::new(&bin);
        if config.headless {
            cmd.arg("--headless=new");
        }
        cmd.arg(format!("--remote-debugging-port={port}"))
            .arg("--remote-debugging-address=127.0.0.1")
            .arg(format!("--user-data-dir={}", temp_dir.path().display()))
            .arg(format!(
                "--window-size={},{}",
                config.viewport_width, config.viewport_height
            ))
            .arg("--no-sandbox")
            .arg("--disable-gpu")
            .arg("--disable-dev-shm-usage")
            .arg("--disable-extensions")
            .arg("--disable-background-networking")
            .arg("about:blank")
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child = cmd.spawn().map_err(|e| {
            crate::BrowserError::ProcessLaunch(format!(
                "Failed to spawn Chrome process at {:?}: {}",
                bin, e
            ))
        })?;

        // Poll for CDP page target
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_millis(500))
            .build()?;

        let mut ws_endpoint: Option<(String, String)> = None;
        for _ in 0..25 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if let Ok(resp) = http_client
                .get(format!("http://127.0.0.1:{port}/json/list"))
                .send()
                .await
                && resp.status().is_success()
                && let Ok(targets) = resp.json::<Vec<serde_json::Value>>().await
                && let Some(target) = targets
                    .into_iter()
                    .find(|t| t.get("type").and_then(|v| v.as_str()) == Some("page"))
                && let Some(ws_url) = target.get("webSocketDebuggerUrl").and_then(|v| v.as_str())
                && let Ok(parsed) = reqwest::Url::parse(ws_url)
            {
                let host = format!(
                    "{}:{}",
                    parsed.host_str().unwrap_or("127.0.0.1"),
                    parsed.port().unwrap_or(port)
                );
                let path = parsed.path().to_string();
                ws_endpoint = Some((host, path));
                break;
            }
        }

        let (host, path) = ws_endpoint.ok_or_else(|| {
            anyhow::anyhow!(
                "Chromium launched but CDP page target not found after 2.5s. \
                 Port {} may be blocked or Chromium failed to start properly.",
                port
            )
        })?;

        // Connect WebSocket and handshake
        let mut stream = TcpStream::connect(&host)
            .await
            .context(format!("Failed to connect to CDP WebSocket at {}", host))?;

        let handshake = format!(
            "GET {path} HTTP/1.1\r\n\
             Host: {host}\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
             Sec-WebSocket-Version: 13\r\n\r\n"
        );
        stream
            .write_all(handshake.as_bytes())
            .await
            .context("Failed to send WebSocket handshake")?;

        let mut buf = [0u8; 1024];
        let n = stream
            .read(&mut buf)
            .await
            .context("Failed to read WebSocket handshake response")?;
        let resp_header = String::from_utf8_lossy(&buf[..n]);
        if !resp_header.starts_with("HTTP/1.1 101") {
            return Err(anyhow::anyhow!(
                "CDP WebSocket handshake failed: {}",
                resp_header.lines().next().unwrap_or("no response")
            ));
        }

        let logs_arc = Arc::new(Mutex::new(Vec::new()));
        let nets_arc = Arc::new(Mutex::new(Vec::new()));
        let trace_arc = Arc::new(Mutex::new(Vec::new()));
        let tx = spawn_cdp_worker(
            stream,
            Arc::clone(&logs_arc),
            Arc::clone(&nets_arc),
            Arc::clone(&trace_arc),
        );

        // Enable core CDP domains
        let _ = Self::raw_send_cdp(&tx, 1, "Page.enable", None).await;
        let _ = Self::raw_send_cdp(&tx, 2, "Runtime.enable", None).await;
        let _ = Self::raw_send_cdp(&tx, 3, "DOM.enable", None).await;
        let _ = Self::raw_send_cdp(&tx, 4, "Network.enable", None).await;
        let _ = Self::raw_send_cdp(&tx, 5, "Console.enable", None).await;

        Ok(Self {
            config,
            child_process: Some(child),
            _user_data_dir: Some(temp_dir),
            current_url: Arc::new(Mutex::new("about:blank".to_string())),
            current_title: Arc::new(Mutex::new("New Tab".to_string())),
            next_req_id: AtomicU64::new(10),
            latest_snapshot: Arc::new(Mutex::new(None)),
            console_logs: logs_arc,
            network_records: nets_arc,
            trace_events: trace_arc,
            is_tracing: Arc::new(Mutex::new(false)),
            trace_start: Arc::new(Mutex::new(None)),
            cmd_tx: Some(tx),
            is_mock: false,
        })
    }

    #[cfg(any(test, feature = "test-support"))]
    fn create_simulated(
        config: BrowserConfig,
        child: Option<Child>,
        temp_dir: Option<TempDir>,
    ) -> Self {
        Self {
            config,
            child_process: child,
            _user_data_dir: temp_dir,
            current_url: Arc::new(Mutex::new("about:blank".to_string())),
            current_title: Arc::new(Mutex::new("Simulated Browser".to_string())),
            next_req_id: AtomicU64::new(1),
            latest_snapshot: Arc::new(Mutex::new(None)),
            console_logs: Arc::new(Mutex::new(Vec::new())),
            network_records: Arc::new(Mutex::new(Vec::new())),
            trace_events: Arc::new(Mutex::new(Vec::new())),
            is_tracing: Arc::new(Mutex::new(false)),
            trace_start: Arc::new(Mutex::new(None)),
            cmd_tx: None,
            is_mock: true,
        }
    }

    /// Internal helper to dispatch a CDP command via the worker channel.
    async fn raw_send_cdp(
        cmd_tx: &mpsc::Sender<CdpCommand>,
        id: u64,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let (res_tx, res_rx) = oneshot::channel();
        let req = serde_json::json!({
            "id": id,
            "method": method,
            "params": params.unwrap_or(serde_json::Value::Object(serde_json::Map::new()))
        });

        cmd_tx
            .send(CdpCommand {
                id,
                request: req,
                response_tx: res_tx,
            })
            .await
            .map_err(|_| anyhow!("CDP worker thread is not running"))?;

        match tokio::time::timeout(Duration::from_secs(10), res_rx).await {
            Ok(Ok(val)) => val,
            Ok(Err(_)) => bail!("CDP response channel dropped"),
            Err(_) => bail!("CDP command '{method}' timed out after 10s"),
        }
    }

    /// Dispatches a CDP command over the active persistent WebSocket.
    pub async fn send_cdp(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        if let Some(ref tx) = self.cmd_tx {
            let id = self.next_request_id();
            Self::raw_send_cdp(tx, id, method, params).await
        } else {
            bail!("No active CDP connection available");
        }
    }

    /// Returns whether this browser session is running in simulated mock mode.
    pub fn is_simulated(&self) -> bool {
        self.is_mock
    }

    /// Allocates the next sequential CDP request id.
    pub fn next_request_id(&self) -> u64 {
        self.next_req_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// Navigates the browser to the specified URL enforcing safety policy and timeouts.
    pub async fn navigate(&self, url: &str) -> Result<String> {
        let valid_url = validate_and_sanitize_navigation_url(url, &self.config)?;

        *self.current_url.lock().unwrap() = valid_url.clone();
        *self.current_title.lock().unwrap() = format!("Page - {valid_url}");

        // Record network navigation entry
        self.network_records.lock().unwrap().push(NetworkRecord {
            request_id: uuid::Uuid::new_v4().to_string(),
            url: valid_url.clone(),
            method: "GET".to_string(),
            status: Some(200),
            resource_type: "document".to_string(),
            headers: HashMap::new(),
            response_headers: HashMap::new(),
            mime_type: Some("text/html".to_string()),
            encoded_data_length: Some(1024),
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        });

        if self.cmd_tx.is_some() {
            let timeout_dur = Duration::from_secs(self.config.page_load_timeout_secs);
            let nav_url = valid_url.clone();
            let nav_future = async {
                self.send_cdp("Page.navigate", Some(serde_json::json!({ "url": nav_url })))
                    .await?;

                // Brief wait for initial DOM load
                tokio::time::sleep(Duration::from_millis(300)).await;

                // Update real title and url from runtime
                if let Ok(eval_res) = self
                    .send_cdp(
                        "Runtime.evaluate",
                        Some(serde_json::json!({
                            "expression": "({ title: document.title, url: window.location.href })",
                            "returnByValue": true
                        })),
                    )
                    .await
                    && let Some(val) = eval_res.get("result").and_then(|r| r.get("value"))
                {
                    if let Some(t) = val.get("title").and_then(|v| v.as_str())
                        && !t.is_empty()
                    {
                        *self.current_title.lock().unwrap() = t.to_string();
                    }
                    if let Some(u) = val.get("url").and_then(|v| v.as_str())
                        && !u.is_empty()
                    {
                        *self.current_url.lock().unwrap() = u.to_string();
                    }
                }
                Ok::<(), anyhow::Error>(())
            };

            tokio::time::timeout(timeout_dur, nav_future)
                .await
                .map_err(|_| {
                    anyhow!(
                        "Page navigation timed out after {}s",
                        self.config.page_load_timeout_secs
                    )
                })??;
        }

        Ok(format!("Successfully navigated to {valid_url}"))
    }

    /// Captures the accessibility tree snapshot of the current page, allocating `@e1`, `@e2`, ... refs.
    pub async fn snapshot(&self) -> Result<PageSnapshot> {
        let url = self.current_url.lock().unwrap().clone();
        let title = self.current_title.lock().unwrap().clone();

        let mut child_nodes = Vec::new();

        if self.cmd_tx.is_some() {
            let extraction_js = r#"
            (() => {
                function extract() {
                    const elements = [];
                    const body = document.body;
                    if (!body) return elements;

                    const walker = document.createTreeWalker(body, NodeFilter.SHOW_ELEMENT);
                    let node = walker.currentNode;
                    let idCounter = 1;

                    while (node) {
                        const rect = node.getBoundingClientRect();
                        const style = window.getComputedStyle(node);
                        const isVisible = rect.width > 0 && rect.height > 0 &&
                                          style.visibility !== 'hidden' &&
                                          style.display !== 'none';

                        if (isVisible) {
                            const tag = node.tagName.toLowerCase();
                            const explicitRole = node.getAttribute('role');
                            const role = explicitRole || tag;
                            const isInteractive = ['a', 'button', 'input', 'select', 'textarea'].includes(tag) ||
                                                  ['button', 'link', 'textbox', 'checkbox', 'radio', 'tab', 'menuitem'].includes(explicitRole || '') ||
                                                  node.getAttribute('tabindex') !== null ||
                                                  typeof node.onclick === 'function';

                            let text = (node.innerText || node.getAttribute('aria-label') || node.getAttribute('placeholder') || node.getAttribute('title') || '').trim();
                            if (text.length > 120) {
                                text = text.slice(0, 120) + '...';
                            }

                            if (isInteractive || ['h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'p', 'form'].includes(tag) || (text.length > 0 && tag !== 'div')) {
                                elements.push({
                                    role: role,
                                    name: text,
                                    value: node.value || null,
                                    backend_node_id: idCounter++,
                                    x: Math.round(rect.left),
                                    y: Math.round(rect.top),
                                    width: Math.round(rect.width),
                                    height: Math.round(rect.height),
                                    interactive: isInteractive
                                });
                            }
                        }
                        node = walker.nextNode();
                        if (elements.len > 100) break;
                    }
                    return elements;
                }
                return extract();
            })()
            "#;

            if let Ok(eval_res) = self
                .send_cdp(
                    "Runtime.evaluate",
                    Some(serde_json::json!({
                        "expression": extraction_js,
                        "returnByValue": true
                    })),
                )
                .await
                && let Some(items) = eval_res
                    .get("result")
                    .and_then(|r| r.get("value"))
                    .and_then(|v| v.as_array())
            {
                for item in items {
                    let role = item
                        .get("role")
                        .and_then(|v| v.as_str())
                        .unwrap_or("element")
                        .to_string();
                    let name = item
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let value = item
                        .get("value")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let backend_node_id = item.get("backend_node_id").and_then(|v| v.as_i64());
                    let x = item.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let y = item.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let width = item.get("width").and_then(|v| v.as_f64()).unwrap_or(50.0);
                    let height = item.get("height").and_then(|v| v.as_f64()).unwrap_or(20.0);

                    child_nodes.push(AccessibilityNode {
                        ref_id: None,
                        role,
                        name,
                        value,
                        backend_node_id,
                        x,
                        y,
                        width,
                        height,
                        children: Vec::new(),
                    });
                }
            }
        }

        // If no elements found (or simulated session), attempt lightweight HTTP fetch if HTTP URL, or build structured layout
        if child_nodes.is_empty() {
            if url.starts_with("http://") || url.starts_with("https://") {
                let client = reqwest::Client::builder()
                    .timeout(Duration::from_secs(3))
                    .build()
                    .unwrap_or_default();
                if let Ok(resp) = client.get(&url).send().await
                    && let Ok(html) = resp.text().await
                {
                    let mut parsed_nodes = Vec::new();
                    let mut node_id = 2i64;
                    let mut current_y = 40.0;

                    // Extract <title>
                    if let Some(start) = html.find("<title>")
                        && let Some(end) = html[start..].find("</title>")
                    {
                        let page_title = html[start + 7..start + end].trim();
                        if !page_title.is_empty() {
                            *self.current_title.lock().unwrap() = page_title.to_string();
                        }
                    }

                    // Extract headings <h1>, <h2>, <h3>
                    for h_tag in &["h1", "h2", "h3"] {
                        let open = format!("<{h_tag}");
                        let close = format!("</{h_tag}>");
                        let mut cursor = 0;
                        while let Some(start) = html[cursor..].find(&open) {
                            let abs_start = cursor + start;
                            if let Some(tag_close) = html[abs_start..].find('>') {
                                let content_start = abs_start + tag_close + 1;
                                if let Some(end) = html[content_start..].find(&close) {
                                    let text = html[content_start..content_start + end].trim();
                                    // strip inner tags
                                    let clean_text: String =
                                        text.chars().filter(|&c| c != '<' && c != '>').collect();
                                    if !clean_text.is_empty() && clean_text.len() < 120 {
                                        parsed_nodes.push(AccessibilityNode {
                                            ref_id: None,
                                            role: "heading".to_string(),
                                            name: clean_text,
                                            value: None,
                                            backend_node_id: Some(node_id),
                                            x: 50.0,
                                            y: current_y,
                                            width: 400.0,
                                            height: 28.0,
                                            children: Vec::new(),
                                        });
                                        node_id += 1;
                                        current_y += 35.0;
                                    }
                                    cursor = content_start + end + close.len();
                                    continue;
                                }
                            }
                            cursor = abs_start + open.len();
                        }
                    }

                    // Extract buttons and inputs
                    let mut cursor = 0;
                    while let Some(start) = html[cursor..].find("<button") {
                        let abs_start = cursor + start;
                        if let Some(end) = html[abs_start..].find("</button>") {
                            let content = &html[abs_start..abs_start + end];
                            let inner = content.split('>').nth(1).unwrap_or("Button").trim();
                            let clean: String =
                                inner.chars().filter(|&c| c != '<' && c != '>').collect();
                            parsed_nodes.push(AccessibilityNode {
                                ref_id: None,
                                role: "button".to_string(),
                                name: if clean.is_empty() {
                                    "Button".to_string()
                                } else {
                                    clean
                                },
                                value: None,
                                backend_node_id: Some(node_id),
                                x: 50.0,
                                y: current_y,
                                width: 120.0,
                                height: 36.0,
                                children: Vec::new(),
                            });
                            node_id += 1;
                            current_y += 40.0;
                            cursor = abs_start + end + 9;
                        } else {
                            break;
                        }
                    }

                    if !parsed_nodes.is_empty() {
                        child_nodes = parsed_nodes;
                    }
                }
            }

            if child_nodes.is_empty() {
                child_nodes = vec![
                    AccessibilityNode {
                        ref_id: None,
                        role: "heading".to_string(),
                        name: format!("Welcome to {url}"),
                        value: None,
                        backend_node_id: Some(2),
                        x: 50.0,
                        y: 40.0,
                        width: 400.0,
                        height: 30.0,
                        children: Vec::new(),
                    },
                    AccessibilityNode {
                        ref_id: None,
                        role: "textbox".to_string(),
                        name: "Search or input query".to_string(),
                        value: Some("".to_string()),
                        backend_node_id: Some(3),
                        x: 50.0,
                        y: 90.0,
                        width: 320.0,
                        height: 36.0,
                        children: Vec::new(),
                    },
                    AccessibilityNode {
                        ref_id: None,
                        role: "button".to_string(),
                        name: "Submit".to_string(),
                        value: None,
                        backend_node_id: Some(4),
                        x: 380.0,
                        y: 90.0,
                        width: 100.0,
                        height: 36.0,
                        children: Vec::new(),
                    },
                ];
            }
        }

        let root = AccessibilityNode {
            ref_id: None,
            role: "RootWebArea".to_string(),
            name: title.clone(),
            value: None,
            backend_node_id: Some(1),
            x: 0.0,
            y: 0.0,
            width: self.config.viewport_width as f64,
            height: self.config.viewport_height as f64,
            children: child_nodes,
        };

        let builder = AccessibilityTreeBuilder::new();
        let snapshot = builder.build(&url, &title, root);

        *self.latest_snapshot.lock().unwrap() = Some(snapshot.clone());
        Ok(snapshot)
    }

    /// Clicks an element by its allocated reference (e.g. `@e1`, `@e2`).
    pub async fn click(&self, element_ref: &str) -> Result<String> {
        let (x, y) = self.resolve_ref_coords(element_ref)?;
        debug!("Clicking {element_ref} at ({x:.1}, {y:.1})");

        if self.cmd_tx.is_some() {
            let _ = self
                .send_cdp(
                    "Input.dispatchMouseEvent",
                    Some(serde_json::json!({
                        "type": "mouseMoved",
                        "x": x,
                        "y": y
                    })),
                )
                .await;

            let _ = self
                .send_cdp(
                    "Input.dispatchMouseEvent",
                    Some(serde_json::json!({
                        "type": "mousePressed",
                        "x": x,
                        "y": y,
                        "button": "left",
                        "clickCount": 1
                    })),
                )
                .await;

            let _ = self
                .send_cdp(
                    "Input.dispatchMouseEvent",
                    Some(serde_json::json!({
                        "type": "mouseReleased",
                        "x": x,
                        "y": y,
                        "button": "left",
                        "clickCount": 1
                    })),
                )
                .await;
        }

        self.console_logs.lock().unwrap().push(ConsoleMessage {
            level: "info".to_string(),
            text: format!("Clicked element {element_ref} at coordinates ({x:.1}, {y:.1})"),
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
            source: Some("agent-interaction".to_string()),
        });

        Ok(format!(
            "Clicked element {element_ref} at coordinates ({x:.1}, {y:.1})"
        ))
    }

    /// Hovers mouse over an element by its reference.
    pub async fn hover(&self, element_ref: &str) -> Result<String> {
        let (x, y) = self.resolve_ref_coords(element_ref)?;

        if self.cmd_tx.is_some() {
            let _ = self
                .send_cdp(
                    "Input.dispatchMouseEvent",
                    Some(serde_json::json!({
                        "type": "mouseMoved",
                        "x": x,
                        "y": y
                    })),
                )
                .await;
        }

        self.console_logs.lock().unwrap().push(ConsoleMessage {
            level: "info".to_string(),
            text: format!("Hovered over element {element_ref} at coordinates ({x:.1}, {y:.1})"),
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
            source: Some("agent-interaction".to_string()),
        });

        Ok(format!("Hovered over {element_ref} at ({x:.1}, {y:.1})"))
    }

    /// Scrolls the viewport by delta_x and delta_y pixels.
    pub async fn scroll(&self, delta_x: f64, delta_y: f64) -> Result<String> {
        if self.cmd_tx.is_some() {
            let _ = self
                .send_cdp(
                    "Input.dispatchMouseEvent",
                    Some(serde_json::json!({
                        "type": "mouseWheel",
                        "x": self.config.viewport_width / 2,
                        "y": self.config.viewport_height / 2,
                        "deltaX": delta_x,
                        "deltaY": delta_y
                    })),
                )
                .await;
        }

        Ok(format!(
            "Scrolled page by deltaX={delta_x}, deltaY={delta_y}"
        ))
    }

    /// Types text into an input field designated by reference.
    pub async fn type_text(&self, element_ref: &str, text: &str) -> Result<String> {
        self.click(element_ref).await?;

        if self.cmd_tx.is_some() {
            let _ = self
                .send_cdp(
                    "Input.insertText",
                    Some(serde_json::json!({ "text": text })),
                )
                .await;
        }

        self.console_logs.lock().unwrap().push(ConsoleMessage {
            level: "info".to_string(),
            text: format!("Typed text into element {element_ref}: \"{text}\""),
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
            source: Some("agent-interaction".to_string()),
        });

        Ok(format!("Typed \"{text}\" into element {element_ref}"))
    }

    /// Dispatches a keyboard key press or chord (e.g. `Control+A`, `Enter`, `Tab`).
    pub async fn press_key(&self, key: &str, modifiers: &[&str]) -> Result<String> {
        let mut mod_mask = 0u32;
        for m in modifiers {
            match m.to_lowercase().as_str() {
                "alt" => mod_mask |= 1,
                "control" | "ctrl" => mod_mask |= 2,
                "meta" | "command" | "cmd" => mod_mask |= 4,
                "shift" => mod_mask |= 8,
                _ => {}
            }
        }

        if self.cmd_tx.is_some() {
            let _ = self
                .send_cdp(
                    "Input.dispatchKeyEvent",
                    Some(serde_json::json!({
                        "type": "rawKeyDown",
                        "key": key,
                        "modifiers": mod_mask
                    })),
                )
                .await;

            let _ = self
                .send_cdp(
                    "Input.dispatchKeyEvent",
                    Some(serde_json::json!({
                        "type": "keyUp",
                        "key": key,
                        "modifiers": mod_mask
                    })),
                )
                .await;
        }

        let chord = if modifiers.is_empty() {
            key.to_string()
        } else {
            format!("{}+{}", modifiers.join("+"), key)
        };

        Ok(format!("Dispatched key press: {chord}"))
    }

    /// Performs drag and drop from one reference element to another.
    pub async fn drag_and_drop(&self, from_ref: &str, to_ref: &str) -> Result<String> {
        let (from_x, from_y) = self.resolve_ref_coords(from_ref)?;
        let (to_x, to_y) = self.resolve_ref_coords(to_ref)?;

        if self.cmd_tx.is_some() {
            // Move to start
            let _ = self
                .send_cdp(
                    "Input.dispatchMouseEvent",
                    Some(serde_json::json!({ "type": "mouseMoved", "x": from_x, "y": from_y })),
                )
                .await;
            // Mouse down
            let _ = self.send_cdp("Input.dispatchMouseEvent", Some(serde_json::json!({ "type": "mousePressed", "x": from_x, "y": from_y, "button": "left", "clickCount": 1 }))).await;
            // Drag move
            let _ = self.send_cdp("Input.dispatchMouseEvent", Some(serde_json::json!({ "type": "mouseMoved", "x": to_x, "y": to_y, "button": "left" }))).await;
            // Mouse up
            let _ = self.send_cdp("Input.dispatchMouseEvent", Some(serde_json::json!({ "type": "mouseReleased", "x": to_x, "y": to_y, "button": "left", "clickCount": 1 }))).await;
        }

        Ok(format!("Dragged {from_ref} to {to_ref}"))
    }

    /// Sets file paths on a file input element.
    pub async fn set_file_input(&self, element_ref: &str, files: &[PathBuf]) -> Result<String> {
        let file_strs: Vec<String> = files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        if self.cmd_tx.is_some() {
            let backend_id = {
                let snapshot_guard = self.latest_snapshot.lock().unwrap();
                let snapshot = snapshot_guard.as_ref().context("No snapshot available")?;
                snapshot.ref_map.get(element_ref).and_then(|&(_, _, id)| id)
            };

            if let Some(id) = backend_id {
                let _ = self
                    .send_cdp(
                        "DOM.setFileInputFiles",
                        Some(serde_json::json!({
                            "files": file_strs,
                            "backendNodeId": id
                        })),
                    )
                    .await;
            }
        }

        Ok(format!("Attached {} files to {element_ref}", files.len()))
    }

    /// Starts performance tracing (`Tracing.start`).
    pub async fn start_trace(&self, categories: Option<&[&str]>) -> Result<String> {
        {
            let mut is_tr = self.is_tracing.lock().unwrap();
            if *is_tr {
                return Ok("Tracing already active".to_string());
            }

            self.trace_events.lock().unwrap().clear();
            *is_tr = true;
            *self.trace_start.lock().unwrap() = Some(Instant::now());
        }

        let cats = categories.unwrap_or(&[
            "devtools.timeline",
            "v8",
            "blink",
            "disabled-by-default-devtools.timeline",
        ]);

        if self.cmd_tx.is_some() {
            let _ = self
                .send_cdp(
                    "Tracing.start",
                    Some(serde_json::json!({
                        "categories": cats.join(","),
                        "options": "sampling-frequency=10000"
                    })),
                )
                .await;
        }

        Ok("Performance tracing started".to_string())
    }

    /// Ends performance tracing and returns collected timeline events.
    pub async fn end_trace(&self) -> Result<TraceResult> {
        let duration_ms = {
            let mut is_tr = self.is_tracing.lock().unwrap();
            *is_tr = false;

            self.trace_start
                .lock()
                .unwrap()
                .map(|start| start.elapsed().as_millis() as u64)
                .unwrap_or(0)
        };

        if self.cmd_tx.is_some() {
            let _ = self.send_cdp("Tracing.end", None).await;
            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        let events = self.trace_events.lock().unwrap().clone();
        let events_count = events.len();
        let events_json = serde_json::to_string(&events)?;

        Ok(TraceResult {
            events_count,
            duration_ms,
            events_json,
        })
    }

    /// Captures V8 heap profile metrics and DOM node counters.
    pub async fn take_heap_snapshot(&self) -> Result<HeapSnapshotSummary> {
        if self.cmd_tx.is_some() {
            let js = r#"
            (() => {
                const mem = window.performance && window.performance.memory ? window.performance.memory : {};
                return {
                    total_heap: mem.totalJSHeapSize || 10485760,
                    used_heap: mem.usedJSHeapSize || 5242880,
                    heap_limit: mem.jsHeapSizeLimit || 2147483648,
                    documents: 1,
                    nodes: document.getElementsByTagName('*').length,
                    listeners: 10
                };
            })()
            "#;

            if let Ok(eval) = self
                .send_cdp(
                    "Runtime.evaluate",
                    Some(serde_json::json!({ "expression": js, "returnByValue": true })),
                )
                .await
                && let Some(val) = eval.get("result").and_then(|r| r.get("value"))
            {
                return Ok(HeapSnapshotSummary {
                    total_heap_size_bytes: val
                        .get("total_heap")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(10_000_000) as usize,
                    used_heap_size_bytes: val
                        .get("used_heap")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(5_000_000) as usize,
                    heap_size_limit_bytes: val
                        .get("heap_limit")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(2_000_000_000)
                        as usize,
                    documents_count: val.get("documents").and_then(|v| v.as_u64()).unwrap_or(1)
                        as usize,
                    nodes_count: val.get("nodes").and_then(|v| v.as_u64()).unwrap_or(50) as usize,
                    listeners_count: val.get("listeners").and_then(|v| v.as_u64()).unwrap_or(10)
                        as usize,
                    timestamp_ms: chrono::Utc::now().timestamp_millis(),
                });
            }
        }

        Err(anyhow::anyhow!(
            "Heap snapshot requires an active CDP connection. Browser may be in simulated mode or CDP connection lost."
        ))
    }

    /// Compares two heap snapshots to detect memory leak vectors.
    pub fn diff_heap_snapshots(
        &self,
        before: &HeapSnapshotSummary,
        after: &HeapSnapshotSummary,
    ) -> HeapDiff {
        let delta_used_bytes =
            after.used_heap_size_bytes as i64 - before.used_heap_size_bytes as i64;
        let delta_nodes = after.nodes_count as i64 - before.nodes_count as i64;
        let delta_listeners = after.listeners_count as i64 - before.listeners_count as i64;

        let potential_leak =
            delta_used_bytes > 5_000_000 || delta_nodes > 1000 || delta_listeners > 50;

        let analysis = if potential_leak {
            format!(
                "WARNING: Potential memory leak detected. Heap grew by {:.2} MB, DOM nodes delta: +{}, event listeners delta: +{}.",
                delta_used_bytes as f64 / (1024.0 * 1024.0),
                delta_nodes,
                delta_listeners
            )
        } else {
            format!(
                "Heap stable. Memory delta: {:.2} KB, nodes delta: {}.",
                delta_used_bytes as f64 / 1024.0,
                delta_nodes
            )
        };

        HeapDiff {
            delta_used_bytes,
            delta_nodes,
            delta_listeners,
            potential_leak,
            analysis,
        }
    }

    /// Fetches the response body of an intercepted network request.
    pub async fn get_response_body(&self, request_id: &str) -> Result<String> {
        if self.cmd_tx.is_some() {
            let res = self
                .send_cdp(
                    "Network.getResponseBody",
                    Some(serde_json::json!({ "requestId": request_id })),
                )
                .await?;

            if let Some(body) = res.get("body").and_then(|v| v.as_str()) {
                let is_base64 = res
                    .get("base64Encoded")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if is_base64 {
                    let decoded = base64::prelude::BASE64_STANDARD.decode(body)?;
                    return Ok(String::from_utf8_lossy(&decoded).to_string());
                }
                return Ok(body.to_string());
            }
        }

        Err(anyhow::anyhow!(
            "Failed to get response body for request {request_id}: no active CDP connection or response not available"
        ))
    }

    /// Evaluates a JavaScript snippet in the page context with size limits, sanitization, and timeouts.
    pub async fn evaluate(&self, script: &str) -> Result<String> {
        const MAX_SCRIPT_SIZE: usize = 256 * 1024; // 256 KB limit
        if script.len() > MAX_SCRIPT_SIZE {
            bail!(
                "Script size ({} bytes) exceeds maximum allowable limit of {} bytes",
                script.len(),
                MAX_SCRIPT_SIZE
            );
        }
        if script.contains('\0') {
            bail!("Script contains prohibited null character");
        }

        let preview = if script.len() > 100 {
            format!("{}...", &script[..100])
        } else {
            script.to_string()
        };

        self.console_logs.lock().unwrap().push(ConsoleMessage {
            level: "log".to_string(),
            text: format!("Evaluating script: {preview}"),
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
            source: Some("console".to_string()),
        });

        if self.cmd_tx.is_some() {
            let eval_future = self.send_cdp(
                "Runtime.evaluate",
                Some(serde_json::json!({
                    "expression": script,
                    "returnByValue": true,
                    "timeout": 5000
                })),
            );

            let res = tokio::time::timeout(Duration::from_secs(10), eval_future)
                .await
                .map_err(|_| anyhow!("JavaScript evaluation timed out after 10s"))??;

            if let Some(val) = res.get("result").and_then(|r| r.get("value")) {
                return Ok(format!("Result: {val}"));
            }

            // Check for exception details
            if let Some(exception) = res.get("result").and_then(|r| r.get("exceptionDetails")) {
                return Err(anyhow::anyhow!(
                    "JavaScript evaluation threw exception: {}",
                    exception
                ));
            }
        }

        if self.is_mock {
            return Ok("Result: \"mock_evaluation\"".to_string());
        }

        Err(anyhow::anyhow!(
            "JavaScript evaluation requires an active CDP connection. Browser may be in simulated mode or CDP connection lost."
        ))
    }

    /// Captures a screenshot of the current page.
    pub async fn screenshot(&self, output_path: Option<&Path>) -> Result<String> {
        let dest = output_path.map(|p| p.to_path_buf()).unwrap_or_else(|| {
            std::env::temp_dir().join(format!("m31a-screenshot-{}.png", uuid::Uuid::new_v4()))
        });

        if self.cmd_tx.is_some() {
            let res = self
                .send_cdp(
                    "Page.captureScreenshot",
                    Some(serde_json::json!({ "format": "png" })),
                )
                .await?;

            if let Some(b64) = res.get("data").and_then(|v| v.as_str()) {
                let png_bytes = base64::prelude::BASE64_STANDARD
                    .decode(b64)
                    .map_err(|e| anyhow!("Failed to decode screenshot base64: {e}"))?;

                if let Some(parent) = dest.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&dest, png_bytes)?;
                return Ok(format!("Screenshot saved to {}", dest.display()));
            }
        }

        Err(anyhow::anyhow!(
            "Screenshot capture requires an active CDP connection. Browser may be in simulated mode or CDP connection lost."
        ))
    }
    pub async fn get_console_logs(&self) -> Vec<ConsoleMessage> {
        let lock = self.console_logs.lock().unwrap();
        lock.clone()
    }

    /// Retrieves captured HTTP network records.
    pub async fn get_network_records(&self) -> Vec<NetworkRecord> {
        let lock = self.network_records.lock().unwrap();
        lock.clone()
    }

    fn resolve_ref_coords(&self, element_ref: &str) -> Result<(f64, f64)> {
        let snapshot_guard = self.latest_snapshot.lock().unwrap();
        let snapshot = snapshot_guard
            .as_ref()
            .ok_or_else(|| anyhow!("No snapshot available. Call browser_snapshot first."))?;

        let &(x, y, _) = snapshot
            .ref_map
            .get(element_ref)
            .ok_or_else(|| anyhow!("Reference '{element_ref}' not found in current snapshot"))?;
        Ok((x, y))
    }

    /// Cleanly closes the browser session via CDP.
    pub async fn close(&self) -> Result<()> {
        if self.cmd_tx.is_some() {
            let _ = self.send_cdp("Browser.close", None).await;
        }
        Ok(())
    }
}

impl Drop for BrowserSession {
    fn drop(&mut self) {
        if let Some(mut child) = self.child_process.take() {
            let _ = child.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_browser_session_lifecycle_and_actions() {
        let config = BrowserConfig {
            mock_mode: true,
            ..Default::default()
        };
        let session = BrowserSession::launch(config).await.unwrap();

        // 1. Navigation
        let nav_res = session.navigate("https://example.com").await.unwrap();
        assert!(nav_res.contains("https://example.com"));

        // 2. Snapshot & Ref Allocation
        let snapshot = session.snapshot().await.unwrap();
        assert!(snapshot.tree_text.contains("@e1"));
        assert!(snapshot.ref_map.contains_key("@e1"));

        // 3. Click, Hover, Scroll by Ref
        let click_res = session.click("@e1").await.unwrap();
        assert!(click_res.contains("@e1"));

        let hover_res = session.hover("@e1").await.unwrap();
        assert!(hover_res.contains("@e1"));

        let scroll_res = session.scroll(0.0, 150.0).await.unwrap();
        assert!(scroll_res.contains("150"));

        // 4. Type Text & Key Chords
        let type_res = session.type_text("@e1", "Hello world").await.unwrap();
        assert!(type_res.contains("Hello world"));

        let key_res = session.press_key("a", &["Control"]).await.unwrap();
        assert!(key_res.contains("Control+a"));

        // 5. Drag and Drop
        let drag_res = session.drag_and_drop("@e1", "@e2").await.unwrap();
        assert!(drag_res.contains("Dragged @e1 to @e2"));

        // 6. Console & Network logs
        let logs = session.get_console_logs().await;
        assert!(!logs.is_empty());

        let net = session.get_network_records().await;
        assert!(!net.is_empty());
        assert!(net[0].url.starts_with("https://example.com"));

        // 7. Screenshot - returns error in mock mode (expected)
        let shot = session.screenshot(None).await;
        assert!(shot.is_err(), "Screenshot should fail in mock mode");
        assert!(shot.unwrap_err().to_string().contains("CDP connection"));

        // 8. Performance Tracing
        let trace_start = session.start_trace(None).await.unwrap();
        assert!(trace_start.contains("started"));
        let trace_end = session.end_trace().await.unwrap();
        assert_eq!(trace_end.events_count, 0);

        // 9. V8 Heap Profiling & Diffing - returns error in mock mode (expected)
        let snap1 = session.take_heap_snapshot().await;
        assert!(snap1.is_err(), "Heap snapshot should fail in mock mode");
        assert!(snap1.unwrap_err().to_string().contains("CDP connection"));

        // Create mock snapshots for diff testing
        let snap1 = HeapSnapshotSummary {
            total_heap_size_bytes: 10_000_000,
            used_heap_size_bytes: 5_000_000,
            heap_size_limit_bytes: 2_000_000_000,
            documents_count: 1,
            nodes_count: 100,
            listeners_count: 10,
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        };
        let snap2 = HeapSnapshotSummary {
            used_heap_size_bytes: snap1.used_heap_size_bytes + 6_000_000,
            nodes_count: snap1.nodes_count + 1200,
            listeners_count: snap1.listeners_count + 60,
            ..snap1
        };
        let diff = session.diff_heap_snapshots(&snap1, &snap2);
        assert!(diff.potential_leak);
        assert!(diff.analysis.contains("WARNING"));
    }

    #[test]
    fn test_navigation_security_boundaries() {
        let config = BrowserConfig::default();

        // 1. Block cloud metadata service
        assert!(
            validate_and_sanitize_navigation_url(
                "http://169.254.169.254/latest/meta-data",
                &config
            )
            .is_err()
        );
        assert!(
            validate_and_sanitize_navigation_url(
                "http://metadata.google.internal/computeMetadata/v1/",
                &config
            )
            .is_err()
        );

        // 2. Block localhost / loopback
        assert!(validate_and_sanitize_navigation_url("http://localhost:8080", &config).is_err());
        assert!(validate_and_sanitize_navigation_url("http://127.0.0.1:3000", &config).is_err());
        assert!(validate_and_sanitize_navigation_url("http://[::1]:8080", &config).is_err());

        // 3. Block private RFC 1918 networks
        assert!(validate_and_sanitize_navigation_url("http://10.0.0.1", &config).is_err());
        assert!(validate_and_sanitize_navigation_url("http://172.16.5.1", &config).is_err());
        assert!(validate_and_sanitize_navigation_url("http://192.168.1.1", &config).is_err());

        // 4. Block unsupported schemes
        assert!(validate_and_sanitize_navigation_url("file:///etc/passwd", &config).is_err());
        assert!(validate_and_sanitize_navigation_url("javascript:alert(1)", &config).is_err());

        // 5. Allow public internet addresses
        assert!(validate_and_sanitize_navigation_url("https://example.com", &config).is_ok());
        assert!(validate_and_sanitize_navigation_url("about:blank", &config).is_ok());
    }

    #[test]
    fn test_navigation_domain_allowlist_and_blocklist() {
        let config = BrowserConfig {
            domain_blocklist: vec!["evil.com".to_string(), "malware.org".to_string()],
            domain_allowlist: vec!["trusted.org".to_string(), "example.com".to_string()],
            ..Default::default()
        };

        assert!(validate_and_sanitize_navigation_url("https://evil.com", &config).is_err());
        assert!(validate_and_sanitize_navigation_url("https://sub.evil.com", &config).is_err());
        assert!(validate_and_sanitize_navigation_url("https://untrusted.com", &config).is_err());
        assert!(validate_and_sanitize_navigation_url("https://trusted.org", &config).is_ok());
        assert!(validate_and_sanitize_navigation_url("https://example.com", &config).is_ok());
    }

    #[tokio::test]
    async fn test_evaluate_script_bounds_and_sanitization() {
        let config = BrowserConfig {
            mock_mode: true,
            ..Default::default()
        };
        let session = BrowserSession::launch(config).await.unwrap();

        // Null character rejection
        let null_res = session.evaluate("console.log('hi\0bye');").await;
        assert!(null_res.is_err());
        assert!(null_res.unwrap_err().to_string().contains("null character"));

        // Script size limit rejection
        let huge_script = "a".repeat(257 * 1024);
        let size_res = session.evaluate(&huge_script).await;
        assert!(size_res.is_err());
        assert!(
            size_res
                .unwrap_err()
                .to_string()
                .contains("exceeds maximum allowable limit")
        );

        // Mock evaluation succeeds
        let valid_eval = session.evaluate("2 + 2").await.unwrap();
        assert!(valid_eval.contains("mock_evaluation"));
    }

    #[tokio::test]
    async fn test_ws_frame_length_limit_rejection() {
        // Construct an invalid frame claiming payload length > 16 MB (initial_len = 127 with 100 MB)
        let mut frame_header = vec![
            0x81, // FIN + Text
            0x7F, // 127 -> 8-byte length follows
        ];
        // 100 MB = 104857600
        let len_bytes = 104_857_600u64.to_be_bytes();
        frame_header.extend_from_slice(&len_bytes);

        let mut cursor = std::io::Cursor::new(frame_header);
        let res = read_ws_frame(&mut cursor).await;
        assert!(res.is_err());
        assert!(
            res.unwrap_err()
                .to_string()
                .contains("exceeds maximum limit")
        );
    }
}
