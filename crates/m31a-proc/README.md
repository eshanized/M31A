# `m31a-proc`

Background process supervisor, asynchronous task monitor, and circular log ring buffers for **M31A**.

---

## Overview

`m31a-proc` manages long-running background processes (such as dev servers, compiler daemons, test watchers, and database containers) spawned during agent turns. It isolates process groups, streams stdout/stderr into circular memory ring buffers, and ensures clean termination on shutdown.

---

## User-Facing Capabilities

- **Process Inspection & Logs (`m31a ps`, `m31a logs <id>`)**:
  ```bash
  m31a ps
  m31a logs proc-1 --follow
  m31a kill proc-1
  ```
- **TUI Process Manager (`/procs`)**: View and terminate background processes directly within the terminal UI.
- **Leak-Free Process Teardown**: Automatically reaps orphaned worker processes and process trees when exiting.

---

## Developer Guide & Architecture

### Key Modules

- [`manager`](src/manager.rs): `ProcessManager` tracking active `ManagedProcess` instances with thread-safe atomic status maps.
- [`buffer`](src/buffer.rs): `CircularOutputBuffer` maintaining fixed-size ring buffers of stdout/stderr lines with memory bounds.
- [`process`](src/process.rs): `ProcessInfo`, `ProcessStatus` (Running, Stopped, Failed), and `ProcessStopResult`.

---

## Testing

```bash
cargo test -p m31a-proc
```
