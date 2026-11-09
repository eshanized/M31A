# `m31a-sandbox`

> Linux Bubblewrap (`bwrap`) containerization, kernel namespace isolation, sensitive credential masking, and Landlock/Seccomp security barriers for M31A.

---

## Why Does This Crate Exist?

Giving an autonomous AI agent the ability to execute arbitrary shell commands (`exec_command`) and apply file patches (`apply_patch`) without sandbox isolation is an unacceptable security risk. An untrusted prompt injection or an errant hallucination could wipe user data (`rm -rf ~`), access private SSH keys (`~/.ssh/id_rsa`), read shell environment credentials (`.env`), or compromise Git hooks (`.git/hooks/pre-commit`).

`m31a-sandbox` establishes an unprivileged Linux container boundary using **Bubblewrap (`bwrap`)**:
- Uses Linux kernel namespaces (PID, IPC, UTS, Mount, and Network) without requiring root/sudo privileges.
- Mounts the host operating system strictly read-only (`/usr`, `/lib`, `/bin`, `/etc`).
- Binds only the active project workspace as writable.
- Actively masks sensitive credential directories (`.git/hooks`, `.ssh`, `.gnupg`, `.aws`, `.m31a`, `.codex`) with empty tmpfs overlays so the agent cannot read or alter them.

---

## Core Concepts & How It Works

### 1. Filesystem Layering & Mount Architecture (`bwrap_builder.rs`)
When an agent command executes, `BwrapBuilder` constructs a temporary filesystem view:
- **Read-Only System Roots**: `/bin`, `/sbin`, `/usr`, `/etc`, `/lib`, `/lib64`, `/nix/store`.
- **Writable Workspace Roots**: Only the current working directory is bound writable.
- **Protected Metadata Masks**: Files matching `.ssh`, `.aws`, `.m31a`, `.codex`, `.env`, `*.pem`, and `*.key` are shielded with empty read-only tmpfs mounts.
- **WSL Interop Shielding**: Masks `/run/WSL` and `/mnt/wslg` on Windows Subsystem for Linux to prevent Windows host process spawning.

### 2. Namespace Isolation
- **PID Namespace (`--unshare-pid`)**: Isolates process IDs. The agent process cannot see or signal host processes.
- **IPC Namespace (`--unshare-ipc`)**: Blocks shared memory segments and semaphores.
- **UTS Namespace (`--unshare-uts`)**: Prevents hostname alterations.
- **Network Namespace (`--unshare-net`)**: Optional network airgapping for offline tasks.

### 3. Process Supervision & Reaping (`process_manager.rs`, `process_reaper.rs`)
Ensures no orphan background processes linger after a turn cancels or times out.

---

## Key Modules & Types

| Module | Key Structs / Enums | Purpose |
| :--- | :--- | :--- |
| [`src/bwrap_builder.rs`](src/bwrap_builder.rs) | `BwrapBuilder`, `BwrapNetworkMode`, `SyntheticMountTarget` | Generates CLI argument vectors and mount tables for `bwrap`. |
| [`src/runner.rs`](src/runner.rs) | `SandboxRunner`, `ExecutionOutput` | Spawns sandboxed processes and captures stdout, stderr, and exit codes. |
| [`src/process_manager.rs`](src/process_manager.rs) | `SandboxedProcessManager` | Tracks active child PIDs and enforces execution timeouts. |
| [`src/process_reaper.rs`](src/process_reaper.rs) | `ProcessReaper` | Kills process trees cleanly on cancellation or timeout. |
| [`src/seccomp_filter.rs`](src/seccomp_filter.rs) | `SeccompFilter` | BPF syscall filter definitions restricting dangerous kernel operations. |

---

## Practical Usage Example

```rust
use m31a_sandbox::bwrap_builder::{BwrapBuilder, BwrapNetworkMode};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let workspace = PathBuf::from("/home/developer/project");

    let mut builder = BwrapBuilder::new(&workspace);
    builder
        .set_network_mode(BwrapNetworkMode::Isolated)
        .add_readonly_bind(PathBuf::from("/usr"))
        .add_writable_bind(workspace.clone())
        .mask_sensitive_metadata();

    let args = builder.build_args(&["cargo", "test"])?;
    println!("Generated bwrap command arguments: {:?}", args);

    Ok(())
}
```

---

## Direct CLI Usage

You can run any shell command inside the M31A Bubblewrap sandbox directly:

```bash
# Run command inside sandboxed workspace
m31a sandbox ls -la

# Run in strictly read-only mode
m31a sandbox --policy read-only cargo test
```

---

## Testing & Verification

```bash
cargo test -p m31a-sandbox
```
