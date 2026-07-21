# Technology Stack

**Analysis Date:** 2026-07-21

## Language & Runtime

**Primary:**
- **Language**: Go 1.25.0 (from `go.mod:3`)
- **Runtime**: CGO_ENABLED=0, static binary (enforced by `Makefile:13`, `Makefile:43`, `Makefile:49`)
- **Build**: `make build` → `CGO_ENABLED=0 go build -trimpath -ldflags "-s -w ..."` (`Makefile:41-44`)

## Frameworks & Libraries

**Core (TUI/CLI):**
- **TUI**: Bubble Tea v1.3.0 (`github.com/charmbracelet/bubbletea`) — Elm architecture, single-threaded event loop
- **UI Components**: Bubbles v0.20.0 (`github.com/charmbracelet/bubbles`) — viewport, textarea, list, spinner, etc.
- **Styling**: Lipgloss v1.1.0 (`github.com/charmbracelet/lipgloss`) — declarative terminal styling
- **Markdown Rendering**: Glamour v0.6.0 (`github.com/charmbracelet/glamour`) — terminal markdown renderer
- **CLI**: Cobra (via `github.com/spf13/cobra` — transitive via Bubble Tea)
- **Config**: Viper (via `github.com/spf13/viper` — transitive)
- **TOML**: BurntSushi/toml v1.6.0 (`github.com/BurntSushi/toml`) — config file parsing

**Provider/HTTP:**
- **HTTP Client**: Standard library `net/http` with custom retry/backoff (`internal/provider/base_client.go`)
- **SSE Parsing**: Custom parser (`internal/provider/sse.go`) — handles OpenAI-compatible streaming
- **Token Counting**: tiktoken-go v0.1.8 (`github.com/pkoukk/tiktoken-go`) — OpenAI-compatible tokenization
- **Concurrency**: `golang.org/x/sync/singleflight` v0.21.0 — request deduplication for model fetching

**File System & OS:**
- **File Watching**: fsnotify v1.10.1 (`github.com/fsnotify/fsnotify`) — config file hot-reload
- **File Locking**: Custom cross-platform (`internal/fileutil/lock_unix.go`, `lock_windows.go`) — flock / LockFileEx
- **Atomic Writes**: Custom (`internal/fileutil/atomic.go`) — temp file + rename
- **Glob Matching**: doublestar v4.10.0 (`github.com/bmatcuk/doublestar/v4`) — file pattern matching
- **Clipboard**: atotto/clipboard v0.1.4 (`github.com/atotto/clipboard`) — OSC52 + system clipboard
- **D-Bus**: godbus/dbus v5.2.2 (`github.com/godbus/dbus/v5`) — Linux Secret Service (keychain)

**Terminal & Text:**
- **Terminal Info**: terminfo v0.0.0-20220910 (`github.com/xo/terminfo`) — terminal capabilities
- **Unicode**: uniseg v0.4.7 (`github.com/rivo/uniseg`) — grapheme cluster segmentation
- **Text Width**: go-runewidth v0.0.19 (`github.com/mattn/go-runewidth`) — CJK-aware width
- **ANSI Parsing**: charmbracelet/x/ansi v0.8.0 — ANSI sequence handling
- **Reflow**: muesli/reflow v0.3.0 — text wrapping

**Markdown & Syntax:**
- **Markdown**: goldmark v1.5.2 (`github.com/yuin/goldmark`) — CommonMark parser
- **Emoji**: goldmark-emoji v1.0.1 — emoji shortcodes
- **Syntax Highlighting**: chroma v0.10.0 (`github.com/alecthomas/chroma`) — code highlighting
- **Tree-sitter**: gotreesitter v0.20.5 (`github.com/odvcencio/gotreesitter`) — code structure analysis

**Logging & Observability:**
- **Structured Logging**: zerolog (`github.com/rs/zerolog` — transitive) — JSON/console output
- **Standard Log**: `log/slog` (stdlib) — used throughout codebase
- **Metrics**: Custom (`internal/metrics/collector.go`) — JSONL session metrics

**Testing:**
- **Test Framework**: Standard library `testing` + testify v1.8.4 (`github.com/stretchr/testify`) — assertions, mocks
- **E2E**: Custom binary compilation + exec (`e2e_test.go`)
- **Race Detector**: Enabled by default in `make test` (`Makefile:79`)

**Linting:**
- **Runner**: golangci-lint (timeout 5m, `Makefile:120`, `.golangci.yml:4`)
- **Enabled Linters**: govet (with shadow), staticcheck, errcheck, ineffassign, unused (`.golangci.yml:7-12`)
- **Test Exclusions**: errcheck, unused disabled for `_test.go` (`.golangci.yml:25-28`)

## Key Dependencies (from `go.mod`)

**Direct:**
| Package | Version | Purpose |
|---------|---------|---------|
| github.com/BurntSushi/toml | v1.6.0 | TOML config parsing |
| github.com/bmatcuk/doublestar/v4 | v4.10.0 | Glob pattern matching |
| github.com/charmbracelet/bubbles | v0.20.0 | TUI components |
| github.com/charmbracelet/bubbletea | v1.3.0 | TUI framework (Elm arch) |
| github.com/charmbracelet/glamour | v0.6.0 | Markdown rendering |
| github.com/charmbracelet/lipgloss | v1.1.0 | Terminal styling |
| github.com/godbus/dbus/v5 | v5.2.2 | Linux Secret Service (keychain) |
| github.com/mattn/go-runewidth | v0.0.19 | Text width calculation |
| github.com/pkoukk/tiktoken-go | v0.1.8 | Token counting |
| golang.org/x/sync | v0.21.0 | singleflight deduplication |

**Notable Indirect:**
| Package | Version | Purpose |
|---------|---------|---------|
| github.com/charmbracelet/x/* | various | Bubble Tea internals (ansi, cellbuf, term) |
| github.com/yuin/goldmark | v1.5.2 | Markdown parsing |
| github.com/alecthomas/chroma | v0.10.0 | Syntax highlighting |
| github.com/odvcencio/gotreesitter | v0.20.5 | Tree-sitter bindings |
| golang.org/x/sys | v0.47.0 | OS-specific syscalls |
| golang.org/x/text | v0.38.0 | Unicode/text processing |

## Build Configuration

**Module:** `github.com/eshanized/M31A` (`go.mod:1`)

**Go Version:** 1.25.0 (`go.mod:3`)

**Build Flags:**
- `-trimpath` — reproducible builds (`Makefile:15`)
- `-ldflags "-s -w -X main.Version=... -X main.Commit=... -X main.Date=... -X main.GoVersion=..."` (`Makefile:12`)
- `CGO_ENABLED=0` — static binary, no C dependencies (`Makefile:13`)

**Cross-Compilation Targets** (`Makefile:24-25, 160-182`):
- linux/amd64, linux/arm64
- darwin/amd64, darwin/arm64
- windows/amd64 (windows/arm64 excluded)

**Release:** goreleaser (`Makefile:189-196`, `.goreleaser.yml` not in repo root)

## Configuration Files

| File | Purpose |
|------|---------|
| `go.mod` / `go.sum` | Go module dependencies |
| `Makefile` | Build, test, lint, cross-compile, release targets |
| `.golangci.yml` | Linter configuration (5 linters, test exclusions) |
| `.env.example` | Environment variable template (API keys) |
| `.env.test` | Test environment (gitignored) |
| `~/.m31a/config.toml` | User config (TOML, loaded by `internal/config/loader.go`) |
| `m31a.toml` | Project-level config (optional, walked up 3 dirs) |

## Platform Requirements

**Development:**
- Go 1.25+
- `golangci-lint` (for `make lint`)
- `goreleaser` (for `make release`)
- `make` (GNU Make)

**Production (Binary):**
- Linux: glibc 2.17+ (static binary, no CGO)
- macOS: 10.13+ (static binary)
- Windows: 10+ (static .exe)

**Runtime Dependencies (Optional):**
- **Linux Keychain**: `dbus-daemon` + `org.freedesktop.secrets` (gnome-keyring, kwallet) OR `pass` CLI
- **macOS Keychain**: `/usr/bin/security` (built-in)
- **Windows Keychain**: Windows Credential Manager (built-in)
- **Clipboard**: `xclip`/`xsel` (X11), `wl-copy` (Wayland), `pbcopy` (macOS), built-in (Windows)

---

*Stack analysis: 2026-07-21*