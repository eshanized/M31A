//! Model Context Protocol (MCP) communication transports (Stdio, SSE, and WebSocket).

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use base64::Engine;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{Mutex, RwLock, oneshot};
use tracing::{debug, info};

use crate::protocol::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};

pub type ResponseMap = Arc<RwLock<HashMap<i64, oneshot::Sender<Result<Value, JsonRpcError>>>>>;

#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn send(&self, req: &JsonRpcRequest) -> Result<()>;
    async fn is_alive(&self) -> bool;
    async fn close(&self);
}

// ---------------------------------------------------------------------------
// 1. Stdio Transport
// ---------------------------------------------------------------------------

pub struct StdioTransport {
    stdin: Arc<Mutex<ChildStdin>>,
    child: Arc<Mutex<Child>>,
}

impl StdioTransport {
    pub async fn spawn(
        name: &str,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        cwd: Option<&Path>,
        pending: ResponseMap,
    ) -> Result<Self> {
        info!(server = %name, cmd = %command, "Spawning MCP Stdio server process");

        let mut cmd = Command::new(command);
        cmd.args(args);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::inherit());

        for (k, v) in env {
            cmd.env(k, v);
        }

        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        let mut child = cmd.spawn().with_context(|| {
            format!("Failed to spawn MCP server '{name}' with command '{command}'")
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Failed to capture stdin for '{name}'"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed to capture stdout for '{name}'"))?;

        let stdin_locked = Arc::new(Mutex::new(stdin));
        let child_locked = Arc::new(Mutex::new(child));

        let server_name = name.to_string();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                debug!(server = %server_name, line = %trimmed, "MCP received stdout frame");

                if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(trimmed)
                    && let Some(id_val) = resp.id
                    && let Some(id_num) = id_val.as_i64()
                {
                    let mut map = pending.write().await;
                    if let Some(sender) = map.remove(&id_num) {
                        if let Some(err) = resp.error {
                            let _ = sender.send(Err(err));
                        } else {
                            let _ = sender.send(Ok(resp.result.unwrap_or(Value::Null)));
                        }
                    }
                }
            }
        });

        Ok(Self {
            stdin: stdin_locked,
            child: child_locked,
        })
    }
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn send(&self, req: &JsonRpcRequest) -> Result<()> {
        let mut payload = serde_json::to_vec(req)?;
        payload.push(b'\n');

        let mut sin = self.stdin.lock().await;
        sin.write_all(&payload).await?;
        sin.flush().await?;
        Ok(())
    }

    async fn is_alive(&self) -> bool {
        let mut child = self.child.lock().await;
        matches!(child.try_wait(), Ok(None))
    }

    async fn close(&self) {
        let mut child = self.child.lock().await;
        let _ = child.kill().await;
    }
}

// ---------------------------------------------------------------------------
// 2. Server-Sent Events (SSE) Transport
// ---------------------------------------------------------------------------

pub struct SseTransport {
    client: reqwest::Client,
    post_endpoint: Arc<RwLock<String>>,
    headers: reqwest::header::HeaderMap,
    is_active: Arc<RwLock<bool>>,
}

impl SseTransport {
    pub async fn connect(
        name: &str,
        sse_url: &str,
        headers_map: &HashMap<String, String>,
        pending: ResponseMap,
    ) -> Result<Self> {
        info!(server = %name, url = %sse_url, "Connecting to MCP SSE endpoint");

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::ACCEPT, "text/event-stream".parse()?);

        for (k, v) in headers_map {
            if let (Ok(h_name), Ok(h_val)) = (
                k.parse::<reqwest::header::HeaderName>(),
                v.parse::<reqwest::header::HeaderValue>(),
            ) {
                headers.insert(h_name, h_val);
            }
        }

        let post_endpoint = Arc::new(RwLock::new(sse_url.to_string()));
        let is_active = Arc::new(RwLock::new(true));

        let res = client
            .get(sse_url)
            .headers(headers.clone())
            .send()
            .await
            .with_context(|| format!("Failed to initiate SSE connection to '{sse_url}'"))?;

        if !res.status().is_success() {
            anyhow::bail!("SSE connection failed with HTTP status {}", res.status());
        }

        let post_ep_clone = post_endpoint.clone();
        let sse_url_base = sse_url.to_string();
        let is_active_clone = is_active.clone();

        tokio::spawn(async move {
            let mut stream = res.bytes_stream();
            use futures::StreamExt;
            let mut buffer = String::new();

            while let Some(chunk_res) = stream.next().await {
                if let Ok(chunk) = chunk_res {
                    buffer.push_str(&String::from_utf8_lossy(&chunk));

                    while let Some(pos) = buffer.find("\n\n") {
                        let block = buffer[..pos].to_string();
                        buffer = buffer[pos + 2..].to_string();

                        let mut event_name = "message".to_string();
                        let mut data_payload = String::new();

                        for line in block.lines() {
                            if let Some(ev) = line.strip_prefix("event:") {
                                event_name = ev.trim().to_string();
                            } else if let Some(dt) = line.strip_prefix("data:") {
                                data_payload.push_str(dt.trim());
                            }
                        }

                        if event_name == "endpoint" {
                            let ep = data_payload.trim();
                            let full_url =
                                if ep.starts_with("http://") || ep.starts_with("https://") {
                                    ep.to_string()
                                } else if let Ok(base) = reqwest::Url::parse(&sse_url_base) {
                                    base.join(ep)
                                        .map(|u| u.to_string())
                                        .unwrap_or_else(|_| ep.to_string())
                                } else {
                                    ep.to_string()
                                };
                            *post_ep_clone.write().await = full_url;
                        } else if let Ok(resp) =
                            serde_json::from_str::<JsonRpcResponse>(&data_payload)
                            && let Some(id_val) = resp.id
                            && let Some(id_num) = id_val.as_i64()
                        {
                            let mut map = pending.write().await;
                            if let Some(sender) = map.remove(&id_num) {
                                if let Some(err) = resp.error {
                                    let _ = sender.send(Err(err));
                                } else {
                                    let _ = sender.send(Ok(resp.result.unwrap_or(Value::Null)));
                                }
                            }
                        }
                    }
                } else {
                    break;
                }
            }
            *is_active_clone.write().await = false;
        });

        Ok(Self {
            client,
            post_endpoint,
            headers,
            is_active,
        })
    }
}

#[async_trait]
impl McpTransport for SseTransport {
    async fn send(&self, req: &JsonRpcRequest) -> Result<()> {
        let endpoint = self.post_endpoint.read().await.clone();
        let resp = self
            .client
            .post(&endpoint)
            .headers(self.headers.clone())
            .json(req)
            .send()
            .await
            .with_context(|| format!("Failed to POST MCP request to '{endpoint}'"))?;

        if !resp.status().is_success() {
            anyhow::bail!("MCP HTTP POST failed with status: {}", resp.status());
        }

        Ok(())
    }

    async fn is_alive(&self) -> bool {
        *self.is_active.read().await
    }

    async fn close(&self) {
        *self.is_active.write().await = false;
    }
}

// ---------------------------------------------------------------------------
// 3. WebSocket Transport
// ---------------------------------------------------------------------------

pub struct WebSocketTransport {
    writer: Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
    is_active: Arc<RwLock<bool>>,
}

impl WebSocketTransport {
    pub async fn connect(
        name: &str,
        ws_url: &str,
        headers_map: &HashMap<String, String>,
        pending: ResponseMap,
    ) -> Result<Self> {
        info!(server = %name, url = %ws_url, "Connecting to MCP WebSocket endpoint");

        let parsed_url = reqwest::Url::parse(ws_url)?;
        let host = parsed_url
            .host_str()
            .ok_or_else(|| anyhow!("Missing host in WebSocket URL"))?;
        let port = parsed_url.port_or_known_default().unwrap_or(80);
        let path = if parsed_url.path().is_empty() {
            "/"
        } else {
            parsed_url.path()
        };

        let addr = format!("{host}:{port}");
        let mut stream = TcpStream::connect(&addr)
            .await
            .with_context(|| format!("Failed to connect TCP to {addr}"))?;

        let mut custom_headers = String::new();
        for (k, v) in headers_map {
            custom_headers.push_str(&format!("{k}: {v}\r\n"));
        }

        let nonce: [u8; 16] = rand::random();
        let sec_ws_key = base64::prelude::BASE64_STANDARD.encode(nonce);

        let handshake = format!(
            "GET {path} HTTP/1.1\r\n\
             Host: {host}:{port}\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Key: {sec_ws_key}\r\n\
             Sec-WebSocket-Version: 13\r\n\
             {custom_headers}\r\n"
        );

        stream.write_all(handshake.as_bytes()).await?;
        stream.flush().await?;

        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).await?;
        let resp_header = String::from_utf8_lossy(&buf[..n]);
        if !resp_header.starts_with("HTTP/1.1 101") {
            anyhow::bail!("WebSocket handshake failed: {resp_header}");
        }

        let (mut reader, writer) = stream.into_split();
        let writer_arc = Arc::new(Mutex::new(writer));
        let writer_for_reader = writer_arc.clone();
        let is_active = Arc::new(RwLock::new(true));
        let is_active_clone = is_active.clone();

        tokio::spawn(async move {
            loop {
                match read_ws_text_message(&mut reader, Some(&writer_for_reader)).await {
                    Ok(msg) => {
                        if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(&msg)
                            && let Some(id_val) = resp.id
                            && let Some(id_num) = id_val.as_i64()
                        {
                            let mut map = pending.write().await;
                            if let Some(sender) = map.remove(&id_num) {
                                if let Some(err) = resp.error {
                                    let _ = sender.send(Err(err));
                                } else {
                                    let _ = sender.send(Ok(resp.result.unwrap_or(Value::Null)));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        debug!("MCP WebSocket connection closed: {e}");
                        break;
                    }
                }
            }
            *is_active_clone.write().await = false;
        });

        Ok(Self {
            writer: writer_arc,
            is_active,
        })
    }

    pub async fn ping(&self) -> Result<()> {
        let mut w = self.writer.lock().await;
        write_ws_ping_frame(&mut *w, b"m31a-keepalive").await
    }
}

#[async_trait]
impl McpTransport for WebSocketTransport {
    async fn send(&self, req: &JsonRpcRequest) -> Result<()> {
        let text = serde_json::to_string(req)?;
        let mut w = self.writer.lock().await;
        write_ws_text_frame(&mut *w, &text).await?;
        Ok(())
    }

    async fn is_alive(&self) -> bool {
        *self.is_active.read().await
    }

    async fn close(&self) {
        *self.is_active.write().await = false;
    }
}

// ---------------------------------------------------------------------------
// RFC 6455 WebSocket Framing Helpers
// ---------------------------------------------------------------------------

pub async fn read_ws_frame<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<(u8, bool, Vec<u8>)> {
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
        u64::from_be_bytes(len_bytes) as usize
    } else {
        initial_len
    };

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

pub async fn read_ws_text_message<R: AsyncReadExt + Unpin, W: AsyncWriteExt + Unpin>(
    reader: &mut R,
    writer: Option<&Arc<Mutex<W>>>,
) -> Result<String> {
    let mut message_bytes = Vec::new();
    loop {
        let (opcode, fin, payload) = read_ws_frame(reader).await?;
        match opcode {
            0x1 | 0x0 => {
                message_bytes.extend_from_slice(&payload);
                if fin {
                    break;
                }
            }
            0x9 => {
                // Ping frame received from peer; reply with Pong (RFC 6455 Sec 5.5.2)
                if let Some(w_arc) = writer {
                    let mut w = w_arc.lock().await;
                    let _ = write_ws_pong_frame(&mut *w, &payload).await;
                }
            }
            0xA => {
                // Pong frame received
                debug!("Received MCP WebSocket Pong frame");
            }
            0x8 => {
                anyhow::bail!("WebSocket connection closed by peer");
            }
            _ => {}
        }
    }
    String::from_utf8(message_bytes).map_err(|e| anyhow!("Invalid UTF-8 in WS frame: {e}"))
}

pub async fn write_ws_text_frame<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    text: &str,
) -> Result<()> {
    let payload = text.as_bytes();
    let mut frame = Vec::with_capacity(14 + payload.len());
    frame.push(0x81); // FIN + Text opcode

    let mask: [u8; 4] = rand::random();
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

pub async fn write_ws_pong_frame<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    payload: &[u8],
) -> Result<()> {
    let mut frame = Vec::with_capacity(6 + payload.len());
    frame.push(0x8A); // FIN + Pong opcode (0xA)
    let mask: [u8; 4] = rand::random();
    let len = payload.len().min(125);
    frame.push(0x80 | (len as u8));
    frame.extend_from_slice(&mask);
    for (i, &b) in payload[..len].iter().enumerate() {
        frame.push(b ^ mask[i % 4]);
    }
    writer.write_all(&frame).await?;
    writer.flush().await?;
    Ok(())
}

pub async fn write_ws_ping_frame<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    payload: &[u8],
) -> Result<()> {
    let mut frame = Vec::with_capacity(6 + payload.len());
    frame.push(0x89); // FIN + Ping opcode (0x9)
    let mask: [u8; 4] = rand::random();
    let len = payload.len().min(125);
    frame.push(0x80 | (len as u8));
    frame.extend_from_slice(&mask);
    for (i, &b) in payload[..len].iter().enumerate() {
        frame.push(b ^ mask[i % 4]);
    }
    writer.write_all(&frame).await?;
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ws_text_framing_roundtrip() {
        let original = "Hello, MCP WebSocket!";
        let mut buffer = Vec::new();
        write_ws_text_frame(&mut buffer, original).await.unwrap();

        let mut slice = &buffer[..];
        let decoded = read_ws_text_message::<_, Vec<u8>>(&mut slice, None)
            .await
            .unwrap();
        assert_eq!(decoded, original);
    }

    #[tokio::test]
    async fn test_ws_ping_pong_response() {
        let ping_payload = b"test-ping";
        let mut client_buf = Vec::new();
        write_ws_ping_frame(&mut client_buf, ping_payload)
            .await
            .unwrap();

        let mut read_slice = &client_buf[..];
        let (opcode, fin, payload) = read_ws_frame(&mut read_slice).await.unwrap();
        assert_eq!(opcode, 0x9);
        assert!(fin);
        assert_eq!(payload, ping_payload);

        let mut pong_buf = Vec::new();
        write_ws_pong_frame(&mut pong_buf, &payload).await.unwrap();

        let mut pong_slice = &pong_buf[..];
        let (p_opcode, p_fin, p_payload) = read_ws_frame(&mut pong_slice).await.unwrap();
        assert_eq!(p_opcode, 0xA);
        assert!(p_fin);
        assert_eq!(p_payload, ping_payload);
    }
}
