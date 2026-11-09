# `m31a-app-server`

> High-performance JSON-RPC 2.0 daemon server, multi-transport IPC (Stdio, Unix Domain Socket, HTTP/SSE), turn admission control, and resource validation for M31A.

---

## Why Does This Crate Exist?

To integrate an autonomous coding assistant into IDE extensions (VS Code, Cursor, JetBrains), external CI/CD pipelines, or multi-language client SDKs (TypeScript and Python), you need a standardized protocol server.

Directly spawning child CLI commands for every interaction is slow, drops state, and lacks real-time bidirectional streaming.

`m31a-app-server` is the daemon layer of M31A:
- Implements the complete **JSON-RPC 2.0 protocol** with over **40 standardized methods** and real-time Server-Sent Events (SSE) notification streams.
- Provides multi-transport IPC:
  - **`stdio`**: Default transport for VS Code / Cursor extensions launching `m31a app-server --stdio` as a subprocess.
  - **Hardened Unix Domain Sockets (`.sock`)**: High-speed local IPC (`~/.m31a/app-server-control/app-server-control.sock`).
  - **HTTP/SSE Server**: For web browsers and remote cloud workers (`m31a serve`).
- Enforces local-first resource validation (`resource_validator.rs`) preventing path traversal and unauthorized tool execution.
- Manages turn admission, request queuing, and active session multiplexing.

---

## Core Concepts & How It Works

### 1. Multi-Transport Connection Multiplexer (`connection_multiplexer.rs`)
Allows multiple clients (e.g., an IDE extension, a background watcher, and a Python SDK script) to connect simultaneously to the same running M31A daemon.

### 2. Comprehensive Protocol Coverage (`protocol.rs`)
Supports 40+ RPC methods and asynchronous notification events:
- **Thread Management**: `thread/create`, `thread/read`, `thread/list`, `thread/archive`, `thread/fork`, `thread/resume`.
- **Turn Execution & Streaming**: `turn/start`, `turn/interrupt`, `turn/steer`, with notification streams (`notification/turn_reasoning_chunk`, `notification/turn_content_chunk`, `notification/tool_*`).
- **Interactive Callbacks**: `turn/approve`, `turn/deny`, `turn/answer`.
- **Checkpoints & Rollbacks**: `checkpoint/create`, `checkpoint/restore`, `checkpoint/list`.
- **Concurrency & Pub-Sub**: `locks/acquire`, `locks/release`, `board/post`, `board/list`.
- **Autonomous Workflows**: `workflow/run`, `workflow/status`, `workflow/cancel`.

### 3. Local-First Resource Validator (`resource_validator.rs`)
Validates every incoming JSON-RPC payload against workspace boundary constraints, preventing clients from accessing files outside the permitted workspace root.

---

## Key Modules & Types

| Module | Key Structs / Traits | Purpose |
| :--- | :--- | :--- |
| [`src/protocol.rs`](src/protocol.rs) | `JsonRpcRequest`, `JsonRpcResponse`, `ProtocolHandler` | Complete JSON-RPC 2.0 method dispatcher and event streamer. |
| [`src/connection_multiplexer.rs`](src/connection_multiplexer.rs) | `ConnectionMultiplexer` | Multiplexes concurrent clients across UDS and stdio channels. |
| [`src/resource_validator.rs`](src/resource_validator.rs) | `ResourceValidator`, `TurnSafetyValidator` | Validates path parameters and enforces security bounds. |
| [`src/http_server.rs`](src/http_server.rs) | `HttpServer` | Axum-based HTTP/SSE server for web clients and remote agents. |
| [`src/turn_admission.rs`](src/turn_admission.rs) | `TurnAdmissionQueue` | Serializes and queues turn execution requests per session. |

---

## Running the App Server

```bash
# Launch server over standard I/O (default for IDE extensions)
m31a app-server --stdio

# Launch server over Unix Domain Socket
m31a app-server --uds ~/.m31a/app-server-control/app-server-control.sock

# Launch HTTP/SSE daemon server for remote clients
m31a serve --host 127.0.0.1 --port 8080
```

---

## Testing & Verification

```bash
cargo test -p m31a-app-server
```
