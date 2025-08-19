# Technology Stack

**Analysis Date:** [YYYY-MM-DD]

## Languages

**Primary:**
- Go 1.24 - Core application logic, backend, and CLI framework

**Secondary:**
- Markdown - Documentation and ledger storage
- Shell - Build/utility scripts (`Makefile`, `install.sh`)

## Runtime

**Environment:**
- Go Runtime 1.24

**Package Manager:**
- go modules (go 1.24)
- Lockfile: present (`go.sum`)

## Frameworks

**Core:**
- BubbleTea (`github.com/charmbracelet/bubbletea` v1.3.0) - Terminal UI framework
- Lipgloss (`github.com/charmbracelet/lipgloss` v1.1.0) - Terminal styling
- Glamour (`github.com/charmbracelet/glamour` v0.6.0) - Markdown rendering in terminal
- Bubbles (`github.com/charmbracelet/bubbles` v0.20.0) - Pre-built TUI components

**Testing:**
- Go standard testing (`testing` package) - Unit and integration tests

**Build/Dev:**
- Make - Build system (`Makefile`)
- GoReleaser - Release automation (`.goreleaser.yaml`)
- golangci-lint - Linting (`.golangci.yml`)

## Key Dependencies

**Critical:**
- `github.com/BurntSushi/toml` v1.6.0 - Configuration file parsing
- `github.com/bmatcuk/doublestar/v4` v4.10.0 - Glob pattern matching for file operations
- `github.com/pkoukk/tiktoken-go` v0.1.8 - Token counting for OpenAI and other models
- `golang.org/x/sync` v0.10.0 - Concurrency primitives (e.g. singleflight)

**Infrastructure:**
- `github.com/godbus/dbus/v5` v5.2.2 - D-Bus interaction for OS integrations

## Configuration

**Environment:**
- TOML configuration file structure defined in `internal/config/types.go`
- Local configurations mapped to structures like `ProviderConfig`, `UIConfig`, `PermissionsConfig`

**Build:**
- `go.mod` / `go.sum`
- `Makefile`
- `.goreleaser.yaml`

## Platform Requirements

**Development:**
- Go 1.24+

**Production:**
- Cross-platform binaries (Windows, macOS, Linux supported via Go compilation)
- Terminal emulator required for TUI execution

---

*Stack analysis: [YYYY-MM-DD]*