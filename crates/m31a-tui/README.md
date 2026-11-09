# `m31a-tui`

> Full-screen interactive Terminal User Interface (TUI) powered by Ratatui and Crossterm, featuring live streaming transcripts, collapsible reasoning traces, diff viewers, and modal palettes for M31A.

---

## Why Does This Crate Exist?

Command-line AI tools are often reduced to simple linear print loops. When an agent outputs large multi-file diffs, verbose tool logs, and deep reasoning traces, standard terminal output scrolls past uncontrollably, clobbering context and making it impossible to review changes before they commit.

`m31a-tui` turns the terminal into an interactive, IDE-grade developer cockpit:
- Built with **Ratatui** and **Crossterm** on an alternate screen buffer (`\x1b[?1049h`), leaving your regular shell history intact.
- Features a **virtualized scrolling transcript** rendering real-time streaming Markdown, collapsible DeepSeek R1 reasoning paths (`Ctrl+T`), and live tool execution cards.
- Provides interactive modal dialogs: **Universal Fuzzy Palette (`Ctrl+P`)**, **Model Selector (`Ctrl+M`)**, **Colorized Git Diff Viewer (`Ctrl+D`)**, **Session Picker (`Ctrl+R`)**, and **Token Analytics Dashboard (`Ctrl+A`)**.
- Supports dual agent modes: **`[⚡ BUILD]`** (autonomous code generation) and **`[📋 PLAN]`** (advisory architecture), toggled with `Tab`.
- Instant local shell execution via `!` prefix (e.g. `!cargo check`).

---

## Core Concepts & How It Works

### 1. The 4-Pane Terminal Layout
The screen is structured into 4 primary functional areas:
1. **Header Bar**: Active version, session thread ID, active NVIDIA NIM model, and health indicators.
2. **Transcript Stream Area**: Virtualized scrolling viewport with Markdown syntax rendering and collapsible reasoning blocks.
3. **Status Line**: Active mode (`[⚡ BUILD]` vs `[📋 PLAN]`), sandbox isolation level, approval policy, and token context meter.
4. **Multiline Composer**: Syntax-highlighted input area with slash-command autocompletion and prompt history.

### 2. Dual-Channel Transcript Rendering (`transcript_view.rs`)
As SSE chunks stream from [`m31a-provider`](../m31a-provider), `ChatWidgetState` separates thinking chunks from assistant text:
- Thinking tokens render inside a dimmed, styled reasoning box that can be toggled closed with `Ctrl+T`.
- Assistant code blocks are highlighted with syntax themes.
- Tool invocations render as interactive cards showing status indicators (⏳ Running $\to$ ✅ Success / ❌ Failed).

### 3. Modal Architecture (`modals.rs`, `model_selector.rs`)
Overlays are rendered conditionally without clearing transcript state:
- `ModelSelector`: Live search querying `/v1/models`.
- `ResumePicker`: Browses past conversation sessions stored in SQLite.
- `ApprovalDialog`: Prompts for human confirmation before mutating files.

---

## Key Modules & Types

| Module | Key Structs / Traits | Purpose |
| :--- | :--- | :--- |
| [`src/app.rs`](src/app.rs) | `App`, `FocusState` | Top-level state machine handling event loops, ticks, and modal transitions. |
| [`src/renderer.rs`](src/renderer.rs) | `Renderer` | Ratatui rendering pipeline drawing panels, status lines, and borders. |
| [`src/transcript_view.rs`](src/transcript_view.rs) | `TranscriptView`, `ChatWidgetState` | Virtualized scrolling viewport for conversation turns and tool cards. |
| [`src/bottom_pane.rs`](src/bottom_pane.rs) | `BottomPaneState`, `BottomPaneMode` | Multiline composer, slash-command popup, and token meter. |
| [`src/model_selector.rs`](src/model_selector.rs) | `ModelSelectorModal` | Interactive modal dialog for switching NVIDIA NIM models on the fly. |
| [`src/diff_render.rs`](src/diff_render.rs) | `DiffRenderer` | Syntactic side-by-side or unified colorized diff viewer. |
| [`src/icons.rs`](src/icons.rs) | `Icon`, `IconMode` | Centralized icon glyph engine (Lucide, Unicode, and ASCII modes). |

---

## Practical Usage Example

Launching the TUI programmatically:

```rust
use m31a_tui::app::App;
use m31a_core::config::Config;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load_with_overrides(None, &[])?;
    let workspace = PathBuf::from(".");

    // Initialize TUI application state
    let mut app = App::new(config, workspace)?;

    // Run the terminal event loop (takes raw terminal control)
    app.run().await?;

    Ok(())
}
```

---

## Keyboard Shortcuts Quick Reference

- `Ctrl + P`: Fuzzy Command Palette
- `Tab` / `Shift+Tab`: Cycle Agent Mode (`[⚡ BUILD]` $\leftrightarrow$ `[📋 PLAN]`)
- `!` *(prefix)*: Instant local shell execution (e.g. `!git status`)
- `Ctrl + M`: NVIDIA NIM Model Selector modal
- `Ctrl + D`: Interactive Git diff viewer
- `Ctrl + R`: Session resume picker
- `Ctrl + T`: Fold / unfold DeepSeek R1 reasoning traces
- `Ctrl + Y`: Yank latest assistant code block to clipboard
- `Ctrl + A`: Token consumption and cost analytics dashboard
- `Ctrl + C`: Cancel active generation / double-press to exit

---

## Testing & Verification

```bash
cargo test -p m31a-tui
```
