# Technology Stack

**Analysis Date:** 2026-07-16

## Languages

**Primary:**
- **Go 1.25.0** — All application code (`go.mod:3`)
  - Module: `github.com/eshanized/M31A`
  - Build constraint: `CGO_ENABLED=0` (hard requirement — fully static binary)

**Secondary:**
- **Shell (POSIX/Windows Batch/PowerShell)** — Build scripts, install helpers, cross-platform file locking
- **Make** — Build orchestration (`Makefile`)

## Runtime

**Environment:**
- **Go 1.25+** (compiled binary, no runtime dependency)
- **Static linking only** — `CGO_ENABLED=0` enforced in `Makefile:13`, `.goreleaser.yaml:7`, CI

**Package Manager:**
- **Go Modules** (`go.mod`, `go.sum`)
- Lockfile: **Present** (`go.sum` — 11045 bytes, committed)

## Frameworks

**Core:**
- **Bubble Tea v1.3.0** (`github.com/charmbracelet/bubbletea`) — TUI framework (Elm architecture: Model/Update/View)
  - Strict single-threaded event loop — all state mutations via `Update(msg Msg) (Model, Cmd)`
  - Goroutines communicate via `tea.Cmd` / `tea.Msg` channels only
- **Bubbles v0.20.0** (`github.com/charmbracelet/bubbles`) — Pre-built TUI components (viewport, textarea, list, spinner, etc.)
- **Lip Gloss v1.1.0** (`github.com/charmbracelet/lipgloss`) — Declarative terminal styling (CSS-like)
- **Glamour v0.6.0** (`github.com/charmbracelet/glamour`) — Markdown rendering in terminal

**Testing:**
- **stdlib `testing`** — Unit/integration tests (`*_test.go`)
- **Race detector** — Enforced in CI and `make test` (`go test -race`)
- **Coverage** — `go test -coverprofile=coverage.out -covermode=atomic`

**Build/Dev:**
- **GoReleaser v2** (`.goreleaser.yaml`) — Cross-compilation, packaging, release automation
- **golangci-lint** (`.golangci.yml`) — Static analysis (5m timeout)
  - Enabled: `govet` (with `shadow`), `staticcheck`, `errcheck`, `ineffassign`, `unused`
  - Test files excluded from `errcheck` and `unused`

## Key Dependencies

**Critical (Direct):**

| Package | Version | Purpose |
|---------|---------|---------|
| `github.com/charmbracelet/bubbletea` | v1.3.0 | TUI runtime (Elm architecture) |
| `github.com/charmbracelet/bubbles` | v0.20.0 | TUI components (viewport, list, textarea, help, etc.) |
| `github.com/charmbracelet/lipgloss` | v1.1.0 | Terminal styling/layout |
| `github.com/charmbracelet/glamour` | v0.6.0 | Markdown → ANSI rendering |
| `github.com/pkoukk/tiktoken-go` | v0.1.8 | Token estimation (OpenAI-compatible models) |
| `github.com/BurntSushi/toml` | v1.6.0 | TOML config parsing (`config.toml`) |
| `github.com/bmatcuk/doublestar/v4` | v4.10.0 | Glob pattern matching (file tools) |
| `github.com/godbus/dbus/v5` | v5.2.2 | Linux keychain (Secret Service D-Bus) |
| `github.com/mattn/go-runewidth` | v0.0.19 | East Asian character width calc |
| `github.com/odvcencio/gotreesitter` | v0.20.5 | Tree-sitter bindings (code analysis tools) |
| `github.com/fsnotify/fsnotify` | v1.10.1 | File watching (config reload, session persistence) |
| `github.com/atotto/clipboard` | v0.1.4 | Cross-platform clipboard access |
| `golang.org/x/sync` | v0.21.0 | `singleflight` (model cache deduplication) |

**Infrastructure (Direct):**

| Package | Version | Purpose |
|---------|---------|---------|
| `github.com/alecthomas/chroma` | v0.10.0 | Syntax highlighting (code tools) |
| `golang.org/x/sys` | v0.46.0 | Low-level OS primitives (file locking, signals) |

**Indirect (Notable):**
- `github.com/yuin/goldmark` v1.5.2 — Markdown parsing (via Glamour)
- `github.com/rivo/uniseg` v0.4.7 — Unicode grapheme clustering (via Lip Gloss)
- `github.com/muesli/reflow` v0.3.0 — Text wrapping (via Bubbles)
- `github.com/google/uuid` v1.3.0 — Session IDs

## Configuration

**Environment:**
- **Primary:** `~/.m31a/config.toml` (TOML, parsed via `BurntSushi/toml`)
- **Override:** `M31A_CONFIG` env var
- **Template:** `.env.example` (API keys only)
- **Runtime:** `.env` files loaded via `config.LoadDotEnv()` (gitignored)
- **Logging:** `~/.m31a/m31a.log` (JSON, daily rotation, 7-day retention)

**Build (`Makefile`, `.goreleaser.yaml`):**
- `CGO_ENABLED=0` (static binary)
- `-trimpath` (reproducible builds)
- `-ldflags "-s -w -X main.Version=... -X main.Commit=... -X main.Date=... -X main.GoVersion=..."`
- Targets: `linux/{amd64,arm64}`, `darwin/{amd64,arm64}`, `windows/amd64`
- Excluded: `windows/arm64`

## Platform Requirements

**Development:**
- Go 1.25+
- `golangci-lint` (CI), `goreleaser` (release), `goimports` (formatting)
- `make` (GNU Make)

**Production (Binary):**
- **Linux:** glibc 2.17+ (static, no deps), or musl (via static build)
- **macOS:** 10.15+ (Catalina+)
- **Windows:** 10+ (64-bit)
- **Architectures:** x86_64, ARM64 (except Windows ARM64)
- **No CGO, no runtime deps** — single ~50MB binary

---

*Stack analysis: 2026-07-16*