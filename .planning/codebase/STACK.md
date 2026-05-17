# Technology Stack

**Analysis Date:** 2026-06-14

## Languages

**Primary:**
- Go 1.24 - Core application language (`go.mod:3`)

**Secondary:**
- Markdown - Prompt templates (`internal/workflow/prompts/*.md`)

## Runtime

**Environment:**
- Go runtime 1.24+ (required by `go.mod`)
- CGO_ENABLED=0 (static binaries)

**Package Manager:**
- Go modules (`go.mod` / `go.sum`)
- Lockfile: present (`go.sum`)

## Frameworks

**Core:**
- Bubble Tea v1.3.0 - Terminal UI framework (TUI) (`internal/tui/app.go`)
- Lipgloss v1.1.0 - Terminal styling (`internal/tui/theme/`)
- Bubbles v0.20.0 - TUI components (`internal/tui/components/`)

**Testing:**
- Go standard testing package (`go test`)
- Race detector support via `-race` flag

**Build/Dev:**
- Makefile - Build orchestration (`Makefile`)
- GoReleaser v2 - Release automation (`.goreleaser.yaml`)
- golangci-lint - Static analysis (`.golangci.yml`)

## Key Dependencies

**Critical:**
- `charmbracelet/bubbletea` v1.3.0 - Core TUI framework
- `charmbracelet/lipgloss` v1.1.0 - Terminal styling
- `charmbracelet/glamour` v0.6.0 - Markdown rendering
- `charmbracelet/bubbles` v0.20.0 - Reusable TUI components
- `godbus/dbus/v5` v5.2.2 - Linux D-Bus Secret Service API (`pkg/keychain/keychain_linux.go`)
- `BurntSushi/toml` v1.6.0 - TOML config parsing (`internal/config/loader.go:16`)
- `bmatcuk/doublestar/v4` v4.10.0 - Glob pattern matching (`internal/tools/glob.go`)
- `fsnotify/fsnotify` v1.10.1 - File system watching for config reload (`internal/config/loader.go:20`)

**Token Estimation:**
- `pkoukk/tiktoken-go` v0.1.8 - Token counting for OpenAI models (`internal/tokens/estimator.go:12`)

**Concurrency:**
- `golang.org/x/sync` v0.10.0 - Singleflight for deduplication (`go.mod:18`)

**TUI Support:**
- `charmbracelet/x/ansi` v0.8.0 - ANSI escape sequence handling
- `charmbracelet/x/term` v0.2.1 - Terminal detection
- `charmbracelet/colorprofile` v0.2.3 - Color profile detection
- `mattn/go-runewidth` v0.0.19 - Character width calculation
- `mattn/go-isatty` v0.0.20 - TTY detection

**Markdown/Content:**
- `yuin/goldmark` v1.5.2 - Markdown parser
- `yuin/goldmark-emoji` v1.0.1 - Emoji support
- `microcosm-cc/bluemonday` v1.0.21 - HTML sanitization

**Platform:**
- `atotto/clipboard` v0.1.4 - Clipboard access

## Configuration

**Build:**
- `go.mod` - Module definition and dependencies
- `.golangci.yml` - Linter configuration
- `.goreleaser.yaml` - Release configuration
- `Makefile` - Build targets and cross-compilation

**Runtime Configuration:**
- `~/.m31a/config.toml` - Global configuration (TOML format)
- `m31a.toml` - Project-level configuration (walked up from cwd, max 3 levels)
- `.env` - Environment variables (auto-loaded from cwd)

**Environment Variables (key ones):**
- `M31A_CONFIG` - Override config path
- `M31A_OPENROUTER_API_KEY` / `OPENROUTER_API_KEY` - OpenRouter API key
- `M31A_ZEN_API_KEY` / `ZEN_API_KEY` - Zen API key
- `M31A_PROVIDER` - Default provider
- `M31A_DEFAULT_MODEL` - Default model
- `M31A_THEME` - Theme (dark/light/auto)
- `M31A_PERMISSION_MODE` - Permission mode (prompt/allow/deny)

## Platform Requirements

**Development:**
- Go 1.24+
- git
- make (optional, for Makefile targets)

**Production/Release:**
- Cross-compilation targets: linux, darwin, windows (amd64 + arm64)
- Static binaries (CGO_ENABLED=0)
- No external runtime dependencies

**Build Targets:**
- `make build` - Current platform optimized binary
- `make debug` - Debug binary with symbols
- `make cross` - All platforms
- `make test` - Tests with race detector + coverage

## Version Pinning

**Pinned Dependencies (with upgrade notes):**
- `BurntSushi/toml` v1.6.0 - In maintenance mode; v2 has different API (`go.mod:6`)
- `bmatcuk/doublestar/v4` v4.10.0 - Pin current version; check for breaking changes before upgrading (`go.mod:8`)
- `pkoukk/tiktoken-go` v0.1.8 - Unmaintained since 2024; newer tokenizers may not be recognized (`internal/tokens/estimator.go:14-15`)

## Key Metrics

- **Binary size:** Built with `-s -w` ldflags (stripped)
- **Test coverage:** `make test` generates `coverage.out`
- **Linting:** golangci-lint with govet, staticcheck, errcheck, ineffassign, unused, gosimple

---

*Stack analysis: 2026-06-14*
