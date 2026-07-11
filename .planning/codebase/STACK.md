# Technology Stack

**Analysis Date:** 2026-07-11

## Languages

**Primary:**
- **Go 1.25.0** - Primary language for all application code (`go.mod:3`)

**Secondary:**
- None detected — pure Go codebase with CGO disabled

## Runtime

**Environment:**
- **Go 1.25+** — Required by `go.mod` (`go 1.25.0`)

**Package Manager:**
- **Go Modules** (built-in) — `go.mod` / `go.sum` present
- Lockfile: **Present** (`go.sum`)

## Frameworks

**Core:**
- **Bubble Tea v1.3.0** (`github.com/charmbracelet/bubbletea`) — TUI framework (Elm architecture)
- **Bubbles v0.20.0** (`github.com/charmbracelet/bubbles`) — TUI components (viewport, textarea, etc.)
- **Lipgloss v1.1.0** (`github.com/charmbracelet/lipgloss`) — Terminal styling/layout
- **Glamour v0.6.0** (`github.com/charmbracelet/glamour`) — Markdown rendering in terminal

**Testing:**
- **Standard library `testing`** — Unit/integration tests
- **Race detector enabled** — `make test` runs with `-race`
- Coverage target: **75% overall, 90% for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`**

**Build/Dev:**
- **Make** (`Makefile`) — Primary build orchestrator
- **goreleaser v2** (`.goreleaser.yaml`) — Cross-platform releases (Linux/macOS/Windows, amd64/arm64)
- **golangci-lint** — Linting (govet, staticcheck, errcheck, ineffassign, unused)
- **goimports** — Import formatting
- **CGO_ENABLED=0** (hard constraint — static binary required)

## Key Dependencies

**Critical (Direct):**

| Package | Version | Purpose |
|---------|---------|---------|
| `github.com/charmbracelet/bubbletea` | v1.3.0 | TUI runtime (Elm architecture) |
| `github.com/charmbracelet/bubbles` | v0.20.0 | TUI components (viewport, textarea, list, spinner, etc.) |
| `github.com/charmbracelet/lipgloss` | v1.1.0 | Terminal styling & layout |
| `github.com/charmbracelet/glamour` | v0.6.0 | Markdown rendering in terminal |
| `github.com/BurntSushi/toml` | v1.6.0 | TOML config parsing (`config.toml`) |
| `github.com/bmatcuk/doublestar/v4` | v4.10.0 | Glob pattern matching (file tools) |
| `github.com/pkoukk/tiktoken-go` | v0.1.8 | Token counting for LLM context management |
| `golang.org/x/sync` | v0.21.0 | Concurrency primitives (singleflight, errgroup) |
| `github.com/godbus/dbus/v5` | v5.2.2 | Linux D-Bus Secret Service (keychain) |
| `github.com/mattn/go-runewidth` | v0.0.19 | Unicode character width calculation |

**Infrastructure (Direct):**
| Package | Version | Purpose |
|---------|---------|---------|
| `github.com/fsnotify/fsnotify` | v1.10.1 | File watching (config reload, session resume) |
| `github.com/odvcencio/gotreesitter` | v0.20.5 | Tree-sitter bindings (code complexity analysis) |
| `github.com/atotto/clipboard` | v0.1.4 | Cross-platform clipboard access |

**Notable Indirect Dependencies:**
- `github.com/yuin/goldmark` v1.5.2 — Markdown parsing (via glamour)
- `golang.org/x/net` v0.56.0 — HTTP/2, websockets (via providers)
- `golang.org/x/text` v0.38.0 — Text processing
- `github.com/google/uuid` v1.3.0 — Session IDs

## Configuration

**Environment:**
- `.env` files loaded at startup (`internal/config/loader.go:LoadDotEnv()`)
- `.env.example` documents required vars (`OPENROUTER_API_KEY`, `ZEN_API_KEY`, `NVIDIA_API_KEY`)
- `.env.test` for test environment (gitignored)

**Build:**
- `Makefile` — Primary build script (321 lines)
- `.goreleaser.yaml` — Release automation (76 lines)
- Build flags: `-trimpath`, `-ldflags "-s -w -X main.Version=..."`
- `CGO_ENABLED=0` enforced everywhere

**Config File:**
- TOML at `~/.m31a/config.toml` (or `$M31A_CONFIG`)
- Schema in `internal/config/types.go` (548 lines)

## Platform Requirements

**Development:**
- Go 1.25+
- `make`, `golangci-lint`, `goreleaser` (for releases)
- Linux: `libsecret` / `dbus` (for keychain) or `pass` CLI fallback
- macOS: `/usr/bin/security` (built-in)
- Windows: `advapi32.dll` (built-in)

**Production (Binary):**
- **Static binary** (`CGO_ENABLED=0`) — no runtime dependencies
- Targets: `linux/amd64`, `linux/arm64`, `darwin/amd64`, `darwin/arm64`, `windows/amd64`
- Windows ARM64 explicitly excluded (`.goreleaser.yaml:25-26`)
- Config dir: `~/.m31a/` (config, keychain, sessions, ledger)
- Project dir: `<cwd>/.m31a/` (sessions, backups, checkpoints)

---

*Stack analysis: 2026-07-11*