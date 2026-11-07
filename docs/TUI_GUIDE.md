# M31A Terminal User Interface (TUI) User Guide

The M31A Terminal User Interface provides a full-screen, high-performance terminal experience powered by [Ratatui](https://ratatui.rs) and [Crossterm](https://github.com/crossterm-rs/crossterm). This guide details all interactive features, keyboard shortcuts, modal dialogs, and workflows.

---

## 1. Launching the TUI

Start the TUI in interactive mode:

```bash
# Launch interactive TUI in current working directory
m31a

# Launch TUI pre-populated with a task prompt
m31a "Examine the test suite and fix any failing assertions"

# Launch TUI with a specific model slug
m31a -m deepseek-ai/deepseek-v4.1-flash
```

---

## 2. Core Screen Layout

The M31A terminal window is divided into three primary functional areas:

```
+-----------------------------------------------------------------------+
|  M31A v0.1.14  |  Thread: #0194...  |  Model: nemotron-3.5-lightning   |
+-----------------------------------------------------------------------+
|                                                                       |
|  [Transcript Stream Area]                                             |
|  - User prompts with timestamps                                       |
|  - Collapsible DeepSeek R1 / Nemotron reasoning thought blocks        |
|  - Streaming Markdown responses with code syntax highlighting        |
|  - Interactive tool call cards (arguments, status, diff outputs)     |
|                                                                       |
+-----------------------------------------------------------------------+
|  [⚡ BUILD] Mode  |  Sandbox: workspace-write  |  Tokens: 1,420 / 8,192 |
+-----------------------------------------------------------------------+
|  > Type prompt, /command, or !shell...                                |
|                                                                       |
+-----------------------------------------------------------------------+
```

1. **Header Bar**: Displays current version, session thread ID, active NIM model, and health status.
2. **Transcript Stream Area**: Virtualized scrolling viewport rendering conversation turns, collapsible thinking traces, and tool results.
3. **Status Line**: Displays active agent mode (`[⚡ BUILD]` vs `[📋 PLAN]`), sandbox isolation level, approval policy, and token count.
4. **Multiline Composer**: Input editor with syntax highlighting, slash-command autocompletion, and prompt history navigation.

---

## 3. Agent Modes: Build vs Plan

Toggle between agent modes using `Tab` or `Shift+Tab`:

- **`[⚡ BUILD]` Mode (Autonomous Execution)**:
  The agent has permission to inspect files, dispatch compiler commands, create multi-file patches, run tests, and iterate until the task is complete.
- **`[📋 PLAN]` Mode (Advisory Architecture)**:
  The agent explores the codebase in read-only mode, formulating architectural strategies, trade-off comparisons, and step-by-step implementation plans without modifying workspace state.

---

## 4. Instant Local Shell Execution (`!` prefix)

To run a quick local command without waiting for the agent loop:
1. Type `!` followed by your command in the composer (e.g. `!cargo check` or `!git status`).
2. Press `Enter`.
3. The command executes immediately in your local workspace and injects its stdout/stderr directly into your conversation history.

---

## 5. Collapsible Reasoning & Thinking Traces

When using reasoning models such as `nvidia/nemotron-3.5-lightning-30b-a3b` or `deepseek-ai/deepseek-v4.1-flash`:
- Thinking steps stream live into a bordered reasoning card.
- Press **`Ctrl+T`** at any time to fold or unfold all reasoning blocks across the transcript.
- Individual blocks can also be toggled with mouse clicks on the header bar.

---

## 6. Modal Dialogs & Palettes

| Modal | Shortcut / Command | Description |
| :--- | :--- | :--- |
| **Universal Fuzzy Palette** | `Ctrl + P` or `/palette` | Search and launch any command, tool, or action instantly. |
| **NVIDIA Model Selector** | `Ctrl + M` or `/model` | Dynamically queries `/v1/models` on NVIDIA NIM, showing context limits and capabilities. |
| **Interactive Git Diff Viewer** | `Ctrl + D` or `/diff` | Side-by-side or unified colored diff viewer of all uncommitted working tree changes. |
| **Session Resume Picker** | `Ctrl + R` or `/resume` | Browse and resume previous SQLite conversation sessions. |
| **Token & Cost Dashboard** | `Ctrl + A` or `/analytics` | Real-time breakdown of prompt tokens, completion tokens, latency, and estimated cost. |
| **Settings Manager** | `/settings` | Adjust General, Security, TUI themes, and Feature flags interactively. |
| **Help & Keybindings Overlay** | `?` (empty prompt) or `/help` | Comprehensive cheatsheet of all keyboard shortcuts and slash commands. |

---

## 7. Keyboard Shortcuts Cheatsheet

| Shortcut | Description |
| :--- | :--- |
| `Ctrl + P` | Open Universal Fuzzy Command Palette |
| `Tab` / `Shift+Tab` | Cycle Agent Mode (`[⚡ BUILD]` vs `[📋 PLAN]`) |
| `Enter` | Submit prompt / Confirm action |
| `Shift + Enter` | Insert newline in composer without submitting |
| `Ctrl + M` | Open Model Selector modal |
| `Ctrl + D` | Open interactive Git diff viewer |
| `Ctrl + R` | Open Session Picker (resume past threads) |
| `Ctrl + T` | Fold / unfold reasoning paths and tool output cards |
| `Ctrl + Y` | Extract and copy latest assistant code block to clipboard |
| `Ctrl + A` | Open Token & Cost Analytics dashboard |
| `Ctrl + L` | Clear active transcript display |
| `Ctrl + C` | Cancel active generation stream / double-press to exit |
| `Ctrl + Q` / `Esc` | Close open popup modal / Exit application |
| `PgUp` / `PgDown` | Scroll conversation transcript history |
| `Up` / `Down` | Navigate multiline composer prompt history |

---

## 8. Slash Commands

Type `/` in the prompt composer to trigger fuzzy autocompletion:

| Command | Action |
| :--- | :--- |
| `/help` | View help card with all keybindings and commands |
| `/settings` | Open interactive Settings Manager |
| `/palette` | Open universal fuzzy command palette |
| `/build` | Switch to autonomous Build mode |
| `/plan` | Switch to advisory Plan mode |
| `/vibe` | Engage continuous background auto-healing vibe loop |
| `/visual [url]` | Run autonomous Visual TDD check on web frontend |
| `/safety` | Audit codebase blast-radius and data-flow invariants |
| `/model [slug]` | Open model switcher modal or switch directly |
| `/diff` | View colorized uncommitted Git diff in working directory |
| `/undo` | Revert session and workspace mutations to previous checkpoint |
| `/verify` | Run autonomous compiler and linter checks on workspace |
| `/test` | Run workspace test suite with diagnostic reporting |
| `/review` | Conduct automated code review of git changes |
| `/analytics` | Show session token accounting and pricing metrics |
| `/apply` | Inspect and verify working tree patch state |
| `/fork` | Fork active conversation session into a new branch |
| `/clear` | Clear the active transcript view |
| `/compact` | Compact conversation history to reduce token context |
| `/resume` | Browse and resume previous SQLite sessions |
| `/checkpoints` | Browse session checkpoints and revert DAG |
| `/procs` | Manage background processes and servers |
| `/locks` | Inspect active path-based concurrency locks |
| `/sandbox` | Cycle sandbox policy (`ReadOnly` → `WorkspaceWrite` → `DangerFullAccess`) |
| `/approval` | Cycle approval policy (`OnRequest` → `Never` → `ApproveForMe`) |
| `/doctor` | Run runtime diagnostic healthchecks |
| `/board` | Display recent multi-agent message board messages |
| `/exit` | Exit M31A cleanly |
