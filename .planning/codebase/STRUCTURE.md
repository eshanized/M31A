---
focus: arch
last_mapped: 2026-05-30
---

# M31A — Directory Structure

```
M31A/
├── AGENTS.md                  # OpenCode project memory — architectural constraints
├── CHANGELOG.md               # Release changelog
├── CONTRIBUTING.md            # Contribution guide
├── LICENSE                    # MIT
├── Makefile                   # Build targets: build, test, lint, vet, clean, release
├── README.md                  # Project overview, quick start
├── install.sh                 # curl-pipe-bash install script
├── opencode.json              # OpenCode agent configuration
├── go.mod / go.sum            # Go module definition (Go 1.22)
├── .golangci.yml              # Linter configuration (govet, staticcheck, errcheck, etc.)
├── .goreleaser.yaml           # Cross-platform release pipeline
│
├── cmd/
│   ├── m31a/main.go           # Binary entry point (flags, config, provider init, TUI launch)
│   └── test_zen/main.go       # Standalone Zen provider test utility
│
├── internal/                  # Private application packages
│   ├── config/
│   │   ├── types.go           # Config struct + nested provider/model/ui/permissions/features/ledger configs
│   │   ├── loader.go          # TOML Load/Save, atomicWrite, ResolveAPIKeys
│   │   └── loader_test.go     # Config load/save tests
│   │
│   ├── errors/
│   │   └── errors.go          # 15 sentinel errors (ErrProviderUnreachable, ErrRateLimited, etc.)
│   │
│   ├── git/
│   │   ├── git.go             # Git wrapper: Init, Commit, Log, Diff, Bisect, HeadHash, Reset
│   │   └── git_test.go        # Git operation tests
│   │
│   ├── log/
│   │   ├── log.go             # Structured slog logger with daily rotation, 7-day retention
│   │   └── log_test.go        # Logger tests
│   │
│   ├── provider/
│   │   ├── interface.go       # LLMProvider interface, ChatRequest, ToolDefinition
│   │   ├── registry.go        # ProviderRegistry: thread-safe name→provider map
│   │   ├── fallback.go        # FindFallbackProvider logic
│   │   ├── cache.go           # ModelCache with TTL, stale fallback for 24h
│   │   ├── sse.go             # SSE line-by-line parser (bufio.Scanner)
│   │   ├── reasoning.go       # Reasoning config per model family, SSE chunk parsing
│   │   ├── openrouter/
│   │   │   ├── client.go      # OpenRouter HTTP client (FetchModels, ChatCompletionStream)
│   │   │   └── client_test.go # Mocked server tests for OpenRouter
│   │   └── zen/
│   │       ├── client.go      # Zen HTTP client (OpenAI-compatible)
│   │       └── client_test.go # Mocked server tests for Zen
│   │   ├── registry_test.go, cache_test.go, sse_test.go, reasoning_test.go
│   │
│   ├── tokens/
│   │   ├── estimator.go       # Token estimation (tiktoken-go + rune fallback + EMA calibration)
│   │   └── estimator_test.go  # Token estimation tests
│   │
│   ├── tools/
│   │   ├── interface.go       # PermissionRequest, PermissionResponse, PermissionGate
│   │   ├── dispatcher.go      # Tool dispatcher: register, execute, permission gate
│   │   ├── bash.go            # Bash execution (PTY on Linux/macOS, pipes on Windows)
│   │   ├── bash_unix.go       # PTY helpers (Linux/macOS)
│   │   ├── bash_windows.go    # Pipe fallback (Windows)
│   │   ├── fileread.go        # File reading with binary detection, 5MB limit
│   │   ├── filewrite.go       # Atomic write with backup
│   │   ├── glob.go            # Glob with doublestar for recursive patterns
│   │   ├── grep.go            # Grep with ripgrep fallback to pure-Go
│   │   └── *_test.go          # Individual tool tests
│   │
│   ├── tui/                   # Bubble Tea application
│   │   ├── app.go             # AppState: Init/Update/View, screen routing, workflow orchestration
│   │   ├── types.go           # Screen enum, message types (AppMsg, PhaseResultMsg, etc.)
│   │   ├── commands.go        # Slash command system: 16 handlers, CommandRegistry, ParseCommand
│   │   ├── repl.go            # ReplModel: streaming chat, viewport, textarea
│   │   ├── firstrun.go        # FirstRunModel: API key setup wizard
│   │   ├── settings.go        # SettingsModel: 6-tab config editor (General/Provider/Model/etc.)
│   │   ├── resume.go          # ResumeModel: session browser/loader
│   │   ├── modelselector.go   # ModelSelector: full-screen model browser with fuzzy search
│   │   ├── plan.go            # PlanModel: task list + cost/time panel
│   │   ├── execute.go         # ExecuteModel: task progress + live tool cards
│   │   ├── verify.go          # VerifyModel: pass/fail checklist
│   │   ├── ship.go            # ShipModel: summary banner
│   │   ├── header.go          # Header: brand, model badge, context bar, health status
│   │   ├── statusbar.go       # Status bar: current operation, timestamp
│   │   ├── health.go          # Health check ticker logic
│   │   ├── cache.go           # Cache refresh ticker logic
│   │   ├── streaming.go       # Streaming token handling
│   │   ├── theme/
│   │   │   ├── theme.go       # Dark/Light/Auto theme with all lipgloss styles
│   │   │   └── theme_test.go
│   │   ├── components/
│   │   │   ├── message.go     # Message bubble rendering
│   │   │   ├── thinking.go    # Collapsible thinking blocks
│   │   │   ├── permission.go  # Permission modal overlay
│   │   │   ├── toolcard.go    # Rich tool call cards
│   │   │   └── *_test.go
│   │   └── *_test.go
│   │
│   ├── types/
│   │   ├── types.go           # Core types: Message, Task, ToolCall, ModelInfo, Session, etc.
│   │   └── constants.go       # Constants: ModelCacheTTL, MaxFileSize, MaxToolOutputChars, etc.
│   │
│   └── workflow/
│       ├── engine.go          # Engine: RunPhase, Transition, task parsing, validation
│       ├── initialize.go      # Phase: create session, detect project type, init git
│       ├── discuss.go         # Phase: generate clarifying questions, capture answers
│       ├── plan.go            # Phase: LLM task list generation, schema validation
│       ├── execute.go         # Phase: task execution with LLM + tool calls + git commits
│       ├── verify.go          # Phase: file checks, syntax, tests, self-heal
│       ├── ship.go            # Phase: final commit, ledger update, archive
│       ├── prompts/           # Embedded prompt templates (*.md via go:embed)
│       └── *_test.go
│
├── pkg/                       # Public packages (reusable)
│   ├── arbitrage/
│   │   ├── arbitrage.go       # Scorer (complexity), CompareModels, Recommend, ShouldArbitrage
│   │   └── arbitrage_test.go
│   ├── autodream/
│   │   ├── autodream.go       # Consolidator: context consolidation, pause/resume, stats
│   │   └── autodream_test.go
│   ├── bisect/
│   │   ├── bisect.go          # Git bisect wrapper for regression detection
│   │   └── bisect_test.go
│   ├── keychain/
│   │   ├── keychain.go        # Keychain interface + fallback
│   │   ├── keychain_linux.go  # freedesktop Secret Service (godbus/dbus)
│   │   ├── keychain_darwin.go # macOS Keychain Services
│   │   ├── keychain_windows.go # Windows Credential Manager
│   │   ├── errors.go          # ErrKeyNotFound, ErrKeychainUnavailable
│   │   └── keychain_test.go
│   ├── ledger/
│   │   ├── ledger.go          # Ledger: append, query, stats, truncate, markdown persistence
│   │   └── ledger_test.go
│   ├── rollback/
│   │   ├── rollback.go        # Chain, SoftReset, HardReset, SafeReset, Preview
│   │   └── rollback_test.go
│   ├── session/
│   │   ├── session.go         # Session struct (wraps types.Session with runtime state)
│   │   ├── manager.go         # Manager: CRUD for session files
│   │   ├── checkpoint.go      # Checkpoint: save/load checkpoints
│   │   ├── planning.go        # Planning files: PROJECT.md, TASKS.md, STATE.md parser/writer
│   │   ├── session_info.go    # SessionInfo for listing
│   │   └── *_test.go
│   └── taskrunner/
│       ├── runner.go          # Runner: Schedule (Kahn's algorithm), ExecuteGroup, status tracking
│       └── runner_test.go
│
├── docs/
│   ├── ARCHITECTURE.md        # Package dependency graph, data flow, threading model
│   ├── INTERFACES.md          # All Go interface definitions (mirror for LLM context)
│   └── TYPES.md               # Env vars, constants, sentinel errors reference
│
├── adrenaline/                # Planning artifacts
│   ├── ROADMAP.md             # Implementation roadmap (21 weeks V1)
│   ├── idea.md                # Original concept
│   └── REFERENCE.md           # Reference specification
│
├── .planning/                 # GSD workflow state (current)
│   ├── PROJECT.md             # Project goal and metadata
│   ├── ROADMAP.md             # Phase roadmap
│   ├── STATE.md               # Current milestone/phase status
│   ├── CONTEXT.md             # Phase 0 context and decisions
│   ├── REQUIREMENTS.md        # Requirements
│   └── codebase/              # This mapping output
│
└── .github/                   # CI/CD workflows
    └── workflows/
        ├── ci.yml             # Lint + test + build matrix
        └── release.yml        # goreleaser release
```

## Naming Conventions

- **Files**: snake_case for multi-word files (e.g., `bash_unix.go`, `modelselector.go`)
- **Go types**: PascalCase exported, camelCase unexported
- **Test files**: `*_test.go` suffix (Go convention)
- **Package names**: lowercase, single word where possible
- **Directories under `internal/`**: feature areas (config, tools, provider, tui, workflow)
- **Directories under `pkg/`**: standalone capabilities (arbitrage, autodream, bisect, keychain, ledger, rollback, session, taskrunner)

## Key Files

| File | Lines | Purpose |
|------|-------|---------|
| `internal/tui/app.go` | 1056 | TUI app state, screen routing, workflow orchestration |
| `internal/workflow/engine.go` | 797 | Workflow engine, phase runner, task parsing/validation |
| `internal/tui/commands.go` | 716 | Slash command system (16 handlers) |
| `internal/tui/repl.go` | ~500 | Main chat REPL with streaming |
| `internal/provider/reasoning.go` | 167 | Reasoning config per model family |
| `internal/provider/openrouter/client.go` | 279 | OpenRouter HTTP client |
| `internal/provider/zen/client.go` | 260 | Zen HTTP client |
| `pkg/ledger/ledger.go` | 511 | Cross-session learning ledger |
| `pkg/arbitrage/arbitrage.go` | 286 | Model cost optimization |
| `pkg/autodream/autodream.go` | 355 | Context consolidation |
