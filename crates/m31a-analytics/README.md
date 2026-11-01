# `m31a-analytics`

Interactive token accounting, latency telemetry, model invocation cost metrics, and terminal dashboard for **M31A**.

---

## Overview

`m31a-analytics` provides real-time token tracking, latency measurements, and financial cost modeling across interactive and non-interactive sessions. It renders compact sparklines and terminal charts directly into the TUI or outputs machine-readable summaries.

---

## User-Facing Capabilities

- **Interactive Analytics Modal (`Ctrl+A` or `/analytics`)**: Displays a real-time modal in the TUI breaking down prompt tokens, completion tokens, cached context savings, and estimated API expenses.
- **Sparklines & Visual Histograms**: Renders token consumption trajectories and latency trends using Unicode block characters directly in standard terminals.
- **Cost Estimation**: Automatically calculates pricing based on NVIDIA NIM model slugs.

---

## Developer Guide & Architecture

### Key Modules

- [`metrics`](src/metrics.rs): `SessionTokenMetrics` struct recording prompt tokens, completion tokens, latency durations, and turn costs.
- [`dashboard`](src/dashboard.rs): `AnalyticsDashboard` managing layout, formatting, and Ratatui widgets for terminal rendering.
- [`charts`](src/charts.rs): Terminal graphing primitives (`render_sparkline`, `render_bar`).

### Programmatic Usage

```rust
use m31a_analytics::{SessionTokenMetrics, AnalyticsDashboard};

let mut metrics = SessionTokenMetrics::new("nvidia/nemotron-3.5-lightning-30b-a3b");
metrics.record_turn(1250 /* prompt_tokens */, 420 /* completion_tokens */, 850 /* latency_ms */);

println!("Total Tokens: {}", metrics.total_tokens());
println!("Estimated Cost: ${:.4}", metrics.estimated_cost_usd());
```

---

## Testing

```bash
cargo test -p m31a-analytics
```
