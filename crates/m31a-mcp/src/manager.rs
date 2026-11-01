//! MCP server manager supervising process lifetimes, multi-transport connections, and tool registries.

use anyhow::{Result, bail};
use m31a_core::config::{McpConfig, McpServerConfig};
use m31a_core::types::{FunctionDefinition, ToolDefinition};
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::client::McpClient;
use crate::protocol::{McpResource, McpResourceContents};

pub const MCP_PREFIX: &str = "mcp__";
pub const MCP_TOOL_NAME_DELIMITER: &str = "__";
pub const MAX_TOOL_NAME_LENGTH: usize = 128;
pub const CALLABLE_NAME_HASH_LEN: usize = 12;

#[derive(Clone)]
pub struct McpManager {
    clients: Arc<RwLock<HashMap<String, Arc<McpClient>>>>,
    cached_tool_specs: Arc<RwLock<HashMap<String, (String, String)>>>,
    config: McpConfig,
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new(McpConfig::default())
    }
}

impl McpManager {
    pub fn new(config: McpConfig) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            cached_tool_specs: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    pub fn with_config(mut self, config: McpConfig) -> Self {
        self.config = config;
        self
    }

    /// Connects to all MCP servers defined in the configuration (Stdio, SSE, or WebSocket).
    pub async fn load_servers(
        &self,
        servers: &HashMap<String, McpServerConfig>,
        cwd: Option<&Path>,
    ) -> Result<Vec<ToolDefinition>> {
        let mut loaded_definitions = Vec::new();

        for (name, srv) in servers {
            match self.connect_server_config(name, srv, cwd).await {
                Ok(tools) => {
                    info!(server = %name, count = tools.len(), "Loaded MCP tools from server");
                    loaded_definitions.extend(tools);
                }
                Err(err) => {
                    warn!(server = %name, error = %err, "Failed to connect to configured MCP server");
                }
            }
        }

        Ok(loaded_definitions)
    }

    /// Connects to a server using its configuration, determining transport automatically.
    pub async fn connect_server_config(
        &self,
        name: &str,
        srv: &McpServerConfig,
        cwd: Option<&Path>,
    ) -> Result<Vec<ToolDefinition>> {
        let empty_headers = HashMap::new();
        let headers = srv.headers.as_ref().unwrap_or(&empty_headers);

        let client = if let Some(ref url) = srv.url {
            let transport_type = srv.transport.as_deref().unwrap_or(
                if url.starts_with("ws://") || url.starts_with("wss://") {
                    "websocket"
                } else {
                    "sse"
                },
            );

            match transport_type {
                "websocket" => McpClient::connect_websocket(name, url, headers).await?,
                _ => McpClient::connect_sse(name, url, headers).await?,
            }
        } else {
            McpClient::spawn(name, &srv.command, &srv.args, &srv.env, cwd).await?
        };

        let client_arc = Arc::new(client);
        self.clients
            .write()
            .await
            .insert(name.to_string(), client_arc.clone());

        self.register_client_tools(name, &client_arc).await
    }

    /// Spawns and initializes a single Stdio MCP server, discovering its tools.
    pub async fn connect_server(
        &self,
        name: &str,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        cwd: Option<&Path>,
    ) -> Result<Vec<ToolDefinition>> {
        let client = McpClient::spawn(name, command, args, env, cwd).await?;
        let client_arc = Arc::new(client);
        self.clients
            .write()
            .await
            .insert(name.to_string(), client_arc.clone());

        self.register_client_tools(name, &client_arc).await
    }

    async fn register_client_tools(
        &self,
        server_name: &str,
        client: &Arc<McpClient>,
    ) -> Result<Vec<ToolDefinition>> {
        let raw_tools = client.list_tools().await?;
        let mut tool_definitions = Vec::new();

        for tool in raw_tools {
            let callable_name = self.format_callable_name(server_name, &tool.name);
            let description = format!(
                "[MCP: {}] {}",
                server_name,
                tool.description
                    .as_deref()
                    .unwrap_or("No description provided")
            );

            let def = ToolDefinition {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name: callable_name.clone(),
                    description,
                    parameters: tool.input_schema,
                },
            };

            self.cached_tool_specs
                .write()
                .await
                .insert(callable_name, (server_name.to_string(), tool.name));

            tool_definitions.push(def);
        }

        Ok(tool_definitions)
    }

    /// Formats a namespaced callable identifier for an MCP tool (`mcp__<server>__<tool>`).
    pub fn format_callable_name(&self, server_name: &str, tool_name: &str) -> String {
        let candidate = format!(
            "{}{}{}{}",
            MCP_PREFIX, server_name, MCP_TOOL_NAME_DELIMITER, tool_name
        );
        if candidate.len() <= MAX_TOOL_NAME_LENGTH {
            candidate
        } else {
            let mut hasher = Sha1::new();
            hasher.update(candidate.as_bytes());
            let hash = format!("{:x}", hasher.finalize());
            let short_hash = &hash[..CALLABLE_NAME_HASH_LEN];
            let prefix = format!("{}{}{}", MCP_PREFIX, server_name, MCP_TOOL_NAME_DELIMITER);
            let max_tool_len = MAX_TOOL_NAME_LENGTH - prefix.len() - CALLABLE_NAME_HASH_LEN - 1;
            let truncated_tool = if tool_name.len() > max_tool_len {
                &tool_name[..max_tool_len]
            } else {
                tool_name
            };
            format!("{prefix}{truncated_tool}_{short_hash}")
        }
    }

    /// Executes an MCP tool call by callable name.
    pub async fn execute_tool(&self, callable_name: &str, arguments_json: &str) -> Result<String> {
        let (server_name, original_tool) = {
            let specs = self.cached_tool_specs.read().await;
            specs
                .get(callable_name)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Unknown MCP tool: '{callable_name}'"))?
        };

        let args: Value = if arguments_json.trim().is_empty() {
            Value::Null
        } else {
            serde_json::from_str(arguments_json)
                .map_err(|e| anyhow::anyhow!("Failed to parse JSON arguments: {e}"))?
        };

        self.execute_tool_with_server(&server_name, &original_tool, args)
            .await
    }

    /// Executes a tool call directly on a specific connected server.
    pub async fn execute_tool_with_server(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: Value,
    ) -> Result<String> {
        let client = {
            let clients = self.clients.read().await;
            clients
                .get(server_name)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("MCP server '{server_name}' is not connected"))?
        };

        let result = client.call_tool(tool_name, Some(arguments)).await?;
        if result.is_error {
            let mut error_msg = String::new();
            for c in result.content {
                if let crate::protocol::McpContent::Text { text } = c {
                    error_msg.push_str(&text);
                }
            }
            bail!(
                "MCP tool execution failed: {}",
                if error_msg.is_empty() {
                    "Unknown error".to_string()
                } else {
                    error_msg
                }
            );
        }

        let mut out = String::new();
        for c in result.content {
            match c {
                crate::protocol::McpContent::Text { text } => out.push_str(&text),
                crate::protocol::McpContent::Image { data, mime_type } => {
                    out.push_str(&format!("[Image: {mime_type}, {} bytes]", data.len()))
                }
                crate::protocol::McpContent::Resource { resource } => {
                    if let Some(txt) = resource.text {
                        out.push_str(&txt);
                    }
                }
            }
        }

        Ok(out)
    }

    /// Lists resources across connected servers or a specific server.
    pub async fn list_resources(&self, server_name: Option<&str>) -> Result<Vec<McpResource>> {
        let clients = self.clients.read().await;
        let mut all_resources = Vec::new();

        if let Some(name) = server_name {
            if let Some(client) = clients.get(name) {
                all_resources.extend(client.list_resources().await?);
            }
        } else {
            for client in clients.values() {
                if let Ok(res) = client.list_resources().await {
                    all_resources.extend(res);
                }
            }
        }

        Ok(all_resources)
    }

    /// Reads resource contents from a specific connected server.
    pub async fn read_resource(
        &self,
        server_name: &str,
        uri: &str,
    ) -> Result<Vec<McpResourceContents>> {
        let client = {
            let clients = self.clients.read().await;
            clients
                .get(server_name)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("MCP server '{server_name}' is not connected"))?
        };

        client.read_resource(uri).await
    }

    pub fn get_client(&self, server_name: &str) -> Option<Arc<McpClient>> {
        let clients = futures::executor::block_on(self.clients.read());
        clients.get(server_name).cloned()
    }

    /// Checks if a tool name belongs to a registered MCP tool.
    pub async fn is_mcp_tool(&self, name: &str) -> bool {
        self.cached_tool_specs.read().await.contains_key(name)
    }

    /// Executes an MCP tool call by callable name and JSON arguments Value.
    pub async fn execute_mcp_tool(&self, name: &str, args: Value) -> Result<String> {
        let (server_name, original_tool) = {
            let specs = self.cached_tool_specs.read().await;
            specs
                .get(name)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Unknown MCP tool: '{name}'"))?
        };

        self.execute_tool_with_server(&server_name, &original_tool, args)
            .await
    }

    /// Returns list of all connected server names.
    pub async fn list_server_names(&self) -> Vec<String> {
        self.clients.read().await.keys().cloned().collect()
    }

    /// Lists resources organized by server name.
    pub async fn list_all_resources(&self) -> HashMap<String, Vec<McpResource>> {
        let clients = self.clients.read().await;
        let mut map = HashMap::new();
        for (name, client) in clients.iter() {
            if let Ok(resources) = client.list_resources().await {
                map.insert(name.clone(), resources);
            }
        }
        map
    }

    /// Closes a single connected server, unregisters its tools, and removes it from the client map.
    pub async fn disconnect_server(&self, name: &str) -> bool {
        let client = {
            let mut clients = self.clients.write().await;
            clients.remove(name)
        };
        if let Some(client) = client {
            client.close().await;
            let mut specs = self.cached_tool_specs.write().await;
            specs.retain(|_, (srv, _)| srv != name);
            true
        } else {
            false
        }
    }

    /// Closes all connected servers and drains registries.
    pub async fn shutdown_all(&self) {
        let mut clients = self.clients.write().await;
        for (_, client) in clients.drain() {
            client.close().await;
        }
        self.cached_tool_specs.write().await.clear();
    }

    /// Synchronously checks if a server is currently connected without blocking.
    pub fn is_server_connected_sync(&self, server_name: &str) -> bool {
        self.clients
            .try_read()
            .map(|c| c.contains_key(server_name))
            .unwrap_or(false)
    }

    /// Synchronously lists cached tool names registered for a specific server without blocking.
    pub fn get_server_tools_sync(&self, server_name: &str) -> Vec<String> {
        if let Ok(specs) = self.cached_tool_specs.try_read() {
            specs
                .values()
                .filter(|(srv, _)| srv == server_name)
                .map(|(_, tool)| tool.clone())
                .collect()
        } else {
            Vec::new()
        }
    }
}
