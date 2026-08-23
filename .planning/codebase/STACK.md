# Technology Stack

**Analysis Date:** 2026-08-23

## Languages

**Primary:**
- Go 1.26.5 — All application code, CLI, TUI, workflow engine, provider integrations, tools

**Secondary:**
- None detected (pure Go codebase)

## Runtime

**Environment:**
- Go 1.26.5 (required per `go.mod`)

**Package Manager:**
- Go Modules (`go mod`)
- Lockfile: Present (`go.sum` committed)

## Frameworks

**Core:**
- **Bubble Tea v1.3.0** (`github.com/charmbracelet/bubbletea`) — TUI framework using Elm architecture (Model-Update-View)
- **Bubbles v0.20.0** (`github.com/charmbracelet/bubbles`) — Reusable TUI components (textinput, viewport, list, etc.)
- **Lipgloss v1.1.0** (`github.com/charmbracelet/lipgloss`) — Terminal styling and layout
- **Glamour v0.6.0** (`github.com/charmbracelet/glamour`) — Markdown rendering in terminal

**Testing:**
- Standard library `testing` package
- Race detector enabled by default (`go test -race`)
- Coverage target: 75% overall, 90% for `pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`

**Build/Dev:**
- **golangci-lint** (v2 config) — Linting with govet, staticcheck, errcheck, ineffassign, unused
- **goreleaser** — Cross-platform binary releases
- **goimports** — Import organization
- **Makefile** — Build orchestration (see `Makefile` for targets)

## Key Dependencies

**Critical:**
| Package | Version | Purpose |
|---------|---------|---------|
| `github.com/charmbracelet/bubbletea` | v1.3.0 | TUI framework (Elm architecture) |
| `github.com/charmbracelet/bubbles` | v0.20.0 | TUI component library |
| `github.com/charmbracelet/lipgloss` | v1.1.0 | Terminal styling |
| `github.com/charmbracelet/glamour` | v0.6.0 | Markdown rendering |
| `github.com/BurntSushi/toml` | v1.6.0 | TOML config parsing |
| `github.com/bmatcuk/doublestar/v4` | v4.10.0 | Glob pattern matching |
| `github.com/fsnotify/fsnotify` | v1.10.1 | File watching for config reload |
| `github.com/pkoukk/tiktoken-go` | v0.1.8 | Token estimation |
| `github.com/godbus/dbus/v5` | v5.2.2 | Linux keychain (D-Bus Secret Service) |
| `golang.org/x/sync` | v0.22.0 | Singleflight, concurrency primitives |

**Infrastructure/Utilities:**
| Package | Version | Purpose |
|---------|---------|---------|
| `github.com/atotto/clipboard` | v0.1.4 | Clipboard access (OSC52) |
| `github.com/mattn/go-runewidth` | v0.0.19 | Unicode width calculation |
| `github.com/odvcencio/gotreesitter` | v0.20.5 | Tree-sitter parsing |
| `golang.org/x/sys` | v0.47.0 | OS-specific syscalls |
| `golang.org/x/time` | v0.15.0 | Time utilities |

**Indirect (notable):**
- `github.com/yuin/goldmark` v1.8.4 — Markdown parser
- `github.com/muesli/termenv` v0.16.0 — Terminal color profiles
- `github.com/rivo/uniseg` v0.4.7 — Unicode segmentation
- `github.com/google/uuid` v1.3.0 — UUID generation

## Configuration

**Environment:**
- `.env` file in CWD (gitignored, loaded via `config.LoadDotEnv()` in `internal/core/config/loader.go:773`)
- Environment variables with `M31A_` prefix take precedence over config file
- Standard provider env vars also supported: `OPENROUTER_API_KEY`, `ZEN_API_KEY`, `NVIDIA_API_KEY`
- OS keychain integration for secure API key storage (`pkg/keychain/` — `internal/integrations/keychain/`)

**Config Files (layered, later overrides earlier):**
1. Defaults (hardcoded in `internal/core/config/loader.go:27`)
2. Global TOML: `~/.m31a/config.toml` (or `$M31A_CONFIG`)
3. Workspace TOML: `.m31a/workspace.toml` (walked up 3 levels from CWD)
4. Environment variables: `M31A_*` prefix
5. Project JSON: `m31a.json` (walked up 3 levels from CWD)
6. Variable substitution: `${VAR}` → env value

**Build:**
- `go.mod` / `go.sum` — Go module definition
- `.golangci.yml` — Linter config (5m timeout, govet+shadow, staticcheck, errcheck, ineffassign, unused)
- `.goreleaser.yaml` — Release automation (cross-compile, packages, checksums, changelog)
- `Makefile` — Primary build/test/lint orchestration

## Platform Requirements

**Development:**
- Go 1.26.5+
- `CGO_ENABLED=0` (hard constraint — static binary)
- Linux/macOS/Windows (see cross-compilation targets)

**Production:**
- Static binary (no CGO dependencies)
- Targets: `linux/{amd64,arm64}`, `darwin/{amd64,arm64}`, `windows/amd64`
- Windows/arm64 explicitly excluded
- OS keychain required for secure credential storage:
  - Linux: D-Bus Secret Service + `pass` CLI fallback
  - macOS: `/usr/bin/security` keychain
  - Windows: Windows Credential Manager
- Debug/profiling: pprof on `localhost:6060` (gated by `--debug` or `M31A_DEBUG=1`)

---

*Stack analysis: 2026-08-23*