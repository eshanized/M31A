# Technology Stack

**Project:** M31A
**Researched:** 2026-08-23

## Recommended Stack

### Core Framework

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| Go | 1.26+ | Primary language | Static binaries, CGO_ENABLED=0, cross-platform, excellent for terminal apps, process supervision, concurrent agent runtime |
| Bubble Tea | v2 (charm.land/bubbletea/v2) | TUI framework | Declarative View() API, 36-screen spec support, mouse/keyboard handling, responsive layouts, alt-screen, mouse modes |
| Lip Gloss | v2 (charm.land/lipgloss/v2) | Terminal styling | Semantic tokens, composable styles, works with Bubble Tea v2 declarative model |
| Bubbles | v0.18+ (github.com/charmbracelet/bubbles) | TUI components | Text input, viewport, table, list, spinner, progress bar — building blocks for 26-component library |
| Zone | v0.1+ (github.com/charmbracelet/zone) | Mouse interaction | Mouse zone tracking for clickable UI elements in complex TUI |

### Database & Persistence

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| modernc.org/sqlite | latest | Embedded SQLite driver | Pure Go (CGO-free), WAL mode, transaction hooks for event sourcing, hot backups, serialization, cross-compile friendly |
| go-sqlite3 | NOT USED | Alternative | Requires CGO — violates static binary constraint |

### LLM Provider Interface

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| go-openai | latest (github.com/sashabaranov/go-openai) | OpenAI-compatible client | Type-safe, streaming support, Azure config, error handling with retry logic, used by NVIDIA Build |
| Custom provider adapter | Internal | Provider abstraction | Isolates NVIDIA-specific chat_template_kwargs (enable_thinking, force_nonempty_content), enables future multi-model routing |

### Code Intelligence

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| go-tree-sitter | latest (github.com/tree-sitter/go-tree-sitter) | Multi-language parsing | Incremental parsing, Tree.Edit for edits, SetIncludedRanges for embedded langs, streaming for large files |
| Tree-sitter grammars | latest per language | Language-specific parsers | tree-sitter-go, tree-sitter-javascript, tree-sitter-python, tree-sitter-rust, tree-sitter-typescript, tree-sitter-java, etc. |
| go-lsp | latest (github.com/sourcegraph/go-lsp) | LSP client | Language Server Protocol client for Go — semantic analysis, go-to-definition, references, hover |
| gopls | External binary | Go language server | Standard Go LSP server — invoked as subprocess |

### Git Operations

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| go-git | v5 (github.com/go-git/go-git/v5) | Pure Go Git | No CGO, worktree support, submodule safety (pathutil.ValidTreePath), billy filesystem abstraction, programmatic access to Git internals |
| git2go | NOT USED | Alternative | Requires libgit2 (CGO) — violates static binary constraint |
| Shell git | Fallback only | Edge cases | Only for operations go-git doesn't support well (e.g., complex merge conflict resolution, Git LFS) |

### Configuration

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| koanf | v2 (github.com/knadh/koanf/v2) | Configuration management | Lightweight, no viper's complexity, supports TOML/JSON/YAML, env var binding, nested maps, providers pattern |
| cleanenv | NOT USED | Alternative | Simpler but less flexible for layered config (file + env + flags) |
| viper | NOT USED | Alternative | Heavy, global state, over-engineered for M31A's needs |

### CLI Framework

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| kong | latest (github.com/alecthomas/kong) | CLI parsing | Declarative struct tags, subcommands, variables, completion, help generation, positional args, type-safe |
| cobra | NOT USED | Alternative | Verbose, imperative, global state — kong is more modern and type-safe |
| urfave/cli | NOT USED | Alternative | v2 is better but kong's declarative approach fits Go idioms better |

### Structured Logging

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| slog | stdlib (log/slog) | Structured logging | Standard library since Go 1.21, zero dependencies, structured output, log levels, handlers (JSON/text), source location |
| zerolog | NOT PRIMARY | Alternative | Faster but external dependency; slog is sufficient and standard |
| zap | NOT USED | Alternative | More complex API, external dependency — slog wins for simplicity |

### Event-Driven Architecture

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| Custom event bus | Internal | Event sourcing | Append-only events in SQLite (events.db), monotonic ordering, replay/recovery, derived state projections |
| watermill | NOT USED | Alternative | Overkill for embedded single-process event bus; M31A needs simple, durable event log |
| nats.go | NOT USED | Alternative | Distributed messaging — not needed for v1 single-process runtime |

### Capability-Based Permissions

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| Custom permission engine | Internal | Capability evaluation | Typed capability registry (filesystem.read, filesystem.write, git.read, git.write, shell.exec, etc.), scope (repository/workspace), risk class (low/medium/high/critical), conditions (deny patterns), runtime evaluation at tool execution |
| OPA/Rego | NOT USED | Alternative | External dependency, overkill for embedded policy evaluation |

### Testing

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| testify | v1.9+ (github.com/stretchr/testify) | Assertions/mocks | Standard Go testing toolkit |
| gomock | v1.6+ (go.uber.org/mock) | Mock generation | Interface mocking for provider adapters, tool registry |
| Bubble Tea test helpers | Internal | TUI testing | Headless program runner, snapshot testing, key/mouse injection, viewport assertions |
| ginkgo/gomega | NOT USED | Alternative | BDD style adds complexity; standard testing + testify is idiomatic Go |

### Development Tools

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| golangci-lint | latest | Linting | govet, staticcheck, errcheck, ineffassign, unused — matches AGENTS.md |
| gofumpt | latest | Formatting | Stricter gofmt |
| goimports | latest | Import organization | Standard |
| goreleaser | latest | Cross-compilation | Linux/amd64, arm64; Darwin/amd64, arm64; Windows/amd64 |

---

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| TUI Framework | Bubble Tea v2 | tview, termui, gocui | Bubble Tea has Elm architecture, best ecosystem, active maintenance, v2 declarative API |
| SQLite Driver | modernc.org/sqlite | mattn/go-sqlite3, go-sqlite3 | CGO dependency breaks static builds and cross-compilation |
| LLM Client | go-openai | anthropic-sdk-go, genai | go-openai supports OpenAI-compatible APIs (NVIDIA Build), streaming, Azure |
| Config | koanf | viper, cleanenv | Viper is heavy with global state; cleanenv lacks layered providers |
| CLI | kong | cobra, urfave/cli | Kong's declarative struct tags are type-safe and maintainable |
| Logging | slog | zerolog, zap | Standard library, zero deps, structured, sufficient performance |
| Git | go-git v5 | git2go, shell git | CGO-free, pure Go, programmatic, worktree/submodule support |
| Event Bus | Custom + SQLite | watermill, nats, kafka | Single-process embedded runtime doesn't need distributed messaging |

---

## Installation

```bash
# Core runtime
go get charm.land/bubbletea/v2@latest
go get charm.land/lipgloss/v2@latest
go get github.com/charmbracelet/bubbles@latest
go get github.com/charmbracelet/zone@latest

# Database
go get modernc.org/sqlite@latest

# LLM provider
go get github.com/sashabaranov/go-openai@latest

# Code intelligence
go get github.com/tree-sitter/go-tree-sitter@latest
go get github.com/tree-sitter/tree-sitter-go/bindings/go@latest
go get github.com/tree-sitter/tree-sitter-javascript/bindings/go@latest
go get github.com/tree-sitter/tree-sitter-python/bindings/go@latest
go get github.com/tree-sitter/tree-sitter-typescript/bindings/go@latest
go get github.com/tree-sitter/tree-sitter-rust/bindings/go@latest
go get github.com/sourcegraph/go-lsp@latest

# Git
go get github.com/go-git/go-git/v5@latest
go get github.com/go-git/go-billy/v5@latest

# Config
go get github.com/knadh/koanf/v2@latest

# CLI
go get github.com/alecthomas/kong@latest

# Testing
go get github.com/stretchr/testify@latest
go get go.uber.org/mock@latest

# Dev tools (install via go install or system package manager)
# go install github.com/golangci/golangci-lint/cmd/golangci-lint@latest
# go install mvdan.cc/gofumpt@latest
# go install golang.org/x/tools/cmd/goimports@latest
# go install github.com/goreleaser/goreleaser@latest
```

---

## Sources

- Bubble Tea v2: https://github.com/charmbracelet/bubbletea/blob/main/UPGRADE_GUIDE_V2.md (Context7, MEDIUM confidence)
- modernc.org/sqlite: https://context7.com/modernc-org/sqlite/llms.txt (Context7, MEDIUM confidence)
- go-tree-sitter: https://context7.com/tree-sitter/go-tree-sitter/llms.txt (Context7, MEDIUM confidence)
- go-openai: https://context7.com/sashabaranov/go-openai/llms.txt (Context7, MEDIUM confidence)
- go-git: https://github.com/go-git/go-git/blob/main/README.md (Context7, MEDIUM confidence)
- Go 1.26 stdlib (slog, context, etc.): Official Go documentation
- koanf: https://github.com/knadh/koanf (community knowledge)
- kong: https://github.com/alecthomas/kong (community knowledge)
- Project constraints: PROJECT.md, CONTEXT_M31A.md, UI-SPEC.md

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| TUI Stack | HIGH | Bubble Tea v2 is mature, v2 upgrade guide is official, matches UI-SPEC requirements exactly |
| Database | HIGH | modernc.org/sqlite is the only pure-Go SQLite with WAL, hooks, backups, serialization |
| LLM Interface | HIGH | go-openai is the standard for OpenAI-compatible APIs; NVIDIA Build uses this exact API |
| Code Intelligence | MEDIUM | go-tree-sitter is standard; go-lsp has fewer docs but sourcegraph is authoritative |
| Git Operations | HIGH | go-git v5 is the only mature pure-Go Git library with worktree/submodule support |
| Config/CLI/Logging | MEDIUM | Based on 2025 community consensus; no major shifts expected |
| Event Bus | MEDIUM | Custom is correct for embedded single-process; patterns well-established |
| Permissions | MEDIUM | Custom engine required for capability-based model; no off-the-shelf fit |