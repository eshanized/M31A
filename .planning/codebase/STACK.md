# Technology Stack

**Analysis Date:** 2026-06-12

## Languages

**Primary:**
- Go 1.24 — Core application language, all source code (`cmd/m31a/main.go`, `internal/`, `pkg/`)

**Secondary:**
- Bash — Install script (`install.sh`), CI/CD workflow (`.github/workflows/ci.yml`)
- YAML — CI/CD configuration, GoReleaser release config (`.goreleaser.yaml`)

## Runtime

**Environment:**
- Go 1.24 runtime (specified in `go.mod:3` and `.github/workflows/ci.yml:18`)
- Built with `CGO_ENABLED=0` for static binaries (`Makefile:19`)
- Cross-compiled for linux/darwin/windows on amd64/arm64 (`Makefile:29-30`)

**Package Manager:**
- Go modules (`go.mod` / `go.sum`)
- Lockfile: `go.sum` present (committed)

## Frameworks

**Core:**
- Bubble Tea (v1.3.0) — Terminal UI framework (`internal/tui/app.go`, all `*_model.go` files)
  - Module: `github.com/charmbracelet/bubbletea`
- Lipgloss (v1.1.0) — TUI styling (`internal/tui/theme/`, `internal/tokens/estimator.go`)
  - Module: `github.com/charmbracelet/lipgloss`
- Bubbles (v0.20.0) — Reusable TUI components (`internal/tui/components/`)
  - Module: `github.com/charmbracelet/bubbles`
- Glamour (v0.6.0) — Markdown rendering in TUI
  - Module: `github.com/charmbracelet/glamour`

**Testing:**
- Go standard library `testing` package — Unit tests (all `*_test.go` files)
- Race detector enabled in CI (`-race` flag, `.github/workflows/ci.yml:48`)

**Build/Dev:**
- GoReleaser (v2) — Release automation (`.goreleaser.yaml`)
- golangci-lint — Linting with govet, staticcheck, errcheck, ineffassign, unused, gosimple (`.golangci.yml`)
- goimports — Code formatting (`Makefile:126`)

## Key Dependencies

**Critical:**
- `github.com/charmbracelet/bubbletea` v1.3.0 — Entire TUI architecture is Bubble Tea
- `github.com/charmbracelet/lipgloss` v1.1.0 — All TUI styling and theming
- `github.com/BurntSushi/toml` v1.6.0 — Config file parsing (`internal/config/loader.go`)
  - NOTE: In maintenance mode; v2 has different API (see `go.mod:6` DEP-3 comment)
- `github.com/pkoukk/tiktoken-go` v0.1.8 — Token estimation for context window management (`internal/tokens/estimator.go`)
  - NOTE: Unmaintained since 2024; new tokenizers may not be recognized

**Infrastructure:**
- `github.com/bmatcuk/doublestar/v4` v4.10.0 — Glob pattern matching (`internal/tools/glob.go`, `internal/tools/grep.go`)
  - NOTE: Pin current version; check for breaking changes before upgrading (see `go.mod:8` DEP-2)
- `github.com/godbus/dbus/v5` v5.2.2 — Linux D-Bus Secret Service for keychain (`pkg/keychain/keychain_linux.go`)
- `golang.org/x/sync` v0.10.0 — Singleflight pattern for concurrent deduplication
  - NOTE: Stable x/ package, appropriate usage (see `go.mod:17` DEP-1)
- `github.com/atotto/clipboard` v0.1.4 — Clipboard access for TUI

**Transitive (notable):**
- `github.com/alecthomas/chroma` v0.10.0 — Syntax highlighting (via glamour)
- `github.com/microcosm-cc/bluemonday` v1.0.21 — HTML sanitization
- `github.com/yuin/goldmark` v1.5.2 — Markdown parsing (via glamour)
- `github.com/olekukonko/tablewriter` v0.0.5 — Table rendering

## Configuration

**Environment:**
- Config file: `~/.m31a/config.toml` (TOML format, parsed in `internal/config/loader.go`)
- Project-level config: `m31a.toml` in working directory (walks up 3 parent dirs, `internal/config/loader.go:197-211`)
- Environment variable overrides via `M31A_*` prefix (`internal/config/loader.go:124-138`)
- `.env` file loading from CWD (`internal/config/loader.go:897-941`)
- Custom config path: `M31A_CONFIG` env var (`internal/config/loader.go:93`)

**Required env vars:**
- `M31A_OPENROUTER_API_KEY` or `OPENROUTER_API_KEY` — OpenRouter API key
- `M31A_ZEN_API_KEY` or `ZEN_API_KEY` — Zen API key
- `M31A_THEME` — UI theme override (dark/light/auto)
- `M31A_DEFAULT_MODEL` — Default model override
- `M31A_PROVIDER` — Default provider override
- `M31A_PERMISSION_MODE` — Permission mode override (prompt/allow/deny)
- `M31A_COMPACT` — Compact mode toggle

**Build:**
- `Makefile` — Build, test, lint, release targets (`Makefile`)
- `.goreleaser.yaml` — Release configuration for GitHub releases (`.goreleaser.yaml`)
- `.golangci.yml` — Linter configuration (`.golangci.yml`)
- `.github/workflows/ci.yml` — GitHub Actions CI pipeline

## Platform Requirements

**Development:**
- Go 1.24+
- `git` CLI (used by `internal/git/git.go` for version control operations)
- `golangci-lint` (for linting)
- `goimports` (for formatting, optional)
- `ripgrep` (optional, for enhanced Glob/Grep tool performance)

**Production:**
- No CGO dependency (`CGO_ENABLED=0`)
- Static binary for linux/darwin/windows on amd64/arm64
- D-Bus Secret Service or `pass` CLI on Linux for keychain (`pkg/keychain/keychain_linux.go`)
- macOS Keychain CLI (`/usr/bin/security`) on macOS (`pkg/keychain/keychain_darwin.go`)
- Windows Credential Manager on Windows (`pkg/keychain/keychain_windows.go`)

## Build System

**Build Commands:**
```bash
make build        # Build optimized binary for current platform
make debug        # Build with debug symbols
make test         # Run all tests with race detector and coverage
make lint         # Run golangci-lint
make cross        # Build for all platforms (linux/darwin/windows × amd64/arm64)
make release      # Run GoReleaser snapshot release
```

**Build Output:**
- Binary: `m31a` (or `m31a-debug` for debug builds)
- Cross-compiled: `dist/m31a-{os}-{arch}[.exe]`
- Coverage: `coverage.out`, `coverage.html`

---

*Stack analysis: 2026-06-12*
