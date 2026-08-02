# Technology Stack

**Last mapped:** 2026-08-02
**Project:** M31 Autonomous (Terminal AI Coding Agent)

## Language & Runtime

- **Primary language:** Go 1.25.12 (from `go.mod`)
- **Build system:** Go modules + Makefile
- **Binary type:** Static binary (CGO_ENABLED=0)
- **Cross-compilation:** linux/{amd64,arm64}, darwin/{amd64,arm64}, windows/{amd64,arm64}

## Core Dependencies

### Direct Dependencies (from `go.mod`)

| Dependency | Version | Purpose |
|------------|---------|---------|
| `github.com/BurntSushi/toml` | v1.6.0 | TOML configuration parsing |
| `github.com/bmatcuk/doublestar/v4` | v4.10.0 | Glob pattern matching |
| `github.com/charmbracelet/bubbles` | v0.20.0 | Terminal UI components |
| `github.com/charmbracelet/bubbletea` | v1.3.0 | Terminal UI framework (Elm architecture) |
| `github.com/charmbracelet/glamour` | v0.6.0 | Markdown rendering |
| `github.com/charmbracelet/lipgloss` | v1.1.0 | Terminal styling |
| `github.com/godbus/dbus/v5` | v5.2.2 | D-Bus integration (Linux) |
| `github.com/mattn/go-runewidth` | v0.0.19 | Unicode character width |
| `github.com/pkoukk/tiktoken-go` | v0.1.8 | Token counting for LLM context |
| `golang.org/x/sync` | v0.22.0 | Synchronization primitives (singleflight) |

### Indirect Dependencies

- `github.com/alecthomas/chroma` - Syntax highlighting
- `github.com/odvcencio/gotreesitter` - Tree-sitter parsing
- Various Charm libraries for terminal UI

## Build Configuration

- **Makefile targets:** build, debug, test, lint, cross-compile, release
- **Build flags:** `-trimpath`, `-ldflags "-s -w"` (strip debug info for release)
- **Test framework:** Go standard testing with race detector
- **Linting:** golangci-lint (govet, staticcheck, errcheck, ineffassign, unused)
- **CI:** GitHub Actions (`.github/workflows/ci.yml`)

## Project Structure

```
M31A/
├── cmd/m31a/          # Main entry point
├── internal/          # Private application code
│   ├── engine/        # Workflow engine (7 phases)
│   ├── provider/      # LLM provider integrations
│   ├── tools/         # Built-in tools (18+)
│   ├── ui/            # Terminal UI (Bubble Tea)
│   └── types/         # Shared type definitions
├── pkg/               # Public packages (keychain, etc.)
├── tests/             # Test utilities and E2E tests
├── scripts/           # Build and release scripts
└── dist/              # Build output directory
```

## Configuration Files

- `m31a.json` - Application metadata and version info
- `go.mod` / `go.sum` - Go module dependencies
- `Makefile` - Build automation
- `.github/workflows/ci.yml` - CI/CD pipeline

## Runtime Requirements

- POSIX shell (bash, zsh, fish)
- Git (for version control integration)
- API keys for LLM providers (OpenRouter, Zen, Nvidia)
- OS keychain for secure credential storage

## Development Tools

- **IDE:** Any Go-compatible IDE (VS Code, GoLand, etc.)
- **Formatting:** `go fmt`, `goimports`
- **Testing:** `go test -race ./...`
- **Benchmarking:** `go test -bench=. -benchmem`
- **Coverage:** `go tool cover -html=coverage.out`

## Platform Support

- **Primary:** Linux, macOS (darwin), Windows
- **Architecture:** amd64, arm64
- **Installation:** Homebrew (macOS), Scoop (Windows), native packages (Linux)