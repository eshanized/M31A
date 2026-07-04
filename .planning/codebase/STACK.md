# Technology Stack

**Analysis Date:** 2026-07-04

## Languages

**Primary:**
- Go 1.25.0 — All source code, build target, tests

**Secondary:**
- Python 3 — Evaluation/analysis scripts in `paper/evaluate/` (Dockerfile only)
- TOML — Configuration format (`go.mod`, `.golangci.yml`, `.goreleaser.yaml`, user config)
- Shell (Makefile) — Build automation, CI/CD

## Runtime

**Environment:**
- Go 1.25+ standard library (`go 1.25.0` in `go.mod`)
- No runtime dependencies — builds to a single static binary
- Cross-platform: linux/{amd64,arm64}, darwin/{amd64,arm64}, windows/{amd64,arm64}

**Package Manager:**
- Go modules (`go mod`)
- Lockfile: `go.sum` present

## Build System

**Core Build:**
- `make build` — Optimized binary (`CGO_ENABLED=0`, static, stripped)
- `make debug` — Debug symbols (no strip)
- `make cross` — Build all platform targets

**Release:**
- GoReleaser (`.goreleaser.yaml`) — Snapshot and release builds
- Produces: tar.gz archives, deb/rpm/apk/archlinux packages, Windows Scoop manifest
- `make release` / `make release-dry`

**Build Flags:**
```
CGO_ENABLED=0        # Hard constraint — binary must be static, no CGO
-trimpath             # Reproducible builds
-ldflags "-s -w"      # Strip debug info for release
-X main.Version       # Inject version, commit, date, go version
```

**Verification Pipeline:**
```
make check    # fmt → tidy → vet → lint → test (canonical full check)
make lint     # golangci-lint standalone
make test     # Race-enabled tests with coverage
```

## Frameworks

**TUI (Terminal UI):**
- Bubble Tea (`github.com/charmbracelet/bubbletea` v1.3.0) — Elm-architecture TUI framework
- Bubbles (`github.com/charmbracelet/bubbles` v0.20.0) — TUI component library
- Lipgloss (`github.com/charmbracelet/lipgloss` v1.1.0) — Terminal styling
- Glamour (`github.com/charmbracelet/glamour` v0.6.0) — Markdown rendering in terminal

**Testing:**
- Go standard `testing` package — All tests
- Race detector (`go test -race`) — Enabled by default
- Coverage profiling (`go test -coverprofile`)

**Linting:**
- golangci-lint (`.golangci.yml`) — Configured linters: govet (shadow), staticcheck, errcheck, ineffassign, unused
- Test files excluded from `errcheck` and `unused`
- 5-minute timeout

## Key Dependencies

**TUI & Rendering:**
- `github.com/charmbracelet/bubbletea` v1.3.0 — Core TUI framework (Elm architecture)
- `github.com/charmbracelet/bubbles` v0.20.0 — Input, viewport, list, spinner components
- `github.com/charmbracelet/lipgloss` v1.1.0 — Styling and layout
- `github.com/charmbracelet/glamour` v0.6.0 — Terminal markdown rendering
- `github.com/charmbracelet/x/ansi` v0.8.0 — ANSI escape code handling (indirect)
- `github.com/charmbracelet/x/cellbuf` v0.0.13 — Cell-based terminal buffer (indirect)
- `github.com/charmbracelet/colorprofile` v0.2.3 — Color profile detection (indirect)

**Syntax & Code Intelligence:**
- `github.com/alecthomas/chroma` v0.10.0 — Syntax highlighting
- `github.com/odvcencio/gotreesitter` v0.20.5 — Tree-sitter code parsing
- `github.com/bmatcuk/doublestar/v4` v4.10.0 — Glob pattern matching

**Token & Model:**
- `github.com/pkoukk/tiktoken-go` v0.1.8 — Token counting for OpenAI-compatible models

**Configuration:**
- `github.com/BurntSushi/toml` v1.6.0 — TOML config parsing and serialization
- `github.com/fsnotify/fsnotify` v1.10.1 — Filesystem notifications for config hot-reload

**System Integration:**
- `github.com/godbus/dbus/v5` v5.2.2 — D-Bus Secret Service API (Linux keychain)
- `github.com/atotto/clipboard` v0.1.4 — System clipboard access
- `github.com/google/uuid` v1.3.0 — UUID generation (indirect)

**String Processing:**
- `github.com/mattn/go-runewidth` v0.0.19 — Unicode-aware string width (indirect)
- `github.com/clipperhouse/uax29/v2` v2.5.0 — Unicode text segmentation (indirect)
- `github.com/rivo/uniseg` v0.4.7 — Unicode text segmentation (indirect)

**HTML & Markdown:**
- `github.com/yuin/goldmark` v1.5.2 — Markdown parser (indirect, via glamour)
- `github.com/yuin/goldmark-emoji` v1.0.1 — Emoji extension (indirect)
- `github.com/microcosm-cc/bluemonday` v1.0.21 — HTML sanitizer (indirect)
- `github.com/olekukonko/tablewriter` v0.0.5 — ASCII table rendering (indirect)

**Concurrency:**
- `golang.org/x/sync` v0.21.0 — singleflight for deduplicating concurrent operations

## Configuration

**Environment:**
- `.env` file — Auto-loaded from CWD (before logger init, goroutine-safe)
- `.env.example` — Documents required API keys
- Environment variables override config file values
- API key resolution: env var → OS keychain → config file

**Required Environment Variables:**
```
OPENROUTER_API_KEY    — OpenRouter LLM provider
ZEN_API_KEY           — Zen LLM provider
NVIDIA_API_KEY        — NVIDIA NIM LLM provider
```

**Optional Environment Variables:**
```
M31A_CONFIG           — Override config file path
M31A_THEME            — Override UI theme
M31A_DEFAULT_MODEL    — Override default model
M31A_PROVIDER         — Override default provider
M31A_PERMISSION_MODE  — Override permission mode
M31A_LOG_LEVEL        — Log level (debug/info/warn/error)
M31A_LOG_FORMAT       — Log format (json/text)
M31A_COMPACT          — Enable compact mode
M31A_OPENROUTER_API_KEY  — OpenRouter key (M31A_ prefix)
M31A_ZEN_API_KEY         — Zen key (M31A_ prefix)
M31A_NVIDIA_API_KEY      — NVIDIA key (M31A_ prefix)
```

**Config Files:**
- `~/.m31a/config.toml` — Global config (loaded on startup)
- `m31a.toml` — Project-level config (walked up from CWD, max 3 levels)
- `m31a.json` — Release metadata (version, architecture, hashes)

**Config Loading Order (later overrides earlier):**
1. `DefaultConfig()` — Zero-valued defaults
2. Global TOML (`~/.m31a/config.toml`)
3. Environment variable overrides (`M31A_*`)
4. Project-level `m31a.toml` (walked up from CWD)
5. Variable substitution (`${VAR}` → env value)
6. Validation (type/range checks)

**Build Config Files:**
- `go.mod` / `go.sum` — Go module dependencies
- `.golangci.yml` — Linter configuration
- `.goreleaser.yaml` — Release automation
- `Makefile` — Build, test, lint, cross-compile targets
- `Dockerfile` — EFIE evaluation Docker image (builds with Go 1.24-alpine)

## Platform Requirements

**Development:**
- Go 1.25+
- git CLI (required for git operations, branch management, bisect)
- `CGO_ENABLED=0` (hard constraint — all dependencies must be pure Go)
- On Linux: D-Bus session bus OR `pass` CLI for keychain
- On macOS: `/usr/bin/security` CLI for keychain
- On Windows: Windows Credential Manager

**Production:**
- Single static binary (no runtime dependencies)
- Git CLI available in PATH (for git-wrapping features)
- Terminal with ANSI support (256-color or truecolor)

**CI/CD:**
- Go 1.25+ toolchain
- golangci-lint
- goreleaser for release builds

---

*Stack analysis: 2026-07-04*
