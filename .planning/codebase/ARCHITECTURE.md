# Architecture

**Last mapped:** 2026-08-02
**Project:** M31 Autonomous (Terminal AI Coding Agent)

## System Architecture

### High-Level Design

M31 Autonomous follows a layered architecture with clear separation of concerns:

```
┌─────────────────────────────────────────────────┐
│                  UI Layer (TUI)                  │
│              Bubble Tea (Elm Architecture)       │
├─────────────────────────────────────────────────┤
│              Workflow Engine (7 Phases)          │
│         Initialize → Discuss → Plan → Execute   │
│              → Verify → Runtime → Ship          │
├─────────────────────────────────────────────────┤
│              Core Services Layer                 │
│   Config │ Types │ Errors │ Session │ Tokens    │
├─────────────────────────────────────────────────┤
│           Infrastructure Layer                   │
│   Retry │ Rate Limit │ File Utils │ Metrics     │
├─────────────────────────────────────────────────┤
│           Integrations Layer                     │
│   Provider │ Git │ Keychain │ Ledger │ AutoDream │
├─────────────────────────────────────────────────┤
│              Tools Layer (18+ Tools)            │
│   FileOps │ Exec │ Search │ AI │ Git │ Network  │
└─────────────────────────────────────────────────┘
```

## Core Components

### 1. Entry Point (`cmd/m31a/main.go`)

- **Purpose:** Application bootstrap, flag parsing, provider registration
- **Responsibilities:**
  - Parse command-line flags (`--goal`, `--headless`, `--mode`)
  - Load configuration via `config.Loader`
  - Register LLM providers (OpenRouter, Zen, Nvidia)
  - Create TUI or headless workflow runner
  - Handle graceful shutdown on SIGINT/SIGTERM

### 2. UI Layer (`internal/ui/tui/`)

- **Framework:** Bubble Tea (Elm architecture)
- **Pattern:** Single-threaded, message-based state updates
- **Key Components:**
  - `AppModel` - Main application model
  - `CommandRegistry` - Slash command handling
  - `Theme` - Styling and visual presentation
  - Components for permissions, plan display, etc.

**Critical Rule:** Never mutate `AppState` from goroutines. All state changes go through `Update()` only.

### 3. Workflow Engine (`internal/engine/workflow/`)

- **Pattern:** Seven-phase sequential workflow
- **Phases:**
  1. **Initialize** - Project detection, code index building
  2. **Discuss** - Clarify requirements with user
  3. **Plan** - Generate implementation plan
  4. **Execute** - Implement changes with tool calls
  5. **Verify** - Run tests, lint, type checks
  6. **Runtime** - Runtime testing and validation
  7. **Ship** - Git commit, PR creation

- **Modes:**
  - `auto` - Adaptive phase selection
  - `full` - All 7 phases
  - `fast` - Skip Plan phase
  - `direct` - Skip Discuss, Plan, Verify

### 4. Provider Layer (`internal/integrations/provider/`)

- **Pattern:** Interface-based design with registry
- **Providers:** OpenRouter, Zen, Nvidia
- **Features:**
  - Dynamic model discovery from APIs
  - Automatic failover between providers
  - Streaming responses
  - Rate limiting and retry logic

### 5. Tools Layer (`internal/tools/`)

- **Dispatcher:** `internal/tools/dispatcher.go`
- **Categories:**
  - **FileOps:** Read, write, edit, delete, move files
  - **Exec:** Bash command execution with security sandboxing
  - **Search:** Web search, web fetch, code search
  - **AI:** Subagent orchestration
  - **Git:** Git operations
  - **Network:** HTTP checks, API calls
  - **CodeAnalysis:** Language-specific code intelligence

## Data Flow

### Request Flow

```
User Input → TUI → Workflow Engine → Tool Dispatcher → Tool Implementation
                                    ↓
                              LLM Provider → API Response
                                    ↓
                              Tool Result → Workflow Engine → TUI → User
```

### State Management

- **Session State:** Maintained in `session.Manager`
- **Workflow State:** `WorkflowState` struct with mutex protection
- **Message History:** `[]m31types.Message` for conversation context
- **Plan State:** Versioned plan markdown with refinement support

## Key Abstractions

### Interfaces

1. **`provider.Provider`** - LLM provider interface
2. **`tools.Tool`** - Tool interface for all built-in tools
3. **`workflow.Phase`** - Phase interface (implicit via engine)
4. **`session.Manager`** - Session persistence interface

### Data Structures

1. **`m31types.Message`** - Conversation message
2. **`m31types.ToolCall`** - Tool invocation request
3. **`m31types.WorkflowPhase`** - Phase enumeration
4. **`config.Config`** - Application configuration

## Concurrency Model

- **TUI:** Single-threaded (Bubble Tea requirement)
- **Workflow Engine:** Goroutine-based with mutex protection
- **Tool Execution:** Concurrent with rate limiting
- **Provider Calls:** Async with timeout protection

## Error Handling

- **Pattern:** Return errors, never panic
- **Wrapping:** `fmt.Errorf("%w", err)` for error chains
- **Types:** Custom error types in `internal/core/errors/`
- **Recovery:** Panic recovery in tool execution

## Security Model

- **Bash Sandboxing:** Dangerous command detection
- **Permission System:** Tool execution requires user approval
- **API Keys:** OS keychain storage only
- **No Telemetry:** Zero phone-home functionality

## Testing Strategy

- **Unit Tests:** Standard Go testing
- **Integration Tests:** `tests/testutil/integration/`
- **E2E Tests:** `tests/testutil/e2e/`
- **Race Detection:** `go test -race`
- **Coverage Target:** 75% overall, 90% for critical paths

## Cross-Cutting Concerns

### Logging

- **Framework:** `log/slog` (structured logging)
- **Levels:** Debug, Info, Warn, Error
- **Output:** stderr (TUI uses terminal)

### Configuration

- **Format:** TOML (`m31a.toml`)
- **Loading:** `internal/core/config/loader.go`
- **Validation:** Schema-based validation

### Metrics

- **Location:** `internal/integrations/metrics/`
- **Purpose:** Performance monitoring, cost tracking
- **Storage:** Local metrics files

## Module Boundaries

### `pkg/` vs `internal/`

- **`pkg/`** - Public packages (keychain)
- **`internal/`** - Private implementation
- **Rule:** `pkg/` must NOT import `internal/`

### Dependency Direction

```
UI → Workflow → Core → Infrastructure → Integrations → Tools
```

**Rule:** Dependencies flow downward. No circular imports.