# M31A — Project Intelligence Report

> **Generated:** 2026-06-12 | **Version:** v1.0.0 | **Status:** Core feature complete

---

## 1. Executive Summary

M31A is a **terminal-based AI coding assistant** written in Go that uses a structured six-phase workflow (Initialize → Discuss → Plan → Execute → Verify → Ship) to autonomously build software from natural language prompts.

| Metric | Value |
|--------|-------|
| **Language** | Go 1.24 |
| **Total Go Files** | 295 |
| **Lines of Code** | 68,192 |
| **Test Files** | 76 |
| **Test Lines** | 21,656 |
| **Test-to-Code Ratio** | 32% |
| **Functions** | 2,519 |
| **Structs** | 376 |
| **Interfaces** | 48 |
| **Total Commits** | 1,612 |
| **Project Size** | 15 MB |
| **Primary Author** | Eshan Roy |

---

## 2. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         cmd/m31a/main.go                            │
│                    Entry Point · Config · Session Init              │
└──────────────────────────────────┬──────────────────────────────────┘
                                   │
┌──────────────────────────────────▼──────────────────────────────────┐
│                              TUI Layer                              │
│              internal/tui/ (Bubble Tea · 10 screens)                │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ │
│  │   REPL   │ │Dashboard │ │ Settings │ │  Plan    │ │ Execute  │ │
│  │  (Chat)  │ │  (Stats) │ │  (Config)│ │ (Review) │ │  (Tasks) │ │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘ │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐ │
│  │  Verify  │ │   Ship   │ │   Diff   │ │  Resume  │ │  Model   │ │
│  │  (Tests) │ │ (Commit) │ │ (Review) │ │ (Session)│ │(Selector)│ │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘ └──────────┘ │
└──────────────────────────────────┬──────────────────────────────────┘
                                   │
┌──────────────────────────────────▼──────────────────────────────────┐
│                         Workflow Engine                             │
│            internal/workflow/ (6-phase pipeline)                    │
│                                                                     │
│  ┌────────────┐   ┌────────────┐   ┌────────────┐                  │
│  │Initialize  │──▶│  Discuss   │──▶│    Plan    │                  │
│  │detect type │   │ask questions│  │task graph  │                  │
│  │init git    │   │collect ans │   │validation  │                  │
│  └────────────┘   └────────────┘   └────────────┘                  │
│                                              │                      │
│  ┌────────────┐   ┌────────────┐   ┌────────▼─────┐                │
│  │   Ship     │◀──│  Verify    │◀──│   Execute    │                │
│  │commit+ship│   │file check  │   │tool dispatch │                │
│  │ledger     │   │syntax/test │   │self-heal     │                │
│  └────────────┘   └────────────┘   └──────────────┘                │
└──────────────────────────────────┬──────────────────────────────────┘
                                   │
┌──────────────────────────────────▼──────────────────────────────────┐
│                           Tool Layer                                │
│               internal/tools/ (13 tools + subagents)                │
│                                                                     │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐      │
│  │  Bash   │ │FileRead │ │FileWrite│ │  Edit   │ │  Glob   │      │
│  │ dangerous│ │  safe   │ │ medium  │ │ medium  │ │  safe   │      │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘ └─────────┘      │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐      │
│  │  Grep   │ │WebFetch │ │TodoWrite│ │AskUser  │ │FileList │      │
│  │  safe   │ │ medium  │ │  safe   │ │  safe   │ │  safe   │      │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘ └─────────┘      │
│  ┌─────────┐ ┌─────────┐ ┌─────────────────────────┐               │
│  │FileDelete│ │FileMove │ │  Agent (subprocess)     │               │
│  │dangerous │ │ medium  │ │  parallel subagents     │               │
│  └─────────┘ └─────────┘ └─────────────────────────┘               │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 3. Package Dependency Graph

```
                         ┌─────────────────────┐
                         │     cmd/m31a        │  ← ENTRY POINT
                         └──────────┬──────────┘
                                    │
              ┌─────────────────────┼─────────────────────┐
              │                     │                     │
              ▼                     ▼                     ▼
    ┌─────────────────┐   ┌─────────────────┐   ┌─────────────────┐
    │    internal/     │   │   internal/      │   │   pkg/          │
    │      tui         │   │    workflow      │   │  (8 packages)   │
    │  (UI layer)      │   │  (orchestration)│   │                 │
    └────────┬────────┘   └────────┬────────┘   └────────┬────────┘
             │                     │                     │
             ▼                     ▼                     ▼
    ┌─────────────────┐   ┌─────────────────┐   ┌─────────────────┐
    │ internal/tools  │   │ internal/       │   │ pkg/taskrunner  │
    │ (13 tools)      │   │   provider      │   │ pkg/session     │
    └────────┬────────┘   └────────┬────────┘   │ pkg/arbitrage   │
             │                     │            │ pkg/autodream   │
             ▼                     ▼            │ pkg/bisect      │
    ┌─────────────────┐   ┌─────────────────┐  │ pkg/ledger      │
    │ internal/config │   │ internal/       │  │ pkg/rollback    │
    └────────┬────────┘   │   git           │  └────────┬────────┘
             │            └────────┬────────┘           │
             ▼                     ▼                     ▼
    ┌───────────────────────────────────────────────────────────┐
    │                    LEAF PACKAGES                          │
    │  internal/types · internal/errors · internal/fileutil     │
    │  internal/log · internal/tokens · pkg/keychain            │
    │  internal/tui/theme                                       │
    └───────────────────────────────────────────────────────────┘
```

### Dependency Rules (enforced)
```
types, errors, fileutil, log     →  ZERO internal imports (leaf nodes)
git, tokens, config, provider    →  import only leaf packages
tools, pkg/*                     →  import foundation layer
workflow                         →  imports everything below it
tui                              →  imports workflow + everything below it
cmd/m31a                         →  imports all layers
```

---

## 4. Package Inventory

### Internal Packages (19)

| Package | Purpose | Key Files |
|---------|---------|-----------|
| `internal/tui/` | Bubble Tea TUI framework | app.go, repl.go, streaming.go, 91 files total |
| `internal/tui/components/` | Reusable UI components | 15+ component files |
| `internal/tui/layout/` | Screen layout management | layout.go |
| `internal/tui/theme/` | Dark/light theme system | theme.go |
| `internal/workflow/` | 6-phase workflow engine | engine.go, discuss.go, plan.go, execute.go, verify.go, ship.go |
| `internal/workflow/prompts/` | LLM prompt templates | 9 markdown templates |
| `internal/tools/` | Tool implementations | 13 tools + dispatcher + permissions |
| `internal/tools/subagent/` | Parallel subagent support | agent.go |
| `internal/provider/` | LLM provider abstraction | interface.go, streaming.go |
| `internal/provider/openrouter/` | OpenRouter API gateway | openrouter.go |
| `internal/provider/zen/` | Zen API gateway | zen.go |
| `internal/config/` | TOML config + keychain | config.go |
| `internal/types/` | Core type definitions | types.go, plan.go, constants.go |
| `internal/errors/` | Sentinel errors | errors.go |
| `internal/git/` | Git operations | git.go |
| `internal/tokens/` | Token counting | tokens.go |
| `internal/fileutil/` | File utilities | fileutil.go |
| `internal/log/` | Structured logging | log.go |

### Public Packages (8)

| Package | Purpose |
|---------|---------|
| `pkg/taskrunner/` | Dependency graph + topological sort + bounded parallelism |
| `pkg/session/` | Session lifecycle, file persistence, checkpoints |
| `pkg/arbitrage/` | Complexity scoring, model cost comparison |
| `pkg/autodream/` | Context consolidation when usage >60% |
| `pkg/bisect/` | Git bisect wrapper for regression finding |
| `pkg/ledger/` | Cross-session learning ledger |
| `pkg/rollback/` | Commit chain browser |
| `pkg/keychain/` | OS keychain integration (Linux/macOS/Windows) |

---

## 5. Tool Inventory

| # | Tool | Risk | Description | Files |
|---|------|------|-------------|-------|
| 1 | **Bash** | 🔴 dangerous | Shell commands with 30min timeout, output capping | bash.go |
| 2 | **FileRead** | 🟢 safe | Read file contents (5MB max) | fileread.go |
| 3 | **FileWrite** | 🟡 medium | Create/rewrite files atomically with backup | filewrite.go |
| 4 | **Edit** | 🟡 medium | Targeted string/line-range replacement | edit.go |
| 5 | **Glob** | 🟢 safe | Find files by pattern (`**` recursive) | glob.go |
| 6 | **Grep** | 🟢 safe | Search file contents with regex (ripgrep) | grep.go |
| 7 | **WebFetch** | 🟡 medium | Fetch URL content (text/markdown, 30s timeout) | webfetch.go |
| 8 | **TodoWrite** | 🟢 safe | Write structured TODO lists | todo.go |
| 9 | **AskUserQuestion** | 🟢 safe | Pause for user input (interactive only) | question.go |
| 10 | **FileList** | 🟢 safe | List directory contents with metadata | filelist.go |
| 11 | **FileDelete** | 🔴 dangerous | Delete with automatic backup | filedelete.go |
| 12 | **FileMove** | 🟡 medium | Rename/move files | filemove.go |
| 13 | **Agent** | ⚪ special | Spawn parallel subagents | agent.go |

### Permission System
```
Risk Level   │ User Prompt? │ Auto-Approve? │ Rate Limit?
─────────────┼──────────────┼───────────────┼────────────
safe         │     No       │     Yes       │    No
medium       │     Yes*     │  Configurable │    No
dangerous    │     Yes      │  Configurable │    No
destructive  │     Yes      │     No        │    No

* = depends on config.toml rules
Rate limiting: Token bucket (burst=20, 10 tools/sec)
```

---

## 6. Workflow Pipeline Detail

### Phase Timing & Behavior

```
Phase         │ LLM Calls │ Tool Calls │ User Interaction │ Checkpoint?
──────────────┼───────────┼────────────┼──────────────────┼───────────
Initialize    │     0     │     0      │       None       │    Yes
Discuss       │     1     │     0      │  2-4 questions   │    Yes
Plan          │   1-3*    │     0      │  Review + refine │    Yes
Execute       │   N**     │   N × M    │  Permission gate │  Per task
Verify        │   0-2***  │    0-2     │       None       │    Yes
Ship          │     1     │     1      │       None       │    Yes

*  Plan retries up to 3× on validation failure
** N = number of tasks, M = tool calls per task
*** Self-heal + bisect if verification fails
```

### Self-Heal Mechanism

```
Task fails
    │
    ▼
┌───────────────────────┐
│ HealsAttempted < 2 ?  │──No──▶ Task marked UNRECOVERABLE
└───────────┬───────────┘
            │ Yes
            ▼
┌───────────────────────┐
│ Build heal context:   │
│ - Fresh LLM call      │
│ - self-heal.md prompt │
│ - Error + file state  │
│ - Original goal       │
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│ LLM diagnoses & fixes │
│ via tool calls        │
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│ Verify fix:           │
│ - Files exist?        │──No──▶ retry heal
│ - Build passes?       │
└───────────┬───────────┘
            │ Yes
            ▼
      Fix committed ✓
```

---

## 7. Prompt Templates

| File | Phase | Purpose |
|------|-------|---------|
| `base.md` | All | Identity, principles, tool philosophy, code style |
| `tool-use.md` | Plan + Execute | Detailed tool usage instructions |
| `discuss-questions.md` | Discuss | Question format, numbering, suggested defaults |
| `plan-format.md` | Plan | Structured plan output format with JSON task schema |
| `execute-task.md` | Execute | Task execution process, file-first approach |
| `self-heal.md` | Execute + Verify | Diagnostic process, common failures, defeat criteria |
| `autonomous.md` | All | Autonomous mode instructions |
| `demonstration-format.md` | Ship | Demo walkthrough generation format |

---

## 8. TUI Screen Map

```
┌─────────────────────────────────────────────────────────────┐
│                     M31A Terminal UI                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─── Main Screens ───────────────────────────────────────┐ │
│  │                                                        │ │
│  │  REPL ─── Chat with M31A, type commands                │ │
│  │    │                                                   │ │
│  │    ├── /workflow <goal>  → starts 6-phase pipeline     │ │
│  │    ├── /model [name]    → model selector               │ │
│  │    ├── /phase [name]    → phase transitions             │ │
│  │    ├── /status          → session info                  │ │
│  │    ├── /undo            → rollback to checkpoint        │ │
│  │    ├── /compress        → context consolidation         │ │
│  │    └── /quit            → exit                          │ │
│  │                                                        │ │
│  │  Dashboard ── Session stats, progress overview          │ │
│  │  Settings ─── Config editor, theme toggle               │ │
│  │  Model Selector ── Pick LLM model                       │ │
│  │  Plan ─────── Review/refine implementation plan         │ │
│  │  Execute ──── Task progress, tool call log              │ │
│  │  Verify ───── Test results, self-heal status            │ │
│  │  Ship ─────── Final commit, demo generation             │ │
│  │  Diff ─────── File changes review                       │ │
│  │  Resume ───── Session browser                           │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                             │
│  ┌─── Sidebar (always visible) ───────────────────────────┐ │
│  │  Session ID │ Model │ Phase │ Token Usage │ Duration    │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

---

## 9. Session Persistence Model

```
~/.m31a/
├── config.toml                  User configuration
├── m31a.log                     Structured logs (7-day rotation)
├── LEDGER.md                    Cross-session learning
│
└── sessions/
    └── <8-char-id>/
        ├── session.json         Metadata (model, provider, phase, timestamps)
        ├── messages.json        Full message history
        ├── MEMORY.md            Consolidated context (AutoDream output)
        │
        ├── planning/
        │   ├── PROJECT.md       Goal, project type, framework, Q&A
        │   ├── TASKS.md         Task list with status + dependencies
        │   ├── STATE.md         Current phase, progress, last action
        │   └── PLAN.md          Implementation plan (versioned)
        │
        ├── checkpoints/
        │   └── <timestamp>.json State snapshots for undo
        │
        ├── backups/
        │   └── <file>.bak       Pre-overwrite file backups
        │
        └── archives/
            └── <session>.json   Post-ship archived sessions
```

---

## 10. Code Quality Metrics

```
Category                    │ Count  │ % of Total
────────────────────────────┼────────┼───────────
Go Source Files             │  219   │   74%
Test Files                  │   76   │   26%
────────────────────────────┼────────┼───────────
Source Lines                │ 46,536 │   68%
Test Lines                  │ 21,656 │   32%
────────────────────────────┼────────┼───────────
Functions                   │ 2,519  │  100%
Structs                     │  376   │  100%
Interfaces                  │   48   │  100%
────────────────────────────┼────────┼───────────
Internal Packages           │   19   │   70%
Public Packages             │    8   │   30%
────────────────────────────┼────────┼───────────
Prompt Templates            │    9   │  100%
Tool Implementations        │   13   │  100%
TUI Screens                 │   10   │  100%
```

---

## 11. Risk Assessment

### Current Strengths
- ✅ Clean layered architecture with enforced dependency rules
- ✅ 32% test coverage by LOC (industry healthy range)
- ✅ Atomic commits per task with rollback capability
- ✅ Self-heal mechanism (2 retries) with git bisect fallback
- ✅ Non-interactive environment variables (CI=true) prevent CLI hangs
- ✅ Package manager detection from lock files
- ✅ JSON comment stripping + trailing comma normalization for LLM output
- ✅ OS keychain integration (no plaintext secrets)

### Known Limitations
- ⚠️ Verify phase uses `exec.CommandContext` directly (bypasses Bash tool permissions)
- ⚠️ Max 2 self-heal attempts per task (may be insufficient for complex failures)
- ⚠️ Plan parser regexes recompiled per call in plan_parser.go (perf opportunity)
- ⚠️ No max task count limit (LLM could generate hundreds of tasks)
- ⚠️ Single author project (bus factor = 1)
- ⚠️ No integration tests for end-to-end workflow execution

---

## 12. Dependency Health

```
Layer               │ Packages         │ Imports From
────────────────────┼──────────────────┼─────────────────
Leaf (no deps)      │ 7                │ —
Foundation          │ 7                │ leaf only
Mid-tier            │ 10               │ foundation + leaf
Orchestration       │ 1 (workflow)     │ everything below
UI                  │ 1 (tui)          │ everything below
Entry point         │ 1 (cmd/m31a)     │ everything

Circular deps       │ 0 ✓
Import depth max    │ 5 layers
Leaf package ratio  │ 37% (7/19) ✓     (healthy: >30%)
```

---

## 13. Git History Summary

```
Total Commits:        1,612
Primary Author:       Eshan Roy (99.9%)
Recent Activity:      Active development on workflow engine, TUI, prompts
Commit Style:         Conventional (docs:, feat:, fix:, update)
Branch Strategy:      Single main branch
```

---

*Report generated by analyzing 295 Go files across 68,192 lines of code.*
