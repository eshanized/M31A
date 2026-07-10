# Codebase Structure

**Analysis Date:** 2026-07-10

## Directory Layout

```
M31A/
├── cmd/
│   └── m31a/                  # CLI entry point
│       ├── main.go            # Flag parsing, config, provider registration, TUI launch
│       └── usage.go           # Help/usage text
├── internal/                  # Private application code
│   ├── codeintel/             # Codebase intelligence (import graph, symbols, relevance)
│   ├── config/                # TOML config loading, merging, types
│   ├── context/               # Dynamic system context sources (date, env, git)
│   ├── decision/              # Decision logging for audit trail
│   ├── errors/                # Sentinel errors (ErrPhaseTransition, ErrContextExceeded, etc.)
│   ├── fileutil/              # Atomic writes, file locking (platform-specific)
│   ├── git/                   # Git operations (commit, branch, diff, worktree)
│   ├── log/                   # Logger initialization (slog)
│   ├── logging/               # Audit logging
│   ├── provider/              # LLM provider abstraction + registry
│   │   ├── interface.go       # LLMProvider interface
│   │   ├── registry.go        # Thread-safe provider registry
│   │   ├── base_client.go     # Shared HTTP client logic
│   │   ├── cache.go           # Model list caching
│   │   ├── capabilities.go    # Model capability detection
│   │   ├── fallback.go        # Provider fallback logic
│   │   ├── reasoning.go       # Reasoning model detection
│   │   ├── sse.go             # Server-sent events parsing
│   │   ├── openrouter/        # OpenRouter provider implementation
│   │   ├── zen/               # Zen provider implementation
│   │   └── nvidia/            # NVIDIA NIM provider implementation
│   ├── shell/                 # Shell command execution (platform-specific)
│   ├── testutil/              # Test helpers
│   ├── tokens/                # Token estimation with EMA calibration
│   ├── tools/                 # 18 built-in tools + dispatcher
│   │   ├── interface.go       # Tool interface definition
│   │   ├── dispatcher.go      # Permission gating, rate limiting, concurrency
│   │   ├── defaults.go        # Default tool registration
│   │   ├── agent.go           # Agent tool (spawns subagents)
│   │   ├── bash.go            # Bash/shell execution tool
│   │   ├── edit.go            # File editing tool
│   │   ├── fileread.go        # File reading tool
│   │   ├── filewrite.go       # File writing tool
│   │   ├── grep.go            # Content search tool
│   │   ├── glob.go            # File pattern matching tool
│   │   ├── webfetch.go        # URL fetching tool
│   │   ├── websearch.go       # Web search tool
│   │   ├── permissions.go     # Permission request/response types
│   │   ├── output_store.go    # Tool output bounding
│   │   └── subagent/          # Subagent manager, profiles, worktrees
│   ├── tui/                   # Bubble Tea terminal UI
│   │   ├── app.go             # AppState Init/Shutdown, Bubble Tea Model
│   │   ├── app_state.go       # AppState struct (single-threaded model)
│   │   ├── app_update.go      # Main Update() router
│   │   ├── app_routing.go     # Screen routing map
│   │   ├── app_handlers.go    # Message handlers
│   │   ├── types.go           # Screen and message type re-exports
│   │   ├── tuitypes/          # Screen enum and message types
│   │   ├── commands/          # Slash command implementations
│   │   ├── components/        # Reusable UI components (56 files)
│   │   ├── layout/            # Layout system (box, stack, responsive, constraints)
│   │   ├── theme/             # Theme management (dark only)
│   │   ├── streaming/         # LLM streaming rendering
│   │   ├── repl*.go           # REPL screen (main chat interface)
│   │   ├── home_model.go      # Home/landing screen
│   │   ├── sidebar_model.go   # Sidebar with file tree, git status
│   │   ├── plan_model.go      # Plan display/approval screen
│   │   ├── execute_model.go   # Task execution screen
│   │   ├── verify_model.go    # Verification screen
│   │   ├── settings_model.go  # Settings screen
│   │   └── *_model.go         # Other screen implementations
│   ├── types/                 # Shared type vocabulary
│   │   ├── types.go           # Core types (Message, Task, ToolCall, ModelInfo, etc.)
│   │   ├── constants.go       # System constants
│   │   ├── plan.go            # Plan-related types
│   │   ├── git.go             # Git-related types
│   │   └── toolcall.go        # Tool call types
│   ├── wiring/                # Dependency wiring (test-only)
│   └── workflow/              # Seven-phase workflow engine
│       ├── engine.go          # Engine struct, RunPhase, phase dispatch
│       ├── state_machine.go   # Phase transition validation and history
│       ├── context_builder.go # System prompt composition
│       ├── prompt_builder.go  # Prompt template loading with overrides
│       ├── phase_coordinator.go # Pre/post phase setup delegation
│       ├── cost_tracker.go    # LLM cost tracking and budget enforcement
│       ├── classify.go        # Intent classification (LLM-based)
│       ├── initialize.go      # Initialize phase
│       ├── discuss.go         # Discuss phase (Q&A with user)
│       ├── plan.go            # Plan phase (task decomposition)
│       ├── execute.go         # Execute phase (task runner + tool dispatch)
│       ├── verify.go          # Verify phase (testing, build verification)
│       ├── runtime.go         # Runtime phase (dev server management)
│       ├── ship.go            # Ship phase (git commit, changelog)
│       ├── retry.go           # LLM retry with exponential backoff
│       ├── workflow_cache.go  # Session-scoped caching
│       ├── prompts/           # Embedded prompt templates (20 .md files)
│       │   ├── base.md        # Base system prompt
│       │   ├── tool-use.md    # Tool usage instructions
│       │   ├── plan-format.md # Plan output format
│       │   ├── execute-task.md # Task execution instructions
│       │   ├── self-heal.md   # Self-healing instructions
│       │   └── ...
│       └── templates/         # Embedded project templates
│           └── website-nextjs/ # Next.js website template
├── pkg/                       # Reusable packages (no internal/ imports)
│   ├── arbitrage/             # Model quality scoring for auto-selection
│   ├── autodream/             # Context consolidation (summarize old messages)
│   ├── bisect/                # Git bisect automation for bug hunting
│   ├── compaction/            # Automatic conversation compaction
│   ├── coordinator/           # Concurrent session drain management
│   ├── history/               # Frecent history (recent + frequent)
│   ├── keychain/              # OS keychain API key storage
│   │   ├── keychain.go        # Interface
│   │   ├── keychain_darwin.go # macOS Keychain
│   │   ├── keychain_linux.go  # Linux Secret Service/D-Bus
│   │   └── keychain_windows.go # Windows Credential Manager
│   ├── ledger/                # Session record persistence (Markdown)
│   ├── metrics/               # Session metrics collection
│   ├── narrative/             # Tool call event classification/grouping
│   ├── retry/                 # Retry policy with exponential backoff
│   ├── rollback/              # Safe git commit rollback
│   ├── session/               # Session persistence and management
│   │   ├── session.go         # Session struct
│   │   ├── manager.go         # Load/save/list sessions
│   │   ├── checkpoint.go      # Checkpoint save/load for resume
│   │   └── planning.go        # Plan/task persistence
│   ├── skills/                # Skill discovery and loading
│   └── taskrunner/            # Task scheduling with dependency resolution
├── e2e_test.go                # End-to-end tests (compiles and runs binary)
├── go.mod                     # Go module: github.com/eshanized/M31A (go 1.25.0)
├── go.sum                     # Dependency checksums
├── Makefile                   # Build, test, lint, cross-compile targets
├── .golangci.yml              # Linter configuration
├── .goreleaser.yaml           # Release automation
├── .env.example               # Environment variable template
├── m31a.json                  # Project-level config (if present)
├── .m31a/                     # Project-local session data
│   ├── session.json           # Current session metadata
│   ├── messages.json          # Session message history
│   └── session.lock           # File lock for concurrent access
├── docs/                      # Documentation
│   ├── ARCHITECTURE.md
│   ├── CONFIG.md
│   ├── TOOLS.md
│   ├── WORKFLOW.md
│   └── ...
├── scripts/                   # Build/release scripts
│   ├── validate-release.sh
│   └── verify_v1.sh
└── .planning/                 # GSD planning documents
    └── codebase/              # Codebase analysis documents
```

## Directory Purposes

**`cmd/m31a/`:**
- Purpose: CLI binary entry point
- Contains: `main.go` (flag parsing, config, provider registration, TUI/headless launch), `usage.go` (help text)
- Key files: `main.go:116-477` (the `run()` function is the real entry point)

**`internal/tui/`:**
- Purpose: Terminal UI implementing Elm Architecture (Bubble Tea)
- Contains: `AppState` (single model), 25+ screen models, 56 reusable components, layout system, theme
- Key files: `app_state.go` (AppState struct), `app_update.go` (Update router), `app.go` (Init/Shutdown)

**`internal/workflow/`:**
- Purpose: Seven-phase workflow engine (Initialize -> Discuss -> Plan -> Execute -> Verify -> Runtime -> Ship)
- Contains: Engine, StateMachine, phase implementations, prompt templates, context builder
- Key files: `engine.go` (Engine struct and RunPhase), `state_machine.go` (transition validation)

**`internal/tools/`:**
- Purpose: 18 built-in tool implementations with permission gating and rate limiting
- Contains: Tool implementations, Dispatcher, Agent tool, Subagent manager
- Key files: `dispatcher.go` (permission/rate/concurrency), `defaults.go` (tool registration), `agent.go` (subagent spawning)

**`internal/provider/`:**
- Purpose: LLM API abstraction layer with provider registry
- Contains: `LLMProvider` interface, Registry, three provider implementations
- Key files: `interface.go` (LLMProvider interface), `registry.go` (thread-safe registry)

**`internal/types/`:**
- Purpose: Canonical type vocabulary shared across all layers
- Contains: `WorkflowPhase`, `Message`, `Task`, `ToolCall`, `ModelInfo`, `Tool` interface, constants
- Key files: `types.go` (core types), `constants.go` (system constants)

**`internal/config/`:**
- Purpose: TOML-based configuration with hot-reload support
- Contains: Config types, loader, merge logic, project context
- Key files: `types.go` (Config struct with all nested configs), `loader.go` (TOML parsing)

**`pkg/session/`:**
- Purpose: Session persistence, checkpoint/resume, task storage
- Contains: Session struct, Manager (load/save/list), checkpoint logic
- Key files: `manager.go` (session CRUD), `session.go` (Session struct)

**`pkg/taskrunner/`:**
- Purpose: Task scheduling with dependency resolution and parallel execution
- Contains: Runner with topological sort, parallel execution groups
- Key files: `runner.go` (Runner struct, Execute method)

**`pkg/keychain/`:**
- Purpose: OS keychain integration for secure API key storage
- Contains: Platform-specific implementations (macOS/Linux/Windows)
- Key files: `keychain.go` (interface), `keychain_linux.go` (D-Bus Secret Service)

**`internal/codeintel/`:**
- Purpose: Codebase intelligence for import graph, symbol lookup, relevance scoring
- Contains: Indexer, parsers (tree-sitter based), import graph, symbol index
- Key files: `codeintel.go` (Indexer), `trie.go` (prefix search), `relevance.go` (scoring)

## Key File Locations

**Entry Points:**
- `cmd/m31a/main.go`: Binary entry point, `main()` -> `run()`
- `internal/tui/app.go:24`: Bubble Tea `Init()` method
- `internal/tui/app_update.go`: Bubble Tea `Update()` method (the only place AppState is mutated)

**Configuration:**
- `internal/config/types.go`: All config struct definitions
- `internal/config/loader.go`: TOML loading and validation
- `.env.example`: Environment variable template (API keys)
- `~/.m31a/config.toml`: Global user config (resolved at runtime)

**Core Logic:**
- `internal/workflow/engine.go`: Workflow engine orchestration
- `internal/tools/dispatcher.go`: Tool dispatch with permissions
- `internal/provider/registry.go`: Provider management

**Testing:**
- `e2e_test.go`: End-to-end tests (compiles and runs binary)
- `*_test.go` files: Co-located unit tests throughout
- `internal/testutil/`: Shared test helpers

## Naming Conventions

**Files:**
- Screen models: `<screen>_model.go` (e.g., `plan_model.go`, `execute_model.go`)
- Screen views: `<screen>_view.go` (e.g., `plan_view.go`, `execute_view.go`)
- Handlers: `handler_<category>.go` (e.g., `handler_tool.go`, `handler_workflow.go`)
- Platform-specific: `<name>_<platform>.go` (e.g., `bash_unix.go`, `keychain_linux.go`)
- Tests: `<name>_test.go` (co-located with source)
- Extra tests: `<name>_extra_test.go` or `<name>_<category>_test.go`

**Directories:**
- Internal packages: lowercase, single word (e.g., `tools/`, `provider/`, `workflow/`)
- Public packages: lowercase, single word (e.g., `session/`, `keychain/`, `metrics/`)
- TUI subdirectories: `components/`, `layout/`, `commands/`, `tuitypes/`, `theme/`, `streaming/`

**Types:**
- Interfaces: PascalCase, noun (e.g., `LLMProvider`, `Tool`, `WorkflowEngine`)
- Structs: PascalCase (e.g., `AppState`, `Engine`, `Dispatcher`)
- Constants: PascalCase for exported, camelCase for unexported (e.g., `PhaseInitialize`, `maxDiscussPlanCycles`)
- Message types: PascalCase with `Msg` suffix (e.g., `PhaseResultMsg`, `StreamChunkMsg`)

## Where to Add New Code

**New Workflow Phase:**
1. Add phase constant to `internal/types/types.go` (`WorkflowPhase` type)
2. Add transition rules to `internal/workflow/state_machine.go` (`validTransitions` map)
3. Implement phase method in `internal/workflow/<phase>.go`
4. Add case to `internal/workflow/engine.go:RunPhase()` switch
5. Add phase model to `internal/tui/` (e.g., `<phase>_model.go`)
6. Add screen constant to `internal/tui/tuitypes/tuitypes.go`
7. Register in `internal/tui/types.go` re-exports

**New Tool:**
1. Create `internal/tools/<toolname>.go` implementing `types.Tool` interface
2. Register in `internal/tools/defaults.go:DefaultDispatcher()`
3. Add tool-specific tests in `internal/tools/<toolname>_test.go`

**New LLM Provider:**
1. Create `internal/provider/<name>/client.go` implementing `provider.LLMProvider`
2. Add provider registration case to `internal/tui/provider_registration.go`
3. Add provider config fields to `internal/config/types.go:ProviderConfig`

**New TUI Screen:**
1. Create `internal/tui/<screen>_model.go` with Bubble Tea model
2. Create `internal/tui/<screen>_view.go` with `View()` method
3. Add screen constant to `internal/tui/tuitypes/tuitypes.go`
4. Register in `internal/tui/types.go` re-exports
5. Add routing in `internal/tui/app_routing.go`

**New pkg/ Package:**
1. Create directory under `pkg/<name>/`
2. Implement with NO imports from `internal/` (except `internal/types`)
3. Add `doc.go` package documentation
4. Add tests in `pkg/<name>/<name>_test.go`

## Special Directories

**`.m31a/`:**
- Purpose: Project-local session data (messages, session metadata, task lists, backups)
- Generated: Yes (by running `m31a`)
- Committed: No (gitignored)

**`~/.m31a/`:**
- Purpose: Global user config, history, tool output cache
- Generated: Yes (by running `m31a`)
- Committed: No (user-specific)

**`internal/workflow/prompts/`:**
- Purpose: Embedded prompt templates (base system prompt, tool use, plan format, etc.)
- Generated: No (hand-written)
- Committed: Yes

**`internal/workflow/templates/`:**
- Purpose: Embedded project templates (e.g., Next.js website template)
- Generated: No (hand-written)
- Committed: Yes

**`dist/`:**
- Purpose: Cross-compiled binary output
- Generated: Yes (by `make cross`)
- Committed: No (gitignored)

---

*Structure analysis: 2026-07-10*
