# Coding Conventions

**Analysis Date:** 2026-06-02

## Language and Build

- **Language:** Go 1.22+ (`go.mod` declares `go 1.22`).
- **Module path:** `github.com/eshanized/M31A`.
- **Build:** `CGO_ENABLED=0` is mandatory. The binary must be statically linked (no CGO).
  - Enforced in `Makefile` (line 9: `CGO_ENABLED=0 go build`).
  - Documented in `AGENTS.md` as a hard rule.
- **Linter:** `golangci-lint` with config at `.golangci.yml` enabling `govet`, `staticcheck`, `errcheck`, `ineffassign`, `unused`, `gosimple`.
- **vet settings:** `govet.check-shadowing: true` — variable shadowing is reported.
- **errcheck:** `check-type-assertions: false` and `check-blank: false` (relaxed for `t, _ := ...` style). Tests are excluded from errcheck/unused.

## File Organization

**Top-level files and directories** (only the conventional ones for new code — see `AGENTS.md` for the full package layout):

- `cmd/m31a/main.go` — binary entry point only; flag parsing, logger init, provider registry wiring, `tea.NewProgram`. **No business logic.**
- `internal/<pkg>/` — implementation that must not be imported by external Go code.
- `pkg/<pkg>/` — public packages (e.g. `pkg/taskrunner`, `pkg/ledger`, `pkg/keychain`).
- `internal/types/` — leaf package containing all shared types and constants. **Zero internal imports.**
- `internal/errors/` — sentinel errors only. **Zero internal imports.**

**Within a package:**
- One primary type per file (e.g. `internal/tools/bash.go` defines `Bash`, `internal/provider/zen/client.go` defines `Client`).
- Interfaces separated into a file named `interface.go` (e.g. `internal/provider/interface.go`, `internal/tools/interface.go`).
- `constants.go` lives in `internal/types/` and is the single source of numeric/timeout constants.
- Tests live next to source as `<file>_test.go` (co-located, same package — `_test` package, not `_test` package suffix).

## Naming Patterns

**Files:**
- Lowercase, single word preferred (`bash.go`, `cache.go`, `engine.go`).
- Multi-word: snake-free — use the type name (`filewrite.go`, `openrouter/` is the only sub-package following directory naming).
- Test files: `<source>_test.go`.

**Types (MixedCaps / PascalCase):**
- Structs: `Bash`, `FileRead`, `Dispatcher`, `Engine`, `AppState`, `ModelInfo`, `RiskLevel`.
- Interface contracts: noun describing capability — `LLMProvider`, `Tool`, `Keychain`, `MsgEmitter`.
- Generic enum types are string aliases: `type RiskLevel string`, `type WorkflowPhase string`, `type TaskStatus string`. Values are the lowercased string itself (`RiskSafe = "safe"`, `PhaseInitialize = "initialize"`, `StatusRunning = "running"`).

**Functions / methods:**
- Constructors: `New<Type>(...)`. Always return a pointer (`*Bash`, `*Dispatcher`, `*Engine`) when the type holds mutable state, or `(value, error)` when construction can fail (`New(apiKey, opts)` in provider clients).
- Exported boolean / getter accessors are noun-style: `IsExpired()`, `IsStale()`, `IsCollapsed()`, `IsResponded()`, `IsPaused()`.
- Sentinels return `bool` and `*T`: `Get(id string) (*ModelInfo, error)`, `GetTool(name string) (types.Tool, bool)`.
- Stream / message producer functions are `Next func() (*T, error)` closures on a struct (see `StreamIterator` in `internal/types/types.go`).

**Variables and parameters:**
- `camelCase` for locals, fields, parameters.
- Receiver names: **single short letter** tied to the type — `m *AppState`, `b *Bash`, `c *Client`, `d *Dispatcher`, `e *Engine`, `r *Runner`, `t *TokenEstimator`/`*types.Tool`. Avoid `self`/`this`.
- **Sentinel value names** are repeated constants, not the package name: a method on `*Bash` uses `b`, not `bash`. (Exception: in test files, `b` is also used for plain `*Bash` instances.)
- Error variables: `Err*` prefix (`ErrProviderUnreachable`, `ErrRateLimited`, `ErrInvalidKey`, `ErrCircularDependency`, `ErrPermissionDenied`).
- Slog logger fields: lowercase keys (`"version"`, `"error"`, `"path"`, `"phase"`).

**Constants:**
- PascalCase, not SCREAMING_SNAKE_CASE: `ModelCacheTTL`, `HTTPDialTimeout`, `BashOutputLimit`, `DefaultContextLength`, `MaxPlanRetries`. Defined exclusively in `internal/types/constants.go`.
- Underscored numeric literals: `5 * 1024 * 1024`, `30_000`, `128_000`, `1_000_000` (price math in `internal/provider/zen/client.go`).

**Package names:**
- Lowercase, single short word: `tools`, `session`, `ledger`, `arbitrage`, `autodream`, `rollback`, `taskrunner`, `keychain`, `provider`, `workflow`, `tokens`, `log`, `git`, `types`, `errors`, `tui`, `config`.
- Sub-packages by provider name: `internal/provider/openrouter/`, `internal/provider/zen/`.

## Code Style

**Formatting:**
- `gofmt`-clean is required (CONTRIBUTING.md).
- `goimports` is recommended for import sorting.
- Imports are grouped in three blocks separated by blank lines:
  1. Standard library
  2. Third-party (`github.com/...`)
  3. Project-internal (`github.com/eshanized/M31A/...`)

  Example from `cmd/m31a/main.go:3-20`:
  ```go
  import (
      "flag"
      "fmt"
      "os"
      // ...
      tea "github.com/charmbracelet/bubbletea"
      "github.com/eshanized/M31A/internal/config"
      "github.com/eshanized/M31A/internal/log"
      // ...
  )
  ```

- Named import for the local errors package to avoid collision with stdlib `errors`:
  ```go
  m31errors "github.com/eshanized/M31A/internal/errors"
  ```
  Used throughout `internal/tools/`, `internal/provider/`, `internal/workflow/`.

**Comments and documentation:**
- Every exported package, type, function, method, and constant has a doc comment starting with the identifier name.
- Inline comments explain *why*, not *what*. Examples: `// Use error channel instead of shared variable to avoid data race` (`internal/tools/bash.go:111`), `// Signal forwarding on cancellation` (`internal/tools/bash.go:90`).
- `//go:embed prompts/*.md` is used for embedded prompt templates (`internal/workflow/engine.go:44`).
- **No emojis** anywhere in code or documentation (per `CONTRIBUTING.md`).
- No `// nolint:...` comments in production code without justification; the only one observed is `m.sessionManager.AddRecentModel(model.ID) //nolint:errcheck` (`internal/tui/app.go:1609`) for an intentionally fire-and-forget call.

**JSON tags:**
- All serialized structs use `snake_case` JSON tags: `json:"prompt_tokens"`, `json:"input_per_m_token"`, `json:"skip_for_llm,omitempty"`, `json:"children_ids,omitempty"`.

**Receiver consistency:**
- Pointer receivers are used whenever the method mutates state, holds a sync primitive, or returns data that should be live (`mu sync.RWMutex`).
- Value receivers are avoided on structs that hold a `sync.Mutex` or other non-copyable fields.

**Branching and early returns:**
- Guard clauses with `if err != nil` placed immediately after the call site, before any other logic.
- Sentinel error comparison uses `errors.Is(err, m31errors.ErrXxx)` — never type assertions on sentinel errors.

## Error Handling

**Sentinel errors** are defined in `internal/errors/errors.go` and are the single source of truth:
- `ErrProviderUnreachable`, `ErrRateLimited`, `ErrInvalidKey`, `ErrContextExceeded`
- `ErrModelNotFound`, `ErrSessionCorrupted`
- `ErrNoBinaryContent`, `ErrFileTooLarge`
- `ErrCircularDependency`, `ErrPermissionDenied`
- `ErrToolExecution`, `ErrTaskFailed`, `ErrPhaseTransition`, `ErrCheckpointNotFound`
- Plus `pkg/keychain/errors.go`: `ErrKeyNotFound`, `ErrKeychainUnavailable`.

**Wrapping pattern:**
- `fmt.Errorf("... %w ...", err)` for wrapping with `%w`.
- `fmt.Errorf("%w: ...", m31errors.ErrXxx)` for sentinel-prefixed wraps. Examples:
  - `fmt.Errorf("%w: unknown tool: %s", m31errors.ErrToolExecution, call.Name)` (`internal/tools/dispatcher.go:79`)
  - `fmt.Errorf("tool %s: invalid input JSON: %w", call.Name, err)` (`internal/tools/dispatcher.go:84`)
  - `fmt.Errorf("git init: %w", err)` (`internal/workflow/initialize.go:30`)

**Tool execution errors:**
- Tools return `types.ToolResult{...Error: "..."}, nil` for **expected failures** that should reach the LLM as a corrective signal (e.g. non-zero exit code, timeout, cancellation). See `internal/tools/bash.go:177-200` — `waitErr != nil` produces a populated `ToolResult` with `Error: "exit code N"` and a `nil` Go error.
- Tools return `(types.ToolResult{}, err)` for **caller / programmer errors** that should bubble up: missing parameters, type mismatches, permission denied by rule. See `internal/tools/bash.go:41-46`, `internal/tools/fileread.go:42-46`.

**HTTP error normalization** in provider clients (`internal/provider/zen/client.go:203-223`):
- `429` → `m31errors.ErrRateLimited`
- `401` → check body for `CreditsError`/`payment`/`billing` keywords first; otherwise `m31errors.ErrInvalidKey`
- `503` → `m31errors.ErrProviderUnreachable`
- Body containing `context_length`/`context` → `m31errors.ErrContextExceeded`
- Default → `fmt.Errorf("unexpected status %d: %s", ...)` with body attached

**No panics** in business code except for unrecoverable programmer error on the dispatcher:
- `panic(fmt.Sprintf("tool already registered: %s", name))` (`internal/tools/dispatcher.go:66`) — duplicate registration indicates a coding bug.

## Logging

**Framework:** `log/slog` (stdlib).
- Created in `cmd/m31a/main.go:64` via `log.NewLogger(Version)`. Log output goes to `~/.m31a/m31a.log` only, never stdout/stderr during TUI operation.
- The default `*slog.Logger` is reachable via `slog.Default()` and is stored in `Engine.logger` (`internal/workflow/engine.go:134`).
- Log levels: `Info` for normal flow, `Warn` for non-fatal anomalies (`failed to save checkpoint`, `failed to set git user`), `Debug` for verbose tracing.

**Key/value style:** alternating `key, value` pairs:
```go
e.logger.Info("initialize phase starting", "goal", goal)
e.logger.Info("detected project type", "type", project.ProjectType)
e.logger.Warn("failed to save checkpoint", "error", err)
```

## Concurrency Model

**Single-threaded Bubble Tea rule** (mandated by `AGENTS.md`):
- All state mutations on `*AppState` happen inside `Update()`.
- Goroutines never touch `*AppState` directly; they emit `tea.Cmd` values that return `tea.Msg` to the main loop.
- Cross-goroutine communication: channels (e.g. `app.msgChan = make(chan tea.Msg, 64)` in `internal/tui/app.go:331`).

**Goroutine emitters in the workflow engine:**
- `Engine` holds a `MsgEmitter` interface (`internal/workflow/engine.go:29-31`).
- Concrete impl `channelEmitter` (`internal/tui/app.go:399-410`) sends to a channel with a non-blocking `select` and `default` case to **drop messages rather than block the engine** when the channel is full — paired with a `slog.Warn` to surface the drop.

**Listener cmd pattern** — a `tea.Cmd` that returns the next event from a channel:
```go
func permissionListenerCmd(dispatcher *tools.Dispatcher) tea.Cmd {
    return func() tea.Msg {
        req := <-dispatcher.RequestCh()
        return PermissionRequestMsg{Request: req}
    }
}
```
Used for `permissionListenerCmd`, `questionListenerCmd`, `workflowMsgDrainer`. Re-scheduled by the consumer in `Update()`.

**Sync primitives:**
- `sync.RWMutex` in shared structures: `Dispatcher.mu` (`internal/tools/dispatcher.go:19`).
- `sync.Once` for at-most-once operations: `killOnce` in `internal/tools/bash.go:91` to ensure `SIGINT` is sent at most once before `SIGKILL` escalation.
- `sync/atomic` for hot counters: `Engine.callCounter` (`internal/workflow/engine.go:99`), `limitWriter.written` (`internal/tools/bash.go:212`).
- `io.Pipe` for streaming child-process output (`internal/tools/bash.go:70-71`) to avoid 50kB buffering in pipes.

**Context:**
- `context.Context` is threaded through every long-running call (`Engine.RunPhase`, provider `ChatCompletionStream`, tool `Execute`).
- `context.WithTimeout(ctx, types.BashTimeout)` (`internal/tools/bash.go:58`) — default 30 minutes.
- `context.WithCancel(context.Background())` for the per-phase workflow context (`internal/tui/app.go:335`).
- A previous phase's context is always cancelled before a new one starts (`internal/tui/app.go:324-326`).

## HTTP Client Pattern

In `internal/provider/zen/client.go:73-75` and `internal/provider/openrouter/`:
- `http.Client` with only `DialContext` set to `(net.Dialer{Timeout: 30s}).DialContext`.
- **No `http.Client.Timeout`** — streaming responses are unbounded by design.
- All requests constructed with `http.NewRequestWithContext(ctx, method, url, body)`.
- `User-Agent` set to `M31A/<version>` in `setCommonHeaders`.
- Authorization: `req.Header.Set("Authorization", "Bearer "+apiKey)`.

## Tool Implementation Pattern

Every tool in `internal/tools/` follows the same shape (e.g. `Bash` in `bash.go:16-34`):
1. **Struct** with the workDir / config (e.g. `Bash{workDir string}`, `FileRead{workDir string}`).
2. **Constructor** `New<Type>(workDir, ...)` returning `*Type`.
3. **Three introspection methods** implementing `types.Tool`:
   - `Name() string` — returns the canonical name (`"Bash"`, `"FileRead"`, `"FileWrite"`, `"Glob"`, `"Grep"`, `"Edit"`, `"TodoWrite"`, `"WebFetch"`, `"AskUserQuestion"`).
   - `Description() string` — single-sentence summary used in tool cards and tool definitions.
   - `RiskLevel() types.RiskLevel` — one of `RiskSafe`, `RiskMedium`, `RiskDangerous`, `RiskDestructive`.
4. **`Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error)`** — does the work.

**Param extraction pattern** in `Execute` (repeated in every tool):
```go
raw, ok := input.Params["command"]
if !ok {
    return types.ToolResult{}, fmt.Errorf("missing parameter: command")
}
str, ok := raw.(string)
if !ok {
    return types.ToolResult{}, fmt.Errorf("parameter command must be a string")
}
```
Numeric params use the `float64` JSON-unmarshal convention:
```go
if customRaw, ok := input.Params["timeout"]; ok {
    if customFloat, ok := customRaw.(float64); ok {
        timeoutSec = int(customFloat)
    }
}
```

**Path safety** (FileRead `fileread.go:55-81`, FileWrite `filewrite.go:74-111`, Grep):
- Resolve relative paths against `workDir` first; absolute paths used as-is.
- `filepath.Abs` → `filepath.EvalSymlinks` → `strings.HasPrefix(resolved, workDir+sep)` guard.

**Atomic write** (FileWrite `filewrite.go:144-178`):
- Write content to `.m31a_tmp_<8 random hex bytes>` sibling file.
- `tmpFile.Sync()` then `os.Rename(tmpPath, targetPath)`.
- A `cleanup bool` flag inside a `defer` removes the temp file if any step failed before the rename.

**Binary detection** (FileRead `fileread.go:108-129`, Bash `bash.go:228-242`):
- Read first 512 bytes; presence of any null byte ⇒ binary.
- Return `[binary file, mime-type, N bytes]` or `[binary output, N bytes]`.

## Bubble Tea Pattern

- `AppState` is a single large struct (`internal/tui/app.go:38-95`) with all screen models, dispatcher, registry, session manager, keychain, ledger, etc. as fields.
- One `Update()` per `tea.Model` (e.g. `app.go:445-1384`, ~940 lines).
- One `View()` that switches on `m.screen`.
- Cross-screen navigation via the `screen` enum (`internal/tui/types.go`) and a `prevScreen` field for back-navigation.
- Batch commands for parallel work: `tea.Batch(permissionListenerCmd(m.dispatcher), questionListenerCmd(m.dispatcher), ...)`.
- `tea.Every` for tickers (health checks, permission countdown).
- Permission flow is a channel-driven handshake: dispatcher pushes `PermissionRequest` → TUI listens via `permissionListenerCmd` → user keystroke produces `PermissionResponse` → `ApprovePermission(...)` writes to the dispatcher's response channel.

## Cross-Cutting Concerns

**API key resolution order** (`cmd/m31a/main.go`, `internal/config/loader.go`):
1. Environment variable (`OPENROUTER_API_KEY`, `ZEN_API_KEY`)
2. OS keychain (Linux `secret-service`/`pass`, macOS Keychain, Windows Credential Manager)
3. Config file (`~/.m31a/config.toml` — `api_key` field is **last resort**, never plaintext in any other path)

**Build tags / platform-specific files** (`pkg/keychain/`):
- `keychain.go` — interface, `newFunc` package var, `New()` factory.
- `keychain_linux.go`, `keychain_darwin.go`, `keychain_windows.go` — each with a `//go:build` constraint (implicit via `_linux` suffix), an `init()` that assigns `newFunc = newLinuxKeychain` etc., and a constructor.
- Test stubs: `keychain_test.go` provides an in-memory `mockKeychain` for table-style tests.

**Atomic session file writes** (`pkg/session/`):
- Write to `*.tmp` sibling, then `os.Rename`. (General pattern; not all session code paths in the repo are atomic — see `CONCERNS.md` for gaps.)

**No global state** outside of `slog.Default()` and `promptFS` (an `//go:embed` variable at `internal/workflow/engine.go:45`).

---

*Convention analysis: 2026-06-02*
