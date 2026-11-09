# `m31a-telemetry`

> Event-driven telemetry, token consumption tracking, latency metrics, and audit logging for M31A.

---

## Why Does This Crate Exist?

In high-throughput autonomous coding systems, observability is essential. When an agent executes a multi-turn task involving dozens of file edits, compiler runs, and tool calls, developers need clear answers to operational questions:
- How many input, output, and reasoning tokens were consumed?
- What was the roundtrip latency of each model invocation?
- Did any tool call fail or trip security circuit breakers?
- Are sensitive API keys or credentials accidentally leaking into audit logs?

`m31a-telemetry` provides a zero-overhead, asynchronous telemetry and audit logging pipeline. It captures structured session events without blocking the agent execution loop and supports multiple exporters (local JSONL files, OpenTelemetry, in-memory buffers).

---

## Core Concepts & How It Works

### 1. Asynchronous Event Pipeline
Events (`SessionStart`, `TurnStart`, `ToolInvocation`, `ModelInvocation`, `TurnEnd`, `SessionEnd`) are dispatched over non-blocking `tokio::sync::mpsc` channels to a background worker. The agent loop never stalls waiting for disk I/O or network export.

### 2. Privacy & Redaction First
All telemetry records pass through sanitization filters before export. File paths are anonymized where appropriate, arguments containing secrets are hashed, and payload tokens are scrubbed of credentials.

### 3. Exporter Abstraction
`TelemetryPipeline` can route events to multiple destinations simultaneously:
- **Local SQLite / JSONL**: For session replay and offline debugging.
- **OpenTelemetry / OTLP**: For enterprise monitoring, Prometheus, and Grafana dashboards.
- **In-Memory Ring Buffers**: Powering the TUI token analytics modal (`Ctrl+A`).

---

## Key Types & Structs

| Struct / Enum | Description |
| :--- | :--- |
| `TelemetryEvent` | Typed enum covering session lifecycles, turns, tool executions, and model calls. |
| `TelemetryPipeline` | Central dispatcher orchestrating event ingestion and background worker threads. |
| `TelemetryExporter` | Pluggable trait implemented by JSONL, OTLP, and memory buffer exporters. |
| `TurnMetrics` | Aggregated latency, token counts, and tool invocation statistics for a single turn. |

---

## Practical Usage Example

```rust
use m31a_telemetry::{TelemetryEvent, TelemetryPipeline};
use m31a_core::types::TokenUsage;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Initialize the telemetry pipeline
    let pipeline = Arc::new(TelemetryPipeline::new(1024));

    // 2. Record the start of an agent turn
    pipeline.record(TelemetryEvent::TurnStart {
        session_id: "thread_01".into(),
        turn_id: "turn_42".into(),
        tools_available: 18,
        timestamp: 1710000000,
    }).await?;

    // 3. Record tool execution latency
    pipeline.record(TelemetryEvent::ToolInvocation {
        session_id: "thread_01".into(),
        turn_id: "turn_42".into(),
        tool_name: "apply_patch".into(),
        tool_category: "filesystem".into(),
        arguments_hash: "sha256_e3b0c44...".into(),
        success: true,
        duration_ms: 24,
        error: None,
        timestamp: 1710000001,
    }).await?;

    // 4. Record token consumption at turn completion
    pipeline.record(TelemetryEvent::TurnEnd {
        session_id: "thread_01".into(),
        turn_id: "turn_42".into(),
        success: true,
        tokens_used: TokenUsage {
            prompt_tokens: 1200,
            completion_tokens: 350,
            total_tokens: 1550,
        },
        duration_ms: 1840,
        timestamp: 1710000002,
    }).await?;

    Ok(())
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-telemetry
```
