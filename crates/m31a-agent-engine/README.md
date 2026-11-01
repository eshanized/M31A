# `m31a-agent-engine`

Autonomous agent execution runtime, multi-turn turn loop, vibe coding engine, and speculative parallel reality arena for **M31A**.

---

## Overview

The `m31a-agent-engine` crate serves as the operational brain of M31A. It coordinates model inference, tool execution, safety reviews, and workspace recovery across multi-turn interactions. It implements the reactive turn loop, speculative multi-branch sandboxing, oscillation detection, continuous background vibe coding, and swarm orchestration.

---

## User-Facing Capabilities

- **Autonomous Vibe Coding Loop (`m31a vibe`)**: Continuously monitors workspace files, detects compilation errors, synthesizes fixes via NVIDIA NIM models, and verifies builds without breaking the developer's creative flow.
- **Speculative Parallel Reality Arena (`m31a speculate`)**: Evaluates multiple candidate solutions simultaneously in isolated temporary sandboxes, scores them against tests and benchmarks, and promotes the winning branch.
- **Multi-Turn Autonomous Execution (`m31a exec`)**: Executes complex tasks requiring multi-step tool dispatches, file inspections, and iterative verification.
- **Anti-Oscillation & Runaway Prevention**: Automatically detects if the agent is stuck in an edit-undo loop or repetitive failing patterns and breaks the cycle with targeted interventions.

---

## Developer Guide & Architecture

### Key Modules

- [`turn_loop`](src/turn_loop.rs): Core `AgentEngine` executing turns, emitting `EngineEvent` streams (reasoning chunks, content tokens, tool start/finish), and managing `PendingApproval` states.
- [`vibe_loop`](src/vibe_loop.rs): `VibeEngine` managing continuous background auto-healing loops, `VibeCheckpoint` captures, and timeline event histories.
- [`speculative`](src/speculative.rs): `SpeculativeArena` running isolated branches, computing composite quality scores (`CandidateEvaluation`), and generating `SpeculativeExecutionReport`.
- [`oscillation`](src/oscillation.rs): `OscillationDetector` tracking edit patterns to prevent infinite repair cycles (`OscillationAlert`, `OscillationKind`).
- [`stream_recovery`](src/stream_recovery.rs): Classifies streaming failures and initiates transparent session recovery.
- [`waves`](src/waves.rs): `WaveScheduler` decomposing complex plans into phased execution waves.
- [`agents`](src/agents.rs): `EngineAgentRuntime` orchestrating subagents and multi-agent roles.

### Programmatic Usage

```rust
use m31a_agent_engine::{AgentEngine, EngineEvent};
use m31a_core::config::Config;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Arc::new(Config::default());
    let engine = AgentEngine::new(config).await?;

    let mut event_stream = engine.run_turn("thread-123", "Fix failing tests in m31a-patch").await?;
    while let Some(event) = event_stream.recv().await {
        match event {
            EngineEvent::ReasoningChunk(chunk) => print!("{chunk}"),
            EngineEvent::ContentChunk(chunk) => print!("{chunk}"),
            EngineEvent::ToolFinished { name, success, .. } => {
                println!("\n[Tool: {name} | Success: {success}]");
            }
            EngineEvent::TurnCompleted { .. } => break,
            _ => {}
        }
    }
    Ok(())
}
```

---

## Testing

```bash
cargo test -p m31a-agent-engine
```
