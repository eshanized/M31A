---
focus: tech
last_mapped: 2026-05-30
---

# M31A — Technology Stack

## Languages

- **Go 1.22+** — primary language. Module path: `github.com/eshanized/M31A`.
- **Static binary** — `CGO_ENABLED=0` enforced at build. Cross-compilation target: linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64.

## Runtime

- **Terminal UI** — runs as a full-screen TUI with alternate screen buffer (`tea.WithAltScreen()`).
- **No daemon mode** — single process, single terminal session. No server components.

## Core Dependencies

| Dependency | Purpose | Used In |
|---|---|---|
| `charmbracelet/bubbletea v1.3.0` | TUI framework (model-view-update) | `internal/tui/`, `internal/workflow/` |
| `charmbracelet/lipgloss v1.1.0` | Terminal styling (colors, borders, layout) | `internal/tui/theme/`, all TUI components |
| `charmbracelet/bubbles v0.20.0` | Reusable TUI widgets (viewport, textarea, list, spinner) | `internal/tui/repl.go`, `modelselector.go` |
| `charmbracelet/glamour v0.6.0` | Markdown rendering (Glamour stylesheet) | `internal/tui/components/message.go` |
| `BurntSushi/toml v1.6.0` | TOML config file parsing | `internal/config/loader.go` |
| `pkoukk/tiktoken-go v0.1.8` | Token estimation for GPT/Claude model families | `internal/tokens/estimator.go` |
| `godbus/dbus/v5 v5.2.2` | Linux freedesktop Secret Service keychain | `pkg/keychain/keychain_linux.go` |
| `bmatcuk/doublestar/v4 v4.10.0` | Recursive glob pattern support (`**`) | `internal/tools/glob.go` |
| `alecthomas/chroma v0.10.0` | Syntax highlighting | Transitive (glamour) |
| `muesli/termenv v0.16.0` | Terminal background detection | Transitive (lipgloss) |
| `yuin/goldmark v1.5.2` | Markdown parser | Transitive (glamour) |

## Build & CI

- **Build system**: Makefile (`cmd/m31a/main.go` → `m31a` binary)
- **Linting**: `golangci-lint` with govet, staticcheck, errcheck, ineffassign, unused, gosimple
- **Release**: goreleaser v2 with checksums (SHA-256), cross-platform tarballs/zips
- **CI**: GitHub Actions with linux/amd64+arm64, darwin/amd64+arm64, windows/amd64 matrix

## Configuration

- Config file: `~/.m31a/config.toml` (TOML format)
- Config override via `M31A_CONFIG` env var
- API key resolution order: env var → OS keychain → config file `api_key` field
- Env vars: `M31A_LOG_FORMAT`, `M31A_LOG_LEVEL`, `M31A_THEME`, `M31A_DEFAULT_MODEL`

## Logging

- Structured `log/slog` JSON output to `~/.m31a/m31a.log`
- Daily rotation, 7-day retention
- Never writes to stdout/stderr during TUI operation

## Threading Model

- Bubble Tea is single-threaded. All state mutations go through `Update()`.
- Goroutines emit `tea.Cmd` functions that return `tea.Msg` — never mutate `AppState` directly.
- Thread boundaries:
  - Stream iterator (reads SSE, emits chunks)
  - Health check ticker (60s poll interval, adaptive on rate-limit)
  - Permission request (blocking user approval via channel)
- `View()` must return in <16ms (60fps frame budget).

## Entry Point

`cmd/m31a/main.go`:
1. Parse flags (`--version`, `--help`)
2. Initialize structured logger
3. Resolve config path (env var → `~/.m31a/config.toml`)
4. Load TOML config, resolve API keys via keychain
5. Create provider registry, register OpenRouter + Zen clients
6. Create and launch Bubble Tea program with alternate screen

## Test Dependencies

- All tests use stdlib `testing` package (no testify or external test frameworks)
- Mock HTTP servers for provider tests (OpenRouter, Zen)
- Temporary directories for filesystem tests (FileRead, FileWrite, Glob, session)
- No integration tests requiring real API keys
