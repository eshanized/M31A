# Technology Stack

**Analysis Date:** 2026-06-11

## Languages

**Primary:**
- Go 1.24 — Entire codebase (cmd, internal, pkg)

**Secondary:**
- Bash — Install script (`install.sh`), CI workflows

## Runtime

**Environment:**
- Go runtime (no external runtime dependency)
- CGO_ENABLED=0 — static binary, no C dependencies
- Cross-compiled: linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64

**Package Manager:**
- Go modules (`go.mod`)
- Lockfile: `go.sum` (present)

## Frameworks

**TUI (Terminal User Interface):**
- `charmbracelet/bubbletea` v1.3.0 — Core TUI framework (Elm architecture)
- `charmbracelet/lipgloss` v1.1.0 — Terminal styling and layout
- `charmbracelet/bubbles` v0.20.0 — TUI components (textarea, viewport, spinner, list)
- `charmbracelet/glamour` v0.6.0 — Markdown rendering

**CLI:**
- `flag` (stdlib) — CLI flag parsing (no external CLI framework)

## Build System & Toolchain

**Build:**
- `go build` — Standard Go build
- Makefile — Primary build interface with targets:
  - `make build` — Optimized binary with ldflags (`-s -w`)
  - `make debug` — Debug binary with symbols (`-gcflags "all=-N -l"`)
  - `make cross` — Cross-compile all platforms
  - `make release` — GoReleaser snapshot
- GoReleaser v2 (`.goreleaser.yaml`) — Release automation

**Code Quality:**
- `golangci-lint` — Linter (`.golangci.yml` config)
  - Enabled linters: govet, staticcheck, errcheck, ineffassign, unused, gosimple
  - Shadow checking enabled
  - Test file exclusions for errcheck/unused
- `go vet` — Static analysis
- `goimports` — Import formatting (optional)

**Testing:**
- `go test` — Standard Go testing
- `-race` flag — Race detector enabled in CI
- `-cover` — Coverage profiling

## Key Dependencies

**Critical:**
- `github.com/charmbracelet/bubbletea` v1.3.0 — TUI architecture (single-threaded state machine)
- `github.com/charmbracelet/lipgloss` v1.1.0 — All terminal styling
- `github.com/pkoukk/tiktoken-go` v0.1.8 — Token estimation (GPT/Claude families)
- `github.com/BurntSushi/toml` v1.6.0 — Config file parsing (in maintenance mode; v2 has different API)
- `github.com/bmatcuk/doublestar/v4` v4.10.0 — Glob pattern matching with `**` support
- `github.com/godbus/dbus/v5` v5.2.2 — Linux D-Bus Secret Service integration
- `golang.org/x/sync` v0.10.0 — `singleflight.Group` for deduplicating model cache refreshes

**Infrastructure:**
- `github.com/atotto/clipboard` v0.1.4 — Clipboard access for paste operations

**Transitive (notable):**
- `github.com/alecthomas/chroma` v0.10.0 — Syntax highlighting (via glamour)
- `github.com/yuin/goldmark` v1.5.2 — Markdown parser (via glamour)
- `github.com/yuin/goldmark-emoji` v1.0.1 — Emoji support (via glamour)
- `github.com/muesli/termenv` v0.16.0 — Terminal environment detection (auto theme)
- `github.com/microcosm-cc/bluemonday` v1.0.21 — HTML sanitization (via glamour)
- `github.com/olekukonko/tablewriter` v0.0.5 — Table rendering (via glamour)
- `github.com/muesli/reflow` v0.3.0 — Text reflow (via glamour)
- `github.com/mattn/go-runewidth` v0.0.19 — Unicode width calculation
- `github.com/rivo/uniseg` v0.4.7 — Unicode segmentation

## Configuration

**Environment Variables:**
- `M31A_CONFIG` — Override config file path (default: `~/.m31a/config.toml`)
- `M31A_OPENROUTER_API_KEY` — OpenRouter API key (highest priority)
- `M31A_ZEN_API_KEY` — Zen API key (highest priority)
- `OPENROUTER_API_KEY` — Fallback env var for OpenRouter
- `ZEN_API_KEY` — Fallback env var for Zen
- `M31A_THEME` — Override theme: `dark`, `light`, `auto`
- `M31A_DEFAULT_MODEL` — Override default model
- `M31A_PROVIDER` — Override default provider
- `M31A_PERMISSION_MODE` — Override permission mode: `prompt`, `allow`, `deny`
- `M31A_COMPACT` — Enable compact mode (`true` or `1`)
- `M31A_LOG_LEVEL` — Log verbosity: `debug`, `info`, `warn`, `error`
- `M31A_LOG_FORMAT` — Log format: `json` (default), `text`

**Config File:**
- `~/.m31a/config.toml` — Primary config (TOML)
- `m31a.toml` — Project-level config (walked up from cwd, max 3 levels)
- `.env` — Auto-loaded from cwd (does not override existing vars; world-writable files skipped)

**API Key Resolution Order (first hit wins):**
1. Environment variable (`M31A_OPENROUTER_API_KEY` / `M31A_ZEN_API_KEY`)
2. Alternate env var (`OPENROUTER_API_KEY` / `ZEN_API_KEY`)
3. OS keychain (D-Bus Secret Service / pass CLI on Linux; Keychain on macOS; Credential Manager on Windows)
4. Config file field (`~/.m31a/config.toml`)

**Build Configuration:**
- `.goreleaser.yaml` — GoReleaser v2 config
- `.golangci.yml` — golangci-lint config
- `.github/workflows/ci.yml` — GitHub Actions CI

## Platform Requirements

**Development:**
- Go 1.24+
- `golangci-lint` (for linting)
- `goimports` (for formatting, optional)
- `rg` (ripgrep, optional) — Used by Grep/Glob tools for performance; falls back to pure-Go

**Production:**
- Linux, macOS, or Windows
- Terminal with 256-color or truecolor support
- D-Bus session bus (Linux, for keychain) — fallback to `pass` CLI
- `/usr/bin/security` (macOS, for keychain)
- Windows Credential Manager (Windows)

## Notable Design Constraints

- **No CGO:** Binary is fully static. Required for cross-compilation and distribution.
- **No telemetry/analytics:** Privacy-first design. No external calls except OpenRouter/Zen APIs.
- **Single-threaded TUI:** Bubble Tea runs on main goroutine. All state mutations via `Update()`. Goroutines communicate via `tea.Cmd`/`tea.Msg`.
- **No body read timeout on HTTP clients:** Streaming responses are unbounded. Only dial timeout (30s) is set.
- **No hardcoded model lists:** Models discovered dynamically from provider APIs at runtime.

---

*Stack analysis: 2026-06-11*
