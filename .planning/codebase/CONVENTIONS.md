# Coding Conventions

**Analysis Date:** 2026-07-16

## Naming Patterns

**Files:**
- Go source: `snake_case.go` (e.g., `engine.go`, `dispatcher.go`, `prompt_builder.go`)
- Test files: `<source>_test.go` (e.g., `engine_test.go`, `dispatcher_test.go`)
- Extra edge-case tests: `<source>_extra_test.go` (e.g., `compaction_extra_test.go`)
- Benchmarks: `benchmarks_test.go` or `<source>_bench_test.go`
- Integration/E2E: `integration_test.go`, `e2e_test.go` (package `m31a_test`)
- Config: `config.toml`, `.goreleaser.yaml`, `.golangci.yml`
- Documentation: `README.md`, `AGENTS.md`, `CHANGELOG.md`, `*.md` in `docs/`, `M31A.wiki/`

**Directories:**
- `cmd/m31a/` — entry point
- `internal/` — private packages (cannot be imported externally)
  externally imported)
- `pkg/` — public packages (can be imported externally)
- `internal/workflow/` — workflow engine (7 phases)
- `internal/tools/` — 18 built-in tools
- `internal/provider/` — LLM providers (OpenRouter, Zen, Nvidia)
- `internal/tui/` — Bubble Tea TUI components
- `pkg/taskrunner/`, `pkg/bisect/`, `pkg/rollback/` — high-coverage libraries

**Functions:**
- Exported: `PascalCase` — `NewEngine`, `RunPhase`, `ExecuteGroup`, `SetPermission`
- Unexported: `camelCase` — `setupTestEngine`, `buildToolDefinitions`, `preflightContextCheck`
- Test helpers: `testDispatcher`, `setupTestEngine`, `newTask`
- Getters: `SessionID()`, `PlanContent()`, `WorkflowMode()`
- Setters: `SetPhaseModel()`, `SetWorkflowMode()`, `SetModel()`, `SetCollector()`
- Boolean: `IsPaused()`, `HasProvider()`, `ShouldCompact()`

**Variables:**
- Exported constants: `PascalCase` — `PhaseInitialize`, `RiskDangerous`, `MaxToolsPerCall`
- Unexported constants: `camelCase` — `maxDiscussPlanCycles`, `defaultContextLength`
- Package-level vars: `camelCase` — `rateTokens`, `batchApprovals`, `stopOnce`
- Local vars: `camelCase` — `sessionID`, `workDir`, `msgEmitter`

**Types:**
- Exported: `PascalCase` — `Engine`, `Dispatcher`, `WorkflowState`, `ToolDefinition`, `ModelInfo`
- Unexported: `camelCase` — `mockProvider`, `toolCallBuilder`, `budgetConfigAdapter`
- Interfaces: `PascalCase` + `er` suffix — `Tool`, `SchemaProvider`, `LLMProvider`, `MsgEmitter`
- Type aliases: `PascalCase` — `WorkflowPhase`, `RiskLevel`, `TaskStatus`, `WorkflowMode`

**Constants:**
- Grouped in `const ( ... )` blocks with descriptive comments
- Prefixed by type name: `PhaseInitialize`, `PhaseDiscuss`, `RiskSafe`, `RiskMedium`
- Sentinel errors: `Err*` prefix — `ErrProviderUnreachable`, `ErrCircularDependency`, `ErrPermissionDenied`

---

## Code Style

**Formatting:**
- **Tool:** `gofmt` (enforced via `make fmt` → `go fmt ./... && goimports -w ...`)
- **Import order:** stdlib → third-party → project (enforced by `goimports`)
- **Line length:** No hard limit; `gofmt` decides
- **Braces:** Same line (Go standard)
- **Tabs:** Go standard (tabs for indentation)

**Linting:**
- **Tool:** `golangci-lint` v2 (config: `.golangci.yml`)
- **Enabled linters:** `govet` (with `shadow`), `staticcheck`, `errcheck`, `ineffassign`, `unused`
- **Test exclusions:** `errcheck` and `unused` disabled for `_test.go` files
- **Run:** `make lint` (5m timeout)

**Key Rules:**
- All code must be `gofmt`-clean
- No emojis in code or docs
- Exported functions/types require doc comments
- Return errors, never panic: `return fmt.Errorf("%w", err)`
- Use `errors.Is`/`errors.As` for error checking
- Wrap errors with context: `fmt.Errorf("context: %w", err)`

---

## Import Organization

**Order (enforced by `goimports`):**
1. Standard library
2. Third-party (github.com, golang.org/x, etc.)
3. Project (`github.com/eshanized/M31A/...`)

**Example:**
```go
import (
    "context"
    "encoding/json"
    "fmt"
    "io"
    "log/slog"
    "os"
    "path/filepath"
    "sync"
    "time"

    "github.com/charmbracelet/bubbletea"
    "github.com/eshanized/M31A/internal/config"
    "github.com/eshanized/M31A/internal/errors"
    "github.com/eshanized/M31A/internal/types"
    "github.com/eshanized/M31A/pkg/session"
)
```

**Path Aliases:** Not used — full module path `github.com/eshanized/M31A/...`

---

## Error Handling

**Patterns:**

1. **Sentinel Errors** (`internal/errors/errors.go`):
```go
var ErrProviderUnreachable = errors.New("provider unreachable")
var ErrCircularDependency = errors.New("circular dependency in task graph")
```

2. **Structured Error Types** with `Unwrap()`:
```go
type ToolError struct {
    Tool string
    Op   string
    Err  error
}

func (e *ToolError) Error() string { ... }
func (e *ToolError) Unwrap() error { return e.Err }
```

3. **User-Friendly Messages** (`errors.UserMessage()`):
```go
func UserMessage(e error) string {
    switch {
    case errors.Is(e, ErrProviderUnreachable):
        return "Provider unreachable — check your internet connection"
    case errors.Is(e, ErrInvalidKey):
        return "Invalid API key — run /settings to update"
    // ... pattern matching for unwrapped errors
    }
    return "An unexpected error occurred — check the logs or try again"
}
```

4. **Error Checking:**
```go
// Sentinel check
if errors.Is(err, m31errors.ErrPermissionDenied) { ... }

// Structured type check
var toolErr *m31errors.ToolError
if errors.As(err, &toolErr) {
    return "Tool " + toolErr.Tool + " failed"
}
```

5. **Never Panic:** All errors returned, wrapped with `%w`.

---

## Logging

**Framework:** Standard library `log/slog` (JSON by default, text via `M31A_LOG_FORMAT=text`)

**Initialization:** `internal/log/log.go` — `NewLogger(version)` returns `*slog.Logger`, cleanup func, error
- Log file: `~/.m31a/m31a.log` (daily rotation, 7-day retention)
- Level: `M31A_LOG_LEVEL` (debug/info/warn/error, default info)

**Patterns:**
```go
// Structured logging
logger.Info("phase started", "phase", phase, "goal", goal)
logger.Error("phase failed", "phase", phase, "error", err)
logger.Warn("checkpoint save failed", "error", err)

// Debug only
logger.Debug("cache miss", "key", key)

// Default logger set globally
slog.SetDefault(logger)
```

**No `fmt.Println` in production code** — use `slog` or `fmt.Fprintln(os.Stderr, ...)` only in `main()` before logger init.

---

## Comments

**When to Comment:**
- Exported types/functions (godoc)
- Non-obvious algorithms/optimizations
- Workarounds for external limitations
- Public API contracts

**Style:**
```go
// WorkflowState groups mutable session state extracted from Engine.
// This struct owns plan state, cached data, intent classification,
// and v1.5 subsystems (decision log, knowledge, budget tracker).
// The Engine retains phase dispatch, LLM streaming, and subsystem orchestration.
type WorkflowState struct { ... }

// compactedMessages builds a new message list with the compaction summary
// prepended and old messages replaced. Keeps the last N messages based on
// the compactor's KeepTokens setting.
func (e *Engine) compactedMessages(original []m31types.Message, summary string) []m31types.Message { ... }
```

**JSDoc/TSDoc:** Not applicable (Go uses godoc format).

**No inline comments for obvious code.**

---

## Function Design

**Size:** Small, single-purpose functions preferred. Large functions decomposed (e.g., `RunPhase` delegates to `runInitialize`, `runDiscuss`, `runExecute`, etc.)

**Parameters:**
- Context first: `func (e *Engine) RunPhase(ctx context.Context, phase WorkflowPhase, goal string)`
- Config structs for multiple params: `EngineOptions`, `GroupConfig`, `TimingConfig`
- Functional options not used (prefer struct configs)

**Return Values:**
- `(T, error)` for fallible operations
- Multiple returns for related values: `(provider.LLMProvider, string)`
- Pointer for mutable/large structs, value for small immutable

**Receivers:**
- Pointer for mutating methods: `func (e *Engine) SetModel(...)`
- Value for read-only: `func (e *Engine) WorkflowMode() WorkflowMode`
- Consistency: prefer pointer receivers for all methods on a type

---

## Module Design

**Exports:** Minimal public API. Internal packages (`internal/`) not importable externally.

**Barrel Files:** Not used — import concrete packages directly.

**Dependency Rule:** `pkg/` MUST NOT import `internal/`. Enforced by Go module system.

**Shared Types:** `internal/types/` and `pkg/types/` — `internal/types/types.go` re-exports `pkg/types` as aliases.

**Interfaces:** Defined in consumer package (e.g., `LLMProvider` in `internal/provider`, `MsgEmitter` in `internal/workflow`).

**Generics:** Used sparingly (Go 1.25+). Example: `sync.Map` for per-request routing in dispatcher.

---

## Key Patterns

### 1. Elm Architecture (Bubble Tea)
```go
// Model holds all state
type App struct { ... }

// Update is the ONLY place state mutates
func (a *App) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
    switch msg := msg.(type) {
    case tea.KeyMsg:
        // handle keys
    case ToolResultMsg:
        // handle tool results
    }
    return a, nil
}

// View derives UI from state
func (a *App) View() string { ... }
```
**Rule:** Never mutate `App` from goroutine. Use `tea.Cmd` / `tea.Msg` via `Program.Send()`.

### 2. Single-Threaded State with Channels
```go
// Dispatcher uses channels for permission requests
requestCh  chan PermissionRequest
responseCh chan PermissionResponse
pendingResponses sync.Map // map[int64]chan PermissionResponse
```

### 3. Builder Pattern for Complex Construction
```go
e := &Engine{ ... }
e.phaseCoordinator = NewPhaseCoordinator(...)
e.contextBuilder = NewContextBuilder(...)
```

### 4. Lazy Initialization with `sync.Once` / `sync.Mutex`
```go
cachedBasePrompt     string
cachedBasePromptOnce sync.Once

func (e *Engine) getBasePrompt() string {
    e.cachedBasePromptOnce.Do(func() { e.cachedBasePrompt = e.buildBasePrompt() })
    return e.cachedBasePrompt
}
```

### 5. Graceful Shutdown
```go
done := make(chan struct{})
cancel context.CancelFunc

func (e *Engine) Shutdown(ctx context.Context) error {
    if e.cancel != nil { e.cancel() }
    select {
    case <-e.done: return nil
    case <-ctx.Done(): return fmt.Errorf("shutdown timeout")
    }
}
```

### 6. Rate Limiting (Token Bucket)
```go
rateTokens chan struct{} // buffered to burst
rateTicker *time.Ticker  // refills per second

// In constructor:
for i := 0; i < burst; i++ { rateTokens <- struct{}{} }
go func() {
    for {
        select {
        case <-rateDone: return
        case <-rateTicker.C:
            select { case rateTokens <- struct{}{} default: }
        }
    }
}()

// Acquire:
<-d.rateTokens
```

### 7. Concurrency Control (Semaphore)
```go
concurrencySem chan struct{} // buffered to MaxConcurrentTools

// Acquire:
d.concurrencySem <- struct{}{}
// Release (in defer):
<-d.concurrencySem
```

### 8. Context Propagation
- All long-running operations accept `context.Context`
- Timeouts via `context.WithTimeout`
- Cancellation respected at each phase boundary

---

## Architectural Constraints

**Threading:** Bubble Tea is strictly single-threaded. All TUI state mutations in `Update()`. Goroutines communicate via `Program.Send(tea.Msg)`.

**Global State:** 
- `slog.Default()` — logger (set once in `main()`)
- Version vars: `Version`, `Commit`, `Date`, `GoVersion` (set via `-ldflags` at build)
- No other package-level mutable globals

**Circular Imports:** 
- `pkg/` → `internal/` forbidden
- `internal/provider` ↔ `internal/tools` avoided via `internal/types`

**Hard Constraints:**
- `CGO_ENABLED=0` — static binary only (build fails if any dep requires CGO)
- Go 1.25.0 minimum (enforced in `go.mod`)
- Provider models discovered dynamically — never hardcode model names
- API keys via OS keychain (`pkg/keychain/`) — never plaintext on disk

---

## Anti-Patterns

### 1. Mutating TUI State from Goroutine
**What happens:** Direct field assignment on `App` from background goroutine.
**Why it's wrong:** Violates Bubble Tea's single-threaded contract; causes race conditions, session corruption.
**Do this instead:**
```go
// In goroutine:
p.Send(tea.Msg{...}) // Send message to TUI

// In Update():
case MyCustomMsg:
    a.state = msg.State // Mutate HERE only
```

### 2. Hardcoding Model Names
**What happens:** `"gpt-4"` or `"claude-3"` in code.
**Why it's wrong:** Models change; providers have different catalogs.
**Do this instead:**
```go
models, _ := provider.FetchModels(ctx)
// Present to user or select via capability matching
```

### 3. Ignoring Context Cancellation
**What happens:** Long-running operations don't check `ctx.Done()`.
**Why it's wrong:** Leaks goroutines, prevents graceful shutdown.
**Do this instead:**
```go
select {
case <-ctx.Done():
    return ctx.Err()
case result := <-resultCh:
    return process(result)
}
```

### 4. Using `panic` for Control Flow
**What happens:** `panic("unexpected")` in library code.
**Why it's wrong:** Crashes entire process; not recoverable by caller.
**Do this instead:** Return `error` with `fmt.Errorf("%w", err)`.

### 5. Importing `internal/` from `pkg/`
**What happens:** `pkg/taskrunner` imports `internal/workflow`.
**Why it's wrong:** Breaks Go module encapsulation; `internal` is not importable.
**Do this instead:** Move shared types to `pkg/types/` or `internal/types/`.

---

*Convention analysis: 2026-07-16*