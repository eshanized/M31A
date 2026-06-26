<div align="center">

# M31 Autonomous

### The terminal-native AI coding agent that ships, not just suggests.

**Seven-phase workflow · Git rollback chain · Zero telemetry · One static binary**

[![Go](https://img.shields.io/github/go-mod/go-version/eshanized/M31A?style=flat-square&logo=go&logoColor=white)](go.mod)
[![License](https://img.shields.io/github/license/eshanized/M31A?style=flat-square)](LICENSE)
[![Release](https://img.shields.io/github/v/release/eshanized/M31A?style=flat-square&logo=github)](https://github.com/eshanized/M31A/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/eshanized/M31A/ci.yml?branch=master&style=flat-square&logo=githubactions&logoColor=white&label=build)](https://github.com/eshanized/M31A/actions)
[![Go Report Card](https://goreportcard.com/badge/github.com/eshanized/M31A?style=flat-square)](https://goreportcard.com/report/github.com/eshanized/M31A)
[![GitHub stars](https://img.shields.io/github/stars/eshanized/M31A?style=flat-square&logo=github)](https://github.com/eshanized/M31A/stargazers)

[Install](#install) · [Wiki](https://github.com/eshanized/M31A/wiki) · [Features](#features) · [How It Works](#how-it-works) · [Contributing](CONTRIBUTING.md) · [Report a Bug](https://github.com/eshanized/M31A/issues/new?template=bug_report.md)

</div>

---

<p align="center">
  <img src=".images/image_0.png" alt="M31 Autonomous TUI" width="700">
</p>

M31 Autonomous is a terminal-based AI coding agent written in Go. Unlike browser-bound assistants, it runs inside your shell, owns the seven-phase workflow end-to-end — **Initialize, Discuss, Plan, Execute, Verify, Runtime, Ship** — and commits verified changes to your git tree. One static binary, zero telemetry, any POSIX shell.

```
$ m31a
 ╭──────────────────────────────────────────╮
 │  Initialize → Discuss → Plan → Execute   │
 │            → Verify  → Ship              │
 ╰──────────────────────────────────────────╯

 > refactor the auth middleware to use JWT with
   RS256, keep backward compat for 30 days
```

> **Status:** v1.3.0 — core feature complete. V1.1 features (ghost mode, picture-in-picture, deferred tools) are on the roadmap.

## Install

Pick your weapon.

```bash
# macOS (Homebrew)
brew install eshanized/tap/m31a

# Linux / macOS (one-liner)
curl -fsSL https://raw.githubusercontent.com/eshanized/M31A/master/install.sh | bash

# From source (any OS)
git clone https://github.com/eshanized/M31A.git
cd M31A
CGO_ENABLED=0 go build -o m31a ./cmd/m31a
```

On first launch, M31 Autonomous prompts for your OpenRouter, Zen, or Nvidia API key. Keys are stored in the OS keychain — **never written to disk in plaintext**.

## Why M31 Autonomous?

Every AI coding tool generates code and walks away. You verify, test, and commit. **M31 Autonomous owns the full loop.**

| | M31 Autonomous | Cursor | Aider | Cline |
|---|:---:|:---:|:---:|:---:|
| Terminal-native (no Electron) | **yes** | no | yes | no |
| Seven-phase workflow engine | **yes** | no | no | no |
| Git commit rollback chain | **yes** | no | partial | no |
| Cross-session learning ledger | **yes** | no | no | no |
| AutoDream context consolidation | **yes** | no | no | no |
| Model arbitrage (cost optimizer) | **yes** | no | no | no |
| Provider auto-fallback (3 providers) | **yes** | no | partial | partial |
| Code intelligence (4 languages) | **yes** | yes | limited | no |
| Workflow modes (full/fast/direct) | **yes** | no | no | no |
| Static binary, no CGO | **yes** | no | no | no |
| Telemetry / phone-home | **none** | yes | none | yes |
| Cost per month | **~$0.01/task** | $20/mo | ~$0.01/task | ~$0.01/task |

M31 Autonomous is the tool you reach for when you want an agent that **owns the loop** — not just an autocomplete with `rm -rf` access.

## How It Works

Every coding task goes through seven phases. The workflow engine supports four modes — **auto** (adaptive), **full** (all 7 phases), **fast** (skip Plan), and **direct** (skip Discuss, Plan, Verify) — so you can dial the process to match the task.

```mermaid
flowchart LR
    subgraph Phase1["1 · Initialize"]
        I1["Detect project type"]
        I2["Build code index"]
        I3["Deep analysis*"]
        I1 --> I2 --> I3
    end

    subgraph Phase2["2 · Discuss"]
        D1["Clarifying questions"]
        D2["Quality scoring*"]
        D3["Completeness check*"]
        D1 --> D2 --> D3
    end

    subgraph Phase3["3 · Plan"]
        P0["Pre-plan research*"]
        P1["Task breakdown"]
        P2["Plan checker*"]
        P3["Coverage + security gates*"]
        P4["Chunked generation*"]
        P0 --> P1 --> P2 --> P3 --> P4
    end

    subgraph Phase4["4 · Execute"]
        E0["Pre-flight validation*"]
        E1["Topological sort"]
        E2["LLM + 18 tools"]
        E3["Loop detection*"]
        E4["Quality gates*"]
        E0 --> E1 --> E2 --> E3 --> E4
    end

    subgraph Phase5["5 · Verify"]
        V1["Build + test"]
        V2["Self-heal (2 retries)"]
        V3["Verification report*"]
        V4["Security scan*"]
        V1 --> V2 --> V3 --> V4
    end

    subgraph Phase5b["6 · Runtime"]
        R1["Runtime checks*"]
        R1 --> R1
    end

    subgraph Phase6["7 · Ship"]
        S1["Pre-ship checklist*"]
        S2["Git commit"]
        S3["Changelog*"]
        S4["Ledger entry"]
        S1 --> S2 --> S3 --> S4
    end

    Phase1 --> Phase2 --> Phase3 --> Phase4 --> Phase5 --> Phase5b --> Phase6

    style Phase1 fill:#1a1b26,stroke:#7aa2f7,color:#c0caf5
    style Phase2 fill:#1a1b26,stroke:#bb9af7,color:#c0caf5
    style Phase3 fill:#1a1b26,stroke:#e0af68,color:#c0caf5
    style Phase4 fill:#1a1b26,stroke:#9ece6a,color:#c0caf5
    style Phase5 fill:#1a1b26,stroke:#f7768e,color:#c0caf5
    style Phase5b fill:#1a1b26,stroke:#73daca,color:#c0caf5
    style Phase6 fill:#1a1b26,stroke:#73daca,color:#c0caf5
```

*\* Optional enhancements — toggle individually via `[features]` in config.*

| Phase | What Happens |
|-------|-------------|
| **Initialize** | Detects project type (Go/Node/Rust/Python), builds code intelligence index — import graphs, symbol lookup, relevance scoring across 4 languages. Optional deep project analysis and environment pre-flight checks. |
| **Discuss** | LLM asks clarifying questions, you answer in the TUI. Built-in quality scoring and answer completeness checks ensure the LLM gathers enough context. |
| **Plan** | Optional pre-plan research step, then structured task plan with file predictions (`[NEW]`/`[MODIFY]`), dependencies, and acceptance criteria. Plan checker runs revision loops with coverage and security gates. Large plans auto-chunk for reliability. |
| **Execute** | Tasks run in dependency order (Kahn's topological sort) with bounded parallelism. The LLM makes tool calls via 18 built-in tools, sees results, iterates. Includes pre-flight validation, tool-call loop detection, and per-task quality gates. |
| **Verify** | Runs your build and test suite. Failed tasks trigger self-healing — error output goes back to the LLM, up to 2 retries. Generates a structured verification report and optional security file scanning. |
| **Runtime** | Runtime checks and validation during execution. |
| **Ship** | Pre-ship checklist, final verified commit, auto-generated changelog, and session metrics recorded in the cross-session learning ledger. |

## Features

### Workflow & Intelligence

- **Seven-phase workflow** — `Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship` with four modes (`auto`, `full`, `fast`, `direct`) to match task complexity. Every run ends with a verified git commit and a ledger entry.
- **Workflow quality gates** — Plan checker with revision loops, coverage gates, security heuristics, and gap analysis. Discuss-phase quality scoring and completeness checks. Execute-phase pre-flight validation and tool-call loop detection.
- **Code intelligence** — Parses Go (via tree-sitter), TypeScript, Python, and Rust. Builds import dependency graphs, indexes symbols, scores file relevance. The LLM gets context about which files matter for the current task.
- **Code complexity analysis** — `CodeComplexity` tool classifies your codebase as simple (<10K lines), moderate (10K–50K), or complex (50K+) across all 4 languages, informing model selection.
- **18 built-in tools** — `Bash`, `FileRead`, `FileWrite`, `Edit`, `Glob`, `Grep`, `WebFetch`, `WebSearch`, `CodeMap`, `CodeComplexity`, `FileDelete`, `FileMove`, `FileList`, `TodoWrite`, `TodoRead`, `DevServer`, `HTTPCheck`, `AskUserQuestion`. All gated by a permission system with rate limiting and concurrency control.
- **Parallel subagents** — The LLM can spawn child agents (up to depth 2) that run in isolated git worktrees, each with their own dispatcher and permissions.
- **Task runner** — Kahn's algorithm for topological sort, bounded parallelism (4 concurrent), per-task timeouts, retry support.

### Model & Provider

- **Triple provider support** — OpenRouter (300+ models), OpenCode Zen, and Nvidia NIM gateways with automatic fallback when a provider degrades. Configurable custom base URLs for self-hosted or proxied gateways.
- **Model arbitrage** — Classifies task complexity (simple/moderate/complex) and recommends the cheapest model that meets quality requirements. Complex tasks require 64K+ context windows.
- **Per-phase model assignment** — Use a cheap model for Discuss/Plan, powerful model for Execute/Verify/Ship. Eight configurable phase slots.
- **AutoDream context compression** — Long conversations get automatically consolidated. Protected messages (initial goal, tool calls, plans, last 5 messages) are never compressed. Auto-compression triggers on context window overflow.
- **Token estimation** — tiktoken-based with EMA self-calibration. Context warning banner at 80%, hard reject at 95%. Configurable warning threshold and EMA alpha.

### Safety & Privacy

- **Commit rollback chain** — Every task gets its own commit. `git bisect` integration finds the offending commit when verification fails. Backup branches created before any destructive reset.
- **Permission system** — Every `Bash` command is gated by a modal: allow / allow always / deny / exit. Configurable rules, risk levels, per-agent profiles. Rate-limited with token bucket (20 burst / 10 sustained) and stricter limits for dangerous tools (5 burst / 2 sustained).
- **Concurrency control** — Max 8 concurrent tool executions via semaphore. Dangerous commands have an additional blocklist for defense-in-depth.
- **OS keychain** — Linux (dbus/secret-service), macOS (Keychain), Windows (Credential Manager). One code path, three backends. API keys **never written to disk in plaintext**.
- **Zero telemetry** — No phone-home, no analytics, no data collection. Works offline once model catalog is cached.
- **SSRF + DNS rebinding protection** — WebFetch blocks private, loopback, and link-local IP addresses. WebSearch uses a DNS cache to prevent TOCTOU rebinding attacks.
- **Edit safety** — 7-strategy cascade replacement with collision-safe backups and replace-all support.

### Terminal UI

- **33-screen Bubble Tea TUI** — 11 themes (Midnight, Daylight, Catppuccin Mocha, Nord Frost, Tokyo Night, Gruvbox Dark, Rose Pine, Dracula, Solarized Dark, Pure Mono, High Contrast), Vim-style navigation, leader key shortcuts (`Ctrl+X`), command palette (`Ctrl+P`).
- **Fuzzy model selector** — search with per-token cost comparison and live context-warning.
- **Diff viewer** — browse git diffs inline with syntax highlighting.
- **Rollback browser** — view commit chain, soft/hard reset with preview.
- **File explorer** — tree view with syntax highlighting.
- **Dashboard** — workflow pipeline overview with phase progress, current goal, and activity timeline.
- **Chat history** — browse and search past conversations within a session.
- **Session persistence** — resume mid-workflow after `Ctrl+C`, network drops, or laptop sleep.
- **Accessibility** — reduced motion mode, configurable animation speed, opacity controls, and breathing effects.

## Quick Tour

### The REPL

```
 /help          list all commands
 /workflow      kick off the seven-phase flow
 /model         open the model selector (fuzzy search)
 /provider      switch provider (openrouter/zen/nvidia)
 /ledger stats  show your cross-session ledger
 /rollback      show the commit chain; --hard to reset
 /compress      trigger AutoDream manually
 /agent         spawn parallel subagents
 /diff          browse git diffs
 /dashboard     workflow pipeline overview
 /ghost         ghost write files
 /bisect        compare model outputs side-by-side
 /memory        manage context memory (view/pause/resume/revert)
```

Full command reference: [`docs/SLASH_COMMANDS.md`](docs/SLASH_COMMANDS.md)

### Key Bindings

| Key | Action |
|---|---|
| `Enter` | Send message in REPL |
| `Ctrl+C` | Cancel active stream; second press exits |
| `Ctrl+P` | Open command palette |
| `Ctrl+Q` | Open quick actions |
| `Esc` | Close model selector / modals |
| `y` / `a` / `n` / `e` | Permission modal: allow / allow always / deny / exit |
| `Ctrl+X` + key | Leader shortcuts (settings, model, theme, rollback, etc.) |

Full keymap: [`docs/KEYBINDINGS.md`](docs/KEYBINDINGS.md)

### Configuration

`~/.m31a/config.toml` (override with `M31A_CONFIG`):

```toml
[provider]
default = "openrouter"
auto_fallback = true
# Custom base URLs for self-hosted gateways:
# openrouter_base_url = ""
# zen_base_url = ""
# nvidia_base_url = ""

[model]
default = ""
auto_arbitrage = false
arbitrage_threshold = 0.1
show_thinking_by_default = false
token_ema_alpha = 0.3

[ui]
theme = "dark"
compact_mode = false
show_cost_estimate = true
sidebar_position = "right"         # "right" or "left"
animation_speed = "normal"         # "fast", "normal", "slow", "none"
reduced_motion = false
welcome_screen = true

[permissions]
default_mode = "ask"
timeout_seconds = 300

[features]
auto_backup = true
resume_on_startup = true
workflow_mode = "auto"             # "auto", "full", "fast", "direct"
budget_limit_usd = 0               # optional per-session budget cap

# Workflow quality enhancements (all opt-in):
plan_research = true               # pre-plan research step
plan_check = true                  # plan checker + revision loop
plan_security_gate = true          # security heuristic gate
plan_coverage_gate = true          # requirements coverage gate
plan_chunked = true                # chunked plan generation
discuss_quality_check = true       # question quality checker
discuss_completeness = true        # answer completeness check
execute_preflight = true           # pre-execution validation
execute_loop_detect = true         # tool call loop detection
verify_report = true               # verification report generation
ship_preflight = true              # pre-ship checklist
ship_changelog = true              # changelog generation
init_deep_analysis = true          # deep project analysis

[tools]
max_glob_results = 50
max_grep_results = 50
bash_kill_grace_secs = 5
websearch_enabled = true

[git]
commit_prefix = "feat"
fix_prefix = "fix"
ship_prefix = "ship"

[verify]
build_command = ""                 # empty = auto-detect
test_command = ""                  # empty = auto-detect

[agents]
default = ""
plan = ""                          # cheap model for planning
execute = ""                       # powerful model for execution
verify = ""
ship = ""
discuss = ""
```

Full reference: [`docs/CONFIG.md`](docs/CONFIG.md)

## Architecture

```mermaid
graph TB
    subgraph entry["cmd/m31a"]
        main["main.go<br/>entry point"]
        usage["usage.go<br/>CLI flags"]
    end

    subgraph internal["internal/ (private)"]
        direction TB
        tui["tui/<br/>33 screens · 11 themes<br/>Bubble Tea app"]
        workflow["workflow/<br/>seven-phase engine<br/>quality gates · chunked plans"]
        provider["provider/<br/>openrouter · zen · nvidia<br/>fallback · cache · SSE"]
        tools["tools/<br/>18 tools · permissions<br/>rate limiting · concurrency"]
        subagent["tools/subagent/<br/>parallel subagents<br/>worktree isolation · depth=2"]
        codeintel["codeintel/<br/>4-language parser<br/>import graph · relevance"]
        config["config/<br/>TOML loader · hot-reload<br/>project context detection"]
        git["git/<br/>commit · rollback<br/>diff · stash · branch"]
        tokens["tokens/<br/>tiktoken estimation<br/>EMA calibration"]
        types["types/<br/>shared types · constants<br/>workflow modes · risk levels"]
        fileutil["fileutil/<br/>atomic file operations"]
        errors["errors/<br/>sentinel errors"]
        log["log/<br/>structured logging<br/>daily rotation"]

        tools --> subagent
    end

    subgraph pkg["pkg/ (public, importable)"]
        direction TB
        session["session/<br/>lifecycle · persistence"]
        ledger["ledger/<br/>cross-session learning<br/>markdown-backed"]
        rollback["rollback/<br/>commit-chain manager<br/>soft · hard · safe reset"]
        bisect["bisect/<br/>git-bisect wrapper"]
        taskrunner["taskrunner/<br/>Kahn's algorithm<br/>bounded parallelism"]
        keychain["keychain/<br/>OS keychain<br/>Linux · macOS · Windows"]
        autodream["autodream/<br/>context consolidation<br/>reentrancy guard"]
        arbitrage["arbitrage/<br/>model-cost optimizer"]
        history["history/<br/>frecent prompt history<br/>scoring"]
    end

    main --> internal
    internal --> pkg

    style entry fill:#1a1b26,stroke:#7aa2f7,color:#c0caf5
    style internal fill:#1a1b26,stroke:#bb9af7,color:#c0caf5
    style pkg fill:#1a1b26,stroke:#9ece6a,color:#c0caf5
```

### Package Summary

| Package | Description |
|---------|-------------|
| `cmd/m31a/` | Binary entry point with CLI flags |
| `internal/tui/` | Bubble Tea TUI app — 33 screens, 11 themes, responsive layout, command palette |
| `internal/workflow/` | Seven-phase orchestration engine with quality gates, chunked plans, and pre-flight checks |
| `internal/provider/` | OpenRouter, Zen, and Nvidia clients with auto-fallback, model cache, and SSE streaming |
| `internal/tools/` | 18 tools with permission system, token-bucket rate limiting, and concurrency control |
| `internal/tools/subagent/` | Parallel subagent manager with git worktree isolation (max depth 2) |
| `internal/codeintel/` | 4-language parser (Go, TypeScript, Python, Rust) with import graph and relevance scoring |
| `internal/config/` | TOML loader with project context detection, hot-reload, and workflow enhancement flags |
| `internal/git/` | Commit, rollback, diff, stash, and branch operations |
| `internal/tokens/` | tiktoken-based estimation with EMA self-calibration |
| `internal/types/` | Shared types, constants, workflow modes, risk levels, and plan structures |
| `internal/fileutil/` | Atomic file write operations |
| `internal/errors/` | Sentinel errors with user-friendly messages |
| `internal/log/` | Structured logging with daily rotation |
| `pkg/session/` | Session lifecycle and persistence (JSON in `~/.m31a/sessions/`) |
| `pkg/ledger/` | Cross-session learning store (markdown-backed) |
| `pkg/rollback/` | Commit-chain manager (soft/hard/safe reset) |
| `pkg/bisect/` | Git-bisect wrapper for model comparison |
| `pkg/taskrunner/` | Parallel task executor with Kahn's algorithm and bounded parallelism |
| `pkg/keychain/` | OS keychain abstraction (Linux dbus, macOS Keychain, Windows Credential Manager) |
| `pkg/autodream/` | Context consolidation with reentrancy guard |
| `pkg/arbitrage/` | Model-cost optimizer with task classification |
| `pkg/history/` | Frecent prompt history with scoring |

Deep dive: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) · [Wiki](https://github.com/eshanized/M31A/wiki)

## Screenshots

<p align="center">
  <img src=".images/image_1.png" alt="M31A Screenshot 1" width="400">
  <img src=".images/image_2.png" alt="M31A Screenshot 2" width="400">
</p>

<p align="center">
  <img src=".images/image_3.png" alt="M31A Screenshot 3" width="400">
  <img src=".images/image_4.png" alt="M31A Screenshot 4" width="400">
</p>

## Project Layout

```
.
├── cmd/m31a/          entry point
├── docs/              architecture, config, workflow, tools, screens, troubleshooting
├── internal/          private packages (not importable)
│   ├── codeintel/     4-language parser, import graph, relevance scoring
│   ├── config/        TOML loader, project context, hot-reload
│   ├── context/       context management
│   ├── errors/        sentinel errors
│   ├── fileutil/      atomic file operations
│   ├── git/           commit, rollback, diff, stash, branch
│   ├── log/           structured logging, daily rotation
│   ├── provider/      openrouter, zen, nvidia clients
│   ├── shell/         shell integration
│   ├── testutil/      test utilities
│   ├── tokens/        tiktoken estimation, EMA calibration
│   ├── tools/         18 tools + subagent manager
│   ├── tui/           Bubble Tea app (33 screens, 11 themes)
│   ├── types/         shared types, constants, workflow modes
│   └── workflow/      seven-phase engine, quality gates
├── pkg/               public packages (importable)
│   ├── autodream/     context consolidation, reentrancy guard
│   ├── arbitrage/     model-cost optimizer
│   ├── bisect/        git-bisect wrapper
│   ├── compaction/    compaction utilities
│   ├── coordinator/   coordination logic
│   ├── history/       frecent prompt history, scoring
│   ├── keychain/      OS keychain abstraction
│   ├── ledger/        cross-session learning store
│   ├── metrics/       metrics collection
│   ├── retry/         retry logic
│   ├── rollback/      commit-chain manager
│   ├── session/       session lifecycle, persistence
│   ├── skills/        skill management
│   └── taskrunner/    Kahn's algorithm, bounded parallelism
├── scripts/           verify_v1.sh acceptance suite
├── install.sh         one-liner installer
├── Makefile           build / test / lint / release targets
└── .goreleaser.yaml   cross-compile + release config
```

## Development

```bash
git clone https://github.com/eshanized/M31A.git
cd M31A

make build          # optimized binary
make test           # race-enabled tests
make lint           # golangci-lint + gofmt check
make dev            # build + run
make cover          # test + coverage report
make bench          # benchmarks
make help           # list all targets
```

Coverage targets: **75%** overall, **90%** for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`.

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for code style, architecture rules, and PR conventions.

## Security

M31 Autonomous executes shell commands on your behalf. Security is layered:

| Layer | Mechanism |
|-------|-----------|
| **Permission modal** | Every `Bash` command gated: allow / allow always / deny, with configurable timeout and default `ask` mode |
| **Rate limiting** | Token bucket: 20 burst / 10 sustained for normal tools; 5 burst / 2 sustained for dangerous tools |
| **Concurrency control** | Max 8 concurrent tool executions via semaphore |
| **Command blocklist** | Dangerous command patterns blocked at the tool boundary (defense-in-depth) |
| **Path traversal** | Blocked at tool boundary; size limits (50MB per file read), stream-size caps |
| **SSRF protection** | WebFetch blocks private, loopback, and link-local IP addresses |
| **DNS rebinding** | WebSearch uses a DNS cache (5 min TTL) to prevent TOCTOU rebinding attacks |
| **Edit safety** | 7-strategy cascade with collision-safe backups |
| **Subagent depth** | Max nesting depth of 2 to prevent runaway agent spawning |

See [`SECURITY.md`](SECURITY.md) to report vulnerabilities. See [`docs/TOOLS.md`](docs/TOOLS.md) for the full tool security model.

## Roadmap

- [ ] **Ghost mode** — headless runs that produce a structured diff without touching the TUI
- [ ] **Picture-in-picture** — run a second agent in a side pane for cross-review
- [ ] **Deferred tools** — queue tool calls that require human approval for batch review
- [x] **Subagents** — parallel child agents with git worktree isolation (v1.0)
- [x] **Code intelligence** — 4-language parser with import graph and relevance scoring (v1.0)
- [x] **Workflow quality gates** — plan checker, coverage gates, security heuristics, loop detection (v1.0)
- [x] **Nvidia NIM provider** — third provider with auto-fallback (v1.0)
- [x] **Code complexity analysis** — polyglot codebase classifier informing model selection (v1.0)
- [x] **Chunked plan generation** — large plans auto-chunk with outline + wave expansion (v1.0)

## Star History

If M31 Autonomous saves you time, [drop a star](https://github.com/eshanized/M31A) — it's the single most effective way to keep the project alive.

[![Star History Chart](https://api.star-history.com/svg?repos=eshanized/M31A&type=Date)](https://star-history.com/#eshanized/M31A&Date)

## Citing M31 Autonomous

If you use M31 Autonomous in your research, please cite it:

```bibtex
@software{m31a2026,
  author       = {Eshan Roy},
  title        = {M31 Autonomous: A Terminal-Native AI Coding Agent with Seven-Phase Workflow Orchestration},
  year         = {2026},
  url          = {https://github.com/eshanized/M31A},
  version      = {v1.3.0}
}
```

Or use the `CITATION.cff` file in this repository for automatic citation generation on GitHub.

## Thanks

Built with [Bubble Tea](https://github.com/charmbracelet/bubbletea), [Lip Gloss](https://github.com/charmbracelet/lipgloss), [Glamour](https://github.com/charmbracelet/glamour), and [tiktoken-go](https://github.com/pkoukk/tiktoken-go).

## License

[MIT](LICENSE) — Copyright (c) Eshanized
