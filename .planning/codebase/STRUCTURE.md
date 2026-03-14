# Codebase Structure

**Analysis Date:** 2026-06-12

## Directory Layout

```
M31A/
├── cmd/                    # CLI entry points
│   ├── m31a/              # Main application binary
│   └── firstrunpreview/   # First-run wizard preview tool
├── internal/              # Private application packages
│   ├── config/            # Configuration loading and validation
│   ├── errors/            # Sentinel error definitions
│   ├── fileutil/          # File operation utilities
│   ├── git/               # Git operations wrapper
│   ├── log/               # Structured logging setup
│   ├── provider/          # LLM provider implementations
│   ├── tokens/            # Token estimation and context tracking
│   ├── tools/             # Tool system (dispatcher, permissions, implementations)
│   │   └── subagent/      # Parallel subagent management
│   ├── tui/               # Terminal UI (Bubble Tea)
│   │   ├── components/    # Reusable UI components
│   │   └── theme/         # Theme management
│   ├── types/             # Shared type definitions
│   └── workflow/          # Workflow engine and phases
│       └── prompts/       # Embedded prompt templates
├── pkg/                   # Public reusable packages
│   ├── arbitrage/         # Model cost optimization
│   ├── autodream/         # Context consolidation
│   ├── bisect/            # Git bisect automation
│   ├── keychain/          # OS keychain integration
│   ├── ledger/            # Action history tracking
│   ├── rollback/          # Git rollback utilities
│   ├── session/           # Session persistence
│   └── taskrunner/        # Task execution utilities
├── docs/                  # Documentation
├── images/                # Assets (logos, screenshots)
├── scripts/               # Build and utility scripts
├── .github/               # GitHub Actions workflows
├── go.mod                 # Go module definition
├── go.sum                 # Dependency checksums
├── Makefile               # Build and development targets
├── .goreleaser.yaml       # Release configuration
├── .golangci.yml          # Linter configuration
└── README.md              # Project overview
```

## Directory Purposes

**`cmd/m31a/`:**
- Purpose: Main application entry point
- Contains: `main.go` (bootstrap), `usage.go` (help text)
- Key files: `main.go` - Config loading, dependency injection, signal handling, TUI launch

**`internal/config/`:**
- Purpose: Multi-layer configuration management
- Contains: Loader, types, validation, project context
- Key files: `loader.go` - Config loading with defaults → global → env → project merging; `types.go` - Config struct definitions

**`internal/workflow/`:**
- Purpose: Orchestrate 6-phase AI development workflow
- Contains: Phase implementations, engine, prompts, tests
- Key files: `engine.go` - Core orchestrator; `plan.go`, `execute.go`, `verify.go`, `ship.go`, `discuss.go` - Phase implementations

**`internal/tools/`:**
- Purpose: Safe execution of file/shell operations with permissions
- Contains: Tool implementations, dispatcher, permission system
- Key files: `dispatcher.go` - Tool registry and execution; `bash.go`, `filewrite.go`, `fileread.go`, `grep.go`, `glob.go` - Tool implementations

**`internal/tui/`:**
- Purpose: Terminal user interface built with Bubble Tea
- Contains: Screen models, views, keybindings, command system
- Key files: `app.go` - Init/Shutdown; `app_state.go` - Central state; `repl.go` - REPL interface; `commands.go` - Command registry

**`internal/provider/`:**
- Purpose: LLM provider abstraction and implementations
- Contains: Provider interface, registry, OpenRouter/Zen implementations
- Key files: `interface.go` - LLMProvider interface; `registry.go` - Provider management; `openrouter/`, `zen/` - Implementations

**`pkg/session/`:**
- Purpose: Session persistence and management
- Contains: Session CRUD, checkpoints, planning state
- Key files: `manager.go` - Session operations; `planning.go` - Planning directory management

**`pkg/ledger/`:**
- Purpose: Track action history for audit trail
- Contains: Ledger append and retrieval
- Key files: `ledger.go` - LEDGER.md file management

**`pkg/rollback/`:**
- Purpose: Git-based rollback utilities
- Contains: Rollback to previous commits
- Key files: `rollback.go` - Git reset operations

**`pkg/autodream/`:**
- Purpose: Automatic context consolidation when messages grow large
- Contains: Message consolidation logic
- Key files: `autodream.go` - Threshold detection and consolidation

## Key File Locations

**Entry Points:**
- `cmd/m31a/main.go`: Application bootstrap and lifecycle

**Configuration:**
- `internal/config/types.go`: All config struct definitions
- `internal/config/loader.go`: Config loading, merging, validation
- `~/.m31a/config.toml`: Global user config (runtime)
- `m31a.toml`: Project-level config (runtime)

**Core Logic:**
- `internal/workflow/engine.go`: Workflow orchestration engine
- `internal/tools/dispatcher.go`: Tool execution and permissions
- `internal/provider/registry.go`: LLM provider management
- `internal/tui/app_state.go`: Central TUI state

**Testing:**
- `*_test.go` files co-located with source (standard Go convention)
- `internal/workflow/*_test.go`: Phase-specific tests
- `internal/tools/*_test.go`: Tool execution tests
- `pkg/session/*_test.go`: Session management tests

## Naming Conventions

**Files:**
- Snake_case for Go files: `app_state.go`, `engine_parse.go`
- Test files: `*_test.go` suffix (standard Go)
- Platform-specific: `bash_unix.go`, `bash_windows.go`
- Embedded resources: `prompts/*.md` in workflow package

**Directories:**
- Lowercase with underscores: `subagent/`, `fileutil/`
- Package name matches directory name

**Functions:**
- CamelCase: `NewEngine()`, `RunPhase()`, `ensurePermission()`
- Private functions: lowercase first letter: `loadPrompts()`, `validateConfig()`

**Types:**
- PascalCase: `AppState`, `Engine`, `Dispatcher`, `WorkflowPhase`
- Interfaces: PascalCase with `-er` suffix: `LLMProvider`, `Tool`, `SchemaProvider`

**Constants:**
- PascalCase for exported: `PhasePlan`, `StatusDone`, `RiskDangerous`
- Private constants: camelCase: `permissionRequestID`, `skipDirsCache`

## Where to Add New Code

**New Workflow Phase:**
1. Add phase constant to `internal/types/types.go:WorkflowPhase`
2. Add transition rule to `internal/workflow/engine.go:validPhaseTransitions`
3. Implement phase in `internal/workflow/<phase>.go`
4. Add prompt template to `internal/workflow/prompts/<phase>.md`
5. Add phase model to `internal/tui/` (e.g., `<phase>_model.go`, `<phase>_view.go`)

**New Tool:**
1. Create implementation in `internal/tools/<tool>.go`
2. Implement `types.Tool` interface: `Name()`, `Description()`, `RiskLevel()`, `Execute()`
3. Optionally implement `types.SchemaProvider` for LLM parameter schemas
4. Register in `internal/tools/defaults.go`

**New Provider:**
1. Create implementation in `internal/provider/<provider>/`
2. Implement `provider.LLMProvider` interface
3. Register in `internal/provider/registry.go`

**New Config Field:**
1. Add field to appropriate struct in `internal/config/types.go`
2. Add default value in `internal/config/loader.go:DefaultConfig()`
3. Add validation in `internal/config/loader.go:validateConfig()`
4. Add TOML tag for serialization

**New UI Screen:**
1. Create model in `internal/tui/<screen>_model.go`
2. Create view in `internal/tui/<screen>_view.go`
3. Add screen constant to `internal/tui/types.go`
4. Add routing in `internal/tui/app_state.go`

## Special Directories

**`.planning/`:**
- Purpose: Project planning artifacts (phases, requirements, codebase docs)
- Generated: Yes (by GSD commands)
- Committed: Yes (for team reference)

**`internal/workflow/prompts/`:**
- Purpose: Embedded prompt templates for workflow phases
- Generated: No (manually maintained)
- Committed: Yes

**`pkg/`:**
- Purpose: Public reusable packages (importable by external tools)
- Generated: No
- Committed: Yes

**`vendor/`:**
- Purpose: Vendored dependencies (if present)
- Generated: Yes
- Committed: No (in `.gitignore`)

---

*Structure analysis: 2026-06-12*
