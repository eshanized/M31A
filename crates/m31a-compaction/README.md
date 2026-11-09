# `m31a-compaction`

> Intelligent context window compaction, progressive transcript degradation, and headroom monitoring for long-running M31A sessions.

---

## Why Does This Crate Exist?

LLM context windows, while increasingly large, are finite. In long autonomous workflows—such as large-scale repository refactorings, multi-agent swarms, or continuous vibe coding loops—accumulated tool outputs, file contents, compiler diagnostics, and thinking traces eventually fill the context window.

When context limits are reached, naive systems either crash with API token errors or abruptly chop off conversation history, losing critical early architectural decisions.

`m31a-compaction` solves this gracefully:
- Continuously tracks token headroom (`ContextHeadroomMonitor`).
- Warns before limits are exceeded (`WARNING`, `CRITICAL`, `EXHAUSTED` thresholds).
- Progressively degrades voluminous past outputs (e.g., trimming large compiler logs or file reads) while preserving key insights and tool call results.
- Synthesizes structured semantic summaries to retain long-term memory without blowing token budgets.

---

## Core Concepts & How It Works

### 1. Headroom Monitoring (`ContextHeadroomMonitor`)
Tracks the ratio of active session tokens against the model's maximum context window:
- **`Normal`**: Safe operating buffer (> 20% remaining).
- **`Warning`** (`WARNING_HEADROOM_PCT = 0.20`): Session approaches capacity; suggests pruning.
- **`Critical`** (`CRITICAL_HEADROOM_PCT = 0.10`): Initiates automated progressive transcript degradation.
- **`Exhausted`** (`EXHAUSTED_HEADROOM_PCT = 0.05`): Emergency compaction synthesizing structured summaries.

### 2. Progressive Transcript Degradation (`degradation.rs`)
Rather than blindly deleting turns, `m31a-compaction` applies multi-stage pruning:
1. Trims verbose stdout/stderr outputs from old tool invocations to head/tail samples.
2. Folds intermediate DeepSeek R1 reasoning traces into concise conclusion summaries.
3. Preserves human instructions, key file paths, and current goal milestones.

### 3. Structured Summarization
When full compaction triggers, an out-of-band LLM call generates a structured summary preserving:
- Original objective and user constraints.
- Key files discovered, modified, or deleted.
- Active architectural decisions and pending next steps.

---

## Key Modules & Types

| Module | Key Structs / Functions | Purpose |
| :--- | :--- | :--- |
| [`src/monitor.rs`](src/monitor.rs) | `ContextHeadroomMonitor`, `HeadroomSnapshot`, `HeadroomSeverity` | Real-time monitoring of available token headroom. |
| [`src/degradation.rs`](src/degradation.rs) | `TranscriptDegradationLevel`, `degrade_transcript` | Progressive pruning of past tool outputs and verbose traces. |
| [`src/lib.rs`](src/lib.rs) | `Compactor`, `CompactionConfig` | Orchestrates LLM summarization and history replacement. |

---

## Practical Usage Example

```rust
use m31a_compaction::{CompactionConfig, Compactor, ContextHeadroomMonitor, HeadroomSeverity};
use m31a_core::types::ChatMessage;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Configure headroom monitor for an 8k context window
    let monitor = ContextHeadroomMonitor::new(8192);

    // 2. Check current usage
    let current_tokens = 7200;
    let snapshot = monitor.evaluate(current_tokens);

    match snapshot.severity {
        HeadroomSeverity::Normal => println!("Context headroom is healthy."),
        HeadroomSeverity::Warning => println!("Approaching context limits. Consider pruning."),
        HeadroomSeverity::Critical | HeadroomSeverity::Exhausted => {
            println!("Severe token exhaustion! Initiating automatic compaction...");
            
            let config = CompactionConfig {
                threshold_pct: 0.85,
                preserve_tool_calls: true,
                preserve_reasoning: false,
                ..Default::default()
            };

            // In an actual session, `compactor.compact(&mut messages)` would be invoked
        }
    }

    Ok(())
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-compaction
```
