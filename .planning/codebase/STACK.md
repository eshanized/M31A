# Technology Stack

**Analysis Date:** 2026-08-03

## Languages

**Primary:**
- Go 1.25.12 - Entire codebase (100% Go, CGO_ENABLED=0 for static binaries)

**Secondary:**
- None

## Runtime

**Environment:**
- Go 1.25.12 (pinned in `go.mod` and `.github/workflows/ci.yml`)
- No CGO (hard constraint: `CGO_ENABLED=0` enforced in Makefile, CI, and goreleaser)

**Package Manager:**
- Go Modules (`go.mod` at root)
- Lockfile: `go.sum` present and committed

## Frameworks

**Core:**
- Bubble Tea v1.3.0 (`github.com/charmbracelet/bubbletea`) - Terminal UI framework (Elm architecture)
- Lipgloss v1.1.0 (`github.com/charmbracelet/lipgloss`) - Terminal styling and layout
- Bubbles v0.20.0 (`github.com/charmbracelet/bubbles`) - Pre-built TUI components (spinners, text input, viewport, etc.)
- Glamour v0.6.0 (`github.com/charmbracelet/glamour`) - Markdown rendering in terminal

**Testing:**
- Go standard `testing` package (no external test framework)
- Race detector (`-race` flag) enabled by default in `make test`

**Build/Dev:**
- Make (`Makefile` at root) - Build orchestration
- GoReleaser v2 (`.goreleaser.yaml`) - Cross-compilation and release packaging
- golangci-lint (`.golangci.yml`) - Static analysis (govet, staticcheck, errcheck, ineffassign, unused)

## Key Dependencies

**Critical:**
- `github.com/charmbracelet/bubbletea` v1.3.0 - TUI runtime; entire UI built on this Elm-architecture framework
- `github.com/BurntSushi/toml` v1.6.0 - Configuration file parsing (TOML format)
- `github.com/godbus/dbus/v5` v5.2.2 - D-Bus Secret Service for Linux keychain integration
- `github.com/pkoukk/tiktoken-go` v0.1.8 - Token estimation for LLM context window management
- `golang.org/x/sync` v0.22.0 - `singleflight` for deduplicating concurrent model fetches

**Infrastructure:**
- `github.com/fsnotify/fsnotify` v1.10.1 - Config file hot-reload via filesystem watcher
- `github.com/bmatcuk/doublestar/v4` v4.10.0 - Glob pattern matching for file search tool
- `github.com/odvcencio/gotreesitter` v0.20.5 - Tree-sitter code analysis for CodeMap/CodeComplexity tools
- `github.com/alecthomas/chroma` v0.10.0 - Syntax highlighting (used by glamour)
- `golang.org/x/sys` v0.47.0 - OS-level syscalls for keychain and file operations

**Indirect/Supporting:**
- `github.com/mattn/go-runewidth` v0.0.19 - Unicode-aware string width for terminal rendering
- `github.com/atotto/clipboard` v0.1.4 - Clipboard integration for TUI copy/paste
- `github.com/yuin/goldmark` v1.8.4 - Markdown parsing (glamour dependency)
- `github.com/olekukonko/tablewriter` v0.0.5 - Table rendering in terminal
- `github.com/google/uuid` v1.3.0 - UUID generation for session IDs

## Configuration

**Environment:**
- Config file: `~/.m31a/config.toml` (global) or `m31a.toml` (project-level)
- Environment variables: `M31A_*` prefix for overrides (`M31A_CONFIG`, `M31A_THEME`, `M31A_DEFAULT_MODEL`, etc.)
- `.env` file auto-loaded from working directory (before logger init, goroutine-safe)
- Config loading priority: Defaults -> Global TOML -> Env vars -> Project TOML -> Variable substitution
- Config hot-reload via `fsnotify` with 50ms debounce (`internal/core/config/loader.go`)

**Build:**
- `Makefile` at root - All build targets (build, test, lint, cross-compile, release)
- `.goreleaser.yaml` at root - Release automation (linux/{amd64,arm64}, darwin/{amd64,arm64}, windows/{amd64})
- `.golangci.yml` at root - Linter configuration
- LDFLAGS inject Version, Commit, Date, GoVersion at build time

**Key Config Files:**
- `go.mod` / `go.sum` - Dependency management
- `Makefile` - Build orchestration
- `.goreleaser.yaml` - Release pipeline
- `.golangci.yml` - Linter settings
- `.env.example` - Environment variable template

## Platform Requirements

**Development:**
- Go 1.25+ (matching `go.mod` directive)
- `CGO_ENABLED=0` (mandatory for static binary)
- `golangci-lint` (for `make lint`)
- `goimports` (optional, for `make fmt`)
- `goreleaser` (for `make release`)

**Production:**
- Static binary (no runtime dependencies)
- Cross-compiled: linux/{amd64,arm64}, darwin/{amd64,arm64}, windows/amd64
- Terminal with Unicode support (optional: ASCII fallback mode available)
- D-Bus session bus or `pass` CLI (Linux keychain, optional)
- macOS Keychain or Windows Credential Manager (platform-native, optional)

**Supported Platforms:**
- Linux amd64/arm64
- macOS amd64/arm64 (Intel/Apple Silicon)
- Windows amd64

---

*Stack analysis: 2026-08-03*
