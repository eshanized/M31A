//! Local TCP HTTP test service implementing the OpenAI/NIM wire protocol.
//!
//! Provides deterministic protocol-conformance testing over real TCP sockets using the
//! actual production HTTP client (`NvidiaNimClient` / `NimProvider`) without external credentials.

use anyhow::Result;
use serde_json::json;
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// Configured wire server response to be served over real HTTP transport.
#[derive(Debug, Clone)]
pub struct WireServerResponse {
    pub content: String,
    pub tool_calls: Vec<serde_json::Value>,
    pub reasoning: Option<String>,
}

impl WireServerResponse {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            tool_calls: Vec::new(),
            reasoning: None,
        }
    }

    pub fn with_tool_call(mut self, name: &str, arguments: &str) -> Self {
        let idx = self.tool_calls.len();
        self.tool_calls.push(json!({
            "index": idx,
            "id": format!("call_{}", uuid::Uuid::new_v4().simple()),
            "type": "function",
            "function": {
                "name": name,
                "arguments": arguments
            }
        }));
        self
    }
}

/// Real local TCP HTTP server for testing provider client protocol conformance.
pub struct LocalWireServer {
    addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
    queued_responses: Arc<Mutex<VecDeque<WireServerResponse>>>,
    received_requests: Arc<Mutex<Vec<String>>>,
}

impl LocalWireServer {
    /// Starts a local TCP test server listening on an ephemeral port.
    pub async fn start() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        let queued_responses = Arc::new(Mutex::new(VecDeque::new()));
        let received_requests = Arc::new(Mutex::new(Vec::new()));

        let queue_clone = queued_responses.clone();
        let reqs_clone = received_requests.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        break;
                    }
                    accept_res = listener.accept() => {
                        let (mut stream, _) = match accept_res {
                            Ok(s) => s,
                            Err(_) => break,
                        };

                        let q = queue_clone.clone();
                        let reqs = reqs_clone.clone();

                        tokio::spawn(async move {
                            let mut buf = Vec::new();
                            let mut tmp = [0u8; 8192];
                            let mut content_length: Option<usize> = None;
                            let mut header_end: Option<usize> = None;

                            loop {
                                let n = match stream.read(&mut tmp).await {
                                    Ok(n) if n > 0 => n,
                                    _ => break,
                                };
                                buf.extend_from_slice(&tmp[..n]);

                                if header_end.is_none()
                                    && let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                                        let end = pos + 4;
                                        header_end = Some(end);
                                        let header_str = String::from_utf8_lossy(&buf[..end]);
                                        for line in header_str.lines() {
                                            if let Some((k, v)) = line.split_once(':')
                                                && k.trim().eq_ignore_ascii_case("content-length")
                                                    && let Ok(len) = v.trim().parse::<usize>() {
                                                        content_length = Some(len);
                                                    }
                                        }
                                    }

                                if let Some(end) = header_end {
                                    if let Some(cl) = content_length {
                                        if buf.len() >= end + cl {
                                            break;
                                        }
                                    } else {
                                        break;
                                    }
                                }
                            }

                            if buf.is_empty() {
                                return;
                            }

                            let request_str = String::from_utf8_lossy(&buf).to_string();
                            reqs.lock().unwrap().push(request_str.clone());

                            // Check if this is a chat completions request
                            let is_chat = request_str.contains("/chat/completions");
                            let is_models = request_str.contains("/models");
                            let is_stream = request_str.contains("\"stream\":true") || request_str.contains("\"stream\": true");

                            if is_models {
                                let body = json!({
                                    "object": "list",
                                    "data": [
                                        {
                                            "id": "meta/llama-3.3-70b-instruct",
                                            "object": "model",
                                            "created": 1700000000,
                                            "owned_by": "nvidia"
                                        },
                                        {
                                            "id": "deepseek-ai/deepseek-r1",
                                            "object": "model",
                                            "created": 1700000000,
                                            "owned_by": "nvidia"
                                        },
                                        {
                                            "id": "nvidia/nemotron-3-ultra-550b-a55b",
                                            "object": "model",
                                            "created": 1700000000,
                                            "owned_by": "nvidia"
                                        }
                                    ]
                                }).to_string();
                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                    body.len(),
                                    body
                                );
                                let _ = stream.write_all(resp.as_bytes()).await;
                                let _ = stream.flush().await;
                                return;
                            }

                            if !is_chat {
                                let not_found = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                                let _ = stream.write_all(not_found.as_bytes()).await;
                                let _ = stream.flush().await;
                                return;
                            }

                            // Fetch configured response or default
                            let response_item = {
                                let mut guard = q.lock().unwrap();
                                guard.pop_front().unwrap_or_else(|| WireServerResponse::text("Default wire server completion response."))
                            };

                            if is_stream {
                                let headers = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n";
                                if stream.write_all(headers.as_bytes()).await.is_err() {
                                    return;
                                }

                                // Stream reasoning content if any
                                if let Some(ref reasoning) = response_item.reasoning {
                                    let reasoning_json = json!({
                                        "id": "chatcmpl-wire-1",
                                        "object": "chat.completion.chunk",
                                        "created": 1700000000,
                                        "model": "meta/llama-3.3-70b-instruct",
                                        "choices": [
                                            {
                                                "index": 0,
                                                "delta": {
                                                    "reasoning_content": reasoning
                                                },
                                                "finish_reason": null
                                            }
                                        ]
                                    });
                                    let r_line = format!("data: {}\n\n", reasoning_json);
                                    let _ = stream.write_all(r_line.as_bytes()).await;
                                }

                                // Stream text content in chunks
                                if !response_item.content.is_empty() {
                                    let chunk_json = json!({
                                        "id": "chatcmpl-wire-1",
                                        "object": "chat.completion.chunk",
                                        "created": 1700000000,
                                        "model": "meta/llama-3.3-70b-instruct",
                                        "choices": [
                                            {
                                                "index": 0,
                                                "delta": {
                                                    "role": "assistant",
                                                    "content": response_item.content
                                                },
                                                "finish_reason": null
                                            }
                                        ]
                                    });
                                    let chunk_line = format!("data: {}\n\n", chunk_json);
                                    let _ = stream.write_all(chunk_line.as_bytes()).await;
                                }

                                // Stream tool calls if any
                                if !response_item.tool_calls.is_empty() {
                                    for tc in &response_item.tool_calls {
                                        let tc_json = json!({
                                            "id": "chatcmpl-wire-1",
                                            "object": "chat.completion.chunk",
                                            "created": 1700000000,
                                            "model": "meta/llama-3.3-70b-instruct",
                                            "choices": [
                                                {
                                                    "index": 0,
                                                    "delta": {
                                                        "tool_calls": [tc]
                                                    },
                                                    "finish_reason": null
                                                }
                                            ]
                                        });
                                        let tc_line = format!("data: {}\n\n", tc_json);
                                        let _ = stream.write_all(tc_line.as_bytes()).await;
                                    }
                                }

                                // Stream final stop chunk with usage
                                let stop_json = json!({
                                    "id": "chatcmpl-wire-1",
                                    "object": "chat.completion.chunk",
                                    "created": 1700000000,
                                    "model": "meta/llama-3.3-70b-instruct",
                                    "choices": [
                                        {
                                            "index": 0,
                                            "delta": {},
                                            "finish_reason": if response_item.tool_calls.is_empty() { "stop" } else { "tool_calls" }
                                        }
                                    ],
                                    "usage": {
                                        "prompt_tokens": 12,
                                        "completion_tokens": 8,
                                        "total_tokens": 20
                                    }
                                });
                                let stop_line = format!("data: {}\n\ndata: [DONE]\n\n", stop_json);
                                let _ = stream.write_all(stop_line.as_bytes()).await;
                                let _ = stream.flush().await;
                            } else {
                                // Non-streaming JSON response
                                let body = json!({
                                    "id": "chatcmpl-wire-1",
                                    "object": "chat.completion",
                                    "created": 1700000000,
                                    "model": "meta/llama-3.3-70b-instruct",
                                    "choices": [
                                        {
                                            "index": 0,
                                            "message": {
                                                "role": "assistant",
                                                "content": response_item.content
                                            },
                                            "finish_reason": "stop"
                                        }
                                    ],
                                    "usage": {
                                        "prompt_tokens": 12,
                                        "completion_tokens": 8,
                                        "total_tokens": 20
                                    }
                                }).to_string();
                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                    body.len(),
                                    body
                                );
                                let _ = stream.write_all(resp.as_bytes()).await;
                                let _ = stream.flush().await;
                            }
                        });
                    }
                }
            }
        });

        Ok(Self {
            addr,
            shutdown_tx: Some(shutdown_tx),
            queued_responses,
            received_requests,
        })
    }

    /// Base URL string for this server, e.g. `http://127.0.0.1:41234/v1`
    pub fn api_base(&self) -> String {
        format!("http://{}/v1", self.addr)
    }

    /// Socket address of the test server.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Enqueue a response to be served by the wire server.
    pub fn enqueue_response(&self, resp: WireServerResponse) {
        self.queued_responses.lock().unwrap().push_back(resp);
    }

    /// Enqueue a text response.
    pub fn enqueue_text(&self, text: impl Into<String>) {
        self.enqueue_response(WireServerResponse::text(text));
    }

    /// Inspect recorded raw HTTP requests received by this server.
    pub fn requests(&self) -> Vec<String> {
        self.received_requests.lock().unwrap().clone()
    }
}

impl Drop for LocalWireServer {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::NvidiaNimClient;

    #[tokio::test]
    async fn test_local_wire_server_http_streaming_roundtrip() {
        let server = LocalWireServer::start().await.unwrap();
        server.enqueue_text("Hello from real HTTP TCP test service!");

        let client = NvidiaNimClient::new(
            server.api_base(),
            "test-token".to_string(),
            "meta/llama-3.3-70b-instruct".to_string(),
        );

        let messages = vec![m31a_core::types::ChatMessage::user("Hi")];
        let res = client
            .stream_chat(&messages, None, None, None)
            .await
            .unwrap();

        assert_eq!(
            res.0.content.as_deref(),
            Some("Hello from real HTTP TCP test service!")
        );
        assert!(!server.requests().is_empty());
        assert!(server.requests()[0].contains("POST /v1/chat/completions"));
    }
}
