# M31A Technology Stack

## Language

- **Go 1.25.0** — module path `github.com/eshanized/M31A` (see [`go.mod`](../../go.mod))
- The project targets the very latest Go release; CI pins to `go-version: "1.25"` in [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml)

## CGO Constraints

CGO is **explicitly disabled** everywhere:

| Location | Setting |
|---|---|
| `Makefile` — `GOFLAGS` | `CGO_ENABLED=0` |
| `Makefile` — `install` | `CGO_ENABLED=0` |
| `.goreleaser.yaml` | `env: [CGO_ENABLED=0]` |
| CI `build` job | `env: CGO_ENABLED: "0"` |

This enforces a single static binary with no shared-library runtime requirements. The
Windows arm64 target is excluded in goreleaser and in the CI matrix due to cross-compilation constraints.

## Build Toolchain

### go build

Standard `go build` with consistent flags across all invocations:

```
-trimpath                       # strip build-system paths from debug info
-s -w                           # strip symbol table + DWARF debug info (release)
-X main.Version=<version>       # injected via ldflags
-X main.Commit=<short-sha>
-X main.Date=<iso8601>
-X main.GoVersion=<go-version>
```

Binary entry point: `./cmd/m31a`

### Makefile Targets

| Target | Purpose |
|---|---|
| `build` | Optimised production binary (`-trimpath -s -w`) |
| `debug` | Debug binary with gcflags (`all=-N -l`), no strip |
| `dev` | Build + run immediately |
| `test` | `go test -race -cover -coverprofile=coverage.out ./...` |
| `test-fast` | Without race detector |
| `test-verbose` | With `-v` flag |
| `test-specific TEST=<name>` | Single test by name |
| `bench` / `bench-verbose` | Run benchmarks with `-benchmem` |
| `cover` | HTML coverage report via `go tool cover` |
| `lint` | `golangci-lint run ./... --timeout=5m` |
| `lint-fix` | Lint with `--fix` |
| `vet` | `go vet ./...` |
| `fmt` | `go fmt ./...` + `goimports -w` |
| `tidy` | `go mod tidy` |
| `check` | `fmt -> tidy -> vet -> lint -> test` |
| `cross` | All 6 platform binaries (linux/darwin/windows x amd64/arm64) |
| `release` | `goreleaser release --snapshot --clean` |
| `release-dry` | Goreleaser dry-run, skip validate + publish |
| `install` | `go install` to `$GOBIN` |
| `clean` | Remove binary, dist/, coverage files |
| `nuke` | Clean + `go clean -cache -modcache -testcache` |
| `validate-release` | Run `scripts/validate-release.sh` |

### GoReleaser (`.goreleaser.yaml`)

Version 2 config. Builds for **linux/darwin/windows x amd64/arm64** (windows arm64 excluded).

**Artifacts produced:**

| Format | Notes |
|---|---|
| Tar/zip archives | Named `m31a_<version>_<os>_<arch>` |
| `.deb` | Ubuntu/Debian |
| `.rpm` | RHEL/CentOS/Fedora |
| `.apk` | Alpine Linux |
| `archlinux` | Arch Linux package |
| Scoop manifest | Windows |
| `checksums.txt` | SHA-256 checksums |

Release is created as a draft (`draft: true`) with `prerelease: auto`. Changelog excludes
commits matching `^docs:`, `^test:`, `^chore:`, `^ci:`, `^build:`, and merge commits.

## Direct Dependencies

| Package | Version | Purpose |
|---|---|---|
| `github.com/charmbracelet/bubbletea` | v1.3.0 | TUI framework — Elm-architecture event loop driving 33 screens |
| `github.com/charmbracelet/bubbles` | v0.20.0 | Reusable TUI components (textarea, viewport, spinner, etc.) |
| `github.com/charmbracelet/lipgloss` | v1.1.0 | Terminal styling — M31A dark theme, layout primitives |
| `github.com/charmbracelet/glamour` | v0.6.0 | Markdown rendering in the terminal (chat output, docs) |
| `github.com/BurntSushi/toml` | v1.6.0 | TOML config loader for `~/.m31a/config.toml` |
| `github.com/pkoukk/tiktoken-go` | v0.1.8 | Token counting for context-window estimation with EMA calibration |
| `github.com/bmatcuk/doublestar/v4` | v4.10.0 | Glob pattern matching for the `Glob` tool and permission rules |
| `github.com/godbus/dbus/v5` | v5.2.2 | D-Bus client for Linux Secret Service keychain backend |
| `github.com/mattn/go-runewidth` | v0.0.19 | Unicode display-width calculation for TUI layout |
| `golang.org/x/sync` | v0.21.0 | `singleflight` for cache deduplication; `errgroup` for concurrency |
| `github.com/alecthomas/chroma` | v0.10.0 | Syntax highlighting in diff viewer and file explorer |
| `github.com/atotto/clipboard` | v0.1.4 | Clipboard access for copy-to-clipboard TUI actions |
| `github.com/fsnotify/fsnotify` | v1.10.1 | Config hot-reload via file system watcher |
| `github.com/odvcencio/gotreesitter` | v0.20.5 | Tree-sitter bindings for Go source parsing (code intelligence) |
| `golang.org/x/sys` | v0.46.0 | Windows Credential Manager syscalls and low-level OS APIs |

## Indirect Dependencies (Notable)

| Package | Pulled by | Purpose |
|---|---|---|
| `github.com/yuin/goldmark` v1.5.2 | glamour | Markdown AST parser |
| `github.com/microcosm-cc/bluemonday` v1.0.21 | glamour | HTML sanitization in markdown |
| `github.com/muesli/termenv` v0.16.0 | lipgloss/bubbles | Terminal colour profile detection |
| `github.com/google/uuid` v1.3.0 | internal use | UUID generation for tool call IDs |
| `github.com/olekukonko/tablewriter` v0.0.5 | glamour | Markdown table rendering |
| `github.com/dlclark/regexp2` v1.10.0 | chroma | PCRE-compatible regex for syntax highlighting |
| `github.com/rivo/uniseg` v0.4.7 | go-runewidth | Unicode grapheme cluster segmentation |
| `golang.org/x/net` v0.56.0 | glamour | HTML parsing via `golang.org/x/net/html` |
| `golang.org/x/text` v0.38.0 | multiple | Unicode normalization and encoding |

## Development Tooling

### Linting — `.golangci.yml`

golangci-lint v2 config with a 5-minute timeout. Enabled linters:

| Linter | What it checks |
|---|---|
| `govet` | Static analysis + shadow variable detection (enabled explicitly) |
| `staticcheck` | Broader static analysis (deprecated APIs, dead code, etc.) |
| `errcheck` | Unchecked error return values |
| `ineffassign` | Assignments that have no effect |
| `unused` | Unused code (variables, types, functions) |

Test files (`_test.go`) are excluded from `errcheck` and `unused`. Type assertion errors
(`check-type-assertions: false`) and blank identifier assignments (`check-blank: false`) are
both suppressed.

### CI/CD — GitHub Actions (`.github/workflows/ci.yml`)

Five jobs, triggered on pushes to `master`, `v*` tags, and PRs:

| Job | Runner | What it does |
|---|---|---|
| `lint` | ubuntu-latest | `gofmt -l`, `golangci-lint`, validate goreleaser config |
| `test` | ubuntu-latest | `go test -race -coverprofile=coverage.out -covermode=atomic ./...`, upload `coverage.out` |
| `security` | ubuntu-latest | Install + run `govulncheck ./...` |
| `build` | matrix: ubuntu/macos/windows x amd64/arm64 | `go build ./cmd/m31a` + `go vet ./...` (CGO_ENABLED=0) |
| `release` | ubuntu-latest | Runs after all jobs pass; only on `v*` tags; executes goreleaser |

Coverage targets (from `TESTING.md` + README): **75%** overall, **90%** for
`pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`.

Additional tooling referenced in `Makefile`:

- `goimports` — import grouping and formatting (used in `fmt` target)
- `goreleaser` — cross-platform release builds
- `govulncheck` — Go vulnerability database scanner (CI only)
