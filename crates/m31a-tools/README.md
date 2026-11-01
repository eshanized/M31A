# `m31a-tools`

18-tool orchestration suite, execution dispatchers, and external integrations for **M31A**.

---

## Overview

`m31a-tools` provides the core tool definitions and execution dispatchers available to models. It houses built-in tools for filesystem operations, shell command executions, patch applications, multi-agent spawning, and mission goal tracking, as well as bridges to external Model Context Protocol (MCP) tools.

---

## User-Facing Capabilities

- **18-Tool Orchestration Suite**:
  - **Execution**: `exec_command`, `write_stdin`
  - **Filesystem**: `read_file`, `write_file`, `apply_patch`, `list_dir`, `file_search`, `view_image`
  - **Interactive & Search**: `request_user_input`, `tool_search`
  - **Multi-Agent Orchestration**: `spawn_agent`, `send_input`, `wait_agent`, `close_agent`, `list_agents`
  - **Goal Tracking**: `create_goal`, `update_goal`, `get_goal`
- **Transparent Execution**: Every tool dispatch is displayed in the TUI with parameters, live output cards, and execution durations.

---

## Developer Guide & Architecture

### Key Modules

- [`harness`](src/harness.rs): `ToolRegistry` managing tool schemas, validation, and asynchronous execution dispatch.
- [`patch`](src/patch.rs): Bridges model tool calls directly to `m31a-patch` engine.
- [`parallel_dispatcher`](src/parallel_dispatcher.rs): Enables concurrent execution of independent read-only tools.
- [`output_schema`](src/output_schema.rs): Standardized JSON schemas emitted to models for function calling.

---

## Testing

```bash
cargo test -p m31a-tools
```
