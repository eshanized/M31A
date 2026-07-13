# Technology Stack

**Analysis Date:** 2026-07-13

## Languages

**Primary:**
- Go 1.25.0 - Core application, all packages (`go.mod` line 3)
- CGO_ENABLED=0 enforced - static binary requirement (Makefile line 13, `.goreleaser.yaml` line 6)

**Secondary:**
- TOML - Configuration files (`internal/config/types.go`)
- Markdown - Prompt templates, documentation, plan files

## Runtime

**Environment:**
- Go 1.25+ runtime
- Static binary (no CGO, no dynamic linking)
- Cross-platform: Linux (amd64/arm64), macOS (amd64/arm64), Windows (amd64)

**Package Manager:**
- Go modules (`go.mod`, `go.sum`)
- Lockfile: `go.sum` present

## Frameworks

**Core:**
- Bubble Tea v1.3.0 - Terminal UI framework, Elm architecture (`github.com/charmbracelet/bubbletea`)
- Lipgloss v1.1.0 - Terminal styling (`github.com/charmbracelet/lipgloss`)
- Glamour v0.6.0 - Markdown rendering (`github.com/charmbracelet/glamour`)

**Testing:**
- Go standard library `testing` package - All test files
- Race detector enabled (`make test` uses `-race`)
- Benchmark support (`make bench`)

**Build/Dev:**
- GoReleaser v2 - Release automation (`.goreleaser.yaml`)
- golangci-lint v2 - Linting (`.golangci.yml`)
- goimports - Import formatting (`Makefile` line 136)

## Key Dependencies

**Critical:**
- `github.com/charmbracelet/bubbletea` v1.3.0 - TUI engine (Elm architecture)
- `github.com/charmbracelet/bubbles` v0.20.0 - UI components
- `github.com/charmbracelet/lipgloss` v1.1.0 - Terminal styling
- `github.com/charmbracelet/glamour` v0.6.0 - Markdown rendering
- `github.com/pkoukk/tiktoken-go` v0.1.8 - Token estimation for LLM context
- `github.com/godbus/dbus/v5` v5.2.2 - Linux keychain (D-Bus Secret Service)
- `golang.org/x/sync` v0.21.0 - Singleflight for deduplication
- `github.com/odvcencio/gotreesitter` v0.20.5 - Code intelligence (tree-sitter parsing)
- `github.com/fsnotify/fsnotify` v1.10.1 - Config file watching
- `github.com/BurntSushi/toml` v1.6.0 - TOML config parsing

**Infrastructure:**
- `github.com/bmatcuk/doublestar/v4` v4.10.0 - Glob pattern matching
- `github.com/alecthomas/chroma` v0.10.0 - Syntax highlighting
- `github.com/atotto/clipboard` v0.1.4 - Clipboard access
- `github.com/mattn/go-runewidth` v0.0.19 - Unicode width calculation

## Configuration

**Environment:**
- `.env.example` - Template with API key placeholders (OPENROUTER_API_KEY, ZEN_API_KEY, NVIDIA_API_KEY)
- `.env.test` - Test environment configuration
- Keys stored in OS keychain (`pkg/keychain/`) - never plaintext on disk

**Build:**
- `go.mod` - Go module definition
- `.golangci.yml` - Linter configuration (govet, staticcheck, errcheck, ineffassign, unused)
- `.goreleaser.yaml` - Release build configuration
- `Makefile` - Build, test, lint, cross-compile targets

**Application:**
- `m31a.json` - Release metadata
- Config loaded from TOML: `~/.m31a/config.toml` or `.m31a/config.toml` (project-level)
- 4-level prompt priority chain: config override → project prompts → global prompts → embedded defaults

## Platform Requirements

**Development:**
- Go 1.25+
- golangci-lint (for `make lint`)
- goimports (optional, for `make fmt`)
- Git (for version info, release builds)
- No CGO - any dependency requiring CGO breaks the build

**Production:**
- Static binary - no runtime dependencies
- Terminal with 24-bit color support (falls back gracefully)
- D-Bus or `pass` CLI on Linux for keychain
- `/usr/bin/security` on macOS for keychain
- Windows Credential Manager on Windows

**Release Targets:**
- Linux: amd64, arm64
- macOS: amd64, arm64
- Windows: amd64 (arm64 excluded)
- Package formats: tar.gz, deb, rpm, apk, archlinux, scoop

---

*Stack analysis: 2026-07-13*
