# Code Conventions

**Analysis Date:** 2026-07-21

## Formatting

- All code must be `gofmt`-clean (enforced via `make fmt` which runs `gofmt` + `goimports`)
- Imports grouped: stdlib / third-party / project (enforced by `goimports`)
- No emojis in code or docs
- Line length: no explicit limit, but long function signatures and string literals are wrapped
- Trailing commas in multi-line composite literals

## Error Handling

**Core Principles:**
- Return errors, never panic
- Wrap with `fmt.Errorf("%w", err)` for error chaining
- Check errors immediately after calls that can fail
- Use sentinel errors (`errors.Is`) for cross-package error matching
- Use structured error types (`errors.As`) for context-rich errors

**From `internal/errors/errors.go`:**
```go
// Sentinel errors for common failure modes
var (
    ErrProviderUnreachable = errors.New("provider unreachable")
    ErrInvalidKey          = errors.New("invalid API key")
    ErrRateLimited         = errors.New("rate limited")
    ErrContextExceeded     = errors.New("context window exceeded")
    ErrPermissionDenied    = errors.New("permission denied")
    ErrCircularDependency  = errors.New("circular dependency in task graph")
    ErrBisectResetFailed   = errors.New("bisect reset failed")
)

// Structured error types with context
type ToolError struct {
    Tool string  // Tool name
    Op   string  // Operation attempted
    Err  error   // Underlying error
}

func (e *ToolError) Error() string {
    if e.Op != "" {
        return "tool " + e.Tool + ": " + e.Op + ": " + e.Err.Error()
    }
    return "tool " + e.Tool + ": " + e.Err.Error()
}
func (e *ToolError) Unwrap() error { return e.Err }

type ProviderError struct {
    Provider   string
    Model      string
    StatusCode int
    Err        error
}
func (e *ProviderError) Error() string {
    msg := "provider " + e.Provider
    if e.Model != "" { msg += " model " + e.Model }
    if e.StatusCode > 0 { msg += " (HTTP " + itoa(e.StatusCode) + ")" }
    return msg + ": " + e.Err.Error()
}
func (e *ProviderError) Unwrap() error { return e.Err }

type ConfigError struct {
    Key string
    Err error
}
func (e *ConfigError) Error() string {
    if e.Key != "" { return "config " + e.Key + ": " + e.Err.Error() }
    return "config: " + e.Err.Error()
}
func (e *ConfigError) Unwrap() error { return e.Err }
```

**User-Facing Messages:**
```go
// UserMessage returns actionable messages for common errors
func UserMessage(e error) string {
    switch {
    case errors.Is(e, ErrProviderUnreachable):
        return "Provider unreachable — check your internet connection"
    case errors.Is(e, ErrInvalidKey):
        return "Invalid API key — run /settings to update"
    case errors.Is(e, ErrRateLimited):
        return "Rate limited — retry in a moment"
    // ... 30+ cases
    }
    // Pattern matching for unwrapped errors
    errStr := strings.ToLower(e.Error())
    switch {
    case strings.Contains(errStr, "connection refused"):
        return "Cannot reach provider — check your internet connection"
    // ...
    }
    return "An unexpected error occurred — check the logs or try again"
}
```

**In Practice (from `internal/tools/dispatcher.go`):**
```go
// Immediate check with context
select {
case d.concurrencySem <- struct{}{}:
    defer func() { <-d.concurrencySem }()
case <-ctx.Done():
    return types.ToolResult{}, ctx.Err()
}

// Error wrapping with context
if !ok {
    available := d.List()
    return types.ToolResult{}, fmt.Errorf("%w: unknown tool: %s. Available tools: %s",
        m31errors.ErrToolExecution, call.Name, strings.Join(available, ", "))
}

// JSON unmarshal with raw input preservation for debugging
var input types.ToolInput
if err := json.Unmarshal(inputBytes, &input); err != nil {
    rawInput := string(inputBytes)
    if len(rawInput) > 200 { rawInput = rawInput[:200] + "…" }
    return types.ToolResult{}, fmt.Errorf("tool %s: invalid input JSON: %w. Raw input: %s",
        call.Name, err, rawInput)
}

// Tool execution with error wrapping
result, err := tool.Execute(ctx, input)
if err != nil {
    res.Error = err.Error()
    return res, fmt.Errorf("tool %s: %w", call.Name, err)
}
```

## Documentation

- Exported functions/types need doc comments (enforced by `golangci-lint` via `govet`)
- Package comments in `doc.go` or package comment block
- **From `internal/taskrunner/doc.go`:**
```go
// Package taskrunner schedules and executes tasks with dependency resolution.
// It performs topological sorting to determine execution order, tracks task
// status, and supports self-healing retries for failed tasks.
package taskrunner
```
- **From `internal/bisect/doc.go`:**
```go
// Package bisect wraps git bisect to automatically identify the commit that
// introduced a regression. It runs a user-supplied check function across a
// commit range and returns the offending commit with its diff.
package bisect
```

## Naming Conventions

| Element | Convention | Example |
|---------|------------|---------|
| Files | snake_case.go | `engine.go`, `runner_test.go` |
| Types | PascalCase | `Engine`, `ToolResult`, `WorkflowPhase` |
| Interfaces | noun or -er suffix | `LLMProvider`, `GitRunner`, `SchemaProvider` |
| Packages | lowercase, singular | `workflow`, `tools`, `bisect` |
| Private | lowercase | `dispatcher`, `rateTokens` |
| Public | PascalCase | `Execute`, `FetchModels`, `RunPhase` |
| Constants | PascalCase | `PhaseInitialize`, `RiskDangerous`, `ModeAuto` |
| Test functions | `Test<Name>` | `TestDispatcher_RegisterAndExecute` |
| Table-driven subtests | `t.Run("case", ...)` | `t.Run("exact match", ...)` |

## Patterns from Codebase

### Error Handling Pattern
**From `internal/tools/dispatcher.go`:**
```go
if err != nil {
    return fmt.Errorf("dispatch tool: %w", err)
}
// or with more context
return types.ToolResult{}, fmt.Errorf("tool %s: %w", call.Name, err)
```

### TUI Pattern (Bubble Tea / Elm Architecture)
**From `cmd/m31a/main.go` and `internal/tui/`:**
- All state mutations go through `Update()` only
- Never mutate `AppState` from a goroutine
- Use `tea.Cmd` / `tea.Msg` for async operations
- Signal handler sends `tea.QuitMsg` through program channel, not direct `app.Shutdown()`

```go
// Signal handler sends through Bubble Tea channel
sigCh := make(chan os.Signal, 1)
signal.Notify(sigCh, syscall.SIGTERM, syscall.SIGINT)
go func() {
    for {
        select {
        case <-sigCh:
            p.Send(tea.QuitMsg{})  // NOT app.Shutdown() directly
        case <-sigDone:
            return
        }
    }
}()
```

### Provider Pattern
**From `internal/provider/interface.go`:**
```go
type LLMProvider interface {
    Name() string
    APIKey() string
    FetchModels(ctx context.Context) ([]types.ModelInfo, error)
    CachedModels() []types.ModelInfo
    ChatCompletionStream(ctx context.Context, req ChatRequest) (*types.StreamIterator, error)
    EstimateCost(modelID string, usage types.Usage) float64
    HealthCheck(ctx context.Context) types.HealthStatus
    GetModel(id string) (*types.ModelInfo, error)
}
```
- Interface in `internal/provider/provider.go`
- Implementations in `internal/provider/{openrouter,zen,nvidia}/`
- Dynamic model discovery from APIs (never hardcode model names)
- Compile-time interface check: `var _ provider.LLMProvider = (*Client)(nil)`

### Tool Dispatcher Pattern
**From `internal/tools/dispatcher.go`:**
- Permissions, rate limiting, concurrency in one place
- Tools registered in `internal/tools/defaults.go`
- `Dispatcher.Execute()` handles: concurrency sem → rate limit → permission check → tool exec → output bounding

```go
// Concurrency limiter (M5)
select {
case d.concurrencySem <- struct{}{}:
    defer func() { <-d.concurrencySem }()
case <-ctx.Done():
    return types.ToolResult{}, ctx.Err()
}

// Rate limit (token bucket)
select {
case <-d.rateTokens:
case <-ctx.Done():
    return types.ToolResult{}, ctx.Err()
}

// Permission evaluation (rules → agent defaults → risk level)
allowed, pctx, err := d.checkPermission(call.Name, input)
// ... prompts user via channel if needed ...

// Execute with metrics
result, err := tool.Execute(ctx, input)
if d.collector != nil {
    d.collector.RecordToolCall(call.Name, err == nil, elapsed)
}
```

### Shared Types (`internal/types/`)
**From `internal/types/types.go`:**
- Central type vocabulary: `Message`, `ToolCall`, `ToolResult`, `Task`, `ModelInfo`, `WorkflowPhase`, `RiskLevel`
- `pkg/` packages import from `internal/types` (not vice versa — enforced by Go module)
- JSON serialization with custom `MarshalJSON` for nil-slice handling

```go
func (m Message) MarshalJSON() ([]byte, error) {
    if m.Segments == nil {
        m.Segments = []MessageSegment{}
    }
    type msgAlias Message
    return json.Marshal(msgAlias(m))
}
```

### State Machine Pattern
**From `internal/workflow/engine.go`:**
```go
type Engine struct {
    stateMachine     *StateMachine
    state            *WorkflowState
    // ... many fields
    pauseMu          sync.Mutex
    pauseCh          chan struct{}
    resumeCh         chan struct{}
    // Phase coordination delegated to PhaseCoordinator
}
```
- `StateMachine` validates phase transitions
- `PhaseCoordinator` handles pre/post-phase side effects
- Mutexes for shared state (`planMu`, `messagesMu`, `transitionMu`)
- Channels for pause/resume coordination

### Dependency Injection for Testability
**From `internal/bisect/bisect.go`:**
```go
type GitRunner interface {
    Run(args ...string) (string, error)
    LogAll() ([]types.CommitInfo, error)
    // ...
}

type Bisect struct {
    workDir string
    logger  *slog.Logger
    git     GitRunner  // interface, not concrete *git.Git
}

func (b *Bisect) SetGit(g GitRunner) { b.git = g }
```
- Interfaces for external dependencies (git, HTTP)
- `SetGit` for post-construction wiring
- Enables mock/fake in tests

### Concurrency Safety
- `sync.RWMutex` for read-heavy shared state
- `sync.Mutex` for write-heavy
- `sync.Once` for initialization (e.g., `Dispatcher.Stop()` uses `stopOnce`)
- `atomic.Bool`/`atomic.Int64` for lock-free flags/counters
- Channels for goroutine coordination (pause/resume, permission requests)
- **Never** shared mutable state across goroutines without synchronization

### Configuration
- TOML config in `~/.m31a/config.toml`
- Environment variables for secrets (API keys)
- OS keychain for secure storage (`pkg/keychain/`)
- `.env` files gitignored (except `.env.example`)

## Key Files for Reference

| File | Purpose |
|------|---------|
| `Makefile` | Build, test, lint targets |
| `.golangci.yml` | Linter config (govet, staticcheck, errcheck, ineffassign, unused) |
| `go.mod` | Module, Go 1.25.0, dependencies |
| `internal/errors/errors.go` | Sentinel errors, structured error types, user messages |
| `internal/tools/dispatcher.go` | Permission, rate limiting, concurrency patterns |
| `internal/provider/interface.go` | Provider interface |
| `internal/provider/base_client.go` | Shared HTTP client, error handling, model cache |
| `internal/types/types.go` | Core type definitions |
| `internal/workflow/engine.go` | Workflow engine, state machine, phase coordination |
| `internal/taskrunner/runner.go` | Task scheduling, topological sort, parallel execution |
| `internal/bisect/bisect.go` | Git bisect wrapper with interface injection |
| `internal/rollback/rollback.go` | Git reset operations with stash/backup |
| `cmd/m31a/main.go` | Entry point, TUI setup, signal handling |

---

*Convention analysis: 2026-07-21*