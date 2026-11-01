# `m31a-tui`

Full-screen Terminal User Interface (TUI) built with Ratatui and Crossterm for **M31A**.

---

## Overview

`m31a-tui` delivers the primary interactive developer interface of M31A. It features a virtualized streaming conversation transcript, syntax-highlighted code diffs, collapsible DeepSeek R1 / Nemotron reasoning thought blocks, modal popups, fuzzy model selectors (`Ctrl+M`), fuzzy command palettes (`Ctrl+P`), Git diff viewers (`Ctrl+D`), and mouse navigation.

---

## User-Facing Capabilities

- **Interactive Full-Screen TUI (`m31a`)**: The main terminal window where prompts are entered, model responses stream live, and tools execute.
- **Model Selector Modal (`Ctrl+M`)**: Switch between frontier NIM models dynamically on the fly.
- **Git Diff Viewer (`Ctrl+D`)**: View working tree changes side-by-side or unified with colorized additions and deletions.
- **Session History Picker (`Ctrl+R`)**: Browse, search, and resume past SQLite sessions.
- **Collapsible Reasoning (`Ctrl+T`)**: Toggle thinking traces and tool output cards.
- **Instant Shell Execution (`!cmd`)**: Execute local shell commands directly from the composer prompt.

---

## Developer Guide & Architecture

### Key Modules

- [`app`](src/app.rs): `TuiApp` state machine managing active views, modals, focused panes, and terminal teardown RAII guards.
- [`transcript_view`](src/transcript_view.rs): Virtualized scrolling transcript renderer with markdown syntax formatting.
- [`model_selector`](src/model_selector.rs): Interactive model switcher popup querying NVIDIA NIM catalogs.
- [`modals`](src/modals.rs): Modal dialog managers (tool approval dialogs, settings manager, help overlays).
- [`renderer`](src/renderer.rs): Ratatui rendering pipelines and frame layout partitions.
- [`keybindings`](src/keybindings.rs): Centralized keyboard dispatch table.

---

## Testing

```bash
cargo test -p m31a-tui
```
