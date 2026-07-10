# Code Conventions

> M31A codebase conventions reference — 2026-07-10

## Go Code Formatting

All code must be `gofmt`-clean. The `Makefile` enforces this in the `check` pipeline:

```makefile
# Makefile lines 133-136
fmt:
	@$(GO) fmt ./...
	@goimports -w $$(find . -name '*.go' -not -path './vendor/*') 2>/dev/null || true
```

Running `make check` executes: `fmt -> tidy -> vet -> lint -> test` in sequence (`Makefile` line 144).

## Import Grouping

Imports are organized into three groups separated by blank lines:
1. **Standard library** (e.g., `context`, `fmt`, `os`)
2. **Third-party** (e.g., `github.com/charmbracelet/bubbletea`)
3. **Project internal** (e.g., `github.com/eshanized/M31A/internal/...`)

Example from `cmd/m31a/main.go` (lines 3-32):

```go
import (
	"context"
	"flag"
	"fmt"
	"log/slog"
	"os"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/log"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/autodream"
	"github.com/eshanized/M31A/pkg/keychain"
)
```

Package aliasing is used when names would conflict:

```go
// internal/workflow/engine.go
m31errors "github.com/eshanized/M31A/internal/errors"
m31types "github.com/eshanized/M31A/internal/types"
ctxsrc "github.com/eshanized/M31A/internal/context"

// internal/tools/dispatcher.go
m31errors "github.com/eshanized/M31A/internal/errors"
```

The `m31errors` alias is the standard convention for the project's error package to avoid conflicts with the stdlib `errors` package.

## Error Handling Patterns

### Return Errors, Never Panic

The codebase consistently returns errors rather than panicking. The only `recover()` calls are in goroutine safety nets:

```go
// cmd/m31a/main.go — goroutine panic recovery
go func() {
	defer func() {
		if r := recover(); r != nil {
			slog.Error("signal handler panic", "error", r)
		}
	}()
	// ...
}()
```

### Error Wrapping with `fmt.Errorf` and `%w`

All errors are wrapped using `fmt.Errorf` with `%w` to preserve the error chain:

```go
// internal/fileutil/atomic.go
return fmt.Errorf("create temp file: %w", err)
return fmt.Errorf("write temp file: %w", err)
return fmt.Errorf("rename temp file: %w", err)

// internal/config/loader.go
return nil, fmt.Errorf("decode global config %s: %w", path, err)
return nil, fmt.Errorf("config validation: %w", err)

// internal/git/git.go
return string(out), fmt.Errorf("git %s: %w: %s", strings.Join(args, " "), err, string(out))
```

The wrapping message follows a consistent pattern: `"<noun/context>: %w"`.

### Sentinel Errors

The project defines sentinel errors in `internal/errors/errors.go` using `errors.New()`:

```go
// internal/errors/errors.go — organized by domain
// Provider errors
ErrProviderUnreachable = errors.New("provider unreachable")
ErrRateLimited         = errors.New("rate limited")
ErrInvalidKey          = errors.New("invalid API key")

// Session and workflow errors
ErrSessionCorrupted   = errors.New("session data corrupted")
ErrPhaseTransition    = errors.New("invalid phase transition")

// Tool errors
ErrToolExecution      = errors.New("tool execution failed")
ErrCircularDependency = errors.New("circular dependency in task graph")

// File errors
ErrPermissionDenied = errors.New("permission denied")
ErrFileTooLarge     = errors.New("file exceeds 5MB limit")
```

### Structured Error Types

Three structured error types provide domain-specific context (`internal/errors/errors.go`):

```go
// ToolError — wraps tool execution failures
type ToolError struct {
	Tool string  // e.g., "bash"
	Op   string  // e.g., "execute"
	Err  error
}
func (e *ToolError) Unwrap() error { return e.Err }

// ProviderError — wraps API provider failures
type ProviderError struct {
	Provider   string
	Model      string
	StatusCode int
	Err        error
}

// ConfigError — wraps config loading/validation failures
type ConfigError struct {
	Key string
	Err error
}
```

### User-Friendly Error Messages

`UserMessage()` in `internal/errors/errors.go` translates technical errors to user-facing messages:

```go
case errors.Is(e, ErrRateLimited):
	return "Rate limited — retry in a moment"
case errors.Is(e, ErrContextExceeded):
	return "Context window exceeded — conversation too long. Use /compress to reduce context."
```

## Doc Comment Conventions

Exported functions, types, and packages must have doc comments. Comments explain **why**, not **what**:

```go
// cmd/m31a/main.go
// restoreTerminal writes ANSI escape sequences to undo alt-screen mode,
// mouse capture, and hidden cursor. Called before os.Exit in the hard
// fallback path so the user's terminal is not left in a broken state.
func restoreTerminal() { ... }

// internal/tools/dispatcher.go
// NewDispatcher creates a new Dispatcher with a background rate-limiter goroutine.
// The caller MUST call Stop() when the Dispatcher is no longer needed to prevent
// goroutine leaks (e.g., during session restart or app shutdown).
func NewDispatcher(cfg *config.PermissionsConfig) *Dispatcher { ... }
```

Design decision references use code markers like `(WP-C03)`, `(M5)`, `(H-24)`, `(C-12)`:

```go
// Working directory — fail fast if Getwd fails (WP-C03)
// Concurrency limiter (M5): limit concurrent tool executions.
// H-24: Hard fallback — force exit after 5 seconds if TUI doesn't quit.
// C-12: Uses sync.Once to prevent TOCTOU race on concurrent calls.
```

## No Emojis Policy

No emojis are used in code or documentation. This is explicitly stated in `CONTRIBUTING.md` (line 41).

## Naming Conventions

### Packages
- Lowercase, single-word names: `tools`, `config`, `provider`, `tui`, `types`, `codeintel`
- Subpackages use specific names: `tools/subagent`, `provider/openrouter`, `tui/theme`

### Types
- MixedCaps for exported: `AppState`, `Dispatcher`, `WorkflowPhase`, `ToolError`
- String-typed enums use `const` blocks:

```go
// internal/types/types.go
type RiskLevel string
const (
	RiskSafe        RiskLevel = "safe"
	RiskMedium      RiskLevel = "medium"
	RiskDangerous   RiskLevel = "dangerous"
	RiskDestructive RiskLevel = "destructive"
)

type WorkflowPhase string
const (
	PhaseIdle       WorkflowPhase = "idle"
	PhaseInitialize WorkflowPhase = "initialize"
	PhaseDiscuss    WorkflowPhase = "discuss"
	// ...
)
```

### Functions
- Constructors: `New<Type>()` (e.g., `NewBash()`, `NewDispatcher()`, `NewRegistry()`)
- Getters: direct name without `Get` prefix for simple accessors (e.g., `Name()`, `Active()`)
- Getters with `Get` prefix for non-trivial operations (e.g., `GetTool()`, `GetModel()`)
- Boolean methods: `Is<Condition>()` (e.g., `IsWorkflowWorthy()`, `IsRateLimited()`)

### Variables
- Package-level constants: `MaxRetries`, `DefaultTimeout`, `ToolRateLimitBurst`
- Unexported struct fields: `camelCase` with trailing underscore for disambiguation: `workDir_`
- Error variables: `Err<Category>` (e.g., `ErrProviderUnreachable`, `ErrSessionCorrupted`)

### File Naming
- Source: lowercase, no separators for single words: `dispatcher.go`, `engine.go`
- Multi-word: underscores: `app_update.go`, `base_client.go`, `cache_refresh.go`
- Tests: `*_test.go` co-located with source: `dispatcher_test.go`, `engine_test.go`
- Security tests: `bash_security_test.go`
- Extra coverage: `extra_test.go` files for additional test cases

## Package Organization

### Dependency Rule

`pkg/` must NOT import `internal/`. This is enforced by the Go module system:

```
cmd/m31a/          # Entry point — imports both internal/ and pkg/
internal/          # Private packages
  types/           # Shared type vocabulary (exception: imported by pkg/)
  errors/          # Sentinel errors
  tools/           # Tool implementations
  tui/             # Bubble Tea TUI
  workflow/        # Workflow engine
  provider/        # LLM providers
pkg/               # Public packages
  taskrunner/      # Kahn's algorithm
  rollback/        # Git rollback
  session/         # Session persistence
  keychain/        # OS keychain
```

**Exception**: `internal/types` and `internal/errors` are imported by `pkg/` packages since they contain shared type vocabulary.

### `internal/types` as Shared Vocabulary

`internal/types/types.go` defines types used across all layers: `ToolInput`, `ToolResult`, `ToolCall`, `Message`, `RiskLevel`, `WorkflowPhase`, `ModelInfo`, etc.

## Bubble Tea Patterns

### Single-Threaded Update Contract

**Critical rule**: Never mutate `AppState` from a goroutine. All state mutations go through `Update()` only.

From `internal/tui/app_update.go` (lines 14-18):

```go
// Update implements tea.Model. It is the single dispatch point for all messages.
// CRITICAL: Never mutate AppState from a goroutine. All mutations go here.
//
// Each case delegates to a handler function in the corresponding handler_*.go
// file. This keeps Update() as a thin dispatcher while preserving all behavior.
func (m *AppState) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
```

### tea.Cmd and tea.Msg Pattern

Background work is done through `tea.Cmd` functions that return `tea.Msg`:

```go
// Signal handling — sends tea.QuitMsg through program channel
// instead of calling app.Shutdown() directly from a goroutine
p.Send(tea.QuitMsg{})

// Async operations return tea.Cmd
baseCmds := []tea.Cmd{
	m.routeToScreen(),
	NextHealthTick(m.shutdownCtx, types.HealthCheckInterval),
	permListenerCmd(m.shutdownCtx, m.dispatcher),
}
return tea.Batch(cmds...)
```

### Message-Based Architecture

`Update()` dispatches on concrete `tea.Msg` types:

```go
case tea.WindowSizeMsg:  // window resize
case tea.KeyMsg:         // keyboard input
case StreamMsg:          // LLM streaming chunks
case StreamDoneMsg:      // stream completed
case SlashCommandMsg:    // slash command executed
case HomeSubmitMsg:      // home screen input submitted
case IntentClassifiedMsg: // intent classification complete
```

### Channel-Based Communication (Goroutine to TUI)

Goroutines communicate with the TUI through typed channels consumed by listener commands:

```go
// Permission listener — bridges dispatcher channels to tea.Msg
baseCmds = append(baseCmds, permListenerCmd(m.shutdownCtx, m.dispatcher))

// Question listener
baseCmds = append(baseCmds, questionListenerCmd(m.shutdownCtx, m.dispatcher))

// Subagent event listener
baseCmds = append(baseCmds, subagentListenerCmd(m.shutdownCtx, m.subagentManager.Events()))
```

## Concurrency Patterns

### sync.RWMutex for Concurrent State

```go
// internal/tools/dispatcher.go
type Dispatcher struct {
	mu    sync.RWMutex
	tools map[string]types.Tool
	// ...
}

func (d *Dispatcher) List() []string {
	d.mu.RLock()
	defer d.mu.RUnlock()
	// ...
}

func (d *Dispatcher) Register(tool types.Tool) error {
	d.mu.Lock()
	defer d.mu.Unlock()
	// ...
}
```

### sync.Once for One-Time Operations

```go
// internal/tools/dispatcher.go — prevent TOCTOU race in Stop()
stopOnce sync.Once

func (d *Dispatcher) Stop() {
	d.stopOnce.Do(func() {
		close(d.rateDone)
		d.rateTicker.Stop()
		// ...
	})
}
```

### Token Bucket Rate Limiting

```go
// Rate limit tool execution
rateTokens chan struct{}  // token bucket
select {
case <-d.rateTokens:     // acquire token
case <-ctx.Done():       // respect context cancellation
	return types.ToolResult{}, ctx.Err()
}
```

### Concurrency Semaphore

```go
// Concurrency limiter (M5): limit concurrent tool executions
concurrencySem chan struct{}
select {
case d.concurrencySem <- struct{}{}:
	defer func() { <-d.concurrencySem }()
case <-ctx.Done():
	return types.ToolResult{}, ctx.Err()
}
```

## Conventional Commits

All commit messages follow the conventional commits format as specified in `CONTRIBUTING.md` (line 124):

| Prefix | Usage |
|--------|-------|
| `feat:` | New features |
| `fix:` | Bug fixes |
| `docs:` | Documentation changes |
| `test:` | Test additions/changes |
| `refactor:` | Code refactoring |
| `chore:` | Build, tooling, dependency changes |

## Lint Configuration

Defined in `.golangci.yml`:

```yaml
version: "2"
run:
  timeout: 5m
linters:
  enable:
    - govet        # with shadow detection enabled
    - staticcheck
    - errcheck
    - ineffassign
    - unused
  settings:
    govet:
      enable:
        - shadow
    errcheck:
      check-type-assertions: false
      check-blank: false
  exclusions:
    rules:
      - path: _test\.go
        linters:
          - errcheck
          - unused
```

Test files (`_test.go`) are excluded from `errcheck` and `unused` linters.

## Build Constraints

- `CGO_ENABLED=0` is a hard constraint — binary must be static
- Go 1.25+ required (per `go.mod`)
- Build flags: `-trimpath -ldflags "-s -w"` for release builds
- No vendor directory — uses module cache

## nolint Directives

When suppressing lint warnings, `//nolint:errcheck` is used with specific linter names:

```go
// cmd/m31a/main.go
defer stream.Close() //nolint:errcheck

// internal/testutil/envtest.go
_ = os.Setenv(key, val) //nolint:errcheck
```
