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
}

impl TaskMeshServer {
    pub fn new(node: Arc<TaskMeshNode>, bind_addr: impl Into<String>) -> Self {
        Self {
            node,
            bind_addr: bind_addr.into(),
        }
    }

    pub async fn run(&self) -> Result<()> {
        let listener = TcpListener::bind(&self.bind_addr).await?;
        info!(addr = %self.bind_addr, "Task Mesh TCP Server listening");

        loop {
            let (stream, peer) = listener.accept().await?;
            let node = Arc::clone(&self.node);

            tokio::spawn(async move {
                if let Err(e) = Self::handle_client(stream, node).await {
                    error!(peer = %peer, error = %e, "Client connection handler error");
                }
            });
        }
    }

    pub async fn handle_client(mut stream: TcpStream, node: Arc<TaskMeshNode>) -> Result<()> {
        let (reader, mut writer) = stream.split();
        let mut lines = BufReader::new(reader).lines();

        while let Some(line) = lines.next_line().await? {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let mut parts = line.splitn(2, ' ');
            let command = parts.next().unwrap_or("");
            let arg = parts.next().unwrap_or("");

            match command {
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
