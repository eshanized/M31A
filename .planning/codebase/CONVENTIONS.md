# Coding Conventions

**Analysis Date:** 2026-07-23

## Language

**Primary:** Go 1.25.0
- Module: `github.com/eshanized/M31A`
- `CGO_ENABLED=0` mandatory (static binary requirement — hard constraint)
- `go:build` tags used for platform-specific files (e.g., `lock_unix.go`, `lock_windows.go`)

## Package Structure

**Dependency Rule:** `pkg/` must NOT import `internal/`. Enforced by Go module system.

**Core Packages:**
| Package | Purpose | Location |
|---------|---------|----------|
| `types` | Shared type vocabulary across all layers | `internal/core/types/` |
| `errors` | Sentinel errors, error types, user messages | `internal/core/errors/` |
| `config` | TOML config, env overrides, keychain resolution | `internal/core/config/` |
| `tools` | Tool dispatcher, permissions, 18 built-in tools | `internal/tools/` |
| `provider` | LLM provider abstraction (OpenRouter, Zen, NVIDIA) | `internal/integrations/provider/` |
| `workflow` | 7-phase engine (Initialize→Ship) | `internal/engine/workflow/` |
| `session` | Session persistence, workflow state | `internal/engine/session/` |
| `tui` | Bubble Tea TUI (Elm architecture) | `internal/ui/tui/` |
| `keychain` | OS keychain integration | `internal/integrations/keychain/` |
| `git` | Git operations wrapper | `internal/integrations/git/` |

**Entry Point:** `cmd/m31a/main.go` — flag parsing, config load, provider registration, TUI construction.

## Naming Patterns

**Files:**
- Source: `snake_case.go` (e.g., `dispatcher.go`, `filewrite_test.go`, `permissions.go`)
- Test: `<name>_test.go` (co-located with source)
- Build tags: `//go:build linux` / `//go:build windows`

**Packages:**
- Lowercase, singular: `tools`, `config`, `types`, `session`, `workflow`
- Test package: same as source (white-box) or `*_test` for black-box (e.g., `tests/e2e/e2e_test.go` uses `m31a_test`)

**Functions/Methods:**
- Exported: `PascalCase` — `Execute`, `Register`, `DefaultDispatcher`, `NewEngine`
- Unexported: `camelCase` — `testDispatcher`, `checkPermission`, `ensurePermission`
- Constructors: `New<Type>` — `NewDispatcher`, `NewFileWrite`, `NewMockProvider`
- Boolean getters: `Is<X>`, `Has<X>`, `Can<X>` — `IsRepo`, `HasCycle`
- Setters: `Set<X>` — `SetPermission`, `SetCollector`

**Variables/Fields:**
- Exported: `PascalCase` — `Name`, `RiskLevel`, `ToolCallID`
- Unexported: `camelCase` — `workDir_`, `rateTokens`, `activeAgent`
- Constants: `PascalCase` with domain prefix — `RiskSafe`, `PhaseInitialize`, `PolicyAllowSafe`
- Package-level vars: `PascalCase` — `Version`, `Commit`, `DefaultAgentName`

**Types:**
- Interfaces: `PascalCase`, often `er` suffix — `Tool`, `SchemaProvider`, `Keychain`, `LLMProvider`, `MsgEmitter`
- Structs: `PascalCase` — `Dispatcher`, `ToolResult`, `ModelInfo`, `AppState`
- Error types: `*Error` suffix — `ToolError`, `ProviderError`, `ConfigError`

## Code Style

**Formatting:**
- `gofmt` / `goimports` mandatory (`make fmt`)
- Run `goimports -w` on all `*.go` files
- Import groups: stdlib → third-party → project (blank line between groups)

**Linting (`.golangci.yml`):**
```yaml
linters:
  enable:
    - govet        # with shadow check
    - staticcheck
    - errcheck     # disabled in test files
    - ineffassign
    - unused       # disabled in test files
```
- `errcheck`: `check-type-assertions: false`, `check-blank: false`
- Test files excluded from `errcheck` and `unused`

**No emojis** in code or docs (per AGENTS.md).

## Error Handling

**Principles:**
- Return errors, never panic (except truly unrecoverable)
- Wrap with `fmt.Errorf("%w", err)` to preserve chain
- Use sentinel errors for programmable checks (`errors.Is`)
- Use structured error types for context (`errors.As`)

**Sentinel Errors** (`internal/core/errors/errors.go`):
```go
var (
    ErrProviderUnreachable = errors.New("provider unreachable")
    ErrRateLimited         = errors.New("rate limited")
    ErrInvalidKey          = errors.New("invalid API key")
    ErrContextExceeded     = errors.New("context window exceeded")
    ErrPermissionDenied    = errors.New("permission denied")
    // ... 30+ sentinels
)
```

**Structured Error Types:**
```go
// ToolError — tool execution failures with context
type ToolError struct {
    Tool string  // tool name
    Op   string  // operation (e.g., "execute", "parse input")
    Err  error   // underlying error
}
func (e *ToolError) Error() string { ... }
func (e *ToolError) Unwrap() error { return e.Err }

// ProviderError — API provider errors with HTTP context
type ProviderError struct {
    Provider   string
    Model      string
    StatusCode int
    Err        error
}

// ConfigError — config loading/validation errors
type ConfigError struct {
    Key string
    Err error
}
```

**User-Facing Messages** (`errors.UserMessage`):
- Maps sentinel/structured errors → actionable user strings
- Pattern matching for unwrapped errors (connection refused, timeouts, TLS, HTTP codes)
- Fallback: `"An unexpected error occurred — check the logs or try again"`

**Usage Pattern:**
```go
// In tool execution:
result, err := tool.Execute(ctx, input)
if err != nil {
    return types.ToolResult{}, fmt.Errorf("tool %s: %w", call.Name, err)
}

// In main/entry points:
if err := engine.RunPhase(ctx, phase, goal); err != nil {
    logger.Error("phase failed", "phase", phase, "error", err)
    return 1
}
```

## Logging

**Framework:** `log/slog` (stdlib)
- Initialized in `main()` via `log.NewLogger(Version)` → `slog.SetDefault`
- Structured: `logger.Info("msg", "key", val, "key2", val2)`
- Levels: `Debug`, `Info`, `Warn`, `Error`
- Tool execution logged at `Debug` with duration: `slog.Debug("tool executed", "tool", name, "duration_ms", elapsed, "error", err)`

## Architecture Patterns

**TUI (Bubble Tea — Elm Architecture):**
- Single `AppState` struct — all state mutations in `Update(msg tea.Msg) (tea.Model, tea.Cmd)`
- Never mutate `AppState` from goroutine — use `tea.Cmd` / `tea.Msg` via `Program.Send()`
- Signal handler sends `tea.QuitMsg` through program channel (not direct `app.Shutdown()`)
- 20+ screen models composed into `AppState` (lazy-initialized via `ensureSubModel`)

**Workflow Engine:**
- 7 phases: `Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship`
- Each phase builds own context (messages) → streams LLM → parses tool calls → executes via dispatcher
- Phase transitions via `engine.Transition(ctx, from, to)` — validates, persists `STATE.md`

**Provider Layer:**
- Interface: `LLMProvider` (`FetchModels`, `ChatCompletionStream`, `GetModel`, `HealthCheck`, `EstimateCost`)
- Three implementations: OpenRouter, Zen, NVIDIA
- Models discovered dynamically — never hardcode model names
- Capability detection from config (tool support, reasoning, context window)

**Tool Dispatcher:**
- Central registry (`map[string]types.Tool`)
- Permission evaluation: rules → agent defaults → risk level → prompt
- Rate limiting: token bucket (global + per-risk-level)
- Concurrency: semaphore (`MaxConcurrentTools`)
- Output bounding: `exec.OutputStore` prevents context window exhaustion
- Metrics: `metrics.Collector` records call count, success/fail, duration

**Dependency Injection:**
- Interfaces for testability: `workflowEngineInterface`, `MsgEmitter`, `Keychain`
- Constructor injection: `NewEngine`, `NewApp`, `NewManager`
- `internal/types` is the shared vocabulary — `pkg/` and `internal/` both import it

## Configuration

**File:** `~/.m31a/config.toml` (or `M31A_CONFIG` env)
**Format:** TOML (`BurntSushi/toml`)
**Layers (priority):**
1. Default config (`config.DefaultConfig()`)
2. Global config file
3. Project config (`m31a.toml` in workspace ancestry)
4. Environment variables (`M31A_THEME`, `M31A_DEFAULT_MODEL`, `M31A_CONFIG`)
5. API keys: env var → OS keychain → config file

**Validation:** `validateConfig` runs on load — returns `ErrValidation` with field paths.

**Variable Substitution:** `${ENV_VAR}` in config values (unresolved preserved for debugging).

## Concurrency

**Rules:**
- Bubble Tea is strictly single-threaded — all state mutation in `Update()`
- Goroutines for: rate limiters, signal handling, provider streams, subagent manager
- Communication via channels (never shared mutable state)
- `sync.Once` for idempotent shutdown (`Dispatcher.Stop()`)
- `sync.RWMutex` for dispatcher internal maps (`tools`, `permissions`, `batchApprovals`)
- `sync.Map` for per-request channels (`pendingResponses`, `pendingQuestions`)
- `atomic.Int64` for request ID generation, pending permission count

**Rate Limiters (goroutines):**
- Global: `ToolRateLimitPerSec` refill
- Dangerous tools: `DangerousRateLimitPerSec` refill
- Both stopped via `close(rateDone)` in `Stop()`

## Security Patterns

**Path Validation:** All file tools resolve relative to `workDir`, reject absolute/outside paths.

**SSRF Protection:** `WebFetch` blocks private IPs (loopback, RFC1918, link-local) and metadata endpoint `169.254.169.254`.

**Command Injection:** `Bash` tool detects obfuscation (double spaces, tabs, mixed case, var expansion, command substitution).

**Binary Content:** Tools reject NUL bytes (`ErrNoBinaryContent`).

**API Keys:** Never written to disk in plaintext — OS keychain (`pkg/keychain/`) with file fallback.

**File Size Limits:** `types.MaxFileSize` (5MB) enforced in `FileRead`, `Edit`, `FileWrite`.

**Permission Model:** Rule-based (allow/deny/ask) with glob patterns (`doublestar`), agent-scoped defaults, batch approval.

## Comments & Documentation

**Exported functions/types:** Doc comments required (`// FunctionName does X...`)
```go
// NewDispatcher creates a new Dispatcher with a background rate-limiter goroutine.
// The caller MUST call Stop() when the Dispatcher is no longer needed to prevent
// goroutine leaks (e.g., during session restart or app shutdown).
func newDispatcher(cfg *config.PermissionsConfig) *Dispatcher { ... }
```

**Internal comments:** Explain *why*, not *what*
```go
// Tolerate nil or empty Input by treating it as an empty JSON object.
inputBytes := call.Input
if len(inputBytes) == 0 {
    inputBytes = []byte("{}")
}
```

**No emojis** in code or docs.

**Build-time vars** (in `main.go`):
```go
var (
    Version   = "dev"
    Commit    = "unknown"
    Date      = "unknown"
    GoVersion = "unknown"
)
```
Set via `-ldflags` in `Makefile`.

## File Organization

**Internal tool structure (`internal/tools/`):**
```
dispatcher.go          # Core dispatcher, permissions, rate limiting
dispatcher_test.go     # 991 lines — comprehensive tests
permissions.go         # Permission rule evaluation
interface.go           # Tool interface, permission types
defaults.go            # DefaultDispatcher() — registers 18 tools
bash.go, fileread.go,  # Individual tool implementations
filewrite.go, edit.go, # (each has *_test.go)
...
exec/                  # Bash execution, output store
fileops/               # Atomic write, lock (platform-specific)
codeanalysis/          # CodeMap, CodeComplexity
search/                # Glob, Grep
git/                   # Git tool
network/               # WebFetch, WebSearch, HTTPCheck
subagent/              # Parallel subagent support
todo/                  # TodoWrite, TodoRead
```

**TUI structure (`internal/ui/tui/`):**
```
app_state.go           # AppState (260+ fields), NewApp
app_update.go          # Update() — message routing
app_update_*.go        # Per-message handlers (phase, tool, stream, etc.)
app_view.go            # View() — screen rendering
*_model.go             # Screen sub-models (REPL, Plan, Execute, etc.)
components/            # Reusable UI components
theme/                 # Lipgloss theme system
commands/              # Command palette, keybindings
```

---

*Convention analysis: 2026-07-23*