# Technology Stack

**Analysis Date:** 2026-08-04

## Languages

**Primary:**
- Go 1.25.12 - Entire application codebase (`cmd/m31a/`, `internal/`, `pkg/`)

**Secondary:**
- TOML - Configuration files (`m31a.json`, `config.toml`)
- Markdown - Documentation, prompts, ledger (`docs/`, `M31A.wiki/`)

## Runtime

**Environment:**
- Go 1.25+ (specified in `go.mod` line 2: `go 1.25.12`)
- CGO_ENABLED=0 (hard constraint for static binary builds)

**Package Manager:**
- Go Modules (`go.mod`, `go.sum`)
- Lockfile: `go.sum` present

## Frameworks

**Core:**
- Bubble Tea v1.3.0 - Terminal UI framework (Elm architecture) (`internal/ui/tui/`)
- Lipgloss v1.1.0 - Terminal styling (`internal/ui/tui/theme/`)
- Glamour v0.6.0 - Markdown rendering in terminal
- Bubbles v0.20.0 - TUI component library

**Testing:**
- Go standard library `testing` - Test framework
- `go test -race` - Race condition detection
- Coverage profiling via `-coverprofile`

**Build/Dev:**
- GoReleaser v2 - Cross-compilation and release automation (`.goreleaser.yaml`)
- golangci-lint - Code quality and linting (`.golangci.yml`)
- Make - Build automation (`Makefile`)

## Key Dependencies

**Critical:**
- `github.com/charmbracelet/bubbletea` v1.3.0 - Core TUI framework, powers the entire interactive interface
- `github.com/charmbracelet/lipgloss` v1.1.0 - Terminal styling and layout
- `github.com/charmbracelet/glamour` v0.6.0 - Markdown rendering in terminal output
- `github.com/charmbracelet/bubbles` v0.20.0 - Pre-built TUI components (text input, viewport, etc.)
- `github.com/BurntSushi/toml` v1.6.0 - TOML config file parsing (`internal/core/config/`)
- `github.com/pkoukk/tiktoken-go` v0.1.8 - Token counting for LLM context management (`internal/engine/tokens/`)
- `golang.org/x/sync` v0.22.0 - `singleflight` for deduplicating concurrent requests

**Infrastructure:**
- `github.com/godbus/dbus/v5` v5.2.2 - D-Bus Secret Service API for Linux keychain (`internal/integrations/keychain/keychain_linux.go`)
- `github.com/fsnotify/fsnotify` v1.10.1 - File system watching for config hot-reload (`internal/core/config/`)
- `github.com/bmatcuk/doublestar/v4` v4.10.0 - Glob pattern matching for file search (`internal/tools/search/`)
- `github.com/odvcencio/gotreesitter` v0.20.5 - Tree-sitter parsing for code analysis (`internal/integrations/codeintel/`)

**UI/Rendering:**
- `github.com/alecthomas/chroma` v0.10.0 - Syntax highlighting
- `github.com/mattn/go-runewidth` v0.0.19 - Unicode character width calculation
- `github.com/atotto/clipboard` v0.1.4 - System clipboard access
- `github.com/clipperhouse/uax29/v2` v2.5.0 - Unicode text segmentation

## Configuration

**Environment:**
- `.env` file loaded via `config.LoadDotEnv()` at startup (`cmd/m31a/main.go:276`)
- `.env.example` documents required variables (gitignored, only `.env.example` committed)
- `M31A_CONFIG` env var overrides default config path

**Key configs required:**
- `OPENROUTER_API_KEY` - OpenRouter API authentication
- `ZEN_API_KEY` - Zen API authentication
- `NVIDIA_API_KEY` - NVIDIA NIM API authentication

**Build:**
- `go.mod` - Module definition and dependencies
- `.golangci.yml` - Linter configuration (govet, staticcheck, errcheck, ineffassign, unused)
- `.goreleaser.yaml` - Release automation config (cross-compilation, packaging)
- `Makefile` - Build targets and automation

## Platform Requirements

**Development:**
- Go 1.25+ installed
- `CGO_ENABLED=0` (required for static builds)
- Git (for version info embedding and git integration tools)
- `golangci-lint` (for linting)
- `goreleaser` (for release builds)

**Production:**
- Static binary (no CGO, no runtime dependencies)
- Cross-compiled for: linux/{amd64,arm64}, darwin/{amd64,arm64}, windows/amd64
- Windows/arm64 excluded from cross-compilation targets

**CI/CD:**
- GitHub Actions (`.github/workflows/ci.yml`)
- Jobs: lint, test, security (govulncheck), build (matrix: os x arch), release
- Release triggered on version tags (`v*`) via GoReleaser

---

*Stack analysis: 2026-08-04*
