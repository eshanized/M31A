# `m31a-cli`

Unified command-line application binary, argument parser, 54 subcommands, and diagnostic healthcheck entrypoint for **M31A**.

---

## Overview

`m31a-cli` is the root executable crate of M31A. It compiles into the single, dependency-complete binary `m31a`. It handles CLI arguments using `clap`, initializes system logging (`tracing-subscriber`), sets up signal handlers, loads user configuration and security profiles, launches the interactive Ratatui TUI, and executes 54 distinct subcommands.

---

## User-Facing Capabilities

- **Interactive TUI Entrypoint (`m31a`)**: Launches the terminal interface when invoked without subcommands.
- **54 Subcommands**: Exposes comprehensive commands for automated task execution (`exec`), vibe coding (`vibe`), speculative sandboxing (`speculate`), multi-agent swarms (`swarm`), code reviews (`review`), session management (`resume`, `fork`, `archive`), daemon operations (`app-server`, `daemon`), process monitoring (`ps`, `logs`, `kill`), and MCP configuration (`mcp`).
- **Runtime Doctor (`m31a doctor`)**: Runs diagnostic validation of local credentials, Bubblewrap sandbox availability, and endpoint health.
- **Shell Completions Generator (`m31a completion`)**: Outputs completion scripts for Bash, Zsh, and Fish.

---

## Developer Guide & Architecture

### Structure

- [`src/main.rs`](src/main.rs): Top-level `main()` dispatching subcommands and initializing the `AgentEngine`, `Storage`, `Provider`, and `TuiApp`.
- Exposes features such as `live-nvidia-tests` for running end-to-end integration tests against live NVIDIA NIM endpoints.

### Build and Install

```bash
cargo build --release -p m31a-cli
# Resulting binary: target/release/m31a
cp target/release/m31a ~/.local/bin/
```

---

## Testing

```bash
cargo test -p m31a-cli
```
