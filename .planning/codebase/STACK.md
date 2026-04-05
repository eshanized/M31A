# STACK.md — Technology Stack

**Last updated:** 2026-06-13
**Project:** M31A — Terminal AI Coding Agent
**Module:** `github.com/eshanized/M31A`

## Language & Runtime

- **Language:** Go 1.24
- **Build:** `go build` with `CGO_ENABLED=0` (fully static binaries)
- **Cross-compilation:** Linux, Darwin, Windows (amd64 + arm64) via `goreleaser`
- **Makefile-based build system** — targets: `build`, `debug`, `dev`, `cross`, `release`

## Framework Architecture

M31A is built on **Bubble Tea** (`charmbracelet/bubbletea v1.3.0`) — the Go Elm-architecture TUI framework. The UI uses:
- `charmbracelet/bubbles v0.20.0` — prebuilt Bubble Tea components (textarea, viewport, spinner, etc.)
- `charmbracelet/lipgloss v1.1.0` — style definitions and layout
- `charmbracelet/glamour v0.6.0` — Markdown rendering in the terminal

## Core Dependencies

### LLM / AI
| Dependency | Version | Purpose |
|---|---|---|
| `github.com/pkoukk/tiktoken-go` | v0.1.8 | Token estimation for LLM context windows |
| `github.com/dlclark/regexp2` | v1.10.0 | Extended regex (used by syntax highlighting) |
| `github.com/yuin/goldmark` | v1.5.2 | Markdown parsing |
| `github.com/yuin/goldmark-emoji` | v1.0.1 | Emoji extension for goldmark |

### Network & HTTP
| Dependency | Version | Purpose |
|---|---|---|
| `golang.org/x/net` | v0.0.0-20221002... | HTTP client utilities |
| `github.com/muesli/cancelreader` | v0.2.2 | Cancellable stdin reader |
| `github.com/gorilla/css` | v1.0.0 | CSS parsing (glamour dependency) |

### Concurrency & System
| Dependency | Version | Purpose |
|---|---|---|
| `golang.org/x/sync` | v0.10.0 | `singleflight` for deduplication |
| `golang.org/x/sys` | v0.30.0 | OS-level syscalls |
| `github.com/fsnotify/fsnotify` | v1.10.1 | File system watcher (config hot-reload) |
| `github.com/godbus/dbus/v5` | v5.2.2 | D-Bus IPC (Linux keychain) |
| `github.com/google/uuid` | v1.3.0 | UUID generation |
| `github.com/rivo/uniseg` | v0.4.7 | Unicode segmentation |
| `github.com/mattn/go-isatty` | v0.0.20 | TTY detection |
| `github.com/mattn/go-runewidth` | v0.0.19 | Terminal rune width |
| `github.com/mattn/go-localereader` | v0.0.1 | Locale-aware reader |

### Configuration & Parsing
| Dependency | Version | Purpose |
|---|---|---|
| `github.com/BurntSushi/toml` | v1.6.0 | TOML config parsing |
| `github.com/bmatcuk/doublestar/v4` | v4.10.0 | Glob pattern matching (extended) |
| `github.com/alecthomas/chroma` | v0.10.0 | Syntax highlighting |
| `github.com/microcosm-cc/bluemonday` | v1.0.21 | HTML sanitization |
| `github.com/aymerick/douceur` | v0.2.0 | CSS parser (bluemonday dep) |
| `github.com/clipperhouse/stringish` | v0.1.1 | String utilities |
| `github.com/clipperhouse/uax29/v2` | v2.5.0 | Unicode segmentation |

### Terminal / UI Utilities
| Dependency | Version | Purpose |
|---|---|---|
| `github.com/charmbracelet/x/ansi` | v0.8.0 | ANSI handling |
| `github.com/charmbracelet/x/cellbuf` | v0.0.13 | Cell buffer for rendering |
| `github.com/charmbracelet/x/term` | v0.2.1 | Terminal operations |
| `github.com/charmbracelet/colorprofile` | v0.2.3-... | Terminal color detection |
| `github.com/atotto/clipboard` | v0.1.4 | Clipboard access |
| `github.com/aymanbagabas/go-osc52/v2` | v2.0.1 | OSC52 escape sequences |
| `github.com/lucasb-eyer/go-colorful` | v1.3.0 | Color manipulation |
| `github.com/muesli/ansi` | v0.0.0-20230316... | ANSI string operations |
| `github.com/muesli/reflow` | v0.3.0 | Text reflow/word wrap |
| `github.com/muesli/termenv` | v0.16.0 | Terminal environment |
| `github.com/erikgeiser/coninput` | v0.0.0-20211004... | Console input (Windows) |
| `github.com/olekukonko/tablewriter` | v0.0.5 | Table formatting |
| `github.com/xo/terminfo` | v0.0.0-20220910... | Terminal info queries |

## Code Organization

### Entry Point
- `cmd/m31a/main.go` — CLI entry, config load, dependency wiring, TUI launch
- `cmd/m31a/usage.go` — Usage/help output
- `cmd/firstrunpreview/main.go` — First-run preview tool

### Internal Packages (`internal/`)
| Package | Purpose |
|---|---|
| `internal/config` | TOML config loading, project context, validation, hot-reload via fsnotify |
| `internal/errors` | Sentinel errors, user-friendly error messages |
| `internal/fileutil` | Atomic file writes, safe file operations |
| `internal/git` | Git client abstraction (commit, diff, log, bisect) |
| `internal/log` | Structured logger (slog-based) |
| `internal/provider` | LLM provider abstraction — registry, base client, SSE streaming, caching, fallback, reasoning |
| `internal/provider/openrouter` | OpenRouter API client implementation |
| `internal/provider/zen` | Zen API client implementation |
| `internal/tokens` | Token estimation (tiktoken-go based), context warnings |
| `internal/tools` | Tool system — dispatcher, 20+ tool definitions, permission gates, subagent manager |
| `internal/tui` | Full TUI — REPL, models, views, commands, components, theme engine, layout system |
| `internal/types` | Shared types, constants, model info, messages, task/phase enums |
| `internal/workflow` | Six-phase GSD workflow engine (discuss→plan→execute→verify→ship) |

### Public Packages (`pkg/`)
| Package | Purpose |
|---|---|
| `pkg/arbitrage` | Model cost arbitrage between providers |
| `pkg/autodream` | AutoDream — autonomous task chaining |
| `pkg/bisect` | Git bisect automation |
| `pkg/keychain` | OS keychain integration (macos/keychain, linux/secret-service via dbus, win32) |
| `pkg/ledger` | Decision ledger for tracking phase decisions |
| `pkg/rollback` | Git-based rollback functionality |
| `pkg/session` | Session management (create, save, list, resume, checkpoints) |
| `pkg/taskrunner` | Parallel task execution runner |

### Other Directories
- `docs/` — help docs, slash command docs, troubleshooting guides
- `images/` — screenshots for README
- `scripts/` — shell scripts for build helpers
- `adrenaline/` — comprehensive audit/analysis reports (28 documents)
- `.github/` — GitHub Actions workflows

## Configuration

- **Config file:** `~/.m31a/config.toml` (TOML format)
- **Config path override:** `M31A_CONFIG` env var
- **Hot-reload:** Supported via `fsnotify` watcher
- **Keychain integration:** API keys resolved via OS keychain (macOS Keychain, Linux Secret Service, Windows Credential Manager)

### Config Sections
| Section | Purpose |
|---|---|
| `[provider]` | LLM provider config (OpenRouter, Zen API base URLs, credentials) |
| `[model]` | Default model, context threshold, arbitrage, token EMA calibration |
| `[ui]` | Theme, animation, layout, sidebar, toasts, tool card style |
| `[permissions]` | Tool permission rules, risk levels, agent-specific profiles |
| `[features]` | Auto-backup, resume, model caching, health check, session retention, budget limits |
| `[ledger]` | Decision ledger settings |
| `[tools]` | Tool execution limits (glob, grep, bash, backups) |
| `[agents]` | Per-phase model assignments |
| `[git]` | Commit message prefixes, user identity |
| `[verify]` | Custom build/test commands |

## Tool System

M31A has a built-in tool system with 20+ tools accessible to the LLM agent:
- `Bash` — shell command execution with timeout and kill
- `Read`, `Write`, `Edit`, `FileDelete`, `FileList`, `FileMove`, `Glob`, `Grep` — filesystem operations
- `WebFetch`, `WebSearch` — web access
- `Question`, `TodoWrite` — user interaction
- `Agent` — subagent spawning (background parallel agents via git worktrees)
- `Permissions` management, browser subagent support

The dispatcher (`internal/tools/dispatcher.go`) handles tool registration, permission gating, and execution routing.
