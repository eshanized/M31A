<div align="center">

# M31 Autonomous

### The terminal-native AI coding agent that ships, not just suggests.

[![Go](https://img.shields.io/github/go-mod/go-version/eshanized/M31A?style=flat-square&logo=go&logoColor=white)](go.mod)
[![License](https://img.shields.io/github/license/eshanized/M31A?style=flat-square)](LICENSE)
[![Release](https://img.shields.io/github/v/release/eshanized/M31A?style=flat-square&logo=github)](https://github.com/eshanized/M31A/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/eshanized/M31A/ci.yml?branch=master&style=flat-square&logo=githubactions&logoColor=white&label=build)](https://github.com/eshanized/M31A/actions)
[![Go Report Card](https://goreportcard.com/badge/github.com/eshanized/M31A?style=flat-square)](https://goreportcard.com/report/github.com/eshanized/M31A)

[Install](#install) · [Docs](docs/) · [Contributing](CONTRIBUTING.md) · [Report a Bug](https://github.com/eshanized/M31A/issues/new?template=bug_report.md)

</div>

---

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

> **Status:** v1.0.0 — core feature complete. V1.1 features (ghost mode, picture-in-picture, subagents, deferred tools) are on the roadmap.

## Install

Pick your weapon.

```bash
# macOS (Homebrew)
brew install eshanized/tap/m31a

# Linux / macOS (one-liner)
curl -fsSL https://raw.githubusercontent.com/eshanized/M31A/main/install.sh | bash

# From source (any OS)
git clone https://github.com/eshanized/M31A.git
cd M31A
CGO_ENABLED=0 go build -o m31a ./cmd/m31a
```

On first launch, M31 Autonomous prompts for your OpenRouter or Zen API key. Keys are stored in the OS keychain — **never written to disk in plaintext**.

## Why M31 Autonomous?

| | M31 Autonomous | Cursor | Aider | Cline |
|---|:---:|:---:|:---:|:---:|
| Terminal-native (no Electron) | **yes** | no | yes | no |
| Six-phase workflow engine | **yes** | no | no | no |
| Git commit rollback chain | **yes** | no | partial | no |
| Cross-session learning ledger | **yes** | no | no | no |
| AutoDream context consolidation | **yes** | no | no | no |
| Provider auto-fallback | **yes** | no | partial | partial |
| Static binary, no CGO | **yes** | no | no | no |
| Telemetry / phone-home | **none** | yes | none | yes |

M31 Autonomous is the tool you reach for when you want an agent that **owns the loop** — not just an autocomplete with `rm -rf` access.

## Features

- **Six-phase workflow** — `Initialize → Discuss → Plan → Execute → Verify → Ship`. Every run ends with a verified git commit and a ledger entry.
- **Dual provider support** — OpenRouter and OpenCode Zen gateways with automatic fallback when a provider degrades.
- **29-screen Bubble Tea TUI** — dark/light themes, keyboard navigation, fuzzy model selector, permission modals.
- **5 core tools** — `Bash`, `FileRead`, `FileWrite`, `Glob`, `Grep`. Deliberately small surface area, aggressively sandboxed.
- **Model selector** — fuzzy search, per-token cost comparison, live context-warning when you approach the window limit.
- **Cross-session learning ledger** — patterns, failures, and recoveries persist across sessions so the agent gets sharper over time.
- **Commit rollback chain** — git-bisect integration with `--hard` reset when verification fails.
- **AutoDream** — automatic context consolidation for long conversations so you don't blow the window mid-task.
- **Model arbitrage** — automatic switching to the cheapest model that still meets the task's capability threshold.
- **Session persistence** — resume mid-workflow after `Ctrl+C`, network drops, or laptop sleep.
- **OS keychain** — Linux (dbus/secret-service), macOS (Keychain), Windows (Credential Manager). One code path, three backends.

## Quick tour

### The REPL

```
 /help          list all commands
 /workflow      kick off the six-phase flow
 /model         open the model selector (fuzzy search)
 /provider      switch provider
 /ledger stats  show your cross-session ledger
 /rollback      show the commit chain; --hard to reset
 /compress      trigger AutoDream manually
```

Full command reference: [`docs/SLASH_COMMANDS.md`](docs/SLASH_COMMANDS.md)

### Key bindings

| Key | Action |
|---|---|
| `Enter` | Send message in REPL |
| `Ctrl+C` | Cancel active stream; second press exits |
| `Esc` | Close model selector / modals |
| `y` / `a` / `n` / `e` | Permission modal: allow / allow always / deny / exit |
| `x` | Dismiss fallback notification banner |

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
```

Full reference: [`docs/CONFIG.md`](docs/CONFIG.md)

## Architecture

```
cmd/m31a/           binary entry point
internal/
  tui/              Bubble Tea app (29 screens, 5 themes)
  workflow/         six-phase orchestration engine
  provider/         OpenRouter + Zen clients, fallback, cache
  tools/            Bash, FileRead, FileWrite, Glob, Grep
  codeintel/        project indexing, relevance scoring
  config/           TOML loader, project context detection
  git/              commit/rollback helpers
  tokens/           tiktoken-based context estimation
pkg/                public, importable packages
  session/          session lifecycle and persistence
  ledger/           cross-session learning store
  rollback/         commit-chain manager
  bisect/           git-bisect wrapper
  taskrunner/       sequential task executor
  keychain/         OS keychain abstraction
  autodream/        context consolidation
  arbitrage/        model-cost optimizer
  history/          session history store
```

Deep dive: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)

## Project layout

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
- [ ] **Subagents** — delegate sub-tasks to specialized agents (code, test, doc)
- [ ] **Deferred tools** — queue tool calls that require human approval for batch review

## Star history

If M31 Autonomous saves you time, [drop a star](https://github.com/eshanized/M31A) — it's the single most effective way to keep the project alive.

[![Star History Chart](https://api.star-history.com/svg?repos=eshanized/M31A&type=Date)](https://star-history.com/#eshanized/M31A&Date)

## Thanks

Built with [Bubble Tea](https://github.com/charmbracelet/bubbletea), [Lip Gloss](https://github.com/charmbracelet/lipgloss), [Glamour](https://github.com/charmbracelet/glamour), and [tiktoken-go](https://github.com/pkoukk/tiktoken-go).

## License

[MIT](LICENSE) — Copyright (c) Eshanized
