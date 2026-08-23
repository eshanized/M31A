# Coding Conventions

**Analysis Date:** 2026-08-23

## Languages

**Primary:**
- Go 1.26.5 - All source code in `cmd/`, `internal/`, `pkg/`, `tests/`

## Runtime

**Environment:**
- Go 1.26.5 (enforced in `go.mod`)

**Package Manager:**
- Go modules (`go.mod`, `go.sum`)
- Lockfile: present (`go.sum`)

## Frameworks

**Core:**
- Bubble Tea v1.3.0 (`github.com/charmbracelet/bubbletea`) — TUI framework (Elm architecture)
- Bubbles v0.20.0 (`github.com/charmbracelet/bubbles`) — UI components
- Lipgloss v1.1.0 (`github.com/charmbracelet/lipgloss`) — Styling
- Glamour v0.6.0 (`github.com/charmbracelet/glamour`) — Markdown rendering

**Testing:**
- Go standard library `testing` package
- No external test framework (no testify, ginkgo, etc.)

**Build/Dev:**
- `golangci-lint` v2 — Linting (govet, staticcheck, errcheck, ineffassign, unused)
- `goimports` — Import formatting
- `goreleaser` — Release automation

## Key Dependencies

**Critical:**
- `github.com/charmbracelet/bubbletea` v1.3.0 — TUI runtime; single-threaded event loop, all state mutations via `Update()`
- `github.com/charmbracelet/lipgloss` v1.1.0 — Declarative styling
- `golang.org/x/sync` v0.22.0 — `singleflight` for request deduplication
- `golang.org/x/time` v0.15.0 — `rate` limiter (zero-goroutine token bucket)

**Infrastructure:**
- `github.com/BurntSushi/toml` v1.6.0 — Config parsing
- `github.com/fsnotify/fsnotify` v1.10.1 — File watching
- `github.com/pkoukk/tiktoken-go` v0.1.8 — Token estimation
- `github.com/godbus/dbus/v5` v5.2.2 — Linux keychain integration

## Configuration

**Environment:**
- Config file: `~/.m31a/config.toml` (TOML)
- `.env` files gitignored except `.env.example`
- API keys stored in OS keychain via `pkg/keychain/`, never written to disk in plaintext
- Key env vars: `OPENROUTER_API_KEY`, `ZEN_API_KEY`, `NVIDIA_API_KEY`, `M31A_CONFIG`, `M31A_DEBUG`, `M31A_LOG_LEVEL`

**Build:**
- `CGO_ENABLED=0` (hard constraint — static binary)
- Build flags: `-trimpath -ldflags "-s -w -X main.Version=... -X main.Commit=..."`
- Cross-compilation targets: linux/{amd64,arm64}, darwin/{amd64,arm64}, windows/amd64

## Naming Patterns

**Files:**
- `snake_case.go` for all Go files
- Test files: `<name>_test.go` (co-located with source)
- Extra test files: `<name>_extra_test.go`, `<name>_fixes_test.go` for supplemental tests
- Benchmark files: `<name>_bench_test.go`

**Packages:**
- Lowercase, single word preferred: `tools`, `config`, `types`, `workflow`, `session`, `provider`
- Subpackages: `internal/tools/search`, `internal/tools/subagent`, `internal/ui/tui/theme`

**Types:**
- PascalCase for exported: `Dispatcher`, `Tool`, `WorkflowPhase`, `ModelInfo`
- PascalCase for unexported: `dispatcher`, `permissionRequest`
- Interface suffix: `Tool`, `SchemaProvider`, `LLMProvider`
- Error types: `ToolError`, `ProviderError`, `ConfigError`

**Functions/Methods:**
- PascalCase for exported: `NewDispatcher()`, `Execute()`, `Register()`
- camelCase for unexported: `newDispatcher()`, `ensurePermission()`, `drainChannels()`
- Constructor pattern: `New<Type>()` returns pointer
- Builder pattern: `DefaultDispatcher()` for factory with defaults

**Variables/Constants:**
- PascalCase for exported constants: `RiskSafe`, `PhaseInitialize`, `DefaultPermissionTimeout`
- camelCase for unexported constants: `PermissionChannelBuffer`, `ToolRateLimitPerSec`
- camelCase for local variables
- `*_` prefix for unused variables (lint: `unused`)

**Files/Directories:**
- `internal/` — Private packages (enforced by Go module system, `pkg/` must NOT import `internal/`)
- `pkg/` — Public packages (importable by external projects)
- `cmd/m31a/` — Main entry point
- `tests/` — Test utilities and integration/e2e tests

## Code Style

**Formatting:**
- Tool: `gofmt` (via `make fmt`) + `goimports` for import grouping
- Run: `make check` (fmt → tidy → vet → lint → test)
- Line length: No hard limit (standard Go style)
- Tabs for indentation

**Linting:**
- Tool: `golangci-lint` v2 (config: `.golangci.yml`)
- Enabled linters:
  - `govet` with `shadow` check
  - `staticcheck`
  - `errcheck` (excludes test files, `check-type-assertions: false`, `check-blank: false`)
  - `ineffassign`
  - `unused` (excludes test files)
- Timeout: 5 minutes

**Import Organization (enforced by goimports):**
1. Standard library
2. Third-party (blank line)
3. Project packages (`github.com/eshanized/M31A/...`)

Example from `cmd/m31a/main.go`:
```go
import (
	"context"
	"flag"
	"fmt"
	"log/slog"
	"net"
	"net/http"
	_ "net/http/pprof"
	"os"
	"os/signal"
	"path/filepath"
	"runtime"
	"strings"
	"sync/atomic"
	"syscall"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/core/types"
	...
)
```

## Error Handling

**Strategy:**
- Return errors, never panic (except `observability.RecoverAndCapture()` for crash reporting)
- Wrap with `errors.Wrap()` / `errors.Wrapf()` from `internal/core/errors/errors.go`
- Use `fmt.Errorf("%w", err)` only via the `errors.Wrap` helpers
- Custom error types with `Unwrap()` for error chaining

**Error Types (`internal/core/errors/errors.go`):**
```go
// Sentinel errors for common failures
var (
    ErrProviderUnreachable = errors.New("provider unreachable")
    ErrRateLimited         = errors.New("rate limited")
    ErrInvalidKey          = errors.New("invalid API key")
    ErrContextExceeded     = errors.New("context window exceeded")
    ErrToolExecution       = errors.New("tool execution failed")
    ErrPermissionDenied    = errors.New("permission denied")
    // ... more
)

// Structured error types
type ToolError struct {
    Tool string
    Op   string
    Err  error
}

type ProviderError struct {
    Provider   string
    Model      string
    StatusCode int
    Err        error
}

type ConfigError struct {
    Key string
    Err error
}
```

**User-Facing Messages:**
- Use `errors.UserMessage(err)` for UI display — maps technical errors to actionable messages
- Pattern matching on error strings for unwrapped errors (connection refused, TLS, timeouts, HTTP status codes)

**In Tool Execution (`internal/tools/dispatcher.go`):**
- Tools return `types.ToolResult{Error: string}` for expected failures (not Go errors)
- Go errors propagated for unexpected failures (context cancellation, I/O errors)
- `toolResultError` wrapper distinguishes rule-level errors from real errors

## Logging

**Framework:**
- `log/slog` (standard library structured logging)
- Logger initialized in `cmd/m31a/main.go` via `internal/integrations/log.NewLogger()`
- Default level: `info`; debug via `--debug` flag or `M31A_DEBUG=1`

**Patterns:**
- Structured logging: `slog.Info("msg", "key", value, "key2", value2)`
- Debug for verbose: `slog.Debug("tool executed", "tool", name, "duration_ms", elapsed)`
- Warn for recoverable issues: `slog.Warn("session cleanup failed", "error", err)`
- Error for failures: `slog.Error("workflow engine init failed", "err", err)`
- Never log secrets (API keys filtered by logger)

## Comments

**When to Comment:**
- All exported functions, types, constants, and variables (go vet enforces)
- Non-obvious logic: `// C-12: sync.Once prevents TOCTOU race in Stop()`
- Architectural decisions: `// W2: Persist session state before cancelling contexts...`
- Bug references: `// B23 fix: ...`, `// H-24: Hard fallback...`
- TODO/FIXME for known issues

**JSDoc/TSDoc:**
- Go doc comments (godoc format)
- Example from `internal/core/types/types.go`:
```go
// ToolError is a structured error type that carries both an error message
// and a hint for LLM self-recovery. Tools should return this when they
// want to provide actionable guidance alongside the error.
type ToolError struct { ... }
```

## Function Design

**Size:**
- Small, focused functions (typically < 50 lines)
- Private helpers extracted for clarity

**Parameters:**
- Context first: `func (d *Dispatcher) Execute(ctx context.Context, call types.ToolCall)`
- Configuration via struct: `func NewManager(globalConfigDir, workDir string, opts ManagerOpts)`
- Options pattern for optional config: `ManagerOpts{CoordinatorTimeoutSecs: 30}`

**Return Values:**
- `(T, error)` for fallible operations
- Single return for infallible
- Named returns used sparingly (mainly for `defer` cleanup)

## Module Design

**Exports:**
- Minimal public API — prefer unexported with public constructors
- Interfaces in `internal/core/types/` (shared vocabulary)
- `pkg/` packages: `extensions`, `taskrunner`, `bisect`, `rollback` — no `internal/` imports

**Barrel Files:**
- Not used; each package exports directly
- `internal/tools/defaults.go` registers all built-in tools

**Dependency Rule:**
```
pkg/ ──────► internal/  (FORBIDDEN)
internal/ ► pkg/        (ALLOWED)
cmd/    ► internal/, pkg/
tests/  ► internal/, pkg/
```
Enforced by Go module system — `pkg/` cannot import `internal/`.

## Architectural Patterns

**Bubble Tea (Elm Architecture):**
- Model: `AppState` in `internal/ui/tui/app_state.go`
- Update: `func (m *AppState) Update(msg tea.Msg) (tea.Model, tea.Cmd)` — ALL state mutations here
- View: `func (m *AppState) View() string`
- Commands: `tea.Cmd` functions returning `tea.Msg`
- Messages: `tea.Msg` types (e.g., `PhaseResultMsg`, `PermissionRequestMsg`)

**Critical Rule:** Never mutate `AppState` from a goroutine. Use `p.Send(msg)` or return `tea.Cmd` from `Update()`.

**Provider Layer:**
- Interface: `provider.LLMProvider` in `internal/integrations/provider/`
- Three implementations: OpenRouter, Zen, Nvidia (dynamic model discovery)
- Registry: `provider.NewLazyRegistry()` — lazy initialization on first LLM call

**Tools:**
- Interface: `types.Tool` in `internal/core/types/types.go`
- 18 built-in tools in `internal/tools/` registered in `defaults.go`
- Dispatcher: `internal/tools/dispatcher.go` — permissions, rate limiting, concurrency

**Workflow Engine:**
- Seven phases: Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship
- Engine: `internal/engine/workflow/engine.go`
- Phase transitions explicit via `engine.Transition()`

## Anti-Patterns

### Mutating State from Goroutines

**What happens:** Direct field assignment on `AppState` from background goroutine
**Why it's wrong:** Bubble Tea is single-threaded; concurrent mutation causes data races and UI corruption
**Do this instead:** Send messages via `tea.Cmd` or `Program.Send()` — all mutations in `Update()`
**Reference:** `cmd/m31a/main.go:634-676` (signal handler sends `tea.QuitMsg` via `p.Send()`)

### Using `fmt.Errorf` Directly for Wrapping

**What happens:** `return fmt.Errorf("failed: %w", err)` scattered in codebase
**Why it's wrong:** Inconsistent wrapping, missing context, harder to grep
**Do this instead:** Use `errors.Wrap(err, "context")` or `errors.Wrapf(err, "context: %s", detail)` from `internal/core/errors/errors.go`

### Hardcoding Model Names

**What happens:** String literals like `"gpt-4"` in provider logic
**Why it's wrong:** Models discovered dynamically from provider APIs; hardcoded names break when providers update catalogs
**Do this instead:** Use `provider.FetchModels(ctx)` and `provider.GetModel(id)`
**Reference:** `cmd/m31a/main.go:197-204` (auto-detects first available model)

### Circular Imports

**What happens:** `internal/tools` imports `internal/core/types`, which imports `internal/tools` via `Tool` interface
**Why it's wrong:** Breaks compilation, violates layering
**Do this instead:** Keep `Tool` interface in `internal/core/types/types.go` — no imports back to tools

## Cross-Cutting Concerns

**Validation:**
- Input validation at tool boundaries (e.g., `dispatcher.go:245-247` max 1000 params)
- Path traversal protection in file tools (`fileops/edit.go`)
- SSRF protection: `ErrPrivateIPBlocked` in `internal/tools/websearch.go`

**Authentication:**
- API keys resolved from OS keychain at provider registration time
- Lazy provider registry defers key resolution until first LLM call
- Never log keys (`slog` attributes filtered)

**Observability:**
- pprof server on `localhost:6060` when `M31A_DEBUG=1` or `--debug`
- Metrics collector (`internal/integrations/metrics/collector.go`) records tool calls, duration, success/fail
- Emitter drop counter tracks channel saturation

---

*Convention analysis: 2026-08-23*