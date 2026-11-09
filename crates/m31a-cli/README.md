# `m31a-cli`

> The unified command-line entrypoint, Clap argument parser, 54 subcommands, and diagnostic healthcheck engine compiling into the single `m31a` binary.

---

## Why Does This Crate Exist?

In a modular monorepo containing 39 crates, you need a single, rock-solid entrypoint that stitches the entire system together into a single distributable binary.

`m31a-cli` is that binary:
- Compiles into the standalone `m31a` executable with zero external runtime dependencies.
- Parses command-line arguments using `clap` (derive macro + environment variable fallbacks).
- Initializes asynchronous Tokio runtimes, structured tracing subscribers (`tracing-subscriber`), and Unix signal handlers (`SIGINT`, `SIGTERM`).
- Launches the full-screen Ratatui terminal UI when run interactively.
- Exposes **54 specialized subcommands** for non-interactive scripting, CI/CD pipelines, autonomous workflows, and process management.

---

## Core Concepts & How It Works

### 1. Unified Dispatch (`src/main.rs`)
When `m31a` is invoked:
- If called with no subcommand (or with an optional prompt argument `m31a "prompt"`), it boots the interactive Ratatui TUI ([`m31a-tui`](../m31a-tui)).
- If called with a subcommand (`vibe`, `speculate`, `workflow`, `exec`, `review`, etc.), it bypasses the terminal UI and routes directly to the appropriate subsystem engine.

### 2. Comprehensive Subcommand Taxonomy (54 Subcommands)
Subcommands are organized into clear functional groups:
1. **Environment & Diagnostics**: `setup`, `doctor`, `login`, `logout`, `trust`, `feedback`, `config-schema`, `completion`.
2. **Autonomous Execution**: `vibe`, `speculate`, `workflow`, `gsd`, `exec`, `swarm`.
3. **Verification & Code Quality**: `verify`, `review`, `apply`, `apply-patch`.
4. **Sessions & Checkpoint Recovery**: `resume`, `fork`, `undo`, `revert`, `archive`, `unarchive`, `delete`, `history`, `export`, `sections`, `memories`.
5. **Daemons, Servers & Transports**: `app-server`, `daemon`, `serve`, `responses-proxy`, `stdio-to-uds`.
6. **Process Supervisor**: `ps`, `logs`, `kill`, `queue`.
7. **Extensibility & Configuration**: `mcp`, `features`, `board`, `plugin`, `sandbox`, `presets`, `worktree`, `template`, `telemetry`, `attribution`, `prompt`, `image`, `cloud`, `goal`, `agents`, `debug`, `migrate`.

### 3. Comprehensive Diagnostic Doctor (`src/doctor.rs`)
`m31a doctor` inspects your local environment, validating:
- NVIDIA NIM API key authorization and latency.
- Linux Bubblewrap (`bwrap`) binary path and unprivileged user namespace permissions.
- SQLite database schema migrations and read/write integrity.
- Git repository trust status and hook protections.

---

## Key Modules

| Module | Purpose |
| :--- | :--- |
| [`src/main.rs`](src/main.rs) | Top-level entrypoint, `Commands` enum, and CLI dispatch. |
| [`src/workflow.rs`](src/workflow.rs) | Subcommand handlers for `m31a workflow` (`run`, `status`, `inspect`, `resume`, `cancel`). |
| [`src/vibe.rs`](src/vibe.rs) | Subcommand handler for continuous background auto-healing (`m31a vibe`). |
| [`src/speculate.rs`](src/speculate.rs) | Subcommand handler for speculative parallel candidate execution (`m31a speculate`). |
| [`src/doctor.rs`](src/doctor.rs) | Diagnostic health checks and environment validation (`m31a doctor`). |
| [`src/setup.rs`](src/setup.rs) | Interactive onboarding setup wizard (`m31a setup`). |
| [`src/gsd.rs`](src/gsd.rs) | Spec-driven autonomous phase workflow runner (`m31a gsd`). |

---

## Practical Usage Examples

```bash
# 1. Launch interactive TUI
m31a

# 2. Continuous vibe coding loop with background auto-healing
m31a vibe "Build dark mode pricing cards" --visual-url http://localhost:3000

# 3. Speculative parallel reality arena
m31a speculate "Optimize database query serialization" \
  "Approach A: SIMD-JSON zerocopy" \
  "Approach B: FlatBuffers zero-allocation" \
  --promote

# 4. Non-interactive task execution with automated self-healing
m31a exec "Write comprehensive unit tests for crates/m31a-patch" --self-heal

# 5. Run automated code review against main branch
m31a review --base main
```

---

## Building and Installing

```bash
# Build release binary
cargo build --release -p m31a-cli

# Copy binary to local user path
cp target/release/m31a ~/.local/bin/

# Verify installation
m31a --version
```

---

## Testing & Verification

```bash
cargo test -p m31a-cli
```
