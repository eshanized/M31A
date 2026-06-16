<div align="center">

# M31 Autonomous

### The terminal-native AI coding agent that ships, not just suggests.

**Six-phase workflow · Git rollback chain · Zero telemetry · One static binary**

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

M31 Autonomous is a terminal-based AI coding agent written in Go. Unlike browser-bound assistants, it runs inside your shell, owns the six-phase workflow end-to-end — **Initialize, Discuss, Plan, Execute, Verify, Ship** — and commits verified changes to your git tree. One static binary, zero telemetry, any POSIX shell.

```
$ m31a
 ╭──────────────────────────────────────────╮
 │  Initialize → Discuss → Plan → Execute   │
 │            → Verify  → Ship              │
 ╰──────────────────────────────────────────╯

 > refactor the auth middleware to use JWT with
   RS256, keep backward compat for 30 days
```

> **Status:** v1.0.0 — core feature complete. V1.1 features (ghost mode, picture-in-picture, deferred tools) are on the roadmap.

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

On first launch, M31 Autonomous prompts for your OpenRouter or Zen API key. Keys are stored in the OS keychain — **never written to disk in plaintext**.

## Why M31 Autonomous?

Every AI coding tool generates code and walks away. You verify, test, and commit. **M31 Autonomous owns the full loop.**

| | M31 Autonomous | Cursor | Aider | Cline |
|---|:---:|:---:|:---:|:---:|
| Terminal-native (no Electron) | **yes** | no | yes | no |
| Six-phase workflow engine | **yes** | no | no | no |
| Git commit rollback chain | **yes** | no | partial | no |
| Cross-session learning ledger | **yes** | no | no | no |
| AutoDream context consolidation | **yes** | no | no | no |
| Model arbitrage (cost optimizer) | **yes** | no | no | no |
| Provider auto-fallback | **yes** | no | partial | partial |
| Code intelligence (4 languages) | **yes** | yes | limited | no |
| Static binary, no CGO | **yes** | no | no | no |
| Telemetry / phone-home | **none** | yes | none | yes |
| Cost per month | **~$0.01/task** | $20/mo | ~$0.01/task | ~$0.01/task |

M31 Autonomous is the tool you reach for when you want an agent that **owns the loop** — not just an autocomplete with `rm -rf` access.

## How It Works

Every coding task goes through six phases:

```
  ┌───────────────┐      ┌──────────┐      ┌───────────┐
  │  Initialize   │────▶│ Discuss  │────▶│  Plan     │
  │               │      │          │      │           │
  │ Detect project│      │ Clarify  │      │ Structured│
  │ Index codebase│      │ questions│      │ task plan │
  └───────────────┘      └──────────┘      └───────────┘
                                                  │
  ┌──────────────┐      ┌──────────┐      ┌───────▼──┐
  │     Ship     │◀────│  Verify  │◀────│ Execute  │
  │              │      │          │      │          │
  │ Git commit   │      │ Build +  │      │ LLM + 14 │
  │ Ledger entry │      │ test     │      │ tools    │
  └──────────────┘      └──────────┘      └──────────┘
```

| Phase | What Happens |
|-------|-------------|
| **Initialize** | Detects project type (Go/Node/Rust/Python), builds code intelligence index — import graphs, symbol lookup, relevance scoring across 4 languages |
| **Discuss** | LLM asks clarifying questions, you answer in the TUI. Answers inform the plan. |
| **Plan** | Generates a structured implementation plan with task breakdown, file predictions (`[NEW]`/`[MODIFY]`), dependencies, and acceptance criteria. Refinable. |
| **Execute** | Tasks run in dependency order (Kahn's topological sort) with bounded parallelism. The LLM makes tool calls, sees results, iterates. Each task gets its own git commit. |
| **Verify** | Runs your build and test suite. Failed tasks trigger self-healing — error output goes back to the LLM, up to 2 retries. |
| **Ship** | Creates the final verified commit. Records session metrics in the cross-session learning ledger. |

## Features

### Workflow & Intelligence

- **Six-phase workflow** — `Initialize → Discuss → Plan → Execute → Verify → Ship`. Every run ends with a verified git commit and a ledger entry.
- **Code intelligence** — Parses Go (via `go/ast`), TypeScript, Python, and Rust. Builds import dependency graphs, indexes symbols, scores file relevance. The LLM gets context about which files matter for the current task.
- **14 built-in tools** — `Bash`, `FileRead`, `FileWrite`, `Edit`, `Glob`, `Grep`, `WebFetch`, `WebSearch`, `CodeMap`, `FileDelete`, `FileMove`, `FileList`, `TodoWrite`, `AskUserQuestion`. All gated by a permission system.
- **Parallel subagents** — The LLM can spawn child agents that run in isolated git worktrees, each with their own dispatcher and permissions.
- **Task runner** — Kahn's algorithm for topological sort, bounded parallelism (4 concurrent), per-task timeouts, retry support.

### Model & Provider

- **Dual provider support** — OpenRouter (300+ models) and OpenCode Zen gateways with automatic fallback when a provider degrades.
- **Model arbitrage** — Classifies task complexity (simple/moderate/complex) and recommends the cheapest model that meets quality requirements. Complex tasks require 64K+ context windows.
- **Per-phase model assignment** — Use a cheap model for Discuss/Plan, powerful model for Execute/Verify.
- **AutoDream context compression** — Long conversations get automatically consolidated. Protected messages (initial goal, tool calls, plans, last 5 messages) are never compressed.
- **Token estimation** — tiktoken-based with EMA self-calibration. Context warning banner at 80%, hard reject at 95%.

### Safety & Privacy

- **Commit rollback chain** — Every task gets its own commit. `git bisect` integration finds the offending commit when verification fails. Backup branches created before any destructive reset.
- **Permission system** — Every `Bash` command is gated by a modal: allow / allow always / deny / exit. Configurable rules, risk levels, per-agent profiles.
- **OS keychain** — Linux (dbus/secret-service), macOS (Keychain), Windows (Credential Manager). One code path, three backends. API keys **never written to disk in plaintext**.
- **Zero telemetry** — No phone-home, no analytics, no data collection. Works offline once model catalog is cached.
- **SSRF protection** — WebFetch blocks private, loopback, and link-local IP addresses.

### Terminal UI

- **29-screen Bubble Tea TUI** — dark/light themes, Vim-style navigation, leader key shortcuts (`Ctrl+X`), command palette (`Ctrl+P`).
- **Fuzzy model selector** — search with per-token cost comparison and live context-warning.
- **Diff viewer** — browse git diffs inline.
- **Rollback browser** — view commit chain, soft/hard reset with preview.
- **File explorer** — tree view with syntax highlighting.
- **Session persistence** — resume mid-workflow after `Ctrl+C`, network drops, or laptop sleep.

## Quick Tour

### The REPL

```
 /help          list all commands
 /workflow      kick off the six-phase flow
 /model         open the model selector (fuzzy search)
 /provider      switch provider
 /ledger stats  show your cross-session ledger
 /rollback      show the commit chain; --hard to reset
 /compress      trigger AutoDream manually
 /agent         spawn parallel subagents
 /diff          browse git diffs
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

[model]
default = ""
auto_arbitrage = false
arbitrage_threshold = 0.1

[ui]
theme = "dark"
compact_mode = false
show_cost_estimate = true

[permissions]
default_mode = "ask"
timeout_seconds = 300

[features]
autodream_enabled = true
auto_backup = true
budget_limit_usd = 0            # optional per-session budget cap

[agents]
plan = ""                       # cheap model for planning
execute = ""                    # powerful model for execution
```

Full reference: [`docs/CONFIG.md`](docs/CONFIG.md)

## Architecture

```
cmd/m31a/           binary entry point
internal/
  tui/              Bubble Tea app (29 screens, 5 themes)
  workflow/         six-phase orchestration engine
  provider/         OpenRouter + Zen clients, fallback, cache
  tools/            14 tools with permission system + rate limiting
  tools/subagent/   parallel subagent manager with worktree isolation
  codeintel/        4-language parser, import graph, relevance scoring
  config/           TOML loader, project context detection, hot-reload
  git/              commit/rollback/diff/stash/branch operations
  tokens/           tiktoken-based estimation with EMA calibration
  errors/           sentinel errors with user-friendly messages
  log/              structured logging with daily rotation
pkg/                public, importable packages
  session/          session lifecycle and persistence
  ledger/           cross-session learning store (markdown-backed)
  rollback/         commit-chain manager (soft/hard/safe reset)
  bisect/           git-bisect wrapper
  taskrunner/       parallel task executor with Kahn's algorithm
  keychain/         OS keychain abstraction (Linux/macOS/Windows)
  autodream/        context consolidation with reentrancy guard
  arbitrage/        model-cost optimizer
  history/          frecent prompt history with scoring
```

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
├── docs/              architecture, config, workflow, tools
├── internal/          private packages (not importable)
├── pkg/               public packages
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
make help           # list all targets
```

Coverage targets: **75%** overall, **90%** for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`.

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for code style, architecture rules, and PR conventions.

## Security

M31 Autonomous executes shell commands on your behalf. Every `Bash` tool call is gated by a permission modal (allow / allow always / deny), with a configurable timeout and default `ask` mode. Path traversal, size limits (50MB per session file read), and stream-size caps are enforced at the tool boundary.

See [`SECURITY.md`](SECURITY.md) to report vulnerabilities. See [`docs/TOOLS.md`](docs/TOOLS.md) for the full tool security model.

## Roadmap

- [ ] **Ghost mode** — headless runs that produce a structured diff without touching the TUI
- [ ] **Picture-in-picture** — run a second agent in a side pane for cross-review
- [ ] **Deferred tools** — queue tool calls that require human approval for batch review
- [x] **Subagents** — parallel child agents with git worktree isolation (v1.0)
- [x] **Code intelligence** — 4-language parser with import graph and relevance scoring (v1.0)

## Star History

If M31 Autonomous saves you time, [drop a star](https://github.com/eshanized/M31A) — it's the single most effective way to keep the project alive.

[![Star History Chart](https://api.star-history.com/svg?repos=eshanized/M31A&type=Date)](https://star-history.com/#eshanized/M31A&Date)

## Thanks

Built with [Bubble Tea](https://github.com/charmbracelet/bubbletea), [Lip Gloss](https://github.com/charmbracelet/lipgloss), [Glamour](https://github.com/charmbracelet/glamour), and [tiktoken-go](https://github.com/pkoukk/tiktoken-go).

## License

[MIT](LICENSE) — Copyright (c) Eshanized
