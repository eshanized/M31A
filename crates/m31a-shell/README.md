# `m31a-shell`

> Persistent pseudo-terminal (PTY) shell sessions, environment inheritance, and head/tail output buffer management for M31A.

---

## Why Does This Crate Exist?

In standard command execution architectures, every tool call runs in an isolated, ephemeral child subprocess. If an agent runs:
1. Turn 1: `cd crates/m31a-core`
2. Turn 2: `cargo check`

In an ephemeral subprocess model, Turn 2 executes back in the project root because the working directory change from Turn 1 evaporated when the subprocess exited. The same issue plagues environment variables (`export KEY=val`) and virtual environments (`source .venv/bin/activate`).

`m31a-shell` solves this by providing stateful **Persistent Shell Sessions**:
- Maintains an active background PTY (pseudo-terminal) process.
- Working directory shifts and environment variable exports persist seamlessly across multiple turns.
- Uses `HeadTailBuffer` to capture output without blowing memory when commands generate megabytes of verbose compiler logs.

---

## Core Concepts & How It Works

### 1. Persistent PTY Execution (`pty.rs`, `session.rs`)
`PersistentShellSession` maintains a master/slave pseudo-terminal pair. Shell commands are written to the PTY stdin, and stdout/stderr streams are multiplexed and captured with terminal control code normalization.

### 2. Head/Tail Ring Buffering (`buffer.rs`)
When a command emits 50,000 lines of output (e.g., a massive build log), feeding the entire stream to the LLM would overwhelm context limits.  
`HeadTailBuffer` retains:
- The first $N$ lines (header context).
- A truncation indicator: `[... 48,200 lines truncated ...]`.
- The final $M$ lines (where the actual compiler error or panic trace occurred).

---

## Key Modules & Types

| Module | Key Structs / Enums | Purpose |
| :--- | :--- | :--- |
| [`src/session.rs`](src/session.rs) | `PersistentShellSession`, `ShellOutput` | Coordinates stateful shell sessions across conversation turns. |
| [`src/pty.rs`](src/pty.rs) | `PersistentPtyProcess` | Manages OS pseudo-terminal descriptors, child forks, and signaling. |
| [`src/buffer.rs`](src/buffer.rs) | `HeadTailBuffer` | Ring buffer retaining initial and trailing output segments. |

---

## Practical Usage Example

```rust
use m31a_shell::session::PersistentShellSession;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut shell = PersistentShellSession::new("session_01", PathBuf::from("."));

    // 1. Run a command that changes directory and sets an environment variable
    let output1 = shell.execute("cd crates && export AGENT_DEBUG=1").await?;
    println!("Exit code: {}, Active CWD: {:?}", output1.exit_code, shell.cwd);

    // 2. Next command inherits the updated CWD and ENV!
    let output2 = shell.execute("pwd && echo $AGENT_DEBUG").await?;
    println!("Output from turn 2:\n{}", output2.stdout);

    Ok(())
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-shell
```
