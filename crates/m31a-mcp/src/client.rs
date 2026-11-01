//! Model Context Protocol (MCP) client communicating over Stdio, SSE, or WebSocket transports.

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;
use tokio::sync::{RwLock, oneshot};

use crate::protocol::*;
use crate::transport::{
    McpTransport, ResponseMap, SseTransport, StdioTransport, WebSocketTransport,
};

pub struct McpClient {
    pub name: String,
    transport: Arc<Box<dyn McpTransport>>,
    next_id: AtomicI64,
    pending: ResponseMap,
    pub server_info: Option<Implementation>,
    pub server_capabilities: Option<ServerCapabilities>,
    pub instructions: Option<String>,
}

impl McpClient {
    /// Spawns a local MCP server subprocess communicating over Stdio.
    pub async fn spawn(
        name: impl Into<String>,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        cwd: Option<&Path>,
    ) -> Result<Self> {
        let name_str = name.into();
        let pending: ResponseMap = Arc::new(RwLock::new(HashMap::new()));
        let transport =
            StdioTransport::spawn(&name_str, command, args, env, cwd, pending.clone()).await?;

        let mut client = Self {
            name: name_str,
            transport: Arc::new(Box::new(transport)),
            next_id: AtomicI64::new(1),
            pending,
            server_info: None,
            server_capabilities: None,
            instructions: None,
        };

        client.initialize().await?;
        Ok(client)
    }

    /// Connects to a remote MCP server via HTTP Server-Sent Events (SSE).
    pub async fn connect_sse(
        name: impl Into<String>,
        url: &str,
        headers: &HashMap<String, String>,
    ) -> Result<Self> {
        let name_str = name.into();
        let pending: ResponseMap = Arc::new(RwLock::new(HashMap::new()));
        let transport = SseTransport::connect(&name_str, url, headers, pending.clone()).await?;

        let mut client = Self {
            name: name_str,
            transport: Arc::new(Box::new(transport)),
            next_id: AtomicI64::new(1),
            pending,
            server_info: None,
            server_capabilities: None,
            instructions: None,
        };

        client.initialize().await?;
        Ok(client)
    }

    /// Connects to a remote MCP server via WebSocket.
    pub async fn connect_websocket(
        name: impl Into<String>,
        url: &str,
        headers: &HashMap<String, String>,
    ) -> Result<Self> {
        let name_str = name.into();
        let pending: ResponseMap = Arc::new(RwLock::new(HashMap::new()));
        let transport =
            WebSocketTransport::connect(&name_str, url, headers, pending.clone()).await?;

        let mut client = Self {
            name: name_str,
            transport: Arc::new(Box::new(transport)),
            next_id: AtomicI64::new(1),
            pending,
            server_info: None,
            server_capabilities: None,
            instructions: None,
        };

        client.initialize().await?;
        Ok(client)
    }

    pub fn next_id(&self) -> i64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Sends a JSON-RPC 2.0 request and awaits the server response.
    pub async fn call_method(&self, method: &str, params: Option<Value>) -> Result<Value> {
        let id = self.next_id();
        let req = JsonRpcRequest::new(id, method, params);

        let (tx, rx) = oneshot::channel();
        self.pending.write().await.insert(id, tx);

        self.transport.send(&req).await?;

        let timeout = Duration::from_secs(30);
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(Ok(val))) => Ok(val),
            Ok(Ok(Err(err))) => bail!("MCP JSON-RPC error: {err}"),
            Ok(Err(_)) => bail!("MCP response channel closed unexpectedly"),
            Err(_) => {
                self.pending.write().await.remove(&id);
                bail!("MCP request '{method}' timed out after {timeout:?}");
            }
        }
    }

    /// Performs the initial protocol handshake.
    pub async fn initialize(&mut self) -> Result<()> {
        let params = InitializeParams {
            protocol_version: MCP_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities {
                roots: Some(RootsCapability {
                    list_changed: Some(true),
                }),
                sampling: None,
            },
            client_info: Implementation {
                name: "m31a-client".to_string(),
                version: "0.1.13".to_string(),
            },
        };

        let result_val = self
            .call_method("initialize", Some(serde_json::to_value(params)?))
            .await
            .context("Failed to initialize MCP connection")?;

        let init_result: InitializeResult = serde_json::from_value(result_val)?;
        self.server_info = Some(init_result.server_info);
        self.server_capabilities = Some(init_result.capabilities);
        self.instructions = init_result.instructions;

        // Send notifications/initialized per spec
        let notif = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "notifications/initialized".to_string(),
            params: None,
        };
        let _ = self
            .transport
            .send(&JsonRpcRequest {
                jsonrpc: notif.jsonrpc,
                id: 0,
                method: notif.method,
                params: None,
            })
            .await;

        Ok(())
    }

    /// Pings the MCP server to verify responsiveness.
    pub async fn ping(&self) -> Result<()> {
        self.call_method("ping", None).await?;
        Ok(())
    }

    /// Lists tools exposed by the MCP server.
    pub async fn list_tools(&self) -> Result<Vec<McpTool>> {
        let result_val = self.call_method("tools/list", None).await?;
        let res: ListToolsResult = serde_json::from_value(result_val)?;
        Ok(res.tools)
    }

    /// Invokes a named tool with arguments on the server.
    pub async fn call_tool(&self, name: &str, arguments: Option<Value>) -> Result<CallToolResult> {
        let params = CallToolParams {
            name: name.to_string(),
            arguments,
        };
        let result_val = self
            .call_method("tools/call", Some(serde_json::to_value(params)?))
            .await?;
        let res: CallToolResult = serde_json::from_value(result_val)?;
        Ok(res)
    }

    /// Lists resources available on the server.
    pub async fn list_resources(&self) -> Result<Vec<McpResource>> {
        let result_val = self.call_method("resources/list", None).await?;
        let res: ListResourcesResult = serde_json::from_value(result_val)?;
        Ok(res.resources)
    }

    /// Reads resource contents for a specified URI.
    pub async fn read_resource(&self, uri: &str) -> Result<Vec<McpResourceContents>> {
        let params = serde_json::json!({ "uri": uri });
        let result_val = self.call_method("resources/read", Some(params)).await?;
        let contents = result_val
            .get("contents")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("Missing contents in resource response"))?;

        let mut res = Vec::new();
        for item in contents {
            res.push(serde_json::from_value(item.clone())?);
        }
        Ok(res)
    }

    pub async fn is_alive(&self) -> bool {
        self.transport.is_alive().await
    }

    pub async fn close(&self) {
        self.transport.close().await;
    }
}
