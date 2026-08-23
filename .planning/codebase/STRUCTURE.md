# Codebase Structure

**Analysis Date:** 2026-08-23

## Directory Layout

```
/home/snigdha/Desktop/M31A/
├── .github/                    # GitHub Actions workflows
│   ├── workflows/
│   │   ├── ci.yml              # Main CI: lint, test (race), build, security, benchmarks
│   │   ├── pr-checks.yml       # PR validation
│   │   ├── nightly.yml         # Nightly runs
│   │   ├── release.yml         # Goreleaser release on tag push
│   │   └── benchmarks.yml      # Benchmark tracking
│   └── ISSUE_TEMPLATE/         # Issue templates
├── .planning/                  # GSD planning artifacts (gitignored)
├── cmd/
│   └── m31a/                   # Main entry point
│       ├── main.go             # CLI, config, provider registry, TUI bootstrap
│       └── .m31a/              # Project-local session data (gitignored)
├── docs/                       # Documentation
├── internal/                   # Private application code (not importable externally)
│   ├── core/                   # Shared vocabulary (types, config, errors)
│   │   ├── config/             # Multi-layer config (TOML, JSON, env, validation)
│   │   ├── errors/             # Sentinel error definitions
│   │   └── types/              # All domain types (WorkflowPhase, Tool, Message, etc.)
│   ├── engine/                 # Workflow execution engine
│   │   ├── workflow/           # 7-phase engine (engine.go + 8 support files)
│   │   ├── taskrunner/         # Dependency resolution + parallel task execution
│   │   ├── session/            # Session persistence (session.json, messages.json, recovery)
│   │   ├── bisect/             # Automated git bisect for regressions
│   │   ├── rollback/           # Git-based rollback to checkpoints
│   │   ├── compaction/         # Proactive context compaction
│   │   ├── coordinator/        # Per-session concurrency control
│   │   ├── decision/           # Decision logging for workflow
│   │   ├── narrative/          # Narrative event system (F-059, F-060)
│   │   └── tokens/             # Token estimation (tiktoken)
│   ├── infrastructure/         # Low-level utilities
│   │   ├── fileutil/           # Atomic writes, safe reads
│   │   └── retry/              # Retry policies with backoff
│   ├── integrations/           # External system adapters
│   │   ├── provider/           # LLM providers (OpenRouter, Zen, Nvidia)
│   │   │   ├── openrouter/
│   │   │   ├── zen/
│   │   │   └── nvidia/
│   │   ├── git/                # Git operations
│   │   ├── keychain/           # OS credential storage
│   │   ├── ledger/             # LEDGER.md persistence
│   │   ├── log/                # Structured logging setup
│   │   ├── metrics/            # Session metrics collection
│   │   ├── arbitrage/          # Model cost optimization
│   │   ├── autodream/          # Context auto-compaction
│   │   ├── codeintel/          # Codebase intelligence indexing
│   │   ├── context/            # Dynamic system context sources
│   │   ├── history/            # Command/frecent history
│   │   ├── shell/              # Shell command execution
│   │   └── skills/             # Slash command skills
│   ├── observability/          # Crash capture, pprof
│   ├── performance/            # Benchmarks
│   ├── tools/                  # 18 built-in tools + dispatcher
│   │   ├── dispatcher.go       # Permission, rate limit, concurrency
│   │   ├── defaults.go         # Tool registration
│   │   ├── fileops/            # FileRead, FileWrite, FileEdit, FileList, FileMove, FileDelete
│   │   ├── exec/               # Bash, DevServer
│   │   ├── search/             # Glob, Grep
│   │   ├── network/            # WebFetch, WebSearch
│   │   ├── git/                # Git tool
│   │   ├── todo/               # TodoWrite, TodoRead
│   │   ├── subagent/           # Agent tool + manager
│   │   ├── ai/                 # AI-specific tools
│   │   └── permissions.go      # Permission rule engine
│   ├── ui/
│   │   └── tui/                # Bubble Tea TUI (100+ files)
│   │       ├── app.go          # AppState (Model), Init, Shutdown, workflow bridge
│   │       ├── router.go       # Screen navigation
│   │       ├── repl*.go        # REPL model, view, streaming, commands
│   │       ├── *_model.go      # Screen models (home, plan, execute, verify, runtime, ship, settings, etc.)
│   │       ├── *_view.go       # Screen rendering
│   │       ├── components/     # Reusable UI components
│   │       ├── layout/         # Layout calculations
│   │       ├── theme/          # Theme system (colors, tokens, borders, motion)
│   │       ├── streaming/      # LLM streaming infrastructure
│   │       ├── commands/       # Slash command palette
│   │       └── tuitypes/       # TUI-specific types
│   ├── tests/                  # Internal test utilities
│   │   └── tools/
│   └── testutil/               # Shared test utilities
│       ├── mocks/              # Mock implementations (provider, dispatcher, tool)
│       ├── integration/        # Integration test helpers
│       └── envtest.go          # Environment test setup
├── pkg/                        # Public packages (importable by external projects)
│   └── extensions/             # Extension subsystem (tools, providers, hooks via subprocess)
│       ├── protocol.go         # JSON-RPC over stdin/stdout
│       ├── adapter_tool.go     # Tool adapter
│       ├── adapter_provider.go # Provider adapter
│       ├── adapter_hook.go     # Phase hook adapter
│       ├── registry.go         # Extension registry
│       ├── subprocess.go       # Subprocess management
│       └── config.go           # Extension config types
├── tests/                      # External test suites
│   ├── e2e/                    # End-to-end binary tests
│   │   └── e2e_test.go         # Version, help, headless, real API tests
│   └── testutil/               # Test utilities
│       ├── ci/                 # CI detection helpers
│       ├── mocks/              # Mocks for integration tests
│       └── integration/        # Tool integration tests
├── scripts/                    # Build/release scripts
│   └── validate-release.sh     # Release validation harness
├── go.mod                      # Go 1.26.5, CGO_ENABLED=0
├── go.sum
├── Makefile                    # Build, test, lint, cross-compile, release targets
├── .golangci.yml               # golangci-lint config (govet, staticcheck, errcheck, ineffassign, unused)
├── .goreleaser.yaml            # Cross-platform release config
├── install.sh                  # Installer script
├── m31a.json                   # Project config template
├── .env.example                # Environment variable template
├── README.md
├── AGENTS.md                   # Agent instructions (this file's context)
├── CHANGELOG.md
├── LICENSE
├── CONTRIBUTING.md
├── CODE_OF_CONDUCT.md
├── SECURITY.md
├── SUPPORT.md
├── TESTING.md
└── AUDIT_REPORT.md
```

## Directory Purposes

**cmd/m31a/:**
- Purpose: Application entry point; CLI parsing, config loading, provider registration, TUI construction
- Contains: `main.go` (691 lines), project-local `.m31a/` session directory
- Key files: `cmd/m31a/main.go`

**internal/core/:**
- Purpose: Shared types, configuration, and error definitions used across all internal packages
- Contains: `config/` (27KB loader, 42KB types), `errors/` (sentinels), `types/` (368-line types.go)
- Key files: `internal/core/types/types.go`, `internal/core/config/loader.go`, `internal/core/config/types.go`

**internal/engine/:**
- Purpose: Core workflow execution logic
- Contains:
  - `workflow/` — 7-phase engine split across 9 files (engine.go + engine_pause.go, engine_streaming.go, engine_checkpoint.go, engine_model.go, engine_helpers.go, engine_concurrency.go, engine_discuss.go, engine_context.go, engine_tools.go)
  - `taskrunner/` — Kahn's algorithm + parallel execution with semaphore
  - `session/` — Project-local session persistence with file locking
  - `bisect/` — Automated regression bisect
  - `rollback/` — Git-based rollback
  - `compaction/`, `coordinator/`, `decision/`, `narrative/`, `tokens/`
- Key files: `internal/engine/workflow/engine.go`, `internal/engine/taskrunner/runner.go`, `internal/engine/session/manager.go`

**internal/integrations/:**
- Purpose: Adapters for external systems (LLM providers, git, keychain, etc.)
- Contains: 13 subdirectories, each a focused integration
- Key files: `internal/integrations/provider/registry.go`, `internal/integrations/provider/interface.go`, provider client implementations

**internal/tools/:**
- Purpose: Tool definitions and dispatching with permissions, rate limiting, concurrency
- Contains: `dispatcher.go` (483 lines), 18 built-in tools in subdirectories, `permissions.go` (19KB)
- Key files: `internal/tools/dispatcher.go`, `internal/tools/defaults.go`, `internal/tools/permissions.go`

**internal/ui/tui/:**
- Purpose: Bubble Tea TUI implementation (Elm architecture)
- Contains: 100+ files organized by concern: `app.go` (model), `router.go`, screen models/views, components, theme, streaming, commands
- Key files: `internal/ui/tui/app.go`, `internal/ui/tui/router.go`, `internal/ui/tui/repl_model.go`, `internal/ui/tui/sidebar_model.go`

**internal/observability/:**
- Purpose: Crash reporting and profiling
- Contains: `crash.go` (crash capture + disk write), pprof endpoints via `net/http/pprof`

**pkg/extensions/:**
- Purpose: Public extension subsystem for external tools/providers/hooks via subprocess
- Contains: Protocol, adapters, registry, subprocess management
- Key files: `pkg/extensions/protocol.go`, `pkg/extensions/adapter_tool.go`, `pkg/extensions/registry.go`

**tests/:**
- Purpose: External test suites (e2e, integration)
- Contains: `e2e/e2e_test.go` (binary tests with real API keys), `testutil/` helpers

## Key File Locations

**Entry Points:**
- `cmd/m31a/main.go` — Main binary entry; CLI, config, TUI bootstrap

**Configuration:**
- `internal/core/config/loader.go` — Multi-layer config loading (TOML, JSON, env)
- `internal/core/config/types.go` — Config struct definitions
- `.env.example` — Environment variable template
- `m31a.json` — Project config template

**Core Logic:**
- `internal/engine/workflow/engine.go` — Workflow engine orchestration
- `internal/engine/workflow/engine_prompts.go` — Phase prompt building
- `internal/engine/workflow/engine_context.go` — Context assembly for LLM
- `internal/engine/taskrunner/runner.go` — Task scheduling + execution
- `internal/engine/session/manager.go` — Session persistence
- `internal/tools/dispatcher.go` — Tool dispatch with permissions
- `internal/tools/defaults.go` — Built-in tool registration
- `internal/integrations/provider/registry.go` — Provider registration
- `internal/integrations/provider/interface.go` — LLMProvider interface

**Testing:**
- `tests/e2e/e2e_test.go` — Binary-level tests
- `internal/testutil/mocks/` — Mock implementations
- `internal/testutil/integration/` — Integration test helpers
- `.github/workflows/ci.yml` — CI pipeline definition

## Naming Conventions

**Files:**
- Go files: `snake_case.go` (e.g., `engine.go`, `dispatcher.go`, `repl_model.go`)
- Test files: `*_test.go` suffix (e.g., `dispatcher_test.go`, `engine_test.go`)
- Extra test files: `*_extra_test.go` for additional test cases
- Benchmarks: `*_bench_test.go` or `*_benchmark_test.go`

**Directories:**
- Package directories: lowercase, singular (e.g., `workflow/`, `provider/`, `session/`)
- Feature directories: lowercase, plural or descriptive (e.g., `fileops/`, `components/`, `streaming/`)

**Packages:**
- Package name matches directory name (e.g., `package workflow` in `workflow/`)
- Exported identifiers: PascalCase (e.g., `Engine`, `Dispatcher`, `WorkflowPhase`)
- Internal identifiers: camelCase (e.g., `sessionID`, `workflowPhase`)

**Types:**
- Enums: `WorkflowPhase`, `RiskLevel`, `TaskStatus`, `ComplexityLevel` (PascalCase)
- Structs: `Engine`, `TaskResult`, `ChatRequest`, `ModelInfo` (PascalCase)
- Interfaces: `LLMProvider`, `Tool`, `SchemaProvider` (PascalCase, often noun)
- Constants: `PhaseInitialize`, `RiskDangerous`, `StatusDone` (PascalCase with prefix)

## Where to Add New Code

**New Workflow Phase:**
- Primary code: `internal/engine/workflow/engine.go` — add case in `RunPhase()` switch, implement `runNewPhase()`
- Prompt: `internal/engine/workflow/engine_prompts.go` — add prompt template
- State: `internal/core/types/types.go` — add to `WorkflowPhase` enum
- TUI: `internal/ui/tui/` — add screen model/view if phase needs UI

**New Tool:**
- Implementation: `internal/tools/<category>/<toolname>.go` — implement `Tool` interface
- Registration: `internal/tools/defaults.go` — add to `DefaultDispatcher()`
- Permission: `internal/tools/permissions.go` — add default rule if needed
- Tests: `internal/tools/<category>/<toolname>_test.go`

**New Provider:**
- Implementation: `internal/integrations/provider/<name>/client.go` — implement `LLMProvider` interface
- Registration: `internal/integrations/provider/registry.go` — add to `RegisterProvider()` / `NewLazyRegistry()`
- Config: `internal/core/config/types.go` — add `ProviderCredentialConfig` field
- Main: `cmd/m31a/main.go` — add registration in lazy registry factory

**New TUI Screen:**
- Model: `internal/ui/tui/<screen>_model.go` — state + update logic
- View: `internal/ui/tui/<screen>_view.go` — rendering
- Router: `internal/ui/tui/router.go` — add route
- AppState: `internal/ui/tui/app.go` — add screen constant, init logic

**New Config Option:**
- Types: `internal/core/config/types.go` — add field to appropriate config struct
- Defaults: `internal/core/config/loader.go` — add to `DefaultConfig()`
- Validation: `internal/core/config/validate.go` — add validation rule

**New Extension Point:**
- Protocol: `pkg/extensions/protocol.go` — define new method types
- Adapter: `pkg/extensions/adapter_<type>.go` — implement adapter
- Registry: `pkg/extensions/registry.go` — register extension type
- Config: `internal/core/config/types.go` — add to `ExtensionsConfig`
- Hook registration: `internal/engine/workflow/engine_hooks.go` — wire into PhaseHookRegistry

## Special Directories

**internal/ui/tui/theme/:**
- Purpose: Complete theme system (colors, spacing, typography, elevation, motion, borders, shadows)
- Generated: No (hand-written)
- Committed: Yes
- Files: `theme.go` (registry), `colors.go`, `tokens.go`, `registry.go`, `cache.go`, `tabs.go`, `borders.go`, `shadow.go`, `unicode.go`, plus token files

**cmd/m31a/.m31a/:**
- Purpose: Project-local session storage (session.json, messages.json, recovery.json, backups/, planning/)
- Generated: Yes (at runtime)
- Committed: No (gitignored via `ensureGitIgnore()` in `session/manager.go:567`)

**~/.m31a/ (global):**
- Purpose: Global config (config.toml), recent models, crash reports, keychain
- Generated: Yes (first run)
- Committed: No (user directory)

**dist/:**
- Purpose: Cross-compilation output (created by `make cross`)
- Generated: Yes
- Committed: No (gitignored)

**bin/:**
- Purpose: Debug binary output (created by `make debug` or CI)
- Generated: Yes
- Committed: No (gitignored)

---

*Structure analysis: 2026-08-23*