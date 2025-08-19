# Codebase Structure

**Analysis Date:** [YYYY-MM-DD]

## Directory Layout

```
[project-root]/
├── cmd/
│   └── m31a/              # Main application entry point
├── internal/
│   ├── config/            # Application configuration structures and parsing
│   ├── errors/            # Custom application error definitions
│   ├── git/               # Git command abstractions and wrappers
│   ├── log/               # Logging initialization and configuration
│   ├── provider/          # LLM Provider integrations (OpenRouter, Zen)
│   ├── tokens/            # Token counting and estimation
│   ├── tools/             # Executable tools and tool dispatcher for LLMs
│   ├── tui/               # BubbleTea Terminal UI application and screens
│   ├── types/             # Shared data types and structures
│   └── workflow/          # Multi-phase agentic workflow engine
├── pkg/
│   ├── arbitrage/         # Arbitrage-related core logic
│   ├── autodream/         # Background LLM consolidation
│   ├── bisect/            # Git bisect and troubleshooting aids
│   ├── keychain/          # OS-level secure credential storage
│   ├── ledger/            # Local ledger for settings/stat tracking
│   ├── rollback/          # Rollback and safe state restoration
│   ├── session/           # Session state, checkpoints, and management
│   └── taskrunner/        # Task execution utilities
└── rush/                  # Audits, logs, and developer documentation
```

## Directory Purposes

**`internal/tui/`:**
- Purpose: Contains all code for the interactive terminal application.
- Contains: BubbleTea components (`app.go`, `repl.go`, `settings.go`, `sidebar.go`), themes, rendering logic.
- Key files: `internal/tui/app.go`, `internal/tui/commands.go`, `internal/tui/app_workflow.go`

**`internal/workflow/`:**
- Purpose: Implements the phased AI workflows.
- Contains: The execution engine, phase definitions, and embedded prompts.
- Key files: `internal/workflow/engine.go`, `internal/workflow/workflow_test.go`, `internal/workflow/prompts/`

**`internal/provider/`:**
- Purpose: Manages communication with external LLM APIs.
- Contains: Standardized `LLMProvider` interface, OpenRouter/Zen specific clients.
- Key files: `internal/provider/interface.go`, `internal/provider/registry.go`

**`internal/tools/`:**
- Purpose: Sandboxes and manages execution of system tools by the AI.
- Contains: Shell execution logic, file readers/writers, command dispatcher.
- Key files: `internal/tools/dispatcher.go`, `internal/tools/bash.go`, `internal/tools/filewrite.go`

## Key File Locations

**Entry Points:**
- `cmd/m31a/main.go`: Main execution, setup of dependencies, launching TUI.

**Configuration:**
- `internal/config/types.go`: Definitions for `Config` structures.
- `internal/config/loader.go`: Config loading from TOML files and environment variables.

**Core Logic:**
- `internal/workflow/engine.go`: Multi-phase workflow runner.
- `pkg/session/manager.go`: Handles session files, checkpoints, and state files.

**Testing:**
- Most tests are co-located with their logic (e.g., `internal/tui/app_test.go`, `internal/workflow/engine_test.go`).

## Naming Conventions

**Files:**
- Go Source: `snake_case.go` (e.g., `app_update.go`, `modelselector_list.go`)
- Test Files: `*_test.go` (e.g., `execute_test.go`)

**Directories:**
- Package directories are exclusively lowercase, generally short (e.g., `internal/provider`, `pkg/session`).

## Where to Add New Code

**New Feature (TUI):**
- Primary code: Create a new model in `internal/tui/` following the existing `[feature].go` or `[feature]_view.go` pattern.
- Tests: `internal/tui/[feature]_test.go`.

**New LLM Provider:**
- Implementation: Create a new subdirectory in `internal/provider/` (e.g., `internal/provider/anthropic/`).
- Implement the `LLMProvider` interface and register it in `cmd/m31a/main.go`.

**New AI Tool:**
- Implementation: Create a new Go file in `internal/tools/` (e.g., `internal/tools/custom.go`).
- Implement the `Tool` interface and register it in the DefaultDispatcher inside `internal/tools/defaults.go`.

## Special Directories

**`rush/`:**
- Purpose: Developer logs, prompt histories, architectural notes, and audits from the development process.
- Generated: Mix of manual and tool-generated.
- Committed: Yes.

**`internal/workflow/prompts/`:**
- Purpose: Contains markdown files embedded into the binary containing LLM instructions.
- Generated: No (Manual).
- Committed: Yes.

---

*Structure analysis: [YYYY-MM-DD]*