# Technology Stack — M31A

> Mapped: 2026-07-09

## Language

- **Go 1.25** — single language for the entire project. No CGO (hard constraint: `CGO_ENABLED=0` for static binaries). Go module path: `github.com/eshanized/M31A`.

## Runtime & Build

| Component | Choice | Notes |
|-----------|--------|-------|
| Build system | `Makefile` + native `go build` | Targets: build, test, lint, cross, release |
| Cross-compilation | goreleaser v2 | linux/{amd64,arm64}, darwin/{amd64,arm64}, windows/amd64 (windows/arm64 excluded) |
| Linting | golangci-lint (v2 config) | govet (with shadow), staticcheck, errcheck, ineffassign, unused |
| Build flags | `-trimpath`, `-ldflags "-s -w"` | Stripped, static, no debug symbols for release |

## Key Dependencies

### TUI (Bubble Tea ecosystem)

| Dependency | Purpose | Location |
|-----------|---------|----------|
| `github.com/charmbracelet/bubbletea v1.3.0` | Elm-architecture TUI framework | `internal/tui/` |
| `github.com/charmbracelet/bubbles v0.20.0` | Reusable TUI components | `internal/tui/` |
| `github.com/charmbracelet/lipgloss v1.1.0` | Terminal styling | `internal/tui/` entire UI |
| `github.com/charmbracelet/glamour v0.6.0` | Markdown rendering | `internal/tui/repl_view.go` |
| `github.com/mattn/go-runewidth v0.0.19` | Unicode width calculation | `internal/tui/` layout |

### LLM Provider Clients

| Dependency | Purpose |
|-----------|---------|
| No direct OpenAI/Anthropic SDK | Custom clients for each provider |
| `github.com/pkoukk/tiktoken-go v0.1.8` | Token estimation (OpenAI-compatible) |
| `golang.org/x/sync v0.21.0` | `singleflight` for provider cache dedup |

### Platform Integration

| Dependency | Purpose |
|-----------|---------|
| `github.com/godbus/dbus/v5 v5.2.2` | Linux secret-service keychain |
| `github.com/atotto/clipboard v0.1.4` | Clipboard access |
| `github.com/fsnotify/fsnotify v1.10.1` | File watcher |
| `golang.org/x/sys v0.46.0` | Low-level system calls |

### Parsing

| Dependency | Purpose |
|-----------|---------|
| `github.com/odvcencio/gotreesitter v0.20.5` | Tree-sitter Go bindings (code intelligence) |
| `github.com/BurntSushi/toml v1.6.0` | Config loading |
| `github.com/bmatcuk/doublestar/v4 v4.10.0` | Glob pattern matching |

### Rendering & Display

| Dependency | Purpose |
|-----------|---------|
| `github.com/yuin/goldmark v1.5.2` | Markdown parsing |
| `github.com/yuin/goldmark-emoji v1.0.1` | Emoji extension |
| `github.com/alecthomas/chroma v0.10.0` | Syntax highlighting |
| `github.com/lucasb-eyer/go-colorful v1.3.0` | Color math |
| `github.com/muesli/termenv v0.16.0` | Terminal color profiles |
| `github.com/olekukonko/tablewriter v0.0.5` | Table rendering |

### Indirect / Utility

- `github.com/google/uuid`, `github.com/rivo/uniseg`, `github.com/xo/terminfo`
- `github.com/microcosm-cc/bluemonday` — HTML sanitization (glamour dep)
- `github.com/dlclark/regexp2`, `github.com/clipperhouse/stringish`, `github.com/clipperhouse/uax29/v2`

## Project Structure

```
M31A/
├── cmd/m31a/           Entry point (main.go, usage.go)
├── internal/           Private packages (18 sub-packages)
│   ├── codeintel/      Code parsing & intelligence
│   ├── config/         TOML config loader & project context
│   ├── context/        Dynamic context registry
│   ├── decision/       Decision logging
│   ├── errors/         Sentinel errors
│   ├── fileutil/       Atomic file operations
│   ├── git/            Git operations
│   ├── log/            Structured logging
│   ├── logging/        Audit & security utilities
│   ├── provider/       LLM providers (OpenRouter, Zen, Nvidia)
│   ├── shell/          Platform shell execution
│   ├── tokens/         Token estimation
│   ├── tools/          18 built-in tools + subagent manager
│   ├── tui/            Bubble Tea TUI (33 screens)
│   ├── types/          Shared types & constants
│   ├── wiring/         Integration tests
│   └── workflow/       Seven-phase engine
├── pkg/                Public packages (15 packages)
│   ├── arbitrage/      Model-cost optimizer
│   ├── autodream/      Context consolidation
│   ├── bisect/         Git-bisect wrapper
│   ├── compaction/     Session compaction
│   ├── coordinator/    Drain session management
│   ├── history/        Prompt history
│   ├── keychain/       OS keychain abstraction
│   ├── ledger/         Cross-session learning store
│   ├── metrics/        Session metrics
│   ├── narrative/      Event to progress description
│   ├── retry/          Exponential backoff
│   ├── rollback/       Commit-chain manager
│   ├── session/        Session lifecycle
│   ├── skills/         Skill management
│   └── taskrunner/     Parallel task executor
├── docs/               13 documentation files
├── scripts/            validate-release.sh, verify_v1.sh
├── e2e_test.go         End-to-end binary tests
├── Makefile            Build orchestration
└── .goreleaser.yaml    Release pipeline
```

## Configuration

- **Primary config:** `~/.m31a/config.toml` (overridden by `M31A_CONFIG` env var)
- **Format:** TOML, loaded by `internal/config/`
- **Env vars:** `OPENROUTER_API_KEY`, `ZEN_API_KEY`, `NVIDIA_API_KEY` (`.env` file supported)
- **Secrets:** Stored in OS keychain (`pkg/keychain/`), never plaintext on disk
- **Config sections:** provider, model, ui, permissions, features, tools, prompts, narrative, git, verify, agents

## Platform Support

| Platform | Support | Details |
|----------|---------|---------|
| Linux amd64 | Full | Primary target |
| Linux arm64 | Full | Cross-compiled |
| macOS amd64 | Full | Cross-compiled |
| macOS arm64 | Full | Cross-compiled |
| Windows amd64 | Full | Cross-compiled |
| Windows arm64 | Excluded | goreleaser skip |
| CGO | Disabled | Hard constraint on all platforms |

## CI/CD

- **CI:** GitHub Actions (ci.yml) — lint, test, security, build matrix, release
- **Go version:** 1.25 (pinned in setup-go action)
- **Lint:** golangci-lint with govet (shadow), staticcheck, errcheck, ineffassign, unused
- **Security:** govulncheck in CI, SSRF/DNS rebinding protection in tools
- **Release:** goreleaser with draft prerelease, nfpm packages (.deb, .rpm, .apk), scoop
