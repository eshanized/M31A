# Technology Stack

**Analysis Date:** 2026-06-11

## Languages

**Primary:**
- Go 1.24+ — Entire codebase (cmd, internal, pkg)

**Secondary:**
- None (pure Go project)

## Runtime

**Environment:**
- Go 1.24 (with CGO_ENABLED=0 for static binaries)
- Cross-compilation: linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64

**Package Manager:**
- Go modules (go.mod/go.sum)
- Lockfile: go.sum present

## Frameworks

**Core:**
- Bubble Tea (charmbracelet/bubbletea) v1.3.0 — TUI framework for terminal AI coding agent
- Lip Gloss (charmbracelet/lipgloss) v1.1.0 — Terminal styling
- Bubbles (charmbracelet/bubbles) v0.20.0 — TUI components (textarea, viewport, spinner, list)
- Glamour (charmbracelet/glamour) v0.6.0 — Markdown rendering

**Testing:**
- Go standard library testing — Test files: *_test.go

**Build/Dev:**
- Makefile — Build, test, lint, release commands
- GoReleaser (.goreleaser.yaml) — Cross-platform release builds
- golangci-lint (.golangci.yml) — Linting (govet, staticcheck, errcheck, ineffassign, unused, gosimple)

## Key Dependencies

**Critical:**
- github.com/BurntSushi/toml v1.6.0 — Config file parsing (config.toml)
- github.com/pkoukk/tiktoken-go v0.1.8 — Token estimation for GPT/Claude families
- github.com/bmatcuk/doublestar/v4 v4.10.0 — Glob pattern matching with ** support

**Infrastructure:**
- github.com/godbus/dbus/v5 v5.2.2 — D-Bus for Linux keychain (secret-service)
- github.com/atotto/clipboard v0.1.4 — System clipboard access
- golang.org/x/sync v0.10.0 — singleflight for concurrent request deduplication

**TUI Utilities:**
- github.com/charmbracelet/colorprofile v0.2.3 — Terminal color profile detection
- github.com/charmbracelet/x/ansi v0.8.0 — ANSI escape sequence handling
- github.com/charmbracelet/x/term v0.2.1 — Terminal capabilities
- github.com/muesli/termenv v0.16.0 — Terminal environment detection

**Indirect (transitive):**
- github.com/alecthomas/chroma v0.10.0 — Syntax highlighting
- github.com/yuin/goldmark v1.5.2 — Markdown parsing
- github.com/microcosm-cc/bluemonday v1.0.21 — HTML sanitization
- github.com/olekukonko/tablewriter v0.0.5 — Table rendering

## Configuration

**Environment:**
- M31A_OPENROUTER_API_KEY — OpenRouter API key (1st priority)
- M31A_ZEN_API_KEY — Zen API key (1st priority)
- M31A_LOG_FORMAT — Log output format (default: json)
- M31A_LOG_LEVEL — Log verbosity level (default: info)

**Config File:**
- Location: `~/.m31a/config.toml`
- Format: TOML
- Sections: provider, model, ui, permissions, features, ledger, ghost

**Build:**
- `.golangci.yml` — Linting configuration
- `.goreleaser.yaml` — Release configuration
- `Makefile` — Build/test/lint commands

## Platform Requirements

**Development:**
- Go 1.24+
- golangci-lint (for linting)
- git (for version info and bisect features)

**Production:**
- CGO_ENABLED=0 static binary
- No external runtime dependencies
- Cross-platform: linux, darwin, windows (amd64/arm64)

---

*Stack analysis: 2026-06-11*
