# `m31a-tools`

> The 18-tool orchestration suite, tool registry, parallel execution dispatcher, and MCP extension bridge for M31A.

---

## Why Does This Crate Exist?

An LLM on its own can only generate text. To write software, an agent needs hands: it must read files, edit code, execute compilers, inspect test results, search directories, and coordinate with sub-agents.

`m31a-tools` provides the standardized tool infrastructure for M31A:
- Implements the complete **18-tool orchestration suite** covering Execution, Filesystem, Interactive Inquiry, Multi-Agent Swarm, and Goal Tracking.
- Provides strict JSON Schema generation for every tool so models receive unambiguous parameter constraints.
- Coordinates parallel tool execution (`ParallelDispatcher`) for independent read operations.
- Integrates external Model Context Protocol (MCP) servers, dynamically registering external tools alongside native ones.
- Captures granular execution metrics, timings, and error traces.

---

## Core Concepts & How It Works

### 1. The 18 Built-in Tools Taxonomy
Tools are registered inside `ToolRegistry` and categorized by domain:

| Domain | Tool | Functionality |
| :--- | :--- | :--- |
| **Execution** | `exec_command` | Sandboxed shell command execution with timeout and output capture. |
| | `write_stdin` | Pipe input into active background process streams. |
| **Filesystem** | `read_file` | Read file contents with line offset and byte range limits. |
| | `write_file` | Atomic workspace file creation or overwrite. |
| | `apply_patch` | Multi-file Codex patch applicator with 4-tier fuzzy seek. |
| | `list_dir` | Directory traversal respecting `.gitignore`. |
| | `file_search` | High-speed regex and glob search across workspace filenames. |
| | `view_image` | Multimodal image inspection. |
| **Interactive** | `request_user_input` | Interactive preference elicitation and clarifying questions. |
| | `tool_search` | Dynamic discovery of registered native and MCP tools. |
| **Multi-Agent** | `spawn_agent` | Launch isolated sub-agent with dedicated role and memory. |
| | `send_input` | Dispatch instructions and payloads to a child agent. |
| | `wait_agent` | Await child agent completion and capture output artifacts. |
| | `close_agent` | Terminate and clean up an active child agent. |
| | `list_agents` | Enumerate active sub-agents and resource usage. |
| **Goal Tracking** | `create_goal` | Initialize high-level mission objective and milestones. |
| | `update_goal` | Record milestone progress and task transitions. |
| | `get_goal` | Retrieve current mission status and remaining items. |

### 2. Parallel Execution Dispatcher (`parallel_dispatcher.rs`)
Read-only tool calls (such as reading 5 distinct files or executing parallel searches) are classified into `ToolConcurrencyCategory::Concurrent` and executed concurrently over Tokio tasks, drastically reducing turn latency. Mutating calls are serialized safely.

### 3. Model Context Protocol (MCP) Bridge (`mcp/`)
Allows third-party MCP servers (stdio or HTTP) to plug directly into the tool registry.

---

## Key Modules & Types

| Module | Key Structs / Traits | Purpose |
| :--- | :--- | :--- |
| [`src/registry.rs`](src/registry.rs) | `ToolRegistry`, `ToolDefinition`, `ToolResult` | Central registry managing schemas and dispatching invocations. |
| [`src/parallel_dispatcher.rs`](src/parallel_dispatcher.rs) | `ParallelDispatcher`, `ToolConcurrencyCategory` | Concurrently dispatches non-conflicting read tool calls. |
| [`src/harness.rs`](src/harness.rs) | `ToolExecutionHarness` | Wraps tool execution with timeouts, tracing, and error formatting. |
| [`src/dispatch_trace.rs`](src/dispatch_trace.rs) | `ToolDispatchTracer`, `ToolDispatchRecord` | Records detailed telemetry on tool arguments, duration, and status. |

---

## Practical Usage Example

```rust
use m31a_tools::registry::ToolRegistry;
use serde_json::json;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let workspace = PathBuf::from(".");
    let registry = ToolRegistry::default_suite(&workspace)?;

    // 1. Inspect registered tools
    println!("Registered tools count: {}", registry.len());
    for def in registry.list_definitions() {
        println!("- {}: {}", def.name, def.description);
    }

    // 2. Execute a sandboxed tool call programmatically
    let result = registry.execute(
        "file_search",
        json!({
            "pattern": "Cargo.toml",
            "max_results": 5
        })
    ).await?;

    println!("Search result: {:?}", result.output);
    Ok(())
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-tools
```
