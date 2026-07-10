# STRUCTURE.md — Directory Layout & Key Locations

**Analysis Date:** 2026-07-10

## Root Layout

```
M31A/
├── cmd/m31a/                    # Entry point
│   └── main.go                  # Flag parsing, config, provider registration, TUI bootstrap
├── internal/                    # Private application code (can import pkg/)
│   ├── config/                  # TOML config loading, validation, file watching
│   ├── codeintel/               # Codebase intelligence indexer (symbols, imports, complexity)
│   ├── context/                 # Dynamic context sources (datetime, env, git)
│   ├── decision/                # Decision logging (v1.5)
│   ├── errors/                  # Sentinel error definitions
│   ├── fileutil/                # File utilities (atomic write, backup, diff)
│   ├── git/                     # Git wrapper (commit, diff, status, worktree)
│   ├── log/                     # Structured logging setup (slog + file rotation)
│   ├── provider/                # LLM provider abstraction + 3 implementations
│   │   ├── openrouter/          # OpenRouter provider
│   │   ├── zen/                 # Z.ai provider
│   │   └── nvidia/              # NVIDIA NIM provider
│   ├── tools/                   # 18 built-in tools + dispatcher
│   │   └── subagent/            # Parallel subagent manager (worktrees, isolated dispatchers)
│   ├── tui/                     # Bubble Tea TUI (Elm architecture)
│   │   ├── a11y/                # Accessibility helpers
│   │   ├── commands/            # Slash command implementations
│   │   ├── components/          # Reusable UI components (sidebar, model picker, etc.)
│   │   ├── layout/              # Layout primitives
│   │   ├── streaming/           # Streaming response rendering
│   │   ├── theme/               # Dark theme (lipgloss)
│   │   ├── tuitypes/            # All tea.Msg types, Screen enum, shared TUI types
│   │   ├── app.go               # AppState (root Model), Init/Update/View/Shutdown
│   │   ├── app_state.go         # AppState fields, getters/setters
│   │   ├── app_update*.go       # Update() handlers by category
│   │   ├── app_view.go          # View() rendering
│   │   ├── app_nav.go           # Screen stack navigation
│   │   ├── repl*.go             # REPL chat interface
│   │   ├── *_model.go / *_view.go  # Per-screen model/view pairs
│   │   └── provider_registration.go  # Provider factory for main.go
│   ├── types/                   # Shared type vocabulary (WorkflowPhase, ModelInfo, Message, Tool, Task, etc.)
│   ├── workflow/                # 7-phase workflow engine
│   │   ├── engine.go            # Engine orchestration, phase runners, streaming
│   │   ├── state_machine.go     # Phase transition validation
│   │   ├── phase_coordinator.go # Pre/post-phase hooks, checkpoints, metrics
│   │   ├── prompt_builder.go    # System prompt composition
│   │   ├── context_builder.go   # Dynamic context injection
│   │   ├── prompts/             # Embedded prompt templates
│   │   └── templates/           # Website template (Next.js)
│   ├── wiring/                  # Integration tests wiring
│   └── logging/                 # Audit logging
├── pkg/                         # Public packages (CANNOT import internal/)
│   ├── arbitrage/               # Model cost optimization recommendations
│   ├── autodream/               # Proactive session compaction
│   ├── bisect/                  # Git bisect automation
│   ├── compaction/              # LLM-based session summarization
│   ├── coordinator/             # Multi-agent coordination
│   ├── history/                 # Command history with frecency
│   ├── keychain/                # OS keyring (macOS/Windows/Linux)
│   ├── ledger/                  # Session record persistence (LEDGER.md)
│   ├── metrics/                 # Tool/LLM call metrics collection
│   ├── narrative/               # Structured narrative event stream
│   ├── retry/                   # Exponential backoff with error classification
│   ├── rollback/                # Git-based rollback to session start
│   ├── session/                 # Session persistence, checkpoints, project state
│   ├── skills/                  # Skill discovery and loading
│   └── taskrunner/              # Parallel task execution with dependencies
├── e2e_test.go                  # End-to-end tests (real API calls when keys present)
├── Makefile                     # Build pipeline (fmt→tidy→vet→lint→test)
├── go.mod                       # Go 1.25.0, CGO_ENABLED=0, Bubble Tea + deps
├── go.sum
├── .goreleaser.yaml             # Cross-compile + release config
├── README.md
├── AGENTS.md                    # Agent instructions
├── CHANGELOG.md
├── CONTRIBUTING.md
├── LICENSE
├── SECURITY.md
├── TESTING.md
├── .env.example                 # Example environment variables
├── .gitignore
├── .golangci.yml                # Linter config (govet, staticcheck, errcheck, ineffassign, unused)
├── install.sh                   # Installer script
├── m31a                         # Built binary (gitignored)
├── m31a.json                    # Build metadata
├── docs/                        # Documentation
├── scripts/                     # Release validation, helpers
└── .m31a/                       # Runtime data (gitignored)
    ├── config.toml              # User config
    ├── LEDGER.md                # Session ledger
    ├── sessions/                # Session JSON files
    ├── backups/                 # Tool backup files
    └── tool-output/             # Bounded tool output store
```

## Directory Purposes

| Directory | Purpose | Key Files |
|-----------|---------|-----------|
| `cmd/m31a/` | Application entry point | `main.go` |
| `internal/config/` | Config loading, validation, hot-reload | `config.go`, `watch.go` |
| `internal/codeintel/` | Symbol indexer, import graph, complexity | `indexer.go` |
| `internal/context/` | Dynamic context sources (datetime, env, git) | `registry.go`, `sources.go` |
| `internal/decision/` | Decision receipts, logger | `logger.go`, `receipt.go`, `query.go` |
| `internal/errors/` | Sentinel errors for typed handling | `errors.go` |
| `internal/fileutil/` | Atomic write, backup, diff utilities | `atomic.go`, `backup.go`, `diff.go` |
| `internal/git/` | Git operations wrapper | `git.go` |
| `internal/log/` | Logger initialization, file rotation | `logger.go` |
| `internal/provider/` | Provider interface + 3 implementations | `interface.go`, `registry.go`, `common.go`, `capabilities.go`, `reasoning.go` |
| `internal/tools/` | Tool definitions, dispatcher, permissions | `dispatcher.go`, `defaults.go`, `bash.go`, `edit.go`, `agent.go`, `subagent/` |
| `internal/tui/` | Full TUI implementation | See Key Locations below |
| `internal/types/` | Shared type definitions | `types.go` |
| `internal/workflow/` | 7-phase workflow engine | `engine.go`, `state_machine.go`, `phase_coordinator.go` |
| `pkg/keychain/` | OS keyring abstraction | `keychain.go`, `keychain_darwin.go`, `keychain_linux.go`, `keychain_windows.go` |
| `pkg/ledger/` | Session ledger persistence | `ledger.go` |
| `pkg/metrics/` | Metrics collector | `collector.go`, `types.go` |
| `pkg/retry/` | Retry policy with classification | `policy.go` |
| `pkg/rollback/` | Git rollback operations | `rollback.go` |
| `pkg/session/` | Session/project persistence | `manager.go`, `session.go`, `checkpoint.go` |
| `pkg/taskrunner/` | Parallel task execution | `runner.go` |
| `pkg/bisect/` | Git bisect automation | `exec.go`, `bisect.go` |
| `pkg/compaction/` | Session compaction | `compaction.go`, `template.go` |
| `pkg/autodream/` | Proactive compression | `autodream.go` |
| `pkg/arbitrage/` | Model cost optimization | `arbitrage.go` |
| `pkg/coordinator/` | Multi-agent coordination | `coordinator.go` |
| `pkg/narrative/` | Narrative event stream | `engine.go`, `bridge.go` |
| `pkg/history/` | Command history | `history.go` |
| `pkg/skills/` | Skill system | `loader.go`, `discovery.go`, `skill.go` |

## Key File Locations

| Concern | Location |
|---------|----------|
| **Entry point** | `cmd/m31a/main.go` |
| **Config loading** | `internal/config/config.go` |
| **TUI App (Model)** | `internal/tui/app.go`, `internal/tui/app_state.go` |
| **TUI Update handlers** | `internal/tui/app_update.go`, `app_update_phase.go`, `app_update_commands.go` |
| **TUI View** | `internal/tui/app_view.go` |
| **REPL (chat interface)** | `internal/tui/repl.go`, `repl_model.go`, `repl_view.go` |
| **Workflow Engine** | `internal/workflow/engine.go` |
| **Phase State Machine** | `internal/workflow/state_machine.go` |
| **Phase Coordinator** | `internal/workflow/phase_coordinator.go` |
| **Provider Interface** | `internal/provider/interface.go` |
| **Provider Registry** | `internal/provider/registry.go` |
| **Provider Implementations** | `internal/provider/openrouter/`, `zen/`, `nvidia/` |
| **Tool Registry** | `internal/tools/defaults.go` |
| **Tool Dispatcher** | `internal/tools/dispatcher.go` |
| **Built-in Tools** | `internal/tools/*.go` (bash, edit, file*, glob, grep, todo*, webfetch, websearch, codemap, codecomplexity, devserver, httpcheck, agent) |
| **Subagent Manager** | `internal/tools/subagent/manager.go` |
| **Shared Types** | `internal/types/types.go` |
| **Session Management** | `pkg/session/manager.go`, `session.go`, `checkpoint.go` |
| **Keychain (OS keyring)** | `pkg/keychain/keychain.go` |
| **Ledger** | `pkg/ledger/ledger.go` |
| **Rollback** | `pkg/rollback/rollback.go` |
| **Metrics** | `pkg/metrics/collector.go` |
| **Compaction** | `pkg/compaction/compaction.go` |
| **AutoDream** | `pkg/autodream/autodream.go` |

## Naming Conventions

| Element | Convention | Example |
|---------|------------|---------|
| Packages | lowercase, singular | `provider`, `tools`, `workflow`, `session` |
| Files | snake_case.go | `engine.go`, `state_machine.go`, `prompt_builder.go` |
| Types | PascalCase | `WorkflowPhase`, `ModelInfo`, `ToolResult` |
| Interfaces | Noun + er | `LLMProvider`, `Dispatcher`, `SchemaProvider` |
| Constants | PascalCase | `PhaseInitialize`, `RiskDangerous`, `MaxHealAttempts` |
| Private fields | camelCase | `sessionID`, `workDir`, `activeModel` |
| Exported functions | PascalCase | `NewEngine()`, `RunPhase()`, `SetModel()` |
| Test files | `_test.go` suffix | `engine_test.go`, `dispatcher_test.go` |
| Test helpers | `testutil` package | `internal/testutil/` |

## Where to Add New Code

### New Feature (Workflow Phase)
1. Add phase constant to `internal/types/types.go` (`WorkflowPhase`)
2. Add transition rules to `internal/workflow/state_machine.go` (`validTransitions`)
3. Implement `run<Phase>()` in `internal/workflow/engine.go`
4. Add TUI screen in `internal/tui/` (`<phase>_model.go`, `<phase>_view.go`)
5. Register screen in `internal/tui/tuitypes/types.go` (`Screen` enum)
6. Add navigation in `internal/tui/app_nav.go`

### New Tool
1. Create `internal/tools/<toolname>.go` implementing `types.Tool` interface
2. Optionally implement `types.SchemaProvider` for JSON Schema
3. Register in `internal/tools/defaults.go` → `DefaultDispatcher()`
4. Add permission rule defaults in `internal/config/config.go` if needed

### New Provider
1. Create `internal/provider/<name>/provider.go` implementing `LLMProvider`
2. Add registration in `internal/tui/provider_registration.go` → `RegisterProvider()`
3. Add config section in `internal/config/config.go` (`ProviderConfig`)

### New Public Package (pkg/)
1. Create `pkg/<name>/` with `doc.go` (package doc)
2. **Must not import** `github.com/eshanized/M31A/internal/...`
3. Can import other `pkg/` packages and stdlib
4. Add tests in `_test.go` files

### New Config Option
1. Add field to appropriate struct in `internal/config/config.go`
2. Add TOML tag (`toml:"field_name"`)
3. Handle default in `DefaultConfig()` or `Load()`
4. Wire through to consumer (e.g., `Engine`, `Dispatcher`, `TUI`)

## Module Boundaries

```
┌─────────────────────────────────────────────────────────────────┐
│                        import graph                              │
├─────────────────────────────────────────────────────────────────┤
│  cmd/m31a                                                        │
│       │                                                          │
│       ▼                                                          │
│  internal/tui ──────────────────────────────────┐               │
│       │                                          │               │
│       ▼                                          ▼               │
│  internal/workflow ←───────────────────── internal/tools        │
│       │                     │                     │              │
│       ▼                     ▼                     ▼              │
│  internal/provider ◄───── internal/types ──────► pkg/*          │
│       │                                          ▲               │
│       └──────────────────────────────────────────┘               │
│                         (pkg CANNOT import internal)             │
└─────────────────────────────────────────────────────────────────┘
```

**Enforcement:** Go module system — `pkg/` has no path to `internal/` packages.

## Special Directories

| Directory | Purpose | Generated | Committed |
|-----------|---------|-----------|-----------|
| `.m31a/` | Runtime data (config, sessions, ledger, backups) | Yes | No (gitignored) |
| `dist/` | Cross-compiled release binaries | Yes (goreleaser) | No |
| `vendor/` | Not used (Go modules) | N/A | No |
| `.planning/` | GSD planning artifacts | Yes | Yes |
| `M31A.wiki/` | Wiki content (separate repo) | No | Yes |

## Build Artifacts

| Artifact | Location | Generated By |
|----------|----------|--------------|
| Binary (current platform) | `./m31a` | `make build` |
| Debug binary | `./m31a-debug` | `make debug` |
| Cross-compiled binaries | `dist/` | `make cross` / goreleaser |
| Coverage report | `coverage.out`, `coverage.html` | `make test`, `make cover` |

---

*Structure analysis: 2026-07-10*