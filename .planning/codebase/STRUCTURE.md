# STRUCTURE.md — M31A Directory Layout

last_mapped_commit: 3836de6de09785c873f87a86dd633ccb4ab886fa

## Top-Level Layout

```
M31A/
├── cmd/m31a/                  # Entry point (main package)
│   ├── main.go                # CLI flags, provider setup, TUI launch (588 lines)
│   ├── main_test.go           # E2E tests (compiles and runs binary)
│   ├── usage.go               # Help text generation
│   └── usage_test.go
├── internal/                  # Private application code
│   ├── core/                  # Foundation types
│   │   ├── config/            # TOML config loading, validation, merge
│   │   ├── errors/            # Custom error types with codes
│   │   └── types/             # Shared type definitions (Tool, Message, etc.)
│   ├── engine/                # Business logic
│   │   ├── bisect/            # Binary search for failing steps
│   │   ├── compaction/        # Context compaction for long sessions
│   │   ├── coordinator/       # Task coordination
│   │   ├── decision/          # Decision logic
│   │   ├── narrative/         # Narrative/story generation
│   │   ├── rollback/          # Git-based rollback
│   │   ├── session/           # Session persistence (18 files)
│   │   ├── taskrunner/        # Task execution
│   │   ├── tokens/            # Token estimation (tiktoken)
│   │   └── workflow/          # Phase orchestration (100+ files)
│   ├── infrastructure/        # Low-level utilities
│   │   ├── fileutil/          # File helpers
│   │   └── retry/             # Retry logic
│   ├── integrations/          # External service clients
│   │   ├── arbitrage/         # Price arbitrage
│   │   ├── autodream/         # Auto-dream feature
│   │   ├── codeintel/         # Code intelligence
│   │   ├── context/           # Context management
│   │   ├── git/               # Git CLI wrapper
│   │   ├── history/           # History tracking
│   │   ├── keychain/          # OS keychain (Linux/macOS/Windows)
│   │   ├── ledger/            # Ledger logging
│   │   ├── log/               # Structured logging (slog)
│   │   ├── metrics/           # Tool execution metrics
│   │   ├── provider/          # LLM providers (3: OpenRouter, Zen, NVIDIA)
│   │   ├── shell/             # Shell integration
│   │   └── skills/            # Skill system
│   ├── tests/                 # Integration tests
│   │   ├── tools/             # Tool integration tests
│   │   └── tui/               # TUI integration tests
│   ├── testutil/              # Test helpers
│   │   ├── ci/                # CI-specific utilities
│   │   └── testtimeout/       # Timeout helpers
│   ├── tools/                 # Tool system (18+ tools)
│   │   ├── ai/                # AI-specific tools (AskUserQuestion)
│   │   ├── codeanalysis/      # CodeMap, CodeComplexity
│   │   ├── exec/              # Bash, DevServer, OutputStore
│   │   ├── fileops/           # FileRead/Write/Edit/Delete/Move/List
│   │   ├── git/               # Git operations tool
│   │   ├── network/           # WebFetch, WebSearch, HTTPCheck
│   │   ├── search/            # Glob, Grep (with DNS cache)
│   │   ├── subagent/          # Subagent management
│   │   └── todo/              # TodoWrite/TodoRead
│   └── ui/tui/                # Presentation layer (180+ files)
│       ├── commands/          # TUI command handlers
│       ├── components/        # Reusable UI components
│       ├── layout/            # Screen layout logic
│       ├── streaming/         # Stream rendering
│       ├── theme/             # Dark theme
│       └── tuitypes/          # TUI-specific types
├── tests/                     # End-to-end tests
│   ├── e2e/                   # Binary compilation + run tests
│   └── testutil/              # E2E test helpers
├── docs/                      # Documentation
├── scripts/                   # Build/release scripts
├── .github/                   # GitHub Actions, templates
├── .planning/                 # GSD planning directory
├── Makefile                   # Build system (321 lines)
├── go.mod / go.sum            # Go module definitions
├── .golangci.yml              # Linter config
├── .goreleaser.yaml           # Release config
├── AGENTS.md                  # AI agent instructions
├── TESTING.md                 # Testing guidelines
├── CONTRIBUTING.md            # Contribution guidelines
└── README.md                  # Project overview
```

## Key Locations

| Concern | Primary Files |
|---------|---------------|
| CLI entry | `cmd/m31a/main.go` |
| Config loading | `internal/core/config/loader.go`, `types.go` |
| TUI app | `internal/ui/tui/app.go`, `app_state.go` |
| REPL model | `internal/ui/tui/repl_model.go` |
| Workflow engine | `internal/engine/workflow/engine.go` |
| Phase handlers | `internal/engine/workflow/initialize.go`, `discuss.go`, `plan.go`, `execute.go`, `verify.go`, `runtime.go`, `ship.go` |
| Tool dispatcher | `internal/tools/dispatcher.go` |
| Tool definitions | `internal/tools/defaults.go`, `tooldefs.go` |
| Provider interface | `internal/integrations/provider/interface.go` |
| Session manager | `internal/engine/session/manager.go` |
| Permission system | `internal/tools/permissions.go`, `persistent_permissions.go` |

## Naming Conventions

| Pattern | Example | Usage |
|---------|---------|-------|
| `*_test.go` | `engine_test.go` | Unit tests |
| `*_extra_test.go` | `bisect_extra_test.go` | Additional test coverage |
| `*_race_test.go` | `engine_race_test.go` | Race condition tests |
| `*_benchmark_test.go` | `edit_benchmark_test.go` | Performance benchmarks |
| `doc.go` | `internal/engine/bisect/doc.go` | Package documentation |
| Platform files | `keychain_linux.go`, `ship_lock_unix.go` | OS-specific implementations |
| Constants | `constants.go` | Package-level constants |

## File Count Summary

| Directory | Files | Notes |
|-----------|-------|-------|
| `internal/ui/tui/` | ~180 | Largest package |
| `internal/engine/workflow/` | ~100 | Core business logic |
| `internal/tools/` | ~50 | Tool implementations |
| `internal/integrations/provider/` | ~26 | LLM provider layer |
| `internal/engine/session/` | ~18 | Session management |
| `cmd/m31a/` | 6 | Entry point |
