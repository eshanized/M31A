# `m31a-analytics`

> Real-time session token accounting, time-to-first-token (TTFT) latency tracking, NVIDIA NIM cost estimation, and analytics dashboard for M31A.

---

## Why Does This Crate Exist?

When using cloud frontier models across long engineering sessions, developers need complete transparency into resource utilization:
- How many input tokens, completion tokens, and DeepSeek R1 reasoning tokens were consumed?
- What is the estimated dollar cost of this session?
- What was the time-to-first-token (TTFT) and total generation latency?
- Which model accounted for the bulk of token consumption?

`m31a-analytics` provides lightweight, accurate cost and performance accounting:
- Embeds up-to-date pricing tables for NVIDIA NIM models (Nemotron, DeepSeek R1, Codestral, Llama 3.1 70B/405B).
- Records `TurnMetrics` on every conversation turn.
- Renders the interactive Analytics Dashboard modal in the TUI (`Ctrl+A`) complete with token sparklines and cost breakdowns.

---

## Core Concepts & How It Works

### 1. Dedicated NIM Pricing Table (`nim_model_pricing`)
Calculates accurate dollar costs per million input and output tokens:
- **`Nemotron 3.5`**: $0.70 / 1M prompt tokens, $2.00 / 1M completion tokens.
- **`DeepSeek R1 / v4.1`**: $0.55 / 1M prompt tokens, $2.19 / 1M completion tokens.
- **`Codestral 22B`**: $0.30 / 1M prompt tokens, $0.90 / 1M completion tokens.
- **`Llama 3.1 70B`**: $0.80 / 1M prompt tokens, $2.40 / 1M completion tokens.

### 2. Time-To-First-Token (TTFT) & Latency Profiling
Measures milliseconds elapsed from HTTP dispatch until the first SSE chunk arrives (`ttft_ms`), isolating network latency from model generation time.

### 3. Sparkline Charts & Dashboard (`charts.rs`, `dashboard.rs`)
Generates ASCII sparkline bars (` ▂▃▅▆▇`) visualising token consumption trends over time for the Ratatui TUI.

---

## Key Modules & Types

| Module | Key Structs / Functions | Purpose |
| :--- | :--- | :--- |
| [`src/metrics.rs`](src/metrics.rs) | `SessionTokenMetrics`, `TurnMetrics`, `nim_model_pricing` | Token accumulation, cost estimation, and latency tracking. |
| [`src/dashboard.rs`](src/dashboard.rs) | `AnalyticsDashboard` | Formats metrics into human-readable terminal dashboards. |
| [`src/charts.rs`](src/charts.rs) | `render_sparkline` | Generates Unicode sparkline charts for token usage history. |

---

## Practical Usage Example

```rust
use m31a_analytics::metrics::{SessionTokenMetrics, TurnMetrics};

fn main() {
    let mut session_metrics = SessionTokenMetrics::default();

    // Record turn completion metrics
    let turn = TurnMetrics {
        model: "nvidia/nemotron-3.5-lightning-30b-a3b".into(),
        prompt_tokens: 2400,
        completion_tokens: 650,
        reasoning_tokens: 310,
        ttft_ms: 180,
        duration_ms: 1250,
    };

    session_metrics.record_turn(turn);

    println!("Total Tokens: {}", session_metrics.total_tokens);
    println!("Reasoning Tokens: {}", session_metrics.reasoning_tokens);
    println!("Estimated Cost: ${:.6} USD", session_metrics.estimated_cost_usd);
    println!("Mean Turn Latency: {} ms", session_metrics.total_duration_ms / session_metrics.turn_count as u64);
}
```

---

## Testing & Verification

```bash
cargo test -p m31a-analytics
```
