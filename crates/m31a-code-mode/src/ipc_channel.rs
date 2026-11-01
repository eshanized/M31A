//! Sovereign sandboxed IPC transport and backpressure governor.
//!
//! Manages asynchronous IPC transport, message frames, and backpressure between host and worker isolates.
//! Implements bidirectional communication between the M31A host agent and
//! sandboxed JS/TS worker isolates:
//! - Length-prefixed framing and JSON message serialization.
//! - In-flight RPC request correlation with timeout protection.
//! - Asynchronous tool invocation routing, KV store synchronization, and log streaming.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::time::{Duration, timeout};

/// Message protocol envelopes exchanged between host and sandboxed isolate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload")]
pub enum IpcMessage {
    /// Host dispatches code to worker to begin evaluation.
    EvaluateCode { code: String },

    /// Worker requests tool invocation on the host.
    CallToolRequest {
        id: u64,
        tool_name: String,
        arguments: serde_json::Value,
    },

    /// Host returns tool result back to worker.
    CallToolResponse {
        id: u64,
        result: Result<serde_json::Value, String>,
    },

    /// Worker requests variable from persistent KV store.
    StoreGetRequest { id: u64, key: String },

    /// Host responds with KV store value.
    StoreGetResponse {
        id: u64,
        value: Option<serde_json::Value>,
    },

    /// Worker sets value in persistent KV store.
    StoreSetNotification {
        key: String,
        value: serde_json::Value,
    },

    /// Worker streams log output.
    LogStream { text: String },

    /// Worker requests cooperative yield.
    YieldNotification { reason: String, output: String },

    /// Execution has finalized.
    ExecutionCompleted {
        result: Option<serde_json::Value>,
        error: Option<String>,
    },
}

/// Length-prefixed framing encoder and decoder.
pub struct IpcFramingCodec;

impl IpcFramingCodec {
    /// Encode a message as a 4-byte big-endian length prefix followed by JSON bytes.
    pub fn encode(message: &IpcMessage) -> Result<Vec<u8>> {
        let json_bytes = serde_json::to_vec(message)?;
        let len = json_bytes.len() as u32;
        let mut frame = Vec::with_capacity(4 + json_bytes.len());
        frame.extend_from_slice(&len.to_be_bytes());
        frame.extend_from_slice(&json_bytes);
        Ok(frame)
    }

    /// Decode a length-prefixed frame from a buffer.
    /// Returns `Ok(Some((message, bytes_consumed)))` if a complete frame is available.
    pub fn decode(buffer: &[u8]) -> Result<Option<(IpcMessage, usize)>> {
        if buffer.len() < 4 {
            return Ok(None);
        }

        let mut len_bytes = [0u8; 4];
        len_bytes.copy_from_slice(&buffer[..4]);
        let frame_len = u32::from_be_bytes(len_bytes) as usize;

        if buffer.len() < 4 + frame_len {
            return Ok(None);
        }

        let json_slice = &buffer[4..4 + frame_len];
        let message: IpcMessage = serde_json::from_slice(json_slice)?;
        Ok(Some((message, 4 + frame_len)))
    }
}

type PendingToolCalls =
    Arc<Mutex<HashMap<u64, oneshot::Sender<Result<serde_json::Value, String>>>>>;
type PendingStoreGets = Arc<Mutex<HashMap<u64, oneshot::Sender<Option<serde_json::Value>>>>>;

/// Host-side IPC peer handling RPC message correlation and dispatch.
pub struct IpcPeer {
    next_id: AtomicU64,
    pending_tool_calls: PendingToolCalls,
    pending_store_gets: PendingStoreGets,
    outbound_tx: mpsc::Sender<IpcMessage>,
    default_timeout: Duration,
}

impl IpcPeer {
    pub fn new(outbound_tx: mpsc::Sender<IpcMessage>, default_timeout: Duration) -> Self {
        Self {
            next_id: AtomicU64::new(1),
            pending_tool_calls: Arc::new(Mutex::new(HashMap::new())),
            pending_store_gets: Arc::new(Mutex::new(HashMap::new())),
            outbound_tx,
            default_timeout,
        }
    }

    /// Process an inbound message received from the worker.
    pub async fn handle_inbound_message(&self, message: IpcMessage) -> Result<Option<IpcMessage>> {
        match message {
            IpcMessage::CallToolResponse { id, result } => {
                let mut map = self.pending_tool_calls.lock().await;
                if let Some(sender) = map.remove(&id) {
                    let _ = sender.send(result);
                }
                Ok(None)
            }
            IpcMessage::StoreGetResponse { id, value } => {
                let mut map = self.pending_store_gets.lock().await;
                if let Some(sender) = map.remove(&id) {
                    let _ = sender.send(value);
                }
                Ok(None)
            }
            // Other messages are handled upstream by orchestrator
            other => Ok(Some(other)),
        }
    }

    /// Send a tool call request to the worker and await correlated response.
    pub async fn call_tool(
        &self,
        tool_name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();

        {
            let mut map = self.pending_tool_calls.lock().await;
            map.insert(id, tx);
        }

        let msg = IpcMessage::CallToolRequest {
            id,
            tool_name: tool_name.into(),
            arguments,
        };

        self.outbound_tx
            .send(msg)
            .await
            .map_err(|_| anyhow!("Failed to send CallToolRequest across IPC channel"))?;

        match timeout(self.default_timeout, rx).await {
            Ok(Ok(Ok(val))) => Ok(val),
            Ok(Ok(Err(tool_err))) => Err(anyhow!("Tool execution failed: {tool_err}")),
            Ok(Err(_)) => Err(anyhow!(
                "Worker channel dropped before responding to tool call"
            )),
            Err(_) => Err(anyhow!(
                "Tool call timed out after {}ms",
                self.default_timeout.as_millis()
            )),
        }
    }

    /// Request a key from worker store.
    pub async fn store_get(&self, key: impl Into<String>) -> Result<Option<serde_json::Value>> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();

        {
            let mut map = self.pending_store_gets.lock().await;
            map.insert(id, tx);
        }

        let msg = IpcMessage::StoreGetRequest {
            id,
            key: key.into(),
        };

        self.outbound_tx
            .send(msg)
            .await
            .map_err(|_| anyhow!("Failed to send StoreGetRequest across IPC channel"))?;

        match timeout(self.default_timeout, rx).await {
            Ok(Ok(val)) => Ok(val),
            Ok(Err(_)) => Err(anyhow!(
                "Worker channel dropped before responding to store_get"
            )),
            Err(_) => Err(anyhow!("store_get timed out")),
        }
    }

    /// Update worker store with key-value pair.
    pub async fn store_set(&self, key: impl Into<String>, value: serde_json::Value) -> Result<()> {
        let msg = IpcMessage::StoreSetNotification {
            key: key.into(),
            value,
        };
        self.outbound_tx
            .send(msg)
            .await
            .map_err(|_| anyhow!("Failed to send StoreSetNotification across IPC channel"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_framing_codec_roundtrip() {
        let original = IpcMessage::CallToolRequest {
            id: 42,
            tool_name: "read_file".to_string(),
            arguments: serde_json::json!({ "path": "test.txt" }),
        };

        let encoded = IpcFramingCodec::encode(&original).expect("encode succeeds");
        assert!(encoded.len() > 4);

        let decoded = IpcFramingCodec::decode(&encoded).expect("decode succeeds");
        assert!(decoded.is_some());
        let (msg, consumed) = decoded.unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(msg, original);
    }

    #[test]
    fn test_framing_codec_partial_buffer() {
        let original = IpcMessage::LogStream {
            text: "Hello world".to_string(),
        };
        let encoded = IpcFramingCodec::encode(&original).unwrap();

        // Incomplete buffer (less than 4 bytes)
        assert!(IpcFramingCodec::decode(&encoded[..2]).unwrap().is_none());

        // Incomplete payload
        assert!(
            IpcFramingCodec::decode(&encoded[..encoded.len() - 3])
                .unwrap()
                .is_none()
        );

        // Complete buffer
        assert!(IpcFramingCodec::decode(&encoded).unwrap().is_some());
    }

    #[tokio::test]
    async fn test_ipc_peer_tool_call_roundtrip() {
        let (tx, mut rx) = mpsc::channel(16);
        let peer = IpcPeer::new(tx, Duration::from_millis(500));

        let peer_clone = Arc::new(peer);
        let p = Arc::clone(&peer_clone);

        tokio::spawn(async move {
            if let Some(IpcMessage::CallToolRequest { id, tool_name, .. }) = rx.recv().await {
                assert_eq!(tool_name, "read_file");
                let response = IpcMessage::CallToolResponse {
                    id,
                    result: Ok(serde_json::json!({ "contents": "file contents" })),
                };
                p.handle_inbound_message(response).await.unwrap();
            }
        });

        let result = peer_clone
            .call_tool("read_file", serde_json::json!({ "path": "test.txt" }))
            .await
            .expect("call_tool succeeds");

        assert_eq!(
            result.get("contents").and_then(|v| v.as_str()),
            Some("file contents")
        );
    }
}
