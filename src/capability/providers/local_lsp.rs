//! local lsp persistent stdio process provider.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, RwLock, mpsc, oneshot};

/// lifecycle states of a persistent lsp session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspSessionState {
    Uninitialized,
    Initializing,
    Ready,
    ShuttingDown,
    Stopped,
}

/// outgoing json-rpc message for the writer task.
#[derive(Debug)]
enum LspOutgoingMessage {
    Request {
        id: i64,
        method: String,
        params: serde_json::Value,
    },
    Notification {
        method: String,
        params: serde_json::Value,
    },
}

type PendingResponseSender = oneshot::Sender<Result<serde_json::Value, String>>;
type PendingRequestsMap = Arc<Mutex<HashMap<i64, PendingResponseSender>>>;

/// persistent lsp session managing a long-lived child stdio process.
#[derive(Clone)]
pub struct PersistentLspSession {
    workspace_root: PathBuf,
    cmd_bin: String,
    cmd_args: Vec<String>,
    next_request_id: Arc<AtomicI64>,
    state: Arc<RwLock<LspSessionState>>,
    open_documents: Arc<RwLock<HashMap<String, i32>>>,
    pending_requests: PendingRequestsMap,
    tx_outgoing: mpsc::Sender<LspOutgoingMessage>,
    child_handle: Arc<Mutex<Option<Child>>>,
}

impl PersistentLspSession {
    /// start a new persistent lsp session, executing the initialize handshake.
    pub async fn start(
        workspace_root: &Path,
        cmd_bin: &str,
        cmd_args: &[&str],
    ) -> Result<Self, String> {
        let mut cmd = Command::new(cmd_bin);
        cmd.args(cmd_args)
            .current_dir(workspace_root)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("failed to spawn lsp server {cmd_bin}: {e}"))?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "failed to capture lsp server stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "failed to capture lsp server stdout".to_string())?;

        let (tx_outgoing, mut rx_outgoing) = mpsc::channel::<LspOutgoingMessage>(128);
        let pending_requests: PendingRequestsMap = Arc::new(Mutex::new(HashMap::new()));
        let state = Arc::new(RwLock::new(LspSessionState::Initializing));
        let open_documents = Arc::new(RwLock::new(HashMap::new()));
        let next_request_id = Arc::new(AtomicI64::new(1));
        let child_handle = Arc::new(Mutex::new(Some(child)));

        // writer loop
        let state_writer = state.clone();
        tokio::spawn(async move {
            while let Some(msg) = rx_outgoing.recv().await {
                let val = match msg {
                    LspOutgoingMessage::Request { id, method, params } => serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "method": method,
                        "params": params,
                    }),
                    LspOutgoingMessage::Notification { method, params } => serde_json::json!({
                        "jsonrpc": "2.0",
                        "method": method,
                        "params": params,
                    }),
                };
                let payload = match serde_json::to_string(&val) {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                let header = format!("Content-Length: {}\r\n\r\n", payload.len());
                if stdin.write_all(header.as_bytes()).await.is_err() {
                    *state_writer.write().await = LspSessionState::Stopped;
                    break;
                }
                if stdin.write_all(payload.as_bytes()).await.is_err() {
                    *state_writer.write().await = LspSessionState::Stopped;
                    break;
                }
                if stdin.flush().await.is_err() {
                    *state_writer.write().await = LspSessionState::Stopped;
                    break;
                }
            }
        });

        // reader loop
        let pending_clone = pending_requests.clone();
        let state_reader = state.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut content_length: Option<usize> = None;
                let mut line = String::new();
                let mut eof = false;

                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) => {
                            eof = true;
                            break;
                        }
                        Ok(_) => {
                            let trimmed = line.trim();
                            if trimmed.is_empty() {
                                break;
                            }
                            if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
                                if let Ok(len) = rest.trim().parse::<usize>() {
                                    content_length = Some(len);
                                }
                            }
                        }
                        Err(_) => {
                            eof = true;
                            break;
                        }
                    }
                }

                if eof {
                    break;
                }

                let Some(len) = content_length else {
                    continue;
                };

                let mut buf = vec![0u8; len];
                if reader.read_exact(&mut buf).await.is_err() {
                    break;
                }

                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&buf) {
                    if let Some(id) = val.get("id").and_then(|v| v.as_i64()) {
                        let mut map = pending_clone.lock().await;
                        if let Some(tx) = map.remove(&id) {
                            if let Some(err) = val.get("error") {
                                let _ = tx.send(Err(format!("lsp error: {err}")));
                            } else {
                                let result = val
                                    .get("result")
                                    .cloned()
                                    .unwrap_or(serde_json::Value::Null);
                                let _ = tx.send(Ok(result));
                            }
                        }
                    }
                }
            }

            // eof or read error: fail pending requests and transition to stopped
            *state_reader.write().await = LspSessionState::Stopped;
            let mut map = pending_clone.lock().await;
            for (_, tx) in map.drain() {
                let _ = tx.send(Err("lsp process terminated unexpectedly".to_string()));
            }
        });

        // handshake: initialize request
        let root_uri = format!(
            "file://{}",
            workspace_root
                .canonicalize()
                .unwrap_or_else(|_| workspace_root.to_path_buf())
                .display()
        );

        let init_id = next_request_id.fetch_add(1, Ordering::SeqCst);
        let (init_tx, init_rx) = oneshot::channel();
        pending_requests.lock().await.insert(init_id, init_tx);

        let init_params = serde_json::json!({
            "processId": null,
            "rootUri": root_uri,
            "capabilities": {
                "textDocument": {
                    "hover": {},
                    "definition": {},
                    "references": {},
                    "documentSymbol": {},
                    "synchronization": {
                        "dynamicRegistration": false,
                        "willSave": false,
                        "willSaveWaitUntil": false,
                        "didSave": false
                    }
                },
                "workspace": {
                    "symbol": {}
                }
            }
        });

        tx_outgoing
            .send(LspOutgoingMessage::Request {
                id: init_id,
                method: "initialize".to_string(),
                params: init_params,
            })
            .await
            .map_err(|e| format!("failed to enqueue initialize request: {e}"))?;

        // wait for initialize response with bounded timeout
        match tokio::time::timeout(Duration::from_secs(10), init_rx).await {
            Ok(Ok(Ok(_))) => {
                // send initialized notification
                let _ = tx_outgoing
                    .send(LspOutgoingMessage::Notification {
                        method: "initialized".to_string(),
                        params: serde_json::json!({}),
                    })
                    .await;
                *state.write().await = LspSessionState::Ready;
            }
            Ok(Ok(Err(e))) => {
                *state.write().await = LspSessionState::Stopped;
                return Err(format!("lsp initialize response error: {e}"));
            }
            Ok(Err(_)) => {
                *state.write().await = LspSessionState::Stopped;
                return Err("lsp initialize channel dropped".to_string());
            }
            Err(_) => {
                *state.write().await = LspSessionState::Stopped;
                return Err("lsp initialize handshake timed out after 10s".to_string());
            }
        }

        Ok(Self {
            workspace_root: workspace_root.to_path_buf(),
            cmd_bin: cmd_bin.to_string(),
            cmd_args: cmd_args.iter().map(|s| s.to_string()).collect(),
            next_request_id,
            state,
            open_documents,
            pending_requests,
            tx_outgoing,
            child_handle,
        })
    }

    /// get the workspace root path.
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// get the command binary.
    pub fn cmd_bin(&self) -> &str {
        &self.cmd_bin
    }

    /// get the command arguments.
    pub fn cmd_args(&self) -> &[String] {
        &self.cmd_args
    }

    /// check if the session is currently in ready state.
    pub async fn is_alive(&self) -> bool {
        *self.state.read().await == LspSessionState::Ready
    }

    /// get the current session lifecycle state.
    pub async fn state(&self) -> LspSessionState {
        *self.state.read().await
    }

    /// execute an lsp json-rpc request with a bounded timeout.
    pub async fn query(
        &self,
        method: &str,
        params: serde_json::Value,
        timeout: Duration,
    ) -> Result<serde_json::Value, String> {
        let current_state = *self.state.read().await;
        if current_state != LspSessionState::Ready {
            return Err(format!(
                "lsp session is not ready (state: {current_state:?})"
            ));
        }

        let id = self.next_request_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending_requests.lock().await.insert(id, tx);

        if let Err(e) = self
            .tx_outgoing
            .send(LspOutgoingMessage::Request {
                id,
                method: method.to_string(),
                params,
            })
            .await
        {
            self.pending_requests.lock().await.remove(&id);
            return Err(format!("failed to send lsp request: {e}"));
        }

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(res)) => res,
            Ok(Err(_)) => {
                self.pending_requests.lock().await.remove(&id);
                Err("lsp response channel dropped".to_string())
            }
            Err(_) => {
                self.pending_requests.lock().await.remove(&id);
                Err(format!("lsp query '{method}' timed out after {timeout:?}"))
            }
        }
    }

    /// notify the lsp server that a document has been opened.
    pub async fn did_open(
        &self,
        file_path: &Path,
        content: &str,
        language_id: &str,
    ) -> Result<(), String> {
        let file_uri = format!(
            "file://{}",
            file_path
                .canonicalize()
                .unwrap_or_else(|_| file_path.to_path_buf())
                .display()
        );
        self.open_documents
            .write()
            .await
            .insert(file_uri.clone(), 1);

        self.tx_outgoing
            .send(LspOutgoingMessage::Notification {
                method: "textDocument/didOpen".to_string(),
                params: serde_json::json!({
                    "textDocument": {
                        "uri": file_uri,
                        "languageId": language_id,
                        "version": 1,
                        "text": content,
                    }
                }),
            })
            .await
            .map_err(|e| format!("failed to send didOpen notification: {e}"))
    }

    /// notify the lsp server of document modifications with monotonic version increments.
    pub async fn did_change(&self, file_path: &Path, content: &str) -> Result<(), String> {
        let file_uri = format!(
            "file://{}",
            file_path
                .canonicalize()
                .unwrap_or_else(|_| file_path.to_path_buf())
                .display()
        );

        let mut docs = self.open_documents.write().await;
        let version = docs
            .entry(file_uri.clone())
            .and_modify(|v| *v += 1)
            .or_insert(1);
        let cur_version = *version;
        drop(docs);

        self.tx_outgoing
            .send(LspOutgoingMessage::Notification {
                method: "textDocument/didChange".to_string(),
                params: serde_json::json!({
                    "textDocument": {
                        "uri": file_uri,
                        "version": cur_version,
                    },
                    "contentChanges": [
                        { "text": content }
                    ]
                }),
            })
            .await
            .map_err(|e| format!("failed to send didChange notification: {e}"))
    }

    /// notify the lsp server that a document has been closed.
    pub async fn did_close(&self, file_path: &Path) -> Result<(), String> {
        let file_uri = format!(
            "file://{}",
            file_path
                .canonicalize()
                .unwrap_or_else(|_| file_path.to_path_buf())
                .display()
        );
        self.open_documents.write().await.remove(&file_uri);

        self.tx_outgoing
            .send(LspOutgoingMessage::Notification {
                method: "textDocument/didClose".to_string(),
                params: serde_json::json!({
                    "textDocument": {
                        "uri": file_uri,
                    }
                }),
            })
            .await
            .map_err(|e| format!("failed to send didClose notification: {e}"))
    }

    /// cleanly shutdown and exit the persistent lsp session.
    pub async fn shutdown(&self) -> Result<(), String> {
        {
            let mut st = self.state.write().await;
            if *st != LspSessionState::Ready {
                *st = LspSessionState::Stopped;
                if let Some(mut child) = self.child_handle.lock().await.take() {
                    let _ = child.kill().await;
                }
                return Ok(());
            }
            *st = LspSessionState::ShuttingDown;
        }

        let id = self.next_request_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending_requests.lock().await.insert(id, tx);

        let _ = self
            .tx_outgoing
            .send(LspOutgoingMessage::Request {
                id,
                method: "shutdown".to_string(),
                params: serde_json::Value::Null,
            })
            .await;

        let _ = tokio::time::timeout(Duration::from_secs(3), rx).await;

        let _ = self
            .tx_outgoing
            .send(LspOutgoingMessage::Notification {
                method: "exit".to_string(),
                params: serde_json::Value::Null,
            })
            .await;

        *self.state.write().await = LspSessionState::Stopped;

        if let Some(mut child) = self.child_handle.lock().await.take() {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = child.kill().await;
        }

        Ok(())
    }

    /// get a snapshot of open document versions.
    pub async fn open_documents(&self) -> HashMap<String, i32> {
        self.open_documents.read().await.clone()
    }
}

/// backward-compatible local lsp process client.
pub struct LocalLspProcessClient;

impl LocalLspProcessClient {
    /// execute stdio json-rpc query with bounded timeout via a persistent session.
    pub async fn query_stdio(
        workspace_root: &Path,
        cmd_bin: &str,
        cmd_args: &[&str],
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let session = PersistentLspSession::start(workspace_root, cmd_bin, cmd_args).await?;
        let res = session.query(method, params, Duration::from_secs(5)).await;
        let _ = session.shutdown().await;
        res
    }
}
