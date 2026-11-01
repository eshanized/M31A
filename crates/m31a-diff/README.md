# `m31a-diff`

Syntactic and intra-line diff engine, theme-aware rendering, and scrollable diff viewer for **M31A**.

---

## Overview

`m31a-diff` calculates unified, side-by-side, and intra-word differences across text buffers. It powers the interactive Git diff viewer in the TUI (`Ctrl+D`), diff output formatting in tool results, and turn-by-turn change tracking.

---

## User-Facing Capabilities

- **Interactive Git Diff Viewer (`Ctrl+D` or `/diff`)**: Full-screen scrollable diff modal displaying working tree changes with colorized additions and deletions.
- **Intra-Line Word Highlighting**: Pinpoints exact word and token differences within modified lines rather than marking entire lines as changed.
- **Diff Minimizer & Churn Analysis**: Identifies high-churn files to assess code risk.

---

## Developer Guide & Architecture

### Key Modules

- [`engine`](src/engine.rs): `DiffEngine` generating `DiffLine`, `DiffLineKind` (Insert, Delete, Context), `DiffStats`, and `WordSpan` intra-line highlights using `similar`.
- [`render`](src/render.rs): `DiffRenderer` applying terminal ANSI colors and Ratatui styles.
- [`view`](src/view.rs): `DiffViewerState` managing keyboard scrolling, fold states, and file selection in the TUI modal.
- [`turn_tracker`](src/turn_tracker.rs): `TurnDiffTracker` recording all mutations (`FileMutationKind`) made during an active turn.

---

## Testing

```bash
cargo test -p m31a-diff
```
