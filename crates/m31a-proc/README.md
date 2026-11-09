# `m31a-proc`

> Background process supervisor, dev server manager, circular log ring buffering, and process lifecycle coordinator for M31A.

---

## Why Does This Crate Exist?

When developing web or networked applications, agents often need to start background services:
- Spawning a Vite or Next.js development server (`npm run dev`).
- Launching an API server or mock database daemon in the background.
- Running continuous test watchers (`cargo watch`).

If an agent runs `npm run dev` with standard command tools, the command never finishes, hanging the entire agent loop indefinitely.

`m31a-proc` provides a full-featured **Background Process Supervisor**:
- Launches long-running processes asynchronously with assigned IDs (`proc_id`).
- Emits real-time stdout/stderr into non-blocking `CircularOutputBuffer` ring buffers.
- Allows agents and users to tail logs (`m31a logs <id> --follow`) and check status (`m31a ps`).
- Enforces graceful termination (SIGTERM $\to$ grace period $\to$ SIGKILL) so ports don't remain bound when a session ends.

---

## Core Concepts & How It Works

### 1. Process Lifecycle Management (`manager.rs`, `process.rs`)
Tracks processes through typed states: `Starting` $\to$ `Running` $\to$ `Stopping` $\to$ `Stopped(ExitStatus)` / `Failed`.

### 2. Circular Log Ring Buffers (`buffer.rs`)
Captures up to thousands of lines per process in a high-speed lock-free circular buffer. Agents can query recent logs without writing temporary log files to disk.

### 3. Graceful Termination & Port Freeing
When stopping a process (via `m31a kill <id>` or session shutdown), `ProcessManager` sends `SIGTERM`, waits for a configurable grace period (`termination_grace_period_ms = 2000`), and falls back to `SIGKILL` only if the process refuses to exit.

---

## Key Modules & Types

| Module | Key Structs / Enums | Purpose |
| :--- | :--- | :--- |
| [`src/manager.rs`](src/manager.rs) | `ProcessManager`, `ProcessManagerConfig` | Central supervisor spawning, tracking, and stopping background processes. |
| [`src/process.rs`](src/process.rs) | `ManagedProcess`, `ProcessInfo`, `ProcessStatus` | Thread-safe handle to a single background process instance. |
| [`src/buffer.rs`](src/buffer.rs) | `CircularOutputBuffer`, `ProcessLine` | High-performance in-memory ring buffer capturing process logs. |

---

## Practical Usage Example

```rust
use m31a_proc::manager::{ProcessManager, ProcessManagerConfig};
use std::path::Path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let manager = ProcessManager::new(ProcessManagerConfig::default());

    // 1. Spawn a background development server
    let proc_info = manager.spawn(
        "vite-dev-server",
        "npx",
        &["vite", "--port", "3000"],
        Path::new("./frontend"),
        None,
    ).await?;

    println!("Started background server: ID={}, PID={:?}", proc_info.id, proc_info.pid);

    // 2. Read recent output lines
    let logs = manager.get_output(&proc_info.id, 10).await?;
    for line in logs {
        println!("[{}] {}", line.stream, line.content);
    }

    // 3. Gracefully terminate process
    manager.stop(&proc_info.id).await?;
    println!("Server terminated cleanly.");

    Ok(())
}
```

---

## CLI Integration

```bash
# List all active background processes and dev servers
m31a ps

# Follow logs from a background process
m31a logs <proc_id> --follow

# Terminate a background process
m31a kill <proc_id>
```

---

## Testing & Verification

```bash
cargo test -p m31a-proc
```
