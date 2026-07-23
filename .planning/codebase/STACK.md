# Technology Stack

**Analysis Date:** 2026-07-23

## Languages

**Primary:**
- **Go 1.25.0** — Main implementation language (specified in `go.mod`)

**Secondary:**
- **Shell (Bash/PowerShell)** — Installation scripts (`install.sh`), cross-platform shell utilities in `internal/integrations/shell/`

## Runtime

**Environment:**
- **Go 1.25+** — Required for compilation (per `go.mod:3` and AGENTS.md)
- **CGO_ENABLED=0** — Hard constraint: binary must be fully static (enforced in Makefile, `.goreleaser.yaml`, AGENTS.md)

**Package Manager:**
- **Go Modules** (`go mod`) — Dependency management
- **Lockfile:** `go.sum` present (1388 lines, verified via `go mod verify`)

## Frameworks

**Core (Runtime):**
- **Bubble Tea v1.3.0** (`github.com/charmbracelet/bubbletea`) — TUI framework using Elm architecture (single-threaded event loop, all state mutations via `Update()` messages)
- **Bubbles v0.20.0** (`github.com/charmbracelet/bubbles`) — Pre-built TUI components (text input, list, viewport, spinner, etc.)
- **Lip Gloss v1.1.0** (`github.com/charmbracelet/lipgloss`) — Terminal styling/layout (CSS-like)
- **Glamour v0.6.0** (`github.com/charmbracelet/glamour`) — Markdown rendering in terminal

**Configuration:**
- **BurntSushi TOML v1.6.0** (`github.com/BurntSushi/toml`) — Config file parsing (`.m31a/config.toml`)

**File Watching:**
- **fsnotify v1.10.1** (`github.com/fsnotify/fsnotify`) — Config file hot-reload, cross-platform

**Clipboard:**
- **atotto/clipboard v0.1.4** — Cross-platform clipboard access
- **aymanbagabas/go-osc52 v2.0.1** — OSC52 terminal clipboard escape sequences

**Terminal/ANSI:**
- **muesli/termenv v0.16.0** — ANSI color/profile detection
- **mattn/go-runewidth v0.0.19** — East Asian character width handling
- **muesli/ansi v0.0.0** — ANSI sequence parsing
- **muesli/reflow v0.3.0** — Text wrapping/reflow

**Syntax Highlighting:**
- **alecthomas/chroma v0.10.0** — Code syntax highlighting

**Markdown:**
- **yuin/goldmark v1.5.2** + **goldmark-emoji v1.0.1** — Markdown parsing/rendering

**HTML/Sanitization:**
- **microcosm-cc/bluemonday v1.0.21** — HTML sanitization for web content

**Parsing/Analysis:**
- **odvcencio/gotreesitter v0.20.5** — Tree-sitter bindings for Go/Python/JS/TS parsing (code intelligence)
- **clipperhouse/uax29/v2 v2.5.0** — Unicode text segmentation

**Concurrency/Utils:**
- **golang.org/x/sync v0.21.0** — `singleflight` for deduplicating concurrent model fetches
- **golang.org/x/net v0.56.0** — Network extensions
- **golang.org/x/text v0.38.0** — Text processing
- **golang.org/x/sys v0.47.0** — System calls

**DBus/Keychain (Linux):**
- **godbus/dbus/v5 v5.2.2** — D-Bus Secret Service integration for OS keychain

**Tokenization:**
- **pkoukk/tiktoken-go v0.1.8** — OpenAI-compatible token counting (model-aware estimation)

**Glob/Pattern Matching:**
- **bmatcuk/doublestar/v4 v4.10.0** — Glob pattern matching for file operations

**Regex:**
- **dlclark/regexp2 v1.10.0** — .NET-compatible regex (for complex pattern matching)

**Testing/Linting:**
- **golangci-lint** — Linter (govet, staticcheck, errcheck, ineffassign, unused)
- **Standard `testing` package** — Unit/integration tests with `-race` flag

**Build/Release:**
- **goreleaser** — Cross-platform binary releases (Linux/amd64,arm64; Darwin/amd64,arm64; Windows/amd64)
- **Make** — Build orchestration (`Makefile` with `build`, `test`, `lint`, `cross`, `release` targets)

## Key Dependencies

**Critical (Direct):**
| Package | Version | Purpose |
|---------|---------|---------|
| `github.com/charmbracelet/bubbletea` | v1.3.0 | TUI application framework (Elm architecture) |
| `github.com/charmbracelet/bubbles` | v0.20.0 | TUI component library |
| `github.com/charmbracelet/lipgloss` | v1.1.0 | Terminal styling/layout engine |
| `github.com/charmbracelet/glamour` | v0.6.0 | Markdown rendering |
| `github.com/BurntSushi/toml` | v1.6.0 | TOML config parsing |
| `github.com/fsnotify/fsnotify` | v1.10.1 | File watching (config reload) |
| `github.com/godbus/dbus/v5` | v5.2.2 | Linux keychain (D-Bus Secret Service) |
| `github.com/odvcencio/gotreesitter` | v0.20.5 | Tree-sitter parsers for code intelligence |
| `github.com/pkoukk/tiktoken-go` | v0.1.8 | Token estimation for cost tracking |
| `golang.org/x/sync` | v0.21.0 | `singleflight` for model fetch deduplication |

**Infrastructure (Indirect — pulled by above):**
- `github.com/charmbracelet/x/*` — Bubble Tea internals (ansi, cellbuf, term)
- `github.com/muesli/*` — ANSI, termenv, reflow, cancelreader
- `github.com/yuin/goldmark*` — Markdown processing
- `github.com/clipperhouse/*` — String algorithms, Unicode segmentation
- `github.com/dlclark/regexp2` — Complex regex patterns
- `github.com/google/uuid` — Session ID generation
- `github.com/alecthomas/chroma` — Syntax highlighting

## Configuration

**Environment:**
- **`.env` file** (gitignored) — Local development API keys (`OPENROUTER_API_KEY`, `ZEN_API_KEY`, `NVIDIA_API_KEY`)
- **`.env.example`** — Template with required env vars
- **OS Keychain** — Primary secure storage for API keys (D-Bus Secret Service on Linux, Keychain on macOS, Credential Manager on Windows)
- **Config file:** `~/.m31a/config.toml` (global) + `./m31a.toml` (project-level, optional)

**Config Loading Order (later overrides earlier):**
1. `DefaultConfig()` — Hardcoded defaults in `internal/core/config/loader.go`
2. Global TOML (`~/.m31a/config.toml` via `M31A_CONFIG` env or default path)
3. Environment variables (`M31A_*` prefix + provider-specific `OPENROUTER_API_KEY`, `ZEN_API_KEY`, `NVIDIA_API_KEY`)
4. Project TOML (`m31a.toml` walked up from CWD, max 3 levels)
5. Variable substitution (`${VAR}` → env value)
6. Validation (type/range checks)

**Build:**
- **LDFLAGS:** `-s -w -X main.Version -X main.Commit -X main.Date -X main.GoVersion`
- **Tags:** `CGO_ENABLED=0`, `-trimpath`
- **Cross-compile targets:** linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64 (windows/arm64 excluded)

## Platform Requirements

**Development:**
- Go 1.25+
- `golangci-lint` (for `make lint`)
- `goreleaser` (for releases)
- `make` (build orchestration)

**Production (Runtime):**
- **Static binary** — No external dependencies, no CGO
- **OS Keychain** — For API key storage (falls back to config file if unavailable)
- **Git** — Required for workflow (commit, rollback, ledger)
- **POSIX shell** — For `bash` tool execution (Windows uses bundled MSYS2/Git Bash detection)
- **Terminal** — True-color support recommended (falls back to 256-color/ASCII)

**Optional (for tools):**
- **pass** (Linux) — GPG-backed password store fallback for keychain
- **SearXNG instance** — Web search (default: `https://search.sagibo.net`)
- **HTTP/HTTPS connectivity** — For LLM providers, web fetch, search

---

*Stack analysis: 2026-07-23*