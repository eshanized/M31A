# `m31a-compaction`

Session context compaction, transcript degradation, and headroom monitoring for **M31A**.

---

## Overview

`m31a-compaction` prevents long-running autonomous sessions from exhausting LLM context windows. It monitors available token headroom, applies progressive transcript degradation (pruning verbose tool outputs and intermediate diffs), and generates structured conversation summaries.

---

## User-Facing Capabilities

- **Automatic Context Compaction (`/compact` or automatic trigger)**: Keeps multi-hour coding sessions responsive and within model context budgets without losing critical design decisions or file modifications.
- **Progressive Degradation**: Gracefully summarizes older turns while preserving recent turns and active tool states with byte-for-byte fidelity.

---

## Developer Guide & Architecture

### Key Modules

- [`monitor`](src/monitor.rs): `ContextHeadroomMonitor` computing `HeadroomSnapshot` and evaluating threshold alerts (`WARNING_HEADROOM_PCT`, `CRITICAL_HEADROOM_PCT`, `EXHAUSTED_HEADROOM_PCT`).
- [`degradation`](src/degradation.rs): `TranscriptDegradationLevel` and `degrade_transcript()` applying tiered reductions:
  1. Truncate long tool execution logs (preserving headers and error traces).
  2. Replace intermediate patch diffs with structured summaries (`STRUCTURED_SUMMARY_TEMPLATE`).
  3. Merge older multi-turn dialogs into consolidated checkpoint milestones.

---

## Testing

```bash
cargo test -p m31a-compaction
```
