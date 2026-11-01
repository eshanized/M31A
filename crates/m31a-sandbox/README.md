# `m31a-sandbox`

Multi-platform command sandboxing, Linux Bubblewrap isolation, read-only protection masks, and Landlock engine for **M31A**.

---

## Overview

`m31a-sandbox` isolates command execution and tool invocations from the host operating system. On Linux, it configures and launches **Bubblewrap** (`bwrap`) unsharing `PID`, `IPC`, `UTS`, and `Mount` namespaces, mounting root system paths strictly read-only, applying protection masks over sensitive subpaths (`.git`, `.m31a`, `.env`, `.ssh`), and optionally unsharing network namespaces for airgapped security.

---

## User-Facing Capabilities

- **Safe Tool Execution**: Autonomous agents can run shell commands, compilers, and test runners without risking accidental host file corruption or secret theft.
- **Direct Sandboxed Shell (`m31a sandbox <cmd>`)**:
  ```bash
  m31a sandbox ls -la
  m31a sandbox --policy read-only cargo test
  ```
- **Read-Only Protection Masks**: Even with write permissions enabled, `.git/`, `.m31a/`, and `.env` files are locked from modification.

---

## Developer Guide & Architecture

### Key Modules

- [`bwrap_builder`](src/bwrap_builder.rs): Constructs Bubblewrap arguments, bind mounts, and device nodes (`/dev`, `/proc`, `/tmp`).
- [`runner`](src/runner.rs): `SandboxRunner` executing sandboxed commands asynchronously with stdout/stderr streaming.
- [`landlock`](src/landlock.rs): Optional Linux Landlock LSM rules for kernel-level path restrictions.
- [`process_reaper`](src/process_reaper.rs): Ensures all spawned sandboxed child processes are cleanly reaped on exit.

---

## Testing

```bash
cargo test -p m31a-sandbox
```
