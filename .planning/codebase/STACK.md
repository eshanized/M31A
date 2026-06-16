# Technology Stack

**Analysis Date:** 2026-06-16

## Languages

**Primary:**
- Go 1.25 - Entire codebase, all components

**Secondary:**
- TOML - Configuration files (`config.toml`, `m31a.toml`)
- Markdown - Documentation, prompts, ledger entries

## Runtime

**Environment:**
- Go 1.25 runtime
- CGO_ENABLED=0 - Fully static binary, no C dependencies
- Binary size: ~15-20MB (stripped with `-s -w` ldflags)

**Package Manager:**
- Go modules (`go.mod` / `go.sum`)
- Lockfile: Present (`go.sum`)

## Frameworks

**Core:**
- Bubble Tea v1.3.0 - TUI application framework (Elm architecture)
- Lip Gloss v1.1.0 - Terminal styling and layout
- Bubbles v0.20.0 - Pre-built TUI components (text input, viewport, spinner)
- Glamour v0.6.0 - Markdown rendering in terminal

**Testing:**
- Go standard `testing` package - No external test frameworks
- Table-driven tests with anonymous structs
- `t.Parallel()` for concurrent test execution
- `t.TempDir()` for filesystem isolation

**Build/Dev:**
- Makefile - Build, test, lint, cross-compile, release targets
- GoReleaser v2 - Cross-platform release automation
- golangci-lint - Static analysis (govet, staticcheck, errcheck, ineffassign, unused)

## Key Dependencies

**Critical:**
- `github.com/charmbracelet/bubbletea` v1.3.0 - Core TUI framework
- `github.com/charmbracelet/lipgloss` v1.1.0 - Terminal styling
- `github.com/charmbracelet/bubbles` v0.20.0 - TUI components
- `github.com/charmbracelet/glamour` v0.6.0 - Markdown rendering

**Infrastructure:**
- `github.com/BurntSushi/toml` v1.6.0 - TOML config parsing
- `github.com/pkoukk/tiktoken-go` v0.1.8 - Token estimation for context window management
- `github.com/godbus/dbus/v5` v5.2.2 - Linux D-Bus Secret Service for API key storage
- `github.com/fsnotify/fsnotify` v1.10.1 - Config file hot-reload watching
- `github.com/bmatcuk/doublestar/v4` v4.10.0 - Extended glob pattern matching
- `github.com/atotto/clipboard` v0.1.4 - System clipboard access
- `golang.org/x/sync` v0.21.0 - `singleflight` for deduplicating concurrent requests

**Code Quality:**
- `github.com/alecthomas/chroma` v0.10.0 - Syntax highlighting

## Configuration

**Environment Variables:**
- `M31A_CONFIG` - Override config file path
- `M31A_LOG_LEVEL` - debug/info/warn/error
- `M31A_LOG_FORMAT` - json/text
- `M31A_THEME` - Override UI theme (dark/light/auto)
- `M31A_DEFAULT_MODEL` - Override default model
- `M31A_PROVIDER` - Override default provider
- `M31A_PERMISSION_MODE` - Override permission mode (prompt/allow/deny)
- `M31A_COMPACT` - Enable compact mode (true/1)
- `M31A_OPENROUTER_API_KEY` / `OPENROUTER_API_KEY` - OpenRouter API key
- `M31A_ZEN_API_KEY` / `ZEN_API_KEY` - Zen API key

**Config Files:**
- Global: `~/.m31a/config.toml` (override with `M31A_CONFIG`)
- Project: `m31a.toml` in cwd (walks up 3 parent directories)
- Environment: `.env` file in cwd (loaded before config)

**Build Configuration:**
- `Makefile` - Build targets (`make build`, `make test`, `make lint`)
- `.goreleaser.yaml` - Release configuration
- `.golangci.yml` - Linter configuration

## Platform Requirements

**Development:**
- Go 1.25+
- POSIX shell (bash, zsh, fish)
- Git 2.0+
- golangci-lint (for linting)

**Production:**
- Linux (amd64, arm64)
- macOS (amd64, arm64)
- Windows (amd64 only)
- Static binary, no runtime dependencies

## Build Targets

**Cross-Compilation:**
- `linux/amd64`, `linux/arm64`
- `darwin/amd64`, `darwin/arm64`
- `windows/amd64` (arm64 excluded)

**Build Flags:**
- `CGO_ENABLED=0` - Static binary
- `-s -w` - Strip debug info
- `-X main.Version=$(VERSION)` - Embed version

---

*Stack analysis: 2026-06-16*
