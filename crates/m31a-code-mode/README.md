# `m31a-code-mode`

> Programmatic multi-tool execution runtime, script pragmas (`@exec`), cell yields, and ambient TypeScript definitions for M31A.

---

## Why Does This Crate Exist?

In standard AI tool-calling architectures, each tool call requires a full roundtrip network turn:
1. Model requests `read_file("file1.rs")` $\to$ Host executes $\to$ Sends response.
2. Model requests `read_file("file2.rs")` $\to$ Host executes $\to$ Sends response.
...
10. Model requests `read_file("file10.rs")` $\to$ Host executes $\to$ Sends response.

Reading 10 files takes 10 sequential turns, consuming hundreds of seconds and multiplying token costs.

`m31a-code-mode` revolutionizes this workflow by allowing LLMs to write programmatic scripts that call multiple tools within an isolated, stateful JavaScript/TypeScript runner:
- Instead of 10 roundtrips, the model writes a single script looping through all 10 files and aggregating results in one turn.
- Generates ambient TypeScript declarations (`tools.*`) dynamically from registered tool schemas so models receive strong type safety.
- Supports execution pragmas (`// @exec: ...`), token budgets, and cooperative cell yields (`yield_control()`).

---

## Core Concepts & How It Works

### 1. Script Pragmas (`// @exec: ...`)
Scripts can declare execution constraints at the very top of the script cell:
```typescript
// @exec: {"yield_time_ms": 10000, "max_output_tokens": 2000}
```
`CodeModeOrchestrator` parses these pragmas to establish timeout deadlines and token buffers before execution begins.

### 2. Ambient TypeScript Definitions (`ambient_dts.rs`)
At runtime, `m31a-code-mode` inspects the active `ToolRegistry` and dynamically compiles an ambient TypeScript declaration file (`declare namespace tools { ... }`). The model sees exact argument types, required fields, and return shapes in its prompt.

### 3. Built-in Script Helpers
Scripts have access to global runtime helpers:
- `text(val)`: Formats and prints structured output.
- `store(key, val)` & `load(key)`: Persists scratchpad data across script cells.
- `yield_control()`: Yields execution back to the host orchestrator cooperatively.

### 4. IPC Channel & Sandboxed Runner (`ipc_channel.rs`, `cell_actor.rs`)
The script executes inside an isolated child worker process communicating with the M31A host via binary length-prefixed IPC pipes. Even if the script contains an infinite loop, the host enforces timeouts and terminates the runner cleanly.

---

## Key Modules & Types

| Module | Key Structs / Traits | Purpose |
| :--- | :--- | :--- |
| [`src/orchestrator.rs`](src/orchestrator.rs) | `CodeModeOrchestrator` | Manages cell lifecycles, pragma parsing, and IPC worker dispatch. |
| [`src/cell_actor.rs`](src/cell_actor.rs) | `CellActor`, `CellExecutionBudget`, `CellLifecycleState` | Tracks memory buffers, deadlines, and execution states. |
| [`src/ambient_dts.rs`](src/ambient_dts.rs) | `AmbientDtsGenerator` | Compiles dynamic TypeScript `.d.ts` definitions from tool schemas. |
| [`src/ipc_channel.rs`](src/ipc_channel.rs) | `IpcChannel`, `IpcMessage` | Binary framed communication with isolated script runner. |
| [`src/runtime.rs`](src/runtime.rs) | `CodeModeRuntime` | High-level interface managing sessions and script execution. |

---

## Practical Usage Example

Here is an example of a Code Mode script written by an agent:

```typescript
// @exec: {"yield_time_ms": 5000}
async function auditConfigs() {
    const files = await tools.file_search({ pattern: "*.toml" });
    const summaries = [];

    for (const file of files.matches.slice(0, 5)) {
        const content = await tools.read_file({ path: file });
        summaries.push({ file, lines: content.lines.length });
    }

    text(JSON.stringify(summaries, null, 2));
}

await auditConfigs();
```

---

## Testing & Verification

```bash
cargo test -p m31a-code-mode
```
