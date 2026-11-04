//! TCP communication server for Distributed Task Mesh.
//!
//! Listens for remote task submissions and status queries over TCP, streaming JSON-RPC responses.

use anyhow::Result;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info};

use crate::lib_engine::TaskMeshNode;
use crate::types::{Priority, TaskDefinition, TaskId};

pub struct TaskMeshServer {
    node: Arc<TaskMeshNode>,
    bind_addr: String,
    auth_token: Option<String>,
}

impl TaskMeshServer {
    pub fn new(node: Arc<TaskMeshNode>, bind_addr: impl Into<String>) -> Self {
        Self {
            node,
            bind_addr: bind_addr.into(),
            auth_token: None,
        }
    }

    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    pub async fn run(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.bind_addr).await?;
        info!(addr = %self.bind_addr, "Task Mesh TCP Server listening");

        loop {
            let (stream, peer) = listener.accept().await?;
            let node = Arc::clone(&self.node);
            let auth_token = self.auth_token.clone();

            tokio::spawn(async move {
                if let Err(e) = Self::handle_client(stream, node, auth_token).await {
                    error!(peer = %peer, error = %e, "Client connection handler error");
                }
            });
        }
    }

    pub async fn handle_client(
        mut stream: TcpStream,
        node: Arc<TaskMeshNode>,
        auth_token: Option<String>,
    ) -> Result<()> {
        let (reader, mut writer) = stream.split();
        let mut lines = BufReader::new(reader).lines();
        let mut authenticated = auth_token.is_none();

        while let Some(line) = lines.next_line().await? {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let mut parts = line.splitn(2, ' ');
            let command = parts.next().unwrap_or("");
            let arg = parts.next().unwrap_or("");

            if !authenticated && command != "AUTH" && command != "PING" {
                let err_resp = serde_json::json!({
                    "status": "error",
                    "message": "Unauthorized: authentication required"
                });
                writer
                    .write_all(format!("{}\n", err_resp).as_bytes())
                    .await?;
                continue;
            }

            match command {
                "AUTH" => {
                    if let Some(ref expected) = auth_token {
                        if arg.trim() == expected.as_str() {
                            authenticated = true;
                            let ok_resp =
                                serde_json::json!({ "status": "ok", "message": "Authenticated" });
                            writer
                                .write_all(format!("{}\n", ok_resp).as_bytes())
                                .await?;
                        } else {
                            let err_resp = serde_json::json!({ "status": "error", "message": "Unauthorized: invalid credentials" });
                            writer
                                .write_all(format!("{}\n", err_resp).as_bytes())
                                .await?;
                        }
                    } else {
                        authenticated = true;
                        let ok_resp = serde_json::json!({ "status": "ok", "message": "No authentication required" });
                        writer
                            .write_all(format!("{}\n", ok_resp).as_bytes())
                            .await?;
                    }
                }
                "PING" => {
                    writer.write_all(b"PONG\n").await?;
                }
                "SUBMIT" => match serde_json::from_str::<serde_json::Value>(arg) {
                    Ok(payload) => {
                        let task = TaskDefinition {
                            id: TaskId::new(),
                            name: "client_task".to_string(),
                            payload,
                            priority: Priority::Normal,
                            max_retries: 3,
                            timeout_ms: 10000,
                            created_at: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                        };
                        let id = task.id;
                        node.submit(task)?;
                        let resp =
                            serde_json::json!({ "status": "submitted", "task_id": id.to_string() });
                        writer.write_all(format!("{}\n", resp).as_bytes()).await?;
                    }
                    Err(e) => {
                        let err_resp =
                            serde_json::json!({ "status": "error", "message": e.to_string() });
                        writer
                            .write_all(format!("{}\n", err_resp).as_bytes())
                            .await?;
                    }
                },
                "POLL" => {
                    let task_id_str = arg.trim();
                    if let Ok(uuid) = uuid::Uuid::parse_str(task_id_str) {
                        let status = node.get_status(&TaskId(uuid));
                        let resp = serde_json::json!({ "task_id": task_id_str, "status": status });
                        writer.write_all(format!("{}\n", resp).as_bytes()).await?;
                    } else {
                        let err_resp = serde_json::json!({ "status": "error", "message": "Invalid TaskId UUID" });
                        writer
                            .write_all(format!("{}\n", err_resp).as_bytes())
                            .await?;
                    }
                }
                "METRICS" => {
                    let snap = node.metrics.snapshot();
                    let resp = serde_json::to_string(&snap)?;
                    writer.write_all(format!("{}\n", resp).as_bytes()).await?;
                }
                "QUIT" => {
                    break;
                }
                _ => {
                    writer.write_all(b"ERROR: Unknown command\n").await?;
                }
            }
        }

        Ok(())
    }
}
