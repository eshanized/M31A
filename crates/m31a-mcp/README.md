# `m31a-mcp`

Production-grade Model Context Protocol (MCP) client, transport manager, and OAuth 2.0 PKCE authentication hub for **M31A**.

---

## Overview

`m31a-mcp` connects M31A to the open Model Context Protocol ecosystem. It manages child process stdio connections, HTTP/SSE streaming, and WebSocket transports. It supports dynamic tool discovery, lazy schema fetching, OAuth 2.0 PKCE authentication, and manifest loading (`CodexAppManifest`).

---

## User-Facing Capabilities

- **MCP Server Management (`m31a mcp`)**:
  ```bash
  m31a mcp list
  m31a mcp add filesystem npx -y @modelcontextprotocol/server-filesystem /path
  m31a mcp remove filesystem
  ```
- **Context Injection**: External resources and tool definitions registered by MCP servers appear natively in the agent's available tools catalog.

---

## Developer Guide & Architecture

### Key Modules

- [`client`](src/client.rs): `McpClient` implementing JSON-RPC 2.0 protocol over stdio, HTTP/SSE, and WebSockets.
- [`manager`](src/manager.rs): `McpManager` coordinating multiple active servers, tool name prefixes (`mcp__<server>__<tool>`), and permissions.
- [`manifest`](src/manifest.rs): `CodexAppManifest` declaring permissions, tools, resources, and prompt templates.
- [`oauth`](src/oauth.rs): Handles OAuth 2.0 with PKCE for remote cloud services and APIs.

---

## Testing

```bash
cargo test -p m31a-mcp
```
