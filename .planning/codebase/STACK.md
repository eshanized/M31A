# Technology Stack

**Analysis Date:** 2026-07-10

## Languages

**Primary:**
- Go 1.25 - Entire application: CLI entrypoint, TUI, workflow engine, providers, tools, all packages

**Secondary:**
- Bash - `install.sh` installer script, shell tool execution (`internal/shell/shell_unix.go`)
- YAML - CI/CD configuration (`go.sum`, `.github/workflows/ci.yml`)
- TOML - Application configuration format (`~/.m31a/config.toml`)

## Runtime

**Environment:**
- Go 1.25 runtime (`go.mod` line 3)
- Build constraint: `CGO_ENABLED=0` (hard constraint - static binary, no CGO)

**Package Manager:**
- Go Modules (`go.mod`, `go.sum`)
- Lockfile: present (`go.sum`)

**Cross-compilation targets:**
- linux/{amd64,arm64}
- darwin/{amd64,arm64}
- windows/amd64 (windows/arm64 excluded)

## Frameworks

**Core:**
- Bubble Tea (Elm architecture) `github.com/charmbracelet/bubbletea` v1.3.0 - Terminal UI framework. All state mutations go through `Update()` only. Single-threaded contract enforced.
- Lip Gloss `github.com/charmbracelet/lipgloss` v1.1.0 - Terminal styling and layout
- Bubbles `github.com/charmbracelet/bubbles` v0.20.0 - Pre-built TUI components (spinners, text inputs, etc.)

**Testing:**
- Go standard library `testing` - Unit and integration tests
- `go test -race` - Race detector enabled in CI

**Build/Dev:**
- GoReleaser `goreleaser` - Cross-compilation, releases, package formats (deb, rpm, apk, archlinux, scoop)
- golangci-lint - Linting (govet, staticcheck, errcheck, ineffassign, unused)
- `goimports` - Import formatting

## Key Dependencies

**Critical:**
- `github.com/charmbracelet/bubbletea` v1.3.0 - TUI framework (core UI loop)
- `github.com/charmbracelet/lipgloss` v1.1.0 - Terminal styling
- `github.com/charmbracelet/glamour` v0.6.0 - Markdown rendering in terminal
- `github.com/pkoukk/tiktoken-go` v0.1.8 - Token estimation (OpenAI tiktoken-compatible)
- `github.com/BurntSushi/toml` v1.6.0 - TOML config parsing
- `github.com/godbus/dbus/v5` v5.2.2 - D-Bus Secret Service (Linux keychain)
- `github.com/fsnotify/fsnotify` v1.10.1 - Config file hot-reload watching
- `github.com/odvcencio/gotreesitter` v0.20.5 - Tree-sitter for code intelligence (`internal/codeintel/`)
- `github.com/bmatcuk/doublestar/v4` v4.10.0 - Glob pattern matching

**Infrastructure:**
- `golang.org/x/sync` v0.21.0 - `singleflight` for deduplicating concurrent requests
- `github.com/mattn/go-runewidth` v0.0.19 - Terminal width calculations
- `github.com/alecthomas/chroma` v0.10.0 - Syntax highlighting
- `github.com/mattn/go-isatty` v0.0.20 - TTY detection

## Configuration

**Environment:**
- `.env` file loaded at startup via `config.LoadDotEnv()` (called before logger init)
- Environment variable override: `M31A_CONFIG` for custom config path
- Log format: `M31A_LOG_FORMAT` (json/text)
- Log level: `M31A_LOG_LEVEL` (debug/info/warn/error)

**Required env vars (for API access):**
- `OPENROUTER_API_KEY` - OpenRouter provider
- `ZEN_API_KEY` - Zen provider
- `NVIDIA_API_KEY` - NVIDIA NIM provider

**Config files:**
- `~/.m31a/config.toml` - Global configuration (TOML format)
- `m31a.json` - Release manifest (version, download URLs, hashes)
- `.m31a/` - Per-project runtime data (sessions, backups)

**Build:**
- `Makefile` - Primary build system (build, test, lint, cross-compile, release)
- `.goreleaser.yaml` - GoReleaser config for automated releases
- `.golangci.yml` - Linter configuration
- `go.mod` / `go.sum` - Go module dependencies

## Platform Requirements

**Development:**
- Go 1.25+
- Git
- `golangci-lint` (for `make lint`)
- `goimports` (for `make fmt`)

**Production:**
- Static binary (`CGO_ENABLED=0`) - no runtime dependencies
- Git (for git operations via `internal/git/git.go`)
- OS keychain (optional - D-Bus Secret Service / `pass` on Linux, `/usr/bin/security` on macOS, Windows Credential Manager)

---

*Stack analysis: 2026-07-10*
