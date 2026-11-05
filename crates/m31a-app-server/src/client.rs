//! In-process and IPC client for communicating with the M31A App Server.
//!
//! Provides typed helper methods for sending JSON-RPC requests, managing turn admission,
//! and subscribing to real-time notification streams.

use anyhow::{Context, Result, anyhow};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, warn};

use crate::protocol::{JsonRpcError, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};

type PendingRequests = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, JsonRpcError>>>>>;

/// Strongly typed client for communicating with an M31A App Server instance over a Unix Domain Socket.
#[derive(Clone)]
pub struct AppServerClient {
    next_id: Arc<AtomicU64>,
    writer_tx: mpsc::UnboundedSender<String>,
    pending_requests: PendingRequests,
    _notification_tx: mpsc::UnboundedSender<JsonRpcNotification>,
}

impl AppServerClient {
    /// Connects to a running M31A App Server at `socket_path` and launches background I/O workers.
    pub async fn connect(
        socket_path: &Path,
    ) -> Result<(Self, mpsc::UnboundedReceiver<JsonRpcNotification>)> {
        let stream = UnixStream::connect(socket_path).await.with_context(|| {
            format!(
                "Failed to connect to M31A App Server socket at {}",
                socket_path.display()
            )
        })?;

        let (reader, mut writer) = stream.into_split();
        let (writer_tx, mut writer_rx) = mpsc::unbounded_channel::<String>();
        let (notif_tx, notif_rx) = mpsc::unbounded_channel::<JsonRpcNotification>();
        let pending: PendingRequests = Arc::new(Mutex::new(HashMap::new()));

        // Writer task
        tokio::spawn(async move {
            while let Some(msg) = writer_rx.recv().await {
                if let Err(e) = writer.write_all(msg.as_bytes()).await {
                    error!("AppServerClient writer error: {}", e);
                    break;
                }
                if let Err(e) = writer.write_all(b"\n").await {
                    error!("AppServerClient writer newline error: {}", e);
                    break;
                }
                let _ = writer.flush().await;
            }
        });

        // Reader task
        let pending_clone = Arc::clone(&pending);
        let notif_tx_clone = notif_tx.clone();
        tokio::spawn(async move {
            let mut buf_reader = BufReader::new(reader);
            let mut line = String::new();

            loop {
                line.clear();
                match buf_reader.read_line(&mut line).await {
                    Ok(0) => {
                        debug!("AppServerClient: connection closed by server");
                        break;
                    }
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }

                        // Parse as JSON-RPC response or notification
                        if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
                            let has_id = val.get("id").map(|v| !v.is_null()).unwrap_or(false);
                            let has_method = val.get("method").and_then(|v| v.as_str()).is_some();

                            if has_method && !has_id {
                                if let Ok(notif) =
                                    serde_json::from_value::<JsonRpcNotification>(val)
                                {
                                    let _ = notif_tx_clone.send(notif);
                                }
                            } else if has_id {
                                if let Ok(resp) = serde_json::from_value::<JsonRpcResponse>(val)
                                    && let Some(Value::Number(num)) = &resp.id
                                    && let Some(id_u64) = num.as_u64()
                                {
                                    let mut map = pending_clone.lock().unwrap();
                                    if let Some(sender) = map.remove(&id_u64) {
                                        if let Some(err) = resp.error {
                                            let _ = sender.send(Err(err));
                                        } else {
                                            let _ =
                                                sender.send(Ok(resp.result.unwrap_or(Value::Null)));
                                        }
                                    }
                                }
                            } else {
                                warn!("Unrecognized payload from app server: {}", trimmed);
                            }
                        } else {
                            warn!("Failed to parse JSON payload from app server: {}", trimmed);
                        }
                    }
                    Err(e) => {
                        error!("AppServerClient reader error: {}", e);
                        break;
                    }
                }
            }
        });

        let client = Self {
            next_id: Arc::new(AtomicU64::new(1)),
            writer_tx,
            pending_requests: pending,
            _notification_tx: notif_tx,
        };

        Ok((client, notif_rx))
    }

    /// Low-level RPC method invocation awaiting a response.
    pub async fn invoke(&self, method: &str, params: Value) -> Result<Value> {
        let req_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::from(req_id)),
            method: method.to_string(),
            params: Some(params),
        };

        let serialized = serde_json::to_string(&req)?;
        let (tx, rx) = oneshot::channel();

        {
            let mut map = self.pending_requests.lock().unwrap();
            map.insert(req_id, tx);
        }

        self.writer_tx
            .send(serialized)
            .map_err(|_| anyhow!("Failed to send request: connection closed"))?;

        // Wait with a 30 second timeout
        match tokio::time::timeout(Duration::from_secs(30), rx).await {
            Ok(Ok(Ok(val))) => Ok(val),
            Ok(Ok(Err(rpc_err))) => Err(anyhow!(
                "JSON-RPC Error [{}]: {}",
                rpc_err.code,
                rpc_err.message
            )),
            Ok(Err(_)) => Err(anyhow!("Request sender dropped before response arrived")),
            Err(_) => {
                let mut map = self.pending_requests.lock().unwrap();
                map.remove(&req_id);
                Err(anyhow!(
                    "App server request timed out (30s) for method: {}",
                    method
                ))
            }
        }
    }

    /// Send a one-way notification to the server without awaiting a response.
    pub fn notify(&self, method: &str, params: Value) -> Result<()> {
        let notif = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: Some(params),
        };
        let serialized = serde_json::to_string(&notif)?;
        self.writer_tx
            .send(serialized)
            .map_err(|_| anyhow!("Failed to send notification: connection closed"))?;
        Ok(())
    }

    // Typed Convenience RPCs

    /// Initialize connection with the app server.
    pub async fn initialize(&self, client_name: &str, client_version: &str) -> Result<Value> {
        let params = serde_json::json!({
            "client_name": client_name,
            "client_version": client_version,
            "capabilities": {
                "streaming": true,
                "diff_events": true
            }
        });
        self.invoke("initialize", params).await
    }

    /// Start a new agent thread in the given workspace.
    pub async fn thread_start(&self, cwd: &Path, role: Option<&str>) -> Result<String> {
        self.thread_start_authorized(cwd, role, false).await
    }

    /// Request trust grant for a workspace path.
    pub async fn grant_workspace_trust(&self, path: &Path, level: &str) -> Result<()> {
        let params = serde_json::json!({
            "path": path.to_string_lossy(),
            "level": level,
        });
        self.invoke("workspace/trust_grant", params).await?;
        Ok(())
    }

    /// Start a new agent thread after granting legitimate workspace trust.
    pub async fn thread_start_authorized(
        &self,
        cwd: &Path,
        role: Option<&str>,
        authorize_session: bool,
    ) -> Result<String> {
        if authorize_session {
            self.grant_workspace_trust(cwd, "session_only").await?;
        }
        let mut params = serde_json::json!({
            "cwd": cwd.to_string_lossy(),
        });
        if let Some(r) = role {
            params["role"] = Value::String(r.to_string());
        }

        let resp = self.invoke("thread/start", params).await?;
        let thread_id = resp
            .get("thread_id")
            .or_else(|| resp.get("id"))
            .and_then(|v| v.as_str())
            .context("Missing 'thread_id' in response")?;
        Ok(thread_id.to_string())
    }

    /// Send a message to an active thread.
    pub async fn thread_send_message(&self, thread_id: &str, text: &str) -> Result<String> {
        let params = serde_json::json!({
            "thread_id": thread_id,
            "message": text,
        });
        let resp = self.invoke("thread/send_message", params).await?;
        let turn_id = resp
            .get("turn_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        Ok(turn_id.to_string())
    }

    /// List active or historical threads.
    pub async fn thread_list(&self) -> Result<Vec<Value>> {
        let resp = self.invoke("thread/list", serde_json::json!({})).await?;
        let list = resp
            .get("threads")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(list)
    }

    /// Interrupt an in-flight turn or subagent run.
    pub async fn thread_interrupt(&self, thread_id: &str) -> Result<()> {
        let params = serde_json::json!({ "thread_id": thread_id });
        self.invoke("thread/interrupt", params).await?;
        Ok(())
    }

    /// Query server rate limits and token usage quotas.
    pub async fn account_rate_limits(&self) -> Result<Value> {
        self.invoke("account/rate_limits/read", serde_json::json!({}))
            .await
    }

    /// Create a checkpoint snapshot.
    pub async fn create_checkpoint(
        &self,
        thread_id: &str,
        label: &str,
        git_commit: Option<&str>,
        turn_index: usize,
        modified_files: &[String],
    ) -> Result<Value> {
        let params = serde_json::json!({
            "thread_id": thread_id,
            "label": label,
            "git_commit": git_commit,
            "turn_index": turn_index,
            "modified_files": modified_files,
        });
        self.invoke("checkpoint/create", params).await
    }

    /// List checkpoints for a thread.
    pub async fn list_checkpoints(&self, thread_id: &str) -> Result<Vec<Value>> {
        let params = serde_json::json!({ "thread_id": thread_id });
        let resp = self.invoke("checkpoint/list", params).await?;
        let list = resp.as_array().cloned().unwrap_or_default();
        Ok(list)
    }

    /// Restore session to a checkpoint snapshot.
    pub async fn restore_checkpoint(
        &self,
        checkpoint_id: &str,
        revert_conversation: bool,
    ) -> Result<Value> {
        let params = serde_json::json!({
            "checkpoint_id": checkpoint_id,
            "revert_conversation": revert_conversation,
        });
        self.invoke("checkpoint/restore", params).await
    }

    /// List active concurrency path locks across the workspace.
    pub async fn list_locks(&self) -> Result<Vec<Value>> {
        let resp = self.invoke("locks/list", serde_json::json!({})).await?;
        let list = resp.as_array().cloned().unwrap_or_default();
        Ok(list)
    }

    /// List active instruction sources and attribution metadata for a workspace.
    pub async fn instructions_list(&self, cwd: Option<&Path>) -> Result<Vec<Value>> {
        let mut params = serde_json::json!({});
        if let Some(p) = cwd {
            params["cwd"] = Value::String(p.to_string_lossy().to_string());
        }
        let resp = self.invoke("instructions/list", params).await?;
        let list = resp.as_array().cloned().unwrap_or_default();
        Ok(list)
    }

    /// Creates a new thread organization section.
    pub async fn create_thread_section(
        &self,
        name: &str,
        color: Option<&str>,
        icon: Option<&str>,
    ) -> Result<Value> {
        let mut params = serde_json::json!({ "name": name });
        if let Some(c) = color {
            params["color"] = Value::String(c.to_string());
        }
        if let Some(i) = icon {
            params["icon"] = Value::String(i.to_string());
        }
        self.invoke("thread/section/create", params).await
    }

    /// Lists all defined thread sections.
    pub async fn list_thread_sections(&self) -> Result<Vec<Value>> {
        let resp = self
            .invoke("thread/section/list", serde_json::json!({}))
            .await?;
        let list = resp.as_array().cloned().unwrap_or_default();
        Ok(list)
    }

    /// Moves a thread into a section (or clears its section if None).
    pub async fn move_thread_to_section(
        &self,
        thread_id: &str,
        section_id: Option<&str>,
    ) -> Result<bool> {
        let mut params = serde_json::json!({ "thread_id": thread_id });
        if let Some(s) = section_id {
            params["section_id"] = Value::String(s.to_string());
        }
        let resp = self.invoke("thread/section/move", params).await?;
        Ok(resp.get("moved").and_then(|v| v.as_bool()).unwrap_or(false))
    }

    /// Deletes a thread section by ID.
    pub async fn delete_thread_section(&self, section_id: &str) -> Result<bool> {
        let resp = self
            .invoke(
                "thread/section/delete",
                serde_json::json!({ "section_id": section_id }),
            )
            .await?;
        Ok(resp
            .get("deleted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false))
    }

    /// Gracefully shutdown the app server daemon.
    pub async fn app_server_stop(&self) -> Result<()> {
        self.invoke("app_server/stop", serde_json::json!({}))
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_app_server_client_roundtrip() {
        use crate::protocol::AppServer;
        use m31a_core::Config;
        use m31a_storage::Storage;
        use m31a_tools::ToolRegistry;

        let dir = tempdir().unwrap();
        let sock_path = dir.path().join("real_app.sock");

        let config = Arc::new(Config {
            model: "meta/llama-3.3-70b-instruct".to_string(),
            model_provider: Some("nim".to_string()),
            api_key: Some("test-api-key".to_string()),
            ..Default::default()
        });
        let storage = Storage::open_in_memory().unwrap();
        let tools = Arc::new(ToolRegistry::new());
        let server = AppServer::new(config, storage, tools).unwrap();

        let server_clone = server.clone();
        let sock_clone = sock_path.clone();
        tokio::spawn(async move {
            let _ = server_clone.run_uds(&sock_clone).await;
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let (client, _notifs) = AppServerClient::connect(&sock_path).await.unwrap();

        // 1. Initialize
        let init_resp = client.initialize("m31a-cli", "0.1.13").await.unwrap();
        assert_eq!(init_resp["server_name"], "m31a-app-server");
        assert_eq!(init_resp["version"], "0.1.13");

        // 2. Thread start in untrusted workspace without authorization fails closed
        let unauth_res = client
            .thread_start(Path::new("/tmp"), Some("general"))
            .await;
        assert!(
            unauth_res.is_err(),
            "Untrusted workspace must fail closed without authorization"
        );

        // 3. Thread start with explicit session authorization succeeds
        let thread_id = client
            .thread_start_authorized(Path::new("/tmp"), Some("general"), true)
            .await
            .unwrap();
        assert!(
            !thread_id.is_empty(),
            "Thread ID should be generated by real AppServer"
        );
    }

    #[tokio::test]
    async fn test_live_app_server_streaming_notifications_and_checkpoints() {
        use crate::protocol::AppServer;
        use m31a_core::Config;
        use m31a_storage::Storage;
        use m31a_tools::ToolRegistry;

        let dir = tempdir().unwrap();
        let sock_path = dir.path().join("live_app_server.sock");

        let config = Arc::new(Config {
            model: "meta/llama-3.3-70b-instruct".to_string(),
            model_provider: Some("nim".to_string()),
            api_key: Some("test-api-key".to_string()),
            ..Default::default()
        });
        let storage = Storage::open_in_memory().unwrap();
        let tools = Arc::new(ToolRegistry::new());
        let server = AppServer::new(config, storage, tools).unwrap();

        let server_clone = server.clone();
        let sock_clone = sock_path.clone();
        tokio::spawn(async move {
            let _ = server_clone.run_uds(&sock_clone).await;
        });

        // Wait for socket to become ready
        tokio::time::sleep(Duration::from_millis(100)).await;

        let (client, mut notif_rx) = AppServerClient::connect(&sock_path).await.unwrap();

        // 1. Initialize
        let init_val = client.initialize("m31a-test", "0.1.13").await.unwrap();
        assert_eq!(init_val["server_name"], "m31a-app-server");

        // 2. Grant workspace trust legitimately before creating thread
        client
            .grant_workspace_trust(Path::new("/tmp"), "session_only")
            .await
            .unwrap();

        let thread_res = client
            .invoke("thread/create", serde_json::json!({ "cwd": "/tmp" }))
            .await
            .unwrap();
        let thread_id = thread_res["id"].as_str().unwrap();

        // 3. Create checkpoint
        let cp_res = client
            .create_checkpoint(
                thread_id,
                "Live Test Checkpoint",
                Some("sha_xyz"),
                1,
                &["crates/test.rs".to_string()],
            )
            .await
            .unwrap();
        let cp_id = cp_res["id"].as_str().unwrap();

        // 4. Verify streaming notification arrived over UDS
        let notif = tokio::time::timeout(Duration::from_secs(3), notif_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(notif.method, "notification/checkpoint_created");
        assert_eq!(notif.params.unwrap()["id"], cp_id);

        // 5. List checkpoints
        let cp_list = client.list_checkpoints(thread_id).await.unwrap();
        assert_eq!(cp_list.len(), 1);
        assert_eq!(cp_list[0]["label"], "Live Test Checkpoint");

        // 6. List locks
        let locks = client.list_locks().await.unwrap();
        assert!(locks.is_empty());

        // 7. List instructions
        let instructions = client.instructions_list(Some(dir.path())).await.unwrap();
        assert!(instructions.is_empty());
    }
}
