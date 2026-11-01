//! `m31a-mcp`: Advanced Model Context Protocol (MCP) Client and Hub.
//!
//! Provides a production-grade MCP client supporting Stdio, HTTP Server-Sent Events (SSE),
//! and WebSocket transports, OAuth 2.0 PKCE auth flow, app package manifests (`CodexAppManifest`),
//! dynamic tool catalog caching, and network security policies.

pub mod client;
pub mod manager;
pub mod manifest;
pub mod oauth;
pub mod protocol;
pub mod transport;

pub use client::McpClient;
pub use manager::{
    CALLABLE_NAME_HASH_LEN, MAX_TOOL_NAME_LENGTH, MCP_PREFIX, MCP_TOOL_NAME_DELIMITER, McpManager,
};
pub use manifest::{
    CodexAppManifest, CodexAppPermissions, CodexAppPrompt, CodexAppPromptArgument,
    CodexAppResource, CodexAppTool,
};
pub use oauth::{
    ElicitationRequest, ElicitationResponse, ElicitationReviewer, ElicitationType, FileAccessType,
    McpNetworkPolicy, McpOAuthConfig,
};
pub use protocol::{
    CallToolParams, CallToolResult, ClientCapabilities, Implementation, InitializeParams,
    InitializeResult, JsonRpcError, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse,
    ListResourcesResult, ListToolsResult, MCP_PROTOCOL_VERSION, McpContent, McpResource,
    McpResourceContents, McpTool, RootsCapability, ServerCapabilities,
};
pub use transport::{McpTransport, SseTransport, StdioTransport, WebSocketTransport};

#[cfg(test)]
mod tests {
    use super::*;
    use m31a_core::config::McpConfig;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_mcp_client_and_manager_real_stdio_server() {
        let dir = tempdir().unwrap();
        let server_script = dir.path().join("calc_mcp_server.py");

        // Python script implementing a real MCP server over Stdio
        let script_code = r#"
import sys, json

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    msg = json.loads(line)
    if "method" in msg and "id" in msg:
        method = msg["method"]
        msg_id = msg["id"]
        if method == "initialize":
            send({
                "jsonrpc": "2.0",
                "id": msg_id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "calc-server", "version": "1.0.0"}
                }
            })
        elif method == "tools/list":
            send({
                "jsonrpc": "2.0",
                "id": msg_id,
                "result": {
                    "tools": [{
                        "name": "add",
                        "description": "Add two numbers",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "a": {"type": "number"},
                                "b": {"type": "number"}
                            },
                            "required": ["a", "b"]
                        }
                    }]
                }
            })
        elif method == "tools/call":
            params = msg.get("params", {})
            args = params.get("arguments", {})
            res = args.get("a", 0) + args.get("b", 0)
            send({
                "jsonrpc": "2.0",
                "id": msg_id,
                "result": {
                    "content": [{"type": "text", "text": str(res)}],
                    "isError": False
                }
            })
        elif method == "ping":
            send({"jsonrpc": "2.0", "id": msg_id, "result": {}})
"#;
        std::fs::write(&server_script, script_code).unwrap();

        let client = McpClient::spawn(
            "calc",
            "python3",
            &[server_script.to_string_lossy().to_string()],
            &HashMap::new(),
            None,
        )
        .await
        .expect("spawn client");

        assert_eq!(client.server_info.as_ref().unwrap().name, "calc-server");

        let tools = client.list_tools().await.expect("list tools");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "add");

        let res = client
            .call_tool("add", Some(serde_json::json!({ "a": 10, "b": 32 })))
            .await
            .expect("call add");

        assert!(!res.is_error);
        if let McpContent::Text { text } = &res.content[0] {
            assert_eq!(text, "42");
        } else {
            panic!("Expected text content");
        }

        // Test McpManager integration
        let manager = McpManager::new(McpConfig::default());
        let defs = manager
            .connect_server(
                "calc",
                "python3",
                &[server_script.to_string_lossy().to_string()],
                &HashMap::new(),
                None,
            )
            .await
            .expect("manager connect");

        assert_eq!(defs.len(), 1);
        assert!(defs[0].function.name.starts_with("mcp__calc__add"));

        let out = manager
            .execute_tool(&defs[0].function.name, "{\"a\": 20, \"b\": 22}")
            .await
            .expect("execute tool via manager");
        assert_eq!(out, "42");
    }
}
