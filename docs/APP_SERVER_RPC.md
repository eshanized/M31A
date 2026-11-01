# M31A App Server & JSON-RPC 2.0 Protocol

The **M31A App Server** (`m31a-app-server`) exposes an industrial-grade JSON-RPC 2.0 daemon allowing IDE extensions (VS Code, Cursor, JetBrains), custom CLI frontends, and automated orchestration scripts to control the M31A agent runtime.

---

## 1. Transports & Connectivity

The App Server supports three bidirectional transports:

1. **Child Process Stdio (`m31a app-server --stdio`)**:
   - Ideal for IDE extensions launching M31A as a managed subprocess.
   - Messages are single-line JSON strings delimited by newline (`\n`).
2. **Unix Domain Socket (`~/.m31a/app-server-control/app-server-control.sock`)**:
   - Ultra-low latency IPC for local applications and background daemons.
3. **HTTP with Server-Sent Events (SSE) (`m31a serve`)**:
   - POST `/rpc` for standard requests; GET `/events` for continuous streaming notifications.

---

## 2. Protocol Conventions

All requests adhere to standard JSON-RPC 2.0:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "thread/create",
  "params": {
    "title": "Refactoring Core Patch Engine"
  }
}
```

Successful responses:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "thread_id": "019483ab-41c3-7a20-b3e1-2384784918ef",
    "created_at": "2026-11-01T20:00:00Z"
  }
}
```

Error responses:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32602,
    "message": "Invalid params: thread_id does not exist",
    "data": null
  }
}
```

---

## 3. Server Management & Diagnostics

### `server/diagnostics`
Query runtime health, system version, active threads, and resource metrics.

**Request**:
```json
{ "jsonrpc": "2.0", "id": 1, "method": "server/diagnostics", "params": {} }
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "version": "0.1.13",
    "uptime_seconds": 3600,
    "active_threads": 2,
    "sandbox_backend": "bubblewrap",
    "database_path": "~/.m31a/m31a.db"
  }
}
```

### `models/list`
Query available NVIDIA NIM models discovered from `/v1/models`.

**Request**:
```json
{ "jsonrpc": "2.0", "id": 2, "method": "models/list", "params": {} }
```

**Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "models": [
      {
        "id": "nvidia/nemotron-3.5-lightning-30b-a3b",
        "owned_by": "nvidia",
        "supports_reasoning": true,
        "supports_tools": true
      },
      {
        "id": "deepseek-ai/deepseek-v4.1-flash",
        "owned_by": "deepseek-ai",
        "supports_reasoning": true,
        "supports_tools": true
      }
    ]
  }
}
```

---

## 4. Thread & Session Management

| Method | Parameters | Description |
| :--- | :--- | :--- |
| `thread/create` | `{ "title"?: string, "model"?: string }` | Create a new conversation thread. |
| `thread/read` | `{ "thread_id": string }` | Retrieve thread metadata and past turn items. |
| `thread/list` | `{ "limit"?: number, "include_archived"?: boolean }` | List recent conversation sessions. |
| `thread/fork` | `{ "thread_id": string, "title"?: string }` | Fork an existing session into a new branch. |
| `thread/archive` | `{ "thread_id": string }` | Archive a completed thread. |

---

## 5. Turn Execution & Streaming Notifications

Turns are initiated with `turn/start`. During turn execution, the server emits asynchronous JSON-RPC notifications (`id: null`) over the transport connection.

### `turn/start`
**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": 10,
  "method": "turn/start",
  "params": {
    "thread_id": "019483ab-41c3-7a20-b3e1-2384784918ef",
    "prompt": "Inspect the workspace and fix any failing tests in crates/m31a-patch",
    "mode": "build"
  }
}
```

### Stream Notifications Emitted by Server

#### Reasoning Token Stream (`notification/turn_reasoning_chunk`)
Delivers real-time thinking tokens (DeepSeek R1 / Nemotron thinking path):
```json
{
  "jsonrpc": "2.0",
  "method": "notification/turn_reasoning_chunk",
  "params": {
    "thread_id": "019483ab-41c3-7a20-b3e1-2384784918ef",
    "turn_id": "turn-42",
    "chunk": "Let's check the seek_sequence tests to inspect whitespace handling..."
  }
}
```

#### Assistant Text Stream (`notification/turn_content_chunk`)
Delivers user-facing Markdown text chunks:
```json
{
  "jsonrpc": "2.0",
  "method": "notification/turn_content_chunk",
  "params": {
    "thread_id": "019483ab-41c3-7a20-b3e1-2384784918ef",
    "turn_id": "turn-42",
    "chunk": "I have inspected the tests and identified a failing unicode normalization test."
  }
}
```

#### Tool Invocation Lifecycle (`notification/tool_started`, `notification/tool_finished`)
```json
{
  "jsonrpc": "2.0",
  "method": "notification/tool_started",
  "params": {
    "call_id": "call-1",
    "tool_name": "apply_patch",
    "arguments": { "path": "src/patch.rs" }
  }
}
```

---

## 6. Interactive Human-in-the-Loop Callbacks

When a model dispatches a mutating tool requiring authorization, the server emits `notification/approval_requested`:

```json
{
  "jsonrpc": "2.0",
  "method": "notification/approval_requested",
  "params": {
    "request_id": "req-99",
    "tool_name": "exec_command",
    "description": "Run shell command: cargo test -p m31a-patch"
  }
}
```

The client responds using `turn/approve` or `turn/deny`:

```json
{
  "jsonrpc": "2.0",
  "id": 11,
  "method": "turn/approve",
  "params": {
    "request_id": "req-99",
    "decision": "allow"
  }
}
```

---

## 7. Checkpoints, Locks & Message Board

- `checkpoint/create`: Capture workspace snapshot.
- `checkpoint/restore`: Rollback workspace and session state.
- `locks/acquire` / `locks/release`: Mutex on file paths across concurrent subagents.
- `board/post` / `board/list`: Distributed pub-sub communication across agents.
