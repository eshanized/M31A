# `m31a-telemetry`

Structured analytics and event-driven telemetry pipeline for **M31A**.

---

## Overview

`m31a-telemetry` collects anonymous session lifecycle metrics, model execution latencies, token consumption, and tool failure frequencies. It helps identify model latency bottlenecks and error clusters while respecting user privacy.

---

## User-Facing Capabilities

- **Session Statistics (`m31a telemetry`)**: View aggregated statistics on session durations, tool invocation frequency, and overall token consumption.
- **Privacy Controls**: All telemetry collection is strictly opt-in and scrubs sensitive paths and code contents.

---

## Developer Guide & Architecture

### Key Types

- `TelemetryEvent`: Strongly typed events (`SessionStart`, `SessionEnd`, `TurnExecuted`, `ToolInvoked`, `ErrorEncountered`).
- `TelemetrySink` trait: Supports file exporters, memory buffers, and custom monitoring sinks.

---

## Testing

```bash
cargo test -p m31a-telemetry
```
