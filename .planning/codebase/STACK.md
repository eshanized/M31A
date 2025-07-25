# Technology Stack

**Analysis Date:** 2026-06-04

## Languages

**Primary:**
- Go 1.24 - All application code (binary entry point, TUI, providers, tools, workflow engine, packages)

**Secondary:**
- TOML - Configuration files (`~/.m31a/config.toml`, `m31a.toml`)
- YAML - CI/CD (`.github/workflows/ci.yml`), release config (`.goreleaser.yaml`)
- Markdown - Session planning files, ledger, documentation

## Runtime

**Environment:**
- Go 1.24+ (as specified in `go.mod`)
- CGO_ENABLED=0 — static binary, no C dependencies
- Cross-platform: linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64

**Package Manager:**
- Go modules (`go.mod`, `go.sum`)
- Lockfile: present (`go.sum`, 120 lines)

## Frameworks

**Core:**
- `charmbracelet/bubbletea` v1.3.0 — Terminal UI framework (Elm-architecture state machine)
- `charmbracelet/lipgloss` v1.1.0 — Terminal styling (colors, layout, borders)
- `charmbracelet/bubbles` v0.20.0 — TUI components (textarea, viewport, spinner, list)
- `charmbracelet/glamour` v0.6.0 — Markdown rendering in terminal

**Testing:**
- Standard `go test` — Unit and integration tests
- `go test -race` — Race condition detection
- Test files co-located with source (e.g., `*_test.go` alongside `*.go`)

**Build/Dev:**
- `goreleaser` v2 — Release automation (cross-compilation, archives, checksums)
- `golangci-lint` — Linting (govet, staticcheck, errcheck, ineffassign, unused, gosimple)
- `goimports` — Import formatting
- Makefile — Build, test, lint, cross-compile, release targets

## Key Dependencies

**Critical:**
- `charmbracelet/bubbletea` v1.3.0 — TUI framework; all screens implemented as Bubble Tea models
- `charmbracelet/lipgloss` v1.1.0 — Terminal styling; theme system, component styling
- `charmbracelet/bubbles` v0.20.0 — TUI components; textarea, viewport, spinner, list
- `charmbracelet/glamour` v0.6.0 — Markdown rendering for assistant responses and tool output

**Infrastructure:**
- `github.com/BurntSushi/toml` v1.6.0 — TOML config file parsing (`~/.m31a/config.toml`)
- `github.com/bmatcuk/doublestar/v4` v4.10.0 — Glob pattern matching with `**` support (Glob tool, Grep tool)
- `github.com/pkoukk/tiktoken-go` v0.1.8 — Token estimation for GPT/Claude model families
- `github.com/godbus/dbus/v5` v5.2.2 — D-Bus Secret Service integration for Linux keychain
- `golang.org/x/sync` v0.10.0 — `singleflight` for model cache refresh deduplication
- `github.com/mattn/go-runewidth` v0.0.19 — Unicode character width calculation for TUI

**Transitive (indirect):**
- `github.com/alecthomas/chroma` v0.10.0 — Syntax highlighting (via glamour)
- `github.com/muesli/termenv` v0.16.0 — Terminal capability detection (auto theme)
- `github.com/yuin/goldmark` v1.5.2 — Markdown parser (via glamour)
- `github.com/microcosm-cc/bluemonday` v1.0.21 — HTML sanitization (via glamour)
- `github.com/sahilm/fuzzy` v0.1.1 — Fuzzy matching (model selector)
- `github.com/clipperhouse/uax29/v2` v2.5.0 — Unicode text segmentation
- `golang.org/x/net` v0.0.0-20221002022538 — Network utilities

## Configuration

**Environment:**
- `~/.m31a/config.toml` — Primary config file (TOML format)
- `m31a.toml` — Project-level config (walked up from cwd, max 3 levels)
- `M31A_CONFIG` env var — Override config file path
- `M31A_THEME` env var — Override theme setting
- `M31A_DEFAULT_MODEL` env var — Override default model
- `M31A_OPENROUTER_API_KEY` env var — OpenRouter API key
- `M31A_ZEN_API_KEY` env var — Zen API key

**Config resolution order:**
1. `DefaultConfig()` — Zero-valued defaults
2. Global TOML (`~/.m31a/config.toml`)
3. Environment variable overrides (`M31A_*`)
4. Project-level `m31a.toml` (walked up from cwd, max 3 levels)
5. Variable substitution — `${VAR}` → env value
6. Validation — type/range checks on known fields

**Key configs required:**
- `provider.default` — Active provider name ("openrouter" or "zen")
- `provider.openrouter.api_key` or `M31A_OPENROUTER_API_KEY` — OpenRouter auth
- `provider.zen.api_key` or `M31A_ZEN_API_KEY` — Zen auth
- `model.default` — Default model ID
- `ui.theme` — Theme ("dark", "light", "auto")

**Build:**
- `Makefile` — Primary build interface
- `.goreleaser.yaml` — Release configuration (cross-compilation, archives)
- `.golangci.yml` — Linter configuration
- `.github/workflows/ci.yml` — CI pipeline

**Build commands:**
```bash
CGO_ENABLED=0 go build -o m31a ./cmd/m31a     # Build binary
go test -race -cover ./...                       # Run tests
golangci-lint run ./... --timeout=5m             # Lint
go vet ./...                                     # Vet
goreleaser release --snapshot --clean            # Release (snapshot)
```

## Platform Requirements

**Development:**
- Go 1.24+ installed
- `golangci-lint` for linting
- `goreleaser` for releases
- `rg` (ripgrep) optional — Grep tool falls back to pure-Go implementation
- `git` — Required for git operations (bisect, rollback, workflow)
- `pass` CLI optional — Linux keychain fallback (D-Bus Secret Service is primary)

**Production:**
- Static binary (CGO_ENABLED=0) — no runtime dependencies
- Terminal with 256-color and Unicode support
- `git` installed — Required for workflow engine (commits, bisect, rollback)
- D-Bus session bus or `pass` CLI — For API key storage on Linux

**CI/CD:**
- GitHub Actions with `ubuntu-latest`, `macos-latest`, `windows-latest`
- Go 1.22 specified in CI (minimum compatible version)
- `goreleaser-action@v5` for release builds
- `GITHUB_TOKEN` secret for GitHub releases

---

*Stack analysis: 2026-06-04*
