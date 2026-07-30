# STACK.md — M31A Technology Stack

last_mapped_commit: 3836de6de09785c873f87a86dd633ccb4ab886fa

## Language & Runtime

| Attribute | Value |
|-----------|-------|
| Language | Go |
| Go version | 1.25.12 |
| Module path | `github.com/eshanized/M31A` |
| Build constraint | `CGO_ENABLED=0` (static binary, no cgo) |
| Binary name | `m31a` |
| Entry point | `cmd/m31a/main.go` |

## Core Dependencies

| Package | Purpose |
|---------|---------|
| `github.com/charmbracelet/bubbletea` v1.3.0 | TUI framework (Elm architecture) |
| `github.com/charmbracelet/bubbles` v0.20.0 | TUI components (text input, viewport, list) |
| `github.com/charmbracelet/lipgloss` v1.1.0 | Terminal styling |
| `github.com/charmbracelet/glamour` v0.6.0 | Markdown rendering |
| `github.com/BurntSushi/toml` v1.6.0 | TOML config parsing |
| `github.com/godbus/dbus/v5` v5.2.2 | Linux keychain integration (Secret Service API) |
| `github.com/pkoukk/tiktoken-go` v0.1.8 | Token estimation for LLM context |
| `github.com/bmatcuk/doublestar/v4` v4.10.0 | Glob pattern matching |
| `github.com/fsnotify/fsnotify` v1.10.1 | File system watching |
| `github.com/odvcencio/gotreesitter` v0.20.5 | Tree-sitter code analysis |
| `golang.org/x/sync` v0.22.0 | `singleflight` for deduplication |
| `golang.org/x/sys` v0.47.0 | OS-level system calls |

## Indirect Dependencies (Notable)

| Package | Purpose |
|---------|---------|
| `github.com/alecthomas/chroma` v0.10.0 | Syntax highlighting |
| `github.com/atotto/clipboard` v0.1.4 | System clipboard access |
| `github.com/muesli/termenv` v0.16.0 | Terminal capability detection |
| `github.com/clipperhouse/uax29/v2` v2.5.0 | Unicode text segmentation |
| `github.com/yuin/goldmark` v1.8.4 | Markdown parsing |
| `github.com/google/uuid` v1.3.0 | UUID generation |

## Configuration

| File | Format | Purpose |
|------|--------|---------|
| `~/.m31a/config.toml` | TOML | Global configuration |
| `.env` | dotenv | API keys (gitignored) |
| `m31a.json` | JSON | Package metadata (version, hashes) |
| `.golangci.yml` | YAML | Linter configuration |
| `.goreleaser.yaml` | YAML | Release automation |

## Build System

| Target | Command | Notes |
|--------|---------|-------|
| Build | `make build` | Optimized, stripped binary |
| Debug build | `make debug` | No strip, debug symbols |
| Test | `make test` | Race detector + coverage |
| Lint | `make lint` | golangci-lint with 5min timeout |
| Full check | `make check` | fmt → tidy → vet → lint → test |
| Cross-compile | `make cross` | linux/darwin/windows × amd64/arm64 |
| Release | `make release` | goreleaser snapshot |

## Platform Support

- **Linux**: amd64, arm64
- **macOS**: amd64, arm64
- **Windows**: amd64, arm64

## Key Build Flags

```
LDFLAGS: -s -w (strip symbols for release)
CGO_ENABLED=0 (static binary)
-trimpath (reproducible builds)
```
