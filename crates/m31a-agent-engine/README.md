# `m31a-agent-engine`

> The cognitive engine of M31A: multi-turn reactive agent loops, continuous vibe coding auto-healing, speculative parallel reality arenas, and DAG workflow wave schedulers.

---

## Why Does This Crate Exist?

An autonomous coding assistant is more than an API caller; it is a complex distributed state machine:
- It must orchestrate multi-turn conversations where tool executions produce intermediate outputs that feed into subsequent turns.
- It must support **continuous vibe coding** (`m31a vibe`), watching files non-intrusively and automatically healing compiler errors without breaking developer flow.
- It must support **speculative execution** (`m31a speculate`), competing rival architectural hypotheses concurrently in isolated sandbox branches and promoting the winning implementation.
- It must manage multi-phase engineering projects via **DAG workflows** (`m31a workflow`), partitioning tasks into deterministic execution waves with worker leases.
- It must detect **tool oscillation** when an agent gets stuck in loops modifying the same file back and forth.

`m31a-agent-engine` is the central brain tying all these capabilities together.

---

## Core Concepts & How It Works

### 1. Reactive Multi-Turn Loop (`turn_loop.rs`)
`AgentEngine` runs the primary turn lifecycle:
1. Ingests user prompts and attaches active workspace context, skills, and prompt fragments.
2. Streams completions from [`m31a-provider`](../m31a-provider), separating DeepSeek R1 reasoning traces from content.
3. Parses tool calls and evaluates them against [`m31a-execpolicy`](../m31a-execpolicy) and [`m31a-guardian`](../m31a-guardian).
4. Dispatches tool calls via parallel workers and feeds outputs back into the conversation history.
5. Monitors token headroom with [`m31a-compaction`](../m31a-compaction).

### 2. Continuous Vibe Coding Loop (`vibe_loop.rs`)
Powers `m31a vibe`:
- Subscribes to [`m31a-watcher`](../m31a-watcher) change batches.
- Runs quick background verification checks (`cargo check`, `npm run build`, `python -m py_compile`).
- If an error is detected, automatically prompts a low-latency NIM model (`bolt-e4b`) to synthesize an atomic multi-file Codex patch, repairing the working tree silently.

### 3. Speculative Parallel Reality Arena (`speculative.rs`)
Powers `m31a speculate`:
- Takes an objective and multiple candidate hypotheses.
- Creates isolated temporary Git worktrees / sandboxes for each candidate.
- Executes candidates concurrently.
- Computes a multi-criteria scoring matrix across build status, test passes, latency, and blast radius.
- Automatically merges the winning branch into the working tree (`--promote`).

### 4. Wave Scheduler & DAG Workflows (`workflow.rs`, `waves.rs`)
Partitions task graphs into independent execution waves: tasks in Wave $N$ run concurrently; Wave $N+1$ begins only after Wave $N$'s dependencies succeed.

### 5. Loop Oscillation Detector (`oscillation.rs`)
Detects when an agent repeatedly alternates between two states (e.g., repeatedly changing a type and then changing it back), breaking the loop and prompting for clarification.

---

## Key Modules & Types

| Module | Key Structs / Enums | Purpose |
| :--- | :--- | :--- |
| [`src/turn_loop.rs`](src/turn_loop.rs) | `AgentEngine`, `EngineEvent`, `PendingApproval` | Multi-turn reactive turn execution loop and event dispatcher. |
| [`src/vibe_loop.rs`](src/vibe_loop.rs) | `VibeEngine`, `VibeConfig`, `VibeEvent`, `VibeTimeline` | Continuous background auto-healing compiler loop. |
| [`src/speculative.rs`](src/speculative.rs) | `SpeculativeArena`, `CandidateEvaluation`, `SpeculativeExecutionReport` | Parallel candidate execution and scoring matrix in isolated sandboxes. |
| [`src/workflow.rs`](src/workflow.rs) | `WorkflowOrchestrator`, `WorkflowExecutionConfig` | Top-level DAG autonomous workflow coordinator. |
| [`src/waves.rs`](src/waves.rs) | `WaveScheduler`, `WorkflowWaveResult` | Schedules parallel execution waves based on task dependency fences. |
| [`src/oscillation.rs`](src/oscillation.rs) | `OscillationDetector`, `OscillationAlert` | Detects repetitive edit loops and thrashing behavior. |
| [`src/agents.rs`](src/agents.rs) | `EngineAgentRuntime` | Spawns, monitors, and communicates with sub-agent swarms. |

---

## Practical Usage Example

```rust
use m31a_agent_engine::turn_loop::{AgentEngine, EngineEvent};
use m31a_core::config::Config;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load_with_overrides(None, &[])?;
    let workspace = PathBuf::from(".");

    // 1. Initialize the agent engine
    let engine = AgentEngine::new(config, workspace).await?;

    // 2. Start a turn and listen to streaming engine events
    let mut rx = engine.start_turn("Add comprehensive unit tests for auth module").await?;

    while let Some(event) = rx.recv().await {
        match event {
            EngineEvent::ReasoningChunk(thought) => print!("[Thinking] {}", thought),
            EngineEvent::ContentChunk(text) => print!("{}", text),
            EngineEvent::ToolStarted { name, .. } => println!("\n[Tool] Running {}...", name),
            EngineEvent::ToolFinished { name, success, .. } => println!("[Tool] {} finished (success={})", name, success),
            EngineEvent::TurnFinished { .. } => {
                println!("\nTurn completed!");
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-agent-engine
```
