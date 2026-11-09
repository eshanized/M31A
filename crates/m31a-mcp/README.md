# `m31a-mcp`

> Model Context Protocol (MCP) client, multi-transport communication (Stdio, SSE, WebSocket), and dynamic external tool integration for M31A.

---

## Why Does This Crate Exist?

Anthropic's open-standard **Model Context Protocol (MCP)** has become the universal standard for integrating external data sources, developer tools, and cloud platforms into AI systems. Dozens of official and community MCP servers exist for GitHub, PostgreSQL, Linear, Brave Search, Figma, and local filesystems.

Rather than writing custom one-off adapters for every service, `m31a-mcp` turns M31A into a native, high-performance MCP client:
- Spawns and manages local MCP server processes over `stdio` pipes.
- Connects to remote MCP servers over HTTP/SSE and WebSockets.
- Performs protocol initialization, capability negotiation, and dynamic schema discovery.
- Exposes discovered MCP tools seamlessly to M31A agents alongside native tools.

---

## Core Concepts & How It Works

### 1. Multi-Transport Architecture (`transport.rs`)
Supports all canonical MCP transports:
- **`StdioTransport`**: Directly launches subprocesses (e.g., `npx -y @modelcontextprotocol/server-postgres ...`) and frames JSON-RPC messages across stdin/stdout.
- **`SseTransport`**: Connects to remote HTTP/SSE endpoints with event stream parsing.
- **`WebSocketTransport`**: Low-latency bidirectional socket transport for distributed setups.

### 2. Protocol Handshake & Tool Translation (`protocol.rs`, `client.rs`)
On connection, `McpClient`:
1. Sends `initialize` request with client info and capabilities.
2. Receives `server_capabilities` (tools, resources, prompts).
3. Invokes `tools/list` to fetch tool JSON schemas.
4. Translates MCP schemas into M31A `ToolDefinition` structs, registering them in [`m31a-tools`](../m31a-tools).

### 3. Server Manager (`manager.rs`)
`McpServerManager` orchestrates multiple MCP servers configured in `~/.m31a/config.toml`, handling automatic restarts and health monitoring.

---

## Key Modules & Types

| Module | Key Structs / Traits | Purpose |
| :--- | :--- | :--- |
| [`src/client.rs`](src/client.rs) | `McpClient` | Client handling protocol handshakes and tool invocations. |
| [`src/manager.rs`](src/manager.rs) | `McpServerManager` | Manages multiple running MCP servers from user configuration. |
| [`src/transport.rs`](src/transport.rs) | `McpTransport`, `StdioTransport`, `SseTransport` | Framing and I/O abstractions across stdio and network sockets. |
| [`src/protocol.rs`](src/protocol.rs) | `InitializeRequest`, `Tool`, `CallToolResult` | Typed JSON-RPC models for the MCP specification. |

---

## Practical Usage Example

```rust
use m31a_mcp::client::McpClient;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Spawn a local MCP server subprocess over stdio
    let mut client = McpClient::spawn(
        "filesystem-server",
        "npx",
        &["-y".into(), "@modelcontextprotocol/server-filesystem".into(), "./".into()],
        &HashMap::new(),
        None,
    ).await?;

    // 2. Perform initialization handshake
    client.initialize().await?;
    println!("Connected to MCP server: {:?}", client.server_info);

    // 3. List discovered tools
    let tools = client.list_tools().await?;
    for tool in tools {
        println!("Discovered MCP Tool: {} - {}", tool.name, tool.description.unwrap_or_default());
    }

    Ok(())
}
```

---

## CLI Integration

```bash
# List configured MCP servers
m31a mcp list

# Add a new MCP server
m31a mcp add postgres npx -y @modelcontextprotocol/server-postgres postgresql://localhost/mydb

# Remove an MCP server
m31a mcp remove postgres
```

---

## Testing & Verification

```bash
cargo test -p m31a-mcp
```
