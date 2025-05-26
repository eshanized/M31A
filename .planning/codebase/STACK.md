# Technology Stack

**Analysis Date:** 2026-06-02

## Languages

**Primary:**
- Go 1.22 — All application code (CLI binary, packages, TUI, tests). Declared in `go.mod:3` (`go 1.22`).

**Secondary:**
- TOML — Configuration file format (parsed via `BurntSushi/toml`).
- Markdown — Session planning files (`PROJECT.md`, `TASKS.md`, `STATE.md`, `TODO.md`, `LEDGER.md`).
- JSON — Session serialization (`session.json`, `messages.json`) and SSE payload parsing.
- YAML/JSON — CI/CD pipeline (`.github/workflows/ci.yml`, `.goreleaser.yaml`).
- Bash — Installer script (`install.sh`), CI shell steps.

## Runtime

**Environment:**
- Go 1.22+ standard library + selected third-party packages
- Targets: `linux`, `darwin`, `windows` (build matrix in `.goreleaser.yaml:9-11`)
- Architectures: `amd64`, `arm64` (excludes `windows/arm64` in `.goreleaser.yaml:20-22`)
- Binary is statically linked: `CGO_ENABLED=0` enforced everywhere (`Makefile:9`, `.goreleaser.yaml:7`, `cmd/m31a/main.go`).
- Single static binary named `m31a` (or `m31a.exe` on Windows), entry point `cmd/m31a/main.go`.

**Package Manager:**
- Go modules (`go.mod`, `go.sum`)
- Lockfile: `go.sum` present (49 lines, 8 direct + ~40 indirect deps)

## Frameworks

**Core TUI:**
- `github.com/charmbracelet/bubbletea` v1.3.0 — Elm-style TUI framework (single-threaded Update/View/Init). The base of `internal/tui/app.go` and all screen files.
- `github.com/charmbracelet/lipgloss` v1.1.0 — Styling/layout for terminal output. Used by `internal/tui/theme/theme.go`, `internal/tui/components/`, and `internal/tokens/estimator.go` for the context warning banner.
- `github.com/charmbracelet/bubbles` v0.20.0 — Higher-level TUI components (textarea, viewport, spinner, list). Imported in `internal/tui/repl.go`, `internal/tui/header.go`, etc.
- `github.com/charmbracelet/glamour` v0.6.0 — Markdown rendering. Used (via `alecthomas/chroma` indirect dep) for tool card / message body styling.

**Configuration & Data:**
- `github.com/BurntSushi/toml` v1.6.0 — `~/.m31a/config.toml` and project `m31a.toml` parser. Used in `internal/config/loader.go`.

**File Matching:**
- `github.com/bmatcuk/doublestar/v4` v4.10.0 — Glob (`**` recursive) matching. Used in `internal/tools/glob.go` and `internal/tools/grep.go` for tool pattern matching and `internal/tools/dispatcher.go` for permission rule glob evaluation.

**System Integration (Linux only):**
- `github.com/godbus/dbus/v5` v5.2.2 — D-Bus client for the Secret Service API. Used in `pkg/keychain/keychain_linux.go` (preferred Linux keychain backend).

**Token Estimation:**
- `github.com/pkoukk/tiktoken-go` v0.1.8 — BPE token counter for OpenAI / GPT-4 / Claude families. Used in `internal/tokens/estimator.go`; falls back to `len([]rune(text))/4 * 1.3` for unsupported models.

**Indirect dependencies (transitive, none imported directly by M31A code):**
- `alecthomas/chroma` v0.10.0 — Syntax highlighting (via glamour).
- `aymanbagabas/go-osc52/v2` — OSC 52 terminal escape sequences (clipboard via glamour).
- `aymerick/douceur` — CSS parser (glamour).
- `charmbracelet/colorprofile`, `charmbracelet/x/{ansi,cellbuf,term}` — bubbletea/lipgloss support libs.
- `clipperhouse/{stringish,uax29/v2}` — text segmentation (glamour).
- `dlclark/regexp2` — full regex (glamour internal).
- `erikgeiser/coninput` — Windows console input.
- `google/uuid` — bubbletea internal.
- `gorilla/css` — CSS tokenizer (glamour).
- `lucasb-eyer/go-colorful` — color space conversions.
- `mattn/go-isatty`, `mattn/go-localereader`, `mattn/go-runewidth` — TTY detection and width measurement.
- `microcosm-cc/bluemonday` — HTML sanitizer (glamour).
- `muesli/{ansi,cancelreader,reflow,termenv}` — terminal handling.
- `olekukonko/tablewriter` — table rendering.
- `rivo/uniseg` — Unicode segmentation.
- `sahilm/fuzzy` — fuzzy string matching (bubbletea list).
- `xo/terminfo` — terminfo database.
- `yuin/goldmark`, `yuin/goldmark-emoji` — Markdown AST.
- `golang.org/x/{net,sync,sys,text}` — stdlib extensions.

## Key Dependencies

**Critical (functional core):**
- `charmbracelet/bubbletea` — entire TUI architecture; cannot be replaced without rewriting the app.
- `charmbracelet/lipgloss` — every styled output element.
- `charmbracelet/bubbles` — REPL viewport, input textarea, spinner.
- `charmbracelet/glamour` — markdown rendering for messages & tool output.
- `BurntSushi/toml` — required for `config.toml` and `m31a.toml`.
- `bmatcuk/doublestar/v4` — required for `**` glob support in Glob tool and permission rules.
- `pkoukk/tiktoken-go` — required for accurate token estimation on OpenAI/Claude models.
- `godbus/dbus/v5` — required for Linux Secret Service keychain (Windows/macOS use different paths).

**Infrastructure:**
- Standard library `log/slog` for structured logging (with JSON handler by default, text optional via `M31A_LOG_FORMAT`).
- Standard library `net/http` for OpenRouter and Zen API clients (no third-party HTTP framework).
- Standard library `crypto/rand`, `crypto/sha256` (transitive via godbus) for session ID and config-tmp random names.
- `golang.org/x/sys` (indirect, via godbus) for Unix syscalls used by the Bash tool's signal handling.

## Configuration

**Configuration files:**
- Global: `~/.m31a/config.toml` — single TOML file parsed by `internal/config/loader.go:40-91`. Sections: `provider`, `model`, `ui`, `permissions`, `features`, `ledger`, `agents`.
- Project-level (optional): `m31a.toml` walked-up from `cwd` (max 3 levels) at `internal/config/loader.go:95-109`. Same schema as global; overlay-merged.

**Environment variables** (declared in `cmd/m31a/main.go:47-52` and `internal/config/loader.go:60-66`):

| Variable | Purpose | Default |
|----------|---------|---------|
| `M31A_CONFIG` | Override config file path | `~/.m31a/config.toml` |
| `OPENROUTER_API_KEY` | Direct OpenRouter API key (1st resolution) | — |
| `ZEN_API_KEY` | Direct OpenCode Zen API key (1st resolution) | — |
| `M31A_OPENROUTER_API_KEY` | OpenRouter key (config loader resolution) | — |
| `M31A_ZEN_API_KEY` | Zen key (config loader resolution) | — |
| `M31A_THEME` | UI theme (`dark`/`light`/`auto`) | — |
| `M31A_DEFAULT_MODEL` | Override default model | — |
| `M31A_LOG_FORMAT` | `json` or `text` log output | `json` |
| `M31A_LOG_LEVEL` | `debug`/`info`/`warn`/`error` | `info` |

**API key resolution order** (`internal/config/loader.go:508-535`):
1. `M31A_OPENROUTER_API_KEY` / `M31A_ZEN_API_KEY` env var
2. OS keychain (`m31a/openrouter`, `m31a/zen`)
3. `provider.openrouter.api_key` / `provider.zen.api_key` field in config TOML

**API keys are never persisted to the TOML file** — `internal/config/loader.go:476-477` blanks them on `Save()`.

**Build configuration:**
- `Makefile` — `build`, `test`, `lint`, `vet`, `clean`, `release` targets. Strips debug info (`-s -w`) and embeds git tag as `main.Version`.
- `.goreleaser.yaml` — multi-OS/arch builds, `tar.gz` archives (`.zip` on Windows), sha256 checksums, draft releases.
- `.golangci.yml` — linters: `govet`, `staticcheck`, `errcheck`, `ineffassign`, `unused`, `gosimple`. Test files excluded from `errcheck`/`unused`.

**CI configuration:**
- `.github/workflows/ci.yml` — four jobs: `lint` (golangci-lint v4), `test` (`go test -race -coverprofile=...`), `build` (matrix: ubuntu/macos/windows × amd64/arm64, excluding windows/arm64), `release` (GoReleaser on `v*` tag).

## Tool Implementations

The `internal/tools/` package contains 9 tools (registered by `internal/tools/dispatcher.go:502-515`):

| Tool | File | Risk | Backends / Notes |
|------|------|------|------------------|
| `Bash` | `internal/tools/bash.go` | `RiskDangerous` | Spawns `bash -c` on Unix (`internal/tools/bash_unix.go`), `cmd /C` on Windows (`internal/tools/bash_windows.go`). Uses `syscall.Setpgid` + process-group signal forwarding on Unix, `taskkill /F` on Windows. No actual PTY despite docs (`creack/pty` not in `go.mod`). |
| `FileRead` | `internal/tools/fileread.go` | `RiskSafe` | Pure Go `os` package. Symlink-resolved, workdir-contained, 5MB size cap, 512-byte header binary detection. |
| `FileWrite` | `internal/tools/filewrite.go` | `RiskDestructive` | Atomic write: `.m31a_tmp_<hex>` + `os.Rename`. Pre-write backup to `~/.m31a/sessions/<id>/backups/`. |
| `Edit` | `internal/tools/edit.go` | `RiskDangerous` | 5-strategy cascading replacement: exact → line-trimmed → whitespace-normalized → fuzzy-anchor (Levenshtein) → line-range. |
| `Glob` | `internal/tools/glob.go` | `RiskSafe` | Prefers `rg --files` (if `.gitignore` present), else `doublestar.Glob`. Capped at 1000 results. |
| `Grep` | `internal/tools/grep.go` | `RiskSafe` | Prefers `rg --json` (structured), falls back to pure-Go `bufio.Scanner` + `doublestar` for `.gitignore`. |
| `WebFetch` | `internal/tools/webfetch.go` | `RiskSafe` | Pure Go `net/http` GET. Pure-Go HTML→Markdown converter (no `goquery`/`bluemonday` despite docs). 5MB cap, max 5 redirects, 1–120s timeout. |
| `TodoWrite` | `internal/tools/todo.go` | `RiskSafe` | Writes `TODO.md` to session dir. |
| `AskUserQuestion` | `internal/tools/question.go` | `RiskSafe` | Blocking channel-based question via TUI modal. |

## Logging & Observability

- **Logger:** stdlib `log/slog` with JSON handler to `~/.m31a/m31a.log` (`internal/log/log.go:30-44`).
- **Log rotation:** daily — file renamed to `m31a.log.YYYY-MM-DD`; 7-day retention (`internal/log/log.go:58-113`).
- **No external log shipper.** No telemetry, no analytics, no phone-home (per `AGENTS.md` "No telemetry" rule).
- **Health check:** built into each LLM provider — every provider has a `HealthCheck(ctx)` returning `live` / `slow` / `degraded` / `offline` based on latency thresholds.

## Platform Requirements

**Development:**
- Go 1.22+ toolchain
- `golangci-lint` for `make lint` (CI uses `golangci-lint-action@v4` with `version: latest`)
- Optional: `ripgrep` (`rg`) — preferred backend for `Grep` and `Glob` tools; falls back to pure Go.
- Optional (Linux): `pass` CLI — used as fallback when D-Bus Secret Service is unavailable.
- Optional (macOS): `/usr/bin/security` — built-in to macOS, always present.
- `goreleaser` for snapshot releases (`make release`).

**Production:**
- Single static binary. No runtime dependencies.
- Linux: optional DBus session bus for keychain; optional `pass` CLI; optional `git` for the workflow engine; optional `rg` for performance.
- macOS: `/usr/bin/security` (always present); `git` (Xcode CLT); `rg` optional.
- Windows: nothing extra (Credential Manager is built-in; `git` and `rg` optional for full workflow).
- Min OS: any Linux/macOS/Windows release that supports the Go 1.22 runtime.

## CI/CD & Release

**Pipeline:** `.github/workflows/ci.yml`
- Triggers: push to `main`/`dev`, PR to `main`, manual dispatch.
- Jobs: `lint` → `test` → `build` (matrix) → `release` (tag-gated).
- Linter: `golangci-lint-action@v4` with timeout 5m.
- Test: `go test -race -coverprofile=coverage.out -covermode=atomic ./...`; coverage uploaded as artifact.
- Build: `CGO_ENABLED=0` on ubuntu-latest, macos-latest, windows-latest × amd64/arm64 (windows/arm64 excluded).
- Release: `goreleaser/goreleaser-action@v5` on `refs/tags/v*`.

**Release artifact:** tar.gz (zip on Windows) per `m31a_<version>_<os>_<arch>.<ext>`; sha256 checksums in `checksums.txt`. Installed via `install.sh` (downloads from GitHub releases, verifies checksum, extracts to `/usr/local/bin`).

**Linter config (`.golangci.yml`):**
- Enabled: `govet`, `staticcheck`, `errcheck`, `ineffassign`, `unused`, `gosimple`.
- `govet.check-shadowing: true`; `errcheck.check-type-assertions: false`, `check-blank: false`.
- Test files excluded from `errcheck` and `unused`.

---

*Stack analysis: 2026-06-02*
