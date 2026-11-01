# `m31a-code-mode`

Programmatic multi-tool execution runtime, script pragmas, cell yields, and ambient TypeScript definitions for **M31A**.

---

## Overview

`m31a-code-mode` allows LLMs to write code that calls multiple tools programmatically within a sandboxed runtime instead of dispatching individual JSON tool calls one turn at a time. It provides type-safe ambient declarations (`tools.*`), execution budgets, and cooperative cell yields.

---

## User-Facing Capabilities

- **High-Velocity Multi-Tool Scripts**: Instead of 10 sequential turns to inspect 10 files, the agent writes a concise script executing in a single turn.
- **Script Pragmas (`// @exec: ...`)**: Configures maximum output tokens and yield timeouts dynamically.
- **Built-in Script Helpers**: `text(val)`, `store(key, val)`, `load(key)`, and `yield_control()`.

---

## Developer Guide & Architecture

### Key Modules

- [`orchestrator`](src/orchestrator.rs): `CodeModeOrchestrator` managing cell lifecycle, script pragmas, and execution dispatch.
- [`cell_actor`](src/cell_actor.rs): `CellActor` tracking cell state (`CellLifecycleState`, `CellExecutionBudget`, `CellYieldReason`).
- [`ambient_dts`](src/ambient_dts.rs): Generates ambient TypeScript definitions (`declare namespace tools { ... }`) from dynamic tool schemas so the model gets compile-time type completion.
- [`ipc_channel`](src/ipc_channel.rs): IPC framing and message exchange between host orchestrator and isolated runner process.

---

## Testing

```bash
cargo test -p m31a-code-mode
```
