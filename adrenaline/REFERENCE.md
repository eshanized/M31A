# M31A — Project Overview for AI Assistants

> **Read this first.** This document distills the full V1 spec (`adrenaline/idea.md`) into a reference that eliminates hallucination and misconceptions.

---

## What Is M31A?

M31A is a **terminal-based AI coding assistant** written in Go. It runs in your terminal, connects to LLMs via API gateways, and helps you build software through a structured six-phase workflow. It is **free, open source (MIT license)**, with no telemetry, no vendor lock-in, and no paid tiers.

**What it is NOT:**
- NOT a VS Code extension — it's a standalone terminal binary
- NOT a chat-only tool — it executes tasks, writes files, runs commands, and commits to git
- NOT connected directly to Anthropic/OpenAI — all traffic goes through gateway providers

---

## Core Identity

| Attribute | Value |
|-----------|-------|
| Name | M31A |
| Language | Go 1.22+ |
| Binary | Single static binary (`CGO_ENABLED=0`) |
| UI Framework | Bubble Tea + Lipgloss + Bubbles + Glamour (all Charm libraries) |
| License | MIT |
| Module Path | `github.com/eshanized/M31A` |
| Target OS | Linux, macOS, Windows |

---

## How It Works (The Six-Phase Workflow)

When a user gives M31A a goal (e.g., "Build me a restaurant website"), it runs through six phases:

1. **Initialize** — Parses the goal, detects project type, creates session directory
2. **Discuss** — Asks 2-4 clarifying questions, collects user answers
3. **Plan** — Generates a task list with dependencies, cost/time estimates, and predicted file changes. User must approve before execution.
4. **Execute** — Runs tasks in dependency order. Each task produces tool calls (Bash, FileRead, FileWrite, etc.) and an atomic git commit.
5. **Verify** — Checks each task's output (file existence, syntax, tests). Failures trigger self-healing (max 2 attempts). If self-heal fails, git bisect pinpoints the exact commit that caused the regression.
6. **Ship** — Final summary, commit log, session archived.

**Key detail:** Each phase launches with a **fresh, pruned context** — the LLM doesn't carry the full conversation history. State persists between phases via human-readable Markdown files (`PROJECT.md`, `TASKS.md`, `STATE.md`).

---

## LLM Providers (Dual-Gateway Architecture)

M31A supports **two** LLM gateways. No direct Anthropic/OpenAI connections.

| Provider | Base URL | Notes |
|----------|----------|-------|
| **OpenRouter** | `https://openrouter.ai/api/v1` | 400+ models, pay-per-use |
| **OpenCode Zen** | `https://opencode.ai/zen/v1` | Curated models, flat pricing |

- User picks a default provider in config, can switch per-session via `/model`
- Both providers support: model discovery, streaming, reasoning/thinking, tool use
- Auto-fallback: if the active provider goes down (429/503), M31A can switch to the other provider automatically
- Model catalog is fetched dynamically (every 5 minutes) — no hardcoded model lists

---

## Key Features (What Makes M31A Different)

### Signature Features (V1)

1. **Inline Thinking/Reasoning** — When a model supports reasoning (DeepSeek R1, Claude with extended thinking, OpenAI o-series), the thought process is rendered as collapsible blocks in the message stream. Not hidden behind a spinner.

2. **Rich Tool Cards** — Tool executions (Bash, FileRead, FileWrite, Glob, Grep) render as styled cards with syntax highlighting, collapsible output, and status badges (`[..]` → `[OK]` / `[ERR]`).

3. **Spec-Driven Planning** — Plans are structured task lists with dependencies, not free-form text. Users can Accept, Edit, or Retry. A diff preview (`D` key) shows predicted file changes before execution.

4. **Self-Healing Verification** — After execution, each task is verified. Failures trigger automatic fix attempts. If those fail, `git bisect` traces the regression to a specific commit.

5. **Cost-Aware Model Arbitrage** — M31A analyzes task complexity and suggests cheaper model alternatives. Press `O` to optimize all tasks at once.

6. **Cross-Session Learning Ledger** — After each completed workflow, M31A logs session data to `~/.m31a/LEDGER.md`. Over time, it builds aggregate stats and injects relevant past experience into new sessions.

7. **Commit Rollback Chain** — Every task produces an atomic git commit. `/rollback` shows an interactive timeline of all session commits with soft/hard rollback options.

### Deferred to V1.1

- **Ghost Mode** — Run the entire workflow in an isolated git worktree. Merge or discard results with one keypress. Zero risk to working code.
- **Terminal PiP** — Floating panel showing live output from long-running processes (dev servers, build watchers) while you continue interacting with the main REPL.

---

## Tools Available to the LLM (V1)

| Tool | Purpose | Risk |
|------|---------|------|
| Bash | Execute shell commands (30-min timeout) | Dangerous |
| FileRead | Read files (5MB limit, binary detection) | Safe |
| FileWrite | Atomic file write with backup, creates dirs | Destructive |
| Glob | File listing with glob patterns | Safe |
| Grep | Code search (uses `rg` or fallback) | Safe |

**V1.1 deferred tools:** FileEdit, WebFetch, WebSearch, AgentTool, TaskTool, AskUserQuestion, GitTool.

**Important:** Dangerous tools trigger a permission modal in the TUI. The user must approve before execution.

---

## State Persistence (File-Based)

All workflow state is stored as **human-readable Markdown and JSON** in `~/.m31a/sessions/<session-id>/`:

```
~/.m31a/sessions/<id>/
├── session.json          # Metadata (ID, model, timestamps)
├── messages.json         # Full conversation history
├── MEMORY.md             # AutoDream consolidated memory
└── planning/
    ├── PROJECT.md        # Goal, project type, Discuss Q&A
    ├── TASKS.md          # Task list with dependencies and status
    ├── STATE.md          # Current phase, progress, last action
    └── config.json       # Per-session overrides
```

Sessions are **resumable** — M31A parses these files on startup and offers to restore the last session.

---

## Configuration

Config file: `~/.m31a/config.toml`

- **API keys**: Resolved in order: env var → OS keychain → config file. Never stored in plaintext by default.
- **Env vars**: `M31A_OPENROUTER_API_KEY`, `M31A_ZEN_API_KEY`, `M31A_CONFIG`, `M31A_THEME`, `M31A_DEFAULT_MODEL`, `M31A_PROVIDER`
- **Key settings**: default provider, default model, theme, auto-fallback, auto-arbitrage, ledger enabled, context warning threshold

---

## TUI Structure

M31A's UI is built with Bubble Tea (single-threaded, frame-based rendering). Key screens:

1. **REPL** (main) — Header (brand + model badge + context bar), scrollable message history, text input area, status bar
2. **Initialize/Discuss** — Shows goal, project detection, clarifying questions
3. **Plan** — Split-pane: task list on left, cost/time estimates on right, dependency graph on Tab
4. **Execute** — Task progress tracker with live tool cards and thinking blocks
5. **Verify** — Pass/fail checklist with self-heal options
6. **Ship** — Summary banner with commit log
7. **Model Selector** — Full-screen searchable overlay with provider filtering
8. **Permission Modal** — Centered overlay for dangerous tool approval
9. **Settings** — Two-column layout with tabs
10. **Resume** — Session browser with metadata

---

## Important Architecture Constraints

- **Bubble Tea is single-threaded** — all state mutations go through the `Update()` loop. V1.1 subagents communicate via channels, not shared state.
- **HTTP timeout is 30s dial-only** — no body read timeout, because streaming responses from reasoning models can be very long.
- **Context pruning is per-phase** — each phase discards prior conversation and works from structured state files only.
- **Token estimation is two-phase** — client-side estimation during streaming (tiktoken-go for GPT/Claude, char÷4 for others), server calibration after each turn.
- **AutoDream pauses during execution** — context consolidation never runs while tools are active.

---

## Slash Commands

| Command | Purpose |
|---------|---------|
| `/new` | Start new workflow |
| `/plan` | Jump to Plan phase |
| `/execute` | Execute current plan |
| `/verify` | Run verification |
| `/ship` | Force-ship workflow |
| `/model` | Model selector |
| `/ledger` | Learning ledger viewer |
| `/ledger stats` | Aggregate statistics |
| `/rollback` | Commit rollback chain |
| `/optimize` | Cost optimization |
| `/memory` | View consolidated memory |
| `/undo` | Roll back to previous phase |
| `/theme` | Cycle theme |
| `/status` | Show session stats |
| `/resume` | Session browser |
| `/ghost` | V1.1: Toggle ghost mode |

---

## What to Avoid When Working on M31A

1. **Do NOT add direct Anthropic/OpenAI provider support** — Only OpenRouter and Zen are first-class providers.
2. **Do NOT use CSS-style animations** — Bubble Tea uses frame-based redraws with Unicode spinners, not CSS easing.
3. **Do NOT add V1.1 tools in V1** — FileEdit, WebFetch, WebSearch, AgentTool, etc. are explicitly deferred.
4. **Do NOT store API keys in plaintext** — Resolution order is env → keychain → config fallback.
5. **Do NOT assume concurrent task execution in V1** — V1 runs tasks sequentially. Concurrency is V1.1.
6. **Do NOT add telemetry or analytics** — Explicitly forbidden by the FOSS mandate.
7. **Do NOT hardcode model lists** — Models are discovered dynamically from provider APIs.
8. **Do NOT use `AskUserQuestion` tool in V1** — Discuss phase uses native TUI text input, not a modal tool.

---

## Files in This Repository

| Path | Purpose |
|------|---------|
| `adrenaline/idea.md` | Complete V1 specification (detailed, implementation-ready) |
| `adrenaline/REFERENCE.md` | This file — condensed overview for AI assistants |
| (future) `go.mod` | Go module definition |
| (future) `cmd/m31a/main.go` | Binary entry point |
| (future) `internal/` | Internal packages (provider, tui, workflow, tools, etc.) |
| (future) `pkg/` | Public packages (taskrunner, arbitrage, bisect, ledger, rollback, etc.) |
