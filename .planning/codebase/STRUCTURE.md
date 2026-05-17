# Codebase Structure

**Analysis Date:** 2026-06-14

## Directory Layout

```
M31A/
├── cmd/                          # Binary entry points
│   ├── m31a/                     # Main application binary
│   │   ├── main.go               # Bootstrap, config, wiring, TUI launch
│   │   └── usage.go              # CLI usage/help text
│   └── firstrunpreview/          # First-run wizard preview binary
│       └── main.go
├── internal/                     # Private application packages
│   ├── config/                   # Configuration types and TOML loader
│   ├── errors/                   # Sentinel errors and user-friendly messages
│   ├── fileutil/                 # Atomic file write utilities
│   ├── git/                      # Git CLI wrapper (add, commit, log, diff, worktree)
│   ├── log/                      # Structured logger initialization
│   ├── provider/                 # LLM provider abstraction layer
│   │   ├── openrouter/           # OpenRouter API client
│   │   └── zen/                  # Zen API client
│   ├── tokens/                   # Token estimation and context tracking
│   ├── tools/                    # Tool definitions and dispatcher
│   │   └── subagent/             # Parallel subagent management
│   ├── tui/                      # Terminal UI (Bubble Tea)
│   │   ├── components/           # Reusable UI components
│   │   ├── layout/               # Layout helpers (responsive, pages)
│   │   └── theme/                # Theme engine (colors, borders, unicode)
│   ├── types/                    # Shared type definitions and constants
│   └── workflow/                 # 6-phase workflow engine
│       └── prompts/              # Embedded markdown prompt templates
├── pkg/                          # Public reusable packages
│   ├── arbitrage/                # Cost-based model recommendation
│   ├── autodream/                # Context consolidation (summarization)
│   ├── bisect/                   # Git bisect automation
│   ├── keychain/                 # OS-native secure key storage
│   ├── ledger/                   # Session audit trail
│   ├── rollback/                 # Git commit rollback
│   ├── session/                  # Session CRUD, checkpoints, planning files
│   └── taskrunner/               # DAG-based task execution
├── docs/                         # Documentation
├── images/                       # Image assets
├── scripts/                      # Build/release scripts
├── .github/workflows/            # CI/CD workflows
├── go.mod                        # Go module definition (Go 1.24)
├── go.sum                        # Dependency checksums
├── Makefile                      # Build, test, lint, release targets
├── .golangci.yml                 # Linter configuration
├── .goreleaser.yaml              # GoReleaser configuration
├── .gitignore                    # Git ignore rules
├── install.sh                    # Installation script
├── CONTRIBUTING.md               # Contribution guidelines
├── CHANGELOG.md                  # Release changelog
├── README.md                     # Project documentation
└── LICENSE                       # License file
```

## Directory Purposes

**`cmd/m31a/`:**
- Purpose: Main application entry point
- Contains: `main.go` (bootstrap and wiring), `usage.go` (CLI help)
- Key files: `main.go:35` (`run()` function — all application logic before `os.Exit`)

**`internal/config/`:**
- Purpose: Configuration loading, validation, and type definitions
- Contains: Config struct definitions, TOML parser, `.env` loader, hot-reload watcher
- Key files: `types.go` (all config structs), `loader.go` (`Load()`, `DefaultConfig()`, hot-reload via fsnotify)

**`internal/errors/`:**
- Purpose: Centralized error definitions and user-friendly message mapping
- Contains: Sentinel errors (`ErrRateLimited`, `ErrContextExceeded`, etc.) and `UserMessage()` function
- Key files: `errors.go` (all sentinels and pattern-matching user messages)

**`internal/fileutil/`:**
- Purpose: Safe file operations
- Contains: `AtomicWrite()` for crash-safe file updates via temp file + rename
- Key files: `atomic.go`

**`internal/git/`:**
- Purpose: Git CLI wrapper for all git operations
- Contains: `Git` struct wrapping `exec.Command("git", ...)`, implements `types.GitClient` interface
- Key files: `git.go` (727 lines — init, add, commit, log, diff, worktree, bisect, stash operations)

**`internal/log/`:**
- Purpose: Structured logger setup
- Contains: `NewLogger()` returning `*slog.Logger` with version metadata
- Key files: `logger.go` (referenced from `main.go:69`)

**`internal/provider/`:**
- Purpose: LLM provider abstraction with multi-provider support
- Contains: `LLMProvider` interface, `Registry`, `BaseClient`, SSE parser, model cache, resilience, OpenRouter/Zen clients
- Key files: `interface.go` (interface definition), `registry.go` (multi-provider management), `base_client.go` (shared HTTP transport), `sse.go` (SSE stream parsing), `cache.go` (model catalog caching), `fallback.go` (auto-fallback on errors)

**`internal/provider/openrouter/`:**
- Purpose: OpenRouter API client implementation
- Contains: HTTP client for OpenRouter's chat completion and model listing APIs
- Key files: `client.go` (implements `LLMProvider`)

**`internal/provider/zen/`:**
- Purpose: Zen API client implementation
- Contains: HTTP client for Zen's chat completion and model listing APIs
- Key files: `client.go` (implements `LLMProvider`)

**`internal/tokens/`:**
- Purpose: Token estimation and context window tracking
- Contains: EMA-calibrated token estimator, context warning thresholds
- Key files: `estimator.go` (`EstimateMessages()`, calibration logic)

**`internal/tools/`:**
- Purpose: Tool definitions, dispatcher, and permission system
- Contains: Individual tools (Bash, FileRead, FileWrite, Edit, Glob, Grep, WebFetch, etc.), `Dispatcher` with rate limiting and permission enforcement
- Key files: `dispatcher.go` (central dispatcher), `defaults.go` (`DefaultDispatcher()` wiring all tools), `permissions.go` (rule evaluation), `bash.go` (shell tool), `edit.go` (file editing), `fileread.go`, `filewrite.go`, `glob.go`, `grep.go`, `webfetch.go`, `agent.go` (subagent spawning tool)

**`internal/tools/subagent/`:**
- Purpose: Parallel child agent management with isolated worktrees
- Contains: `Manager` (lifecycle), `Subagent` (handle), git worktree creation, event channel
- Key files: `manager.go` (spawn, shutdown, event routing), `worktree.go` (git worktree isolation), `loop.go` (child agent execution loop)

**`internal/tui/`:**
- Purpose: Terminal user interface (Bubble Tea Elm Architecture)
- Contains: Root `AppState` model, screen-specific sub-models, key handling, command system, streaming
- Key files:
  - `app.go` (`Init()`, `Shutdown()`)
  - `app_state.go` (root model with all fields)
  - `app_update.go` (`Update()` — single message dispatch point, 2273 lines)
  - `app_view.go` (`View()` — screen rendering)
  - `repl.go` (chat interface sub-model)
  - `commands.go` (slash command registry and `CommandHandler` type)
  - `commands_*.go` (command implementations by domain)
  - `agent_loop.go` (autonomous LLM → tool → LLM loop)
  - `streaming.go` (LLM streaming to TUI)

**`internal/tui/components/`:**
- Purpose: Reusable UI components
- Contains: Permission modal, question modal, thinking indicator, tool cards, sparklines, progress bars, badges, data tables, etc.
- Key files: `permission.go`, `question.go`, `thinking.go`, `toolcard.go`, `spinner.go`, `progress.go`, `taskgraph.go`, `workflow_phasebar.go`

**`internal/tui/layout/`:**
- Purpose: Layout management
- Contains: Responsive breakpoint handling, page layout, minimum screen size enforcement
- Key files: `responsive.go`, `page.go`, `minscreen.go`

**`internal/tui/theme/`:**
- Purpose: Theme engine
- Contains: Color definitions, border styles, unicode characters, theme registry, shadow effects
- Key files: `theme.go`, `colors.go`, `borders.go`, `registry.go`, `unicode.go`

**`internal/types/`:**
- Purpose: Shared type definitions and constants
- Contains: All domain types (`Message`, `Task`, `ToolCall`, `ModelInfo`, `WorkflowPhase`, etc.), application constants
- Key files: `types.go` (all shared structs and interfaces), `constants.go` (all application constants), `plan.go` (Plan struct), `git.go` (GitClient interface)

**`internal/workflow/`:**
- Purpose: 6-phase workflow orchestration engine
- Contains: Phase runners, prompt templates, plan parser, message builders
- Key files:
  - `engine.go` (Engine struct, `RunPhase()`, `Transition()`, LLM streaming helpers — 882 lines)
  - `initialize.go` (project detection, git init, planning dir creation)
  - `discuss.go` (clarifying questions via LLM)
  - `plan.go` (implementation plan generation with retry)
  - `plan_parser.go` (markdown → structured Plan extraction)
  - `execute.go` (task execution with tool dispatch and self-heal)
  - `verify.go` (acceptance check with bisect regression detection)
  - `ship.go` (final commit, ledger entry, session archival)
  - `classify.go` (prompt complexity classification)
  - `engine_verify.go` (task verification helpers)
  - `engine_parse.go` (LLM response parsing)
  - `engine_messages.go` (message building for each phase)

**`internal/workflow/prompts/`:**
- Purpose: Embedded markdown prompt templates
- Contains: System prompts, tool-use instructions, plan format, discussion questions, self-heal, autonomous mode
- Key files: `base.md`, `tool-use.md`, `plan-format.md`, `execute-task.md`, `discuss-questions.md`, `self-heal.md`, `demonstration-format.md`, `autonomous.md`

**`pkg/session/`:**
- Purpose: File-based session persistence
- Contains: Session CRUD, checkpoint management, task persistence, planning file I/O, session list with caching
- Key files: `manager.go` (904 lines — Create, Load, Save, List, Cleanup), `planning.go` (PROJECT.md, STATE.md, PLAN.md, TASKS.md I/O), `checkpoint.go`, `session.go`

**`pkg/taskrunner/`:**
- Purpose: DAG-based task execution with dependency resolution
- Contains: Topological sort (Kahn's algorithm), bounded parallel execution, retry support
- Key files: `runner.go` (363 lines — Schedule, ExecuteGroup, Summary)

**`pkg/arbitrage/`:**
- Purpose: Cost-based model recommendation
- Contains: Task complexity scoring, token estimation, multi-model cost comparison
- Key files: `arbitrage.go` (Scorer, Arbitrate, Recommend)

**`pkg/autodream/`:**
- Purpose: Context consolidation to reduce token usage
- Contains: Message summarization, pause/resume, statistics
- Key files: `autodream.go` (Consolidator, SetMessages, Consolidate)

**`pkg/bisect/`:**
- Purpose: Git bisect automation for regression detection
- Contains: Binary search between commits using verification function
- Key files: `bisect.go` (Run, Reset), `exec.go` (git command execution fallback)

**`pkg/keychain/`:**
- Purpose: OS-native secure API key storage
- Contains: Platform-specific implementations via build tags
- Key files: `keychain.go` (interface), `keychain_linux.go` (D-Bus Secret Service + pass), `keychain_darwin.go` (macOS Keychain), `keychain_windows.go` (Windows Credential Manager)

**`pkg/ledger/`:**
- Purpose: Persistent session audit trail
- Contains: Markdown-based ledger with aggregate statistics
- Key files: `ledger.go` (551 lines — Add, Stats, Entries, TopFailures)

**`pkg/rollback/`:**
- Purpose: Safe git commit rollback
- Contains: Commit chain inspection, stash-aware reset, diff generation
- Key files: `rollback.go` (Chain, Reset, Stash)

## Key File Locations

**Entry Points:**
- `cmd/m31a/main.go`: Main application bootstrap
- `cmd/firstrunpreview/main.go`: First-run preview binary
- `internal/tui/app.go:22`: TUI initialization (`Init()`)
- `internal/tui/app_update.go:26`: Message dispatch (`Update()`)
- `internal/tui/agent_loop.go:79`: Autonomous agent loop

**Configuration:**
- `internal/config/types.go`: All config struct definitions
- `internal/config/loader.go`: TOML loading, validation, hot-reload
- `internal/types/constants.go`: Application-wide constants
- `.golangci.yml`: Linter configuration
- `.goreleaser.yaml`: Release configuration
- `Makefile`: Build, test, lint commands

**Core Logic:**
- `internal/workflow/engine.go`: Workflow orchestration engine
- `internal/tools/dispatcher.go`: Tool execution hub
- `internal/provider/registry.go`: Multi-provider management
- `pkg/session/manager.go`: Session persistence
- `pkg/taskrunner/runner.go`: Task DAG execution

**Testing:**
- Test files are co-located with source files (e.g., `engine_test.go` next to `engine.go`)
- Run tests: `make test` or `go test ./...`

## Naming Conventions

**Files:**
- snake_case for all Go files (e.g., `app_update.go`, `plan_parser.go`)
- `_test.go` suffix for test files (co-located with source)
- `_extra_test.go` suffix for additional test files beyond the primary test file
- Platform-specific: `bash_unix.go`, `bash_windows.go`, `keychain_linux.go`
- Phase-specific: `commands_config.go`, `commands_git.go`, `commands_session.go`

**Directories:**
- lowercase with underscores not used (all single-word: `components`, `subagent`, `fileutil`)
- Package names match directory names

**Functions/Methods:**
- `run<Phase>()` for workflow phase implementations (e.g., `runPlan`, `runExecute`)
- `handle<Key/Msg>()` for TUI message handling (e.g., `handleStreamMsg`)
- `build<Thing>()` for message/prompt construction (e.g., `buildPlanContext`)
- `New<Thing>()` for constructors (e.g., `NewEngine`, `NewDispatcher`)

## Where to Add New Code

**New Tool:**
- Implementation: `internal/tools/<toolname>.go`
- Register in: `internal/tools/defaults.go:5` (`DefaultDispatcher()`)
- Tests: `internal/tools/<toolname>_test.go`

**New Workflow Phase:**
- Phase runner: `internal/workflow/<phase>.go`
- Message types: `internal/workflow/engine_messages.go`
- Phase constants: `internal/types/types.go` (add to `WorkflowPhase` enum)
- Transition rules: `internal/workflow/engine.go:322` (`validPhaseTransitions`)
- Prompt template: `internal/workflow/prompts/<phase>.md`
- TUI screen: `internal/tui/<phase>_model.go` + `<phase>_view.go`
- Command handler: `internal/tui/commands_workflow.go`

**New LLM Provider:**
- Client: `internal/provider/<provider>/client.go`
- Register in: `internal/tui/provider_registration.go:17` (`RegisterProvider()`)
- Config: `internal/config/types.go` (add to `ProviderConfig`)

**New Slash Command:**
- Handler: `internal/tui/commands_<domain>.go`
- Register in: `internal/tui/commands.go` (add to `DefaultCommands()`)
- Tests: `internal/tui/commands_extra_test.go`

**New UI Component:**
- Implementation: `internal/tui/components/<component>.go`
- Tests: `internal/tui/components/extra_test.go` (or `<component>_test.go`)

**New Public Package:**
- Implementation: `pkg/<package>/`
- Include: `doc.go` for package documentation

**New Configuration Option:**
- Add field to appropriate config struct in: `internal/config/types.go`
- Add default in: `internal/config/loader.go` (`DefaultConfig()`)
- Document in TOML: `~/.m31a/config.toml`

## Special Directories

**`.planning/`:**
- Purpose: GSD workflow planning and tracking
- Generated: Yes (by GSD commands)
- Committed: No (gitignored)

**`dist/`:**
- Purpose: Cross-compiled binary output
- Generated: Yes (by `make cross`)
- Committed: No

**`vendor/`:**
- Purpose: Vendored Go dependencies (if enabled)
- Generated: Yes (by `go mod vendor`)
- Committed: No

**`.github/workflows/`:**
- Purpose: CI/CD pipeline definitions
- Generated: No
- Committed: Yes

---

*Structure analysis: 2026-06-14*
