//! Model Context Protocol (MCP) integration module.
//!
//! Re-exports client connections, configuration loaders, multi-transport abstractions,
//! and process supervision from the dedicated `m31a-mcp` crate.

pub use m31a_mcp::*;

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

        // Real Python script implementing an MCP server over stdio
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
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "calc-server", "version": "1.0.0" }
                }
            })
        elif method == "tools/list":
            send({
                "jsonrpc": "2.0",
                "id": msg_id,
                "result": {
                    "tools": [{
                        "name": "add",
                        "description": "Add two numbers together",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "a": { "type": "number" },
                                "b": { "type": "number" }
                            },
                            "required": ["a", "b"]
                        }
                    }]
                }
            })
        elif method == "tools/call":
            params = msg.get("params", {})
            args = params.get("arguments", {})
            a = args.get("a", 0)
            b = args.get("b", 0)
            total = a + b
            send({
                "jsonrpc": "2.0",
                "id": msg_id,
                "result": {
                    "content": [{
                        "type": "text",
                        "text": f"Sum: {total}"
                    }],
                    "isError": False
                }
            })
        elif method == "ping":
            send({
                "jsonrpc": "2.0",
                "id": msg_id,
                "result": {}
            })
        else:
            send({
                "jsonrpc": "2.0",
                "id": msg_id,
                "error": { "code": -32601, "message": "Method not found" }
            })
"#;
        std::fs::write(&server_script, script_code).unwrap();

        let manager = McpManager::new(McpConfig::default());
        let tools = manager
            .connect_server(
                "calc",
                "python3",
                &[server_script.to_str().unwrap().to_string()],
                &HashMap::new(),
                Some(dir.path()),
            )
            .await
            .expect("Failed to connect to calc MCP server");

        assert_eq!(tools.len(), 1);
        let tool = &tools[0];
        assert_eq!(tool.function.name, "mcp__calc__add");
        assert!(tool.function.description.contains("[MCP: calc]"));

        // Verify is_mcp_tool
        assert!(manager.is_mcp_tool("mcp__calc__add").await);
        assert!(!manager.is_mcp_tool("exec_command").await);

        // Execute tool call via McpManager
        let result_str = manager
            .execute_mcp_tool("mcp__calc__add", serde_json::json!({ "a": 40, "b": 2 }))
            .await
            .expect("Tool call failed");

        assert_eq!(result_str.trim(), "Sum: 42");

        manager.shutdown_all().await;
    }
}
