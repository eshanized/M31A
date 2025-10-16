# Technology Stack

**Analysis Date:** 2026-06-07

## Languages

**Primary:**
- Go 1.24 — All application code (`cmd/m31a/`, `internal/`, `pkg/`). Module: `github.com/eshanized/M31A`

**Secondary:**
- YAML — CI/CD workflows (`.github/workflows/ci.yml`), goreleaser config (`.goreleaser.yaml`)
- TOML — Configuration format (`~/.m31a/config.toml`), parsed via `BurntSushi/toml`
- Markdown — Planning files, ledger, documentation

## Runtime

**Environment:**
- Go 1.24+ (specified in `go.mod`)
- Static binary: `CGO_ENABLED=0` enforced in all builds (`Makefile`, `.goreleaser.yaml`, CI)
- Cross-compiled for: linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64

**Package Manager:**
- Go Modules (`go.mod` / `go.sum`)
- Lockfile: `go.sum` present and committed

## Frameworks

**Core:**
- Bubble Tea v1.3.0 — Terminal UI framework (single-threaded event loop, `tea.Model`, `Update()`, `View()`)
  - Location: `internal/tui/`
- Lipgloss v1.1.0 — Terminal styling (colors, borders, layout)
  - Location: `internal/tui/theme/`
- Bubbles v0.20.0 — TUI components (textarea, viewport, spinner, list)
  - Location: `internal/tui/components/`
- Glamour v0.6.0 — Markdown rendering in terminal
  - Location: `internal/tui/` (rendering pipeline)

**Testing:**
- Go standard testing (`go test`) — All test files use `*_test.go` convention
- Race detector: `go test -race` enforced in CI
- Coverage: `go test -coverprofile=coverage.out`

**Build/Dev:**
- Makefile — Build, test, lint, cross-compile, release targets
- GoReleaser v2 — Automated release builds (`.goreleaser.yaml`)
- golangci-lint — Linting (`.golangci.yml`) with govet, staticcheck, errcheck, ineffassign, unused, gosimple

## Key Dependencies

**Critical:**
- `github.com/charmbracelet/bubbletea` v1.3.0 — TUI framework; all screens and state management
- `github.com/charmbracelet/lipgloss` v1.1.0 — Terminal styling; theme system
- `github.com/charmbracelet/bubbles` v0.20.0 — Reusable TUI components
- `github.com/charmbracelet/glamour` v0.6.0 — Markdown rendering
- `github.com/BurntSushi/toml` v1.6.0 — Config file parsing (in maintenance mode; DEP-3 noted)
- `github.com/pkoukk/tiktoken-go` v0.1.8 — Token estimation for GPT/Claude families (pinned; unmaintained upstream)
- `github.com/bmatcuk/doublestar/v4` v4.10.0 — Glob pattern matching with `**` support
- `golang.org/x/sync` v0.10.0 — `singleflight` for model cache deduplication

**Infrastructure:**
- `github.com/godbus/dbus/v5` v5.2.2 — D-Bus Secret Service for Linux keychain
- `github.com/mattn/go-runewidth` v0.0.19 — Unicode character width calculation for TUI layout
- `github.com/alecthomas/chroma` v0.10.0 — Syntax highlighting (indirect, via glamour)
- `github.com/microcosm-cc/bluemonday` v1.0.21 — HTML sanitization (indirect, via glamour)

**Security:**
- SSRF protection via DNS caching and private IP blocking in `internal/tools/webfetch.go`
- OS keychain integration: Linux (D-Bus Secret Service + `pass` CLI fallback), macOS (`/usr/bin/security`), Windows (Credential Manager via `advapi32.dll`)

## Configuration

**Environment Variables:**
- `M31A_OPENROUTER_API_KEY` — OpenRouter API key (highest priority)
- `M31A_ZEN_API_KEY` — Zen API key (highest priority)
- `M31A_CONFIG` — Override config file path
- `M31A_THEME` — Override theme setting
- `M31A_DEFAULT_MODEL` — Override default model
- `M31A_LOG_FORMAT` — Log output format (default: `json`)
- `M31A_LOG_LEVEL` — Log verbosity level (default: `info`)

**Config File:**
- `~/.m31a/config.toml` — Main config file (TOML format)
- Project-level: `m31a.toml` (walked up from cwd, max 3 levels)
- Multi-layer merge: defaults → global TOML → env vars → project TOML → `${VAR}` substitution

**Build Config:**
- `Makefile` — Primary build orchestration
- `.goreleaser.yaml` — Release configuration
- `.golangci.yml` — Linter configuration
- `.github/workflows/ci.yml` — CI pipeline

## Platform Requirements

**Development:**
- Go 1.24+
- `golangci-lint` (for linting)
- Git (for version info embedding via `ldflags`)
- `gofmt` (for formatting checks)

**Production:**
- Static binary (no runtime dependencies)
- `CGO_ENABLED=0` — No C dependencies
- Terminal with ANSI color support (uses `termenv` for detection)
- Git (required for workflow operations, bisect, rollback)

**Cross-Platform:**
- Linux: D-Bus Secret Service or `pass` CLI for keychain
- macOS: `/usr/bin/security` CLI for keychain
- Windows: `advapi32.dll` (Credential Manager) for keychain

---

*Stack analysis: 2026-06-07*
