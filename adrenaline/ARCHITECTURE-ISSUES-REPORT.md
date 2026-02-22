# Architecture Issues Report — M31A

**Date:** 2026-06-11
**Scope:** Full codebase deep study — `cmd/`, `internal/`, `pkg/`
**Method:** Systematic read of every architecturally significant file across all layers

---

## Executive Summary

M31A has a solid foundation: clean module layout, good separation of `internal` vs `pkg`, a well-defined six-phase workflow, and defensive coding practices (atomic writes, size limits, context guards). However, several architectural issues have accumulated that will impede scaling, testability, and maintainability. This report catalogs **20 distinct issues** grouped by severity and category, with concrete code references and remediation guidance.

---

## CRITICAL — Structural Issues

### C-1: God Object — `AppState` in `internal/tui/`

**Location:** `internal/tui/app_state.go:49-180`

`AppState` carries **50+ fields** including 26 sub-model pointers, workflow state, permission modal state, session state, provider/model state, theme, config, toast management, command registry, key registry, sidebar state, confirmation dialogs, health tracking, transition overlays, stream cancellation, and more.

**Impact:**
- `Update()` (`app_update.go`) is a **650-line switch** with **40+ message type cases**
- `handleWindowResize()` manually resizes each of **24 sub-models** (`app_update.go:673-800`)
- `applyTheme()` manually propagates theme to **22 sub-models** (`app_update.go:1377-1465`)
- `routeKeyMsg()` and the `default:` block in `Update()` each duplicate the **same 20+ screen routing switch** (`app_update.go:802-1037` and `470-642`)

This violates the Single Responsibility Principle and makes the TUI extremely brittle — adding a new screen requires touching 5+ switch statements.

**Remediation:**
- Introduce a `ScreenManager` that owns sub-model lifecycle, resize propagation, theme propagation, and key routing
- Extract permission modal state into a `PermissionController`
- Extract workflow state into a `WorkflowController`
- Use a registry pattern for screens instead of hard-coded switch cases

---

### C-2: Massive Code Duplication Between Provider Clients

**Location:** `internal/provider/openrouter/client.go` vs `internal/provider/zen/client.go`

The OpenRouter and Zen clients share **~80% identical code**:

| Method | OpenRouter Lines | Zen Lines | Identical? |
|--------|-----------------|-----------|------------|
| `New()` | 47-88 | 45-82 | Structurally identical (HTTP client setup, cache init, defaults) |
| `Name()` | 90-92 | 84-86 | Identical pattern |
| `APIKey()` | 94-99 | 88-93 | Byte-for-byte identical |
| `FetchModels()` | 121-167 | 107-153 | Same cache-check → refresh → stale-fallback pattern |
| `ChatCompletionStream()` | 169-191 | 155-202 | Same body-build → marshal → request → error-handle → SSE flow |
| `makeIterator()` | 258-285 | 204-224 | Identical SSE wrapping |
| `EstimateCost()` | 287-289 | 226-228 | Identical delegation |
| `HealthCheck()` | 291-320 | 230-259 | Same latency-threshold pattern |
| `GetModel()` | 322-324 | 261-263 | Identical delegation |
| `CachedModels()` | 327-329 | 266-268 | Identical delegation |

Each client is ~330 lines; ~250 lines are duplicated. The `common.go` helpers (`SetCommonHeaders`, `BuildChatBody`, `EstimateCost`, `GetModel`, `CachedModels`, `StaleFallback`) partially address this but don't eliminate struct-level duplication.

**Remediation:**
- Create a `baseClient` struct in `internal/provider/` that embeds common fields (`apiKey`, `baseURL`, `httpClient`, `cache`, health thresholds) and implements shared methods
- Each provider embeds `baseClient` and overrides only `FetchModels()` model-mapping and provider-specific error handling
- Alternatively, use a strategy pattern with provider-specific `ModelMapper` and `ErrorMapper` interfaces

---

### C-3: Workflow Engine Leaks Bubble Tea Framework Types

**Location:** `internal/workflow/engine.go:23`, `internal/workflow/engine_messages.go:8`

```go
import tea "github.com/charmbracelet/bubbletea"

type MsgEmitter interface {
    Emit(msg tea.Msg)
}
```

The workflow engine — the core business logic layer — directly imports `bubbletea` and uses `tea.Msg` as its event type. This means:
- The workflow engine cannot be tested or used outside a Bubble Tea context
- Every workflow message type (20+ types in `engine_messages.go`) is a `tea.Msg`
- The engine's `emit()` sends framework-specific types through its callback

**Remediation:**
- Define a domain-specific `Event` interface in `internal/types/` or `internal/workflow/`
- Have the TUI layer adapt domain events to `tea.Msg` via a thin adapter
- This decouples the workflow from the UI framework

---

### C-4: Workflow Engine Has Too Many Responsibilities

**Location:** `internal/workflow/engine.go:63-93`

The `Engine` struct directly manages:
1. Session ID and working directory tracking
2. Backup directory management
3. Planning directory path computation
4. Provider and model management (including per-phase model overrides)
5. Configuration access
6. Git operations
7. Tool dispatch
8. Token estimation and context window guards
9. Session manager interaction
10. Prompt loading and composition
11. LLM streaming (both buffered and streaming modes)
12. Self-healing logic
13. Checkpoint management
14. Discuss Q&A state
15. Plan refinement state (version tracking, feedback storage)
16. Budget tracking (cumulative cost)
17. Event emission to TUI
18. Tool definition building

`NewEngine()` takes **10 parameters** (`engine.go:154-155`). The execute phase (`execute.go`) adds git commit logic, tool call dispatch loops, and multi-round heal-then-retry cycles.

**Remediation:**
- Extract `PromptManager` for prompt loading/composition
- Extract `LLMClient` wrapper for streaming and context checks
- Extract `HealManager` for self-heal logic
- Extract `BudgetTracker` for cost tracking
- Use an `EngineOptions` struct instead of 10 positional parameters

---

## HIGH — Design Issues

### H-1: Mutable Global State for Version

**Location:** `internal/provider/openrouter/client.go:20`, `internal/provider/zen/client.go:20`

```go
var Version = "dev"  // openrouter
var Version = "dev"  // zen
```

And `internal/tools/interface.go:13`:
```go
var permissionRequestID atomic.Int64
```

`main.go:112-114` sets these globals directly:
```go
openrouter.Version = Version
zen.Version = Version
tools.SetVersion(Version)
```

**Impact:** Global mutable state is set by `main` and read by provider/tool code. This creates hidden dependencies, makes testing fragile, and prevents running multiple instances.

**Remediation:** Pass version through constructor options or context values.

---

### H-2: Duplicate Message Type Definitions

**Location:** `internal/tui/types.go` vs `internal/workflow/engine_messages.go`

Three message types are defined in both packages:

| Type | TUI (`types.go`) | Workflow (`engine_messages.go`) |
|------|-------------------|-------------------------------|
| `PlanApproveMsg` | Line 240 | Line 113 |
| `PlanRefineMsg` | Line 243 | Line 116 |
| `DemonstrationReadyMsg` | Line 248 | Line 121 |

The TUI's `app_update.go` handles both `PlanApproveMsg` (from TUI) and `workflow.PhaseTransitionStartMsg` (from workflow), creating confusion about which package owns a message type.

**Remediation:** Define workflow messages in `internal/workflow/` only. The TUI should reference them directly or through a mapping layer.

---

### H-3: No Session Schema Versioning

**Location:** `pkg/session/session.go`, `pkg/session/manager.go`

Sessions are serialized directly via `json.Marshal(session)` with no version field:

```go
data, err := json.Marshal(session)  // manager.go:166
```

**Impact:** Adding or removing fields from `Session` struct breaks backward compatibility with existing session files. There's no migration path — old sessions silently lose data or fail to load.

**Remediation:**
- Add a `SchemaVersion int` field to `Session`
- Create a `migrateSession(data []byte, version int) (*Session, error)` function
- On load, check version and apply migrations sequentially

---

### H-4: Config Merging Uses Reflection — Fragile and Non-Type-Safe

**Location:** `internal/config/loader.go:196-287`

```go
func mergeStructs(base, overlay reflect.Value, defined map[string]bool, prefix string) {
```

The config merge uses `reflect.ValueOf`, manual kind-switching, and a hand-written `toTOMLKey()` function for PascalCase → snake_case conversion. This is:
- Fragile: new field types require adding switch cases
- Non-obvious: the `defined` map tracks TOML keys to handle bool false vs unset
- Error-prone: `toTOMLKey()` doesn't handle all edge cases (e.g., consecutive capitals like `HTTPSProxy`)

**Remediation:** Use a typed merge approach, or leverage the TOML library's `MetaData` to determine which fields were explicitly set. Consider a library like `github.com/imdario/mergo`.

---

### H-5: Dispatcher Mixes Permission Logic with Tool Execution

**Location:** `internal/tools/dispatcher.go:121-231`

The `Execute()` method handles rate limiting, tool lookup, JSON parsing, input normalization, permission checking (with 5 branching paths), interactive/non-interactive mode detection, and tool execution — all in one 110-line method.

The `Dispatcher` struct carries:
- Tool registry
- Permission rules and agents
- Permission request/response channels
- Question request/response channels
- Rate limiter (token bucket goroutine)
- Pending response routing (sync.Map)

**Remediation:**
- Extract `PermissionEvaluator` interface for permission checking
- Extract `RateLimiter` as a standalone component
- Make `Execute()` a thin orchestrator that delegates to focused components

---

### H-6: TUI Screen Lifecycle Management Is Fully Manual

**Location:** `internal/tui/app_update.go:1146-1281` (`ensureSubModel`), `app_update.go:673-800` (`handleWindowResize`)

Adding a new screen requires:
1. Add enum constant in `types.go`
2. Add `Label()` case in `types.go`
3. Add model field in `app_state.go`
4. Add `case` to `Update()` default block
5. Add `case` to `routeKeyMsg()`
6. Add `case` to `ensureSubModel()`
7. Add resize logic in `handleWindowResize()`
8. Add theme propagation in `applyTheme()`

Missing any of these 8 steps causes silent bugs (nil pointer, unresponsive screen, wrong size).

**Remediation:** Define a `ScreenModel` interface:
```go
type ScreenModel interface {
    Init() tea.Cmd
    Update(tea.Msg) (ScreenModel, tea.Cmd)
    View() string
    SetDimensions(w, h int)
    SetTheme(t theme.Theme)
}
```
Use a `map[Screen]ScreenModel` registry with lazy initialization.

---

### H-7: Session Manager Is a Monolith

**Location:** `pkg/session/manager.go` (806 lines)

The `Manager` struct handles:
1. Session CRUD (create, load, save, delete)
2. Session listing with caching
3. Session archiving
4. Session forking
5. Sibling/children navigation
6. Recent models management
7. Session cleanup by age
8. Markdown/JSON export
9. Session renaming/filtering
10. Checkpoint management (via separate file)
11. Task persistence
12. Plan persistence
13. Project state persistence
14. State file management

This is at least 5 distinct responsibilities packed into one struct with no interface.

**Remediation:**
- Split into `SessionRepository` (CRUD), `SessionCache` (listing), `SessionExporter`, `ModelHistory`
- Define interfaces for each concern

---

## MEDIUM — Code Quality Issues

### M-1: `main.go` Is a 235-Line Manual Wiring Function

**Location:** `cmd/m31a/main.go:39-235`

The `run()` function manually creates and connects 15+ components:
- Logger → Config → Keychain → API key resolution → Provider registry → Session manager → Working directory → Tool dispatcher → Git client → Ledger → Rollback → AutoDream → Theme → TUI App → Signal handler

No dependency injection, no wiring layer. Testing individual initialization paths requires running the full `run()` function.

**Remediation:** Extract an `AppBuilder` or use a simple DI pattern that separates component construction from wiring.

---

### M-2: `StreamIterator` Is a Pair of Closures

**Location:** `internal/types/types.go:179-182`

```go
type StreamIterator struct {
    Next  func() (*StreamChunk, error)
    Close func() error
}
```

This closure-based iterator:
- Cannot be inspected for state (is it exhausted? is it the first chunk?)
- Cannot be cancelled independently of the underlying HTTP response
- Has no way to report progress metadata without modifying the struct

**Remediation:** Define a proper `StreamIterator` interface with `Next()`, `Close()`, and `Done() bool` methods. Consider adding `Context()` for cancellation propagation.

---

### M-3: Error Handling Inconsistency

Multiple patterns exist for similar operations:

- **Silent swallow with log:** `manager.go:83` — `e.sessionMgr.SaveState()` failures are logged as warnings but execution continues
- **Return error:** `engine.go:259` — `Transition()` returns errors for checkpoint failures
- **Mixed:** `execute.go:61-63` — pre-task checkpoint failure is logged but task execution continues; post-group checkpoint failure is also just logged
- **Non-atomic write:** `manager.go:782` — `ExportSessionMarkdown` uses `os.WriteFile` directly, while all other writes use `atomicWrite`

This inconsistency makes it hard to reason about failure modes.

**Remediation:** Establish a clear error handling policy: which operations are best-effort (log and continue) vs. critical (return error). Document the policy and apply consistently.

---

### M-4: `UIConfig` Has 40+ Fields Mixing Unrelated Concerns

**Location:** `internal/config/types.go:66-137`

`UIConfig` contains fields for:
- Theme and colors (`AccentColor`, `CustomBackground`, `BorderStyle`)
- Typography (`BoldHeaders`, `ItalicThinking`, `TabWidth`)
- Layout (`SidebarPosition`, `SidebarAutoShow`, `CardPadding`, `ZenModeKey`)
- Animation (`AnimationSpeed`, `SpinnerStyle`, `TransitionStyle`, `BreathingEffects`, `LogoAnimation`)
- Status bar (`StatusBarStyle`, `StatusBarPosition`, `ShowSpinnerInStatus`)
- Tool cards (`ToolCardStyle`, `ToolOutputMaxLines`, `SyntaxHighlight`)
- Toasts (`ToastPosition`, `ToastDurationSecs`, `ToastMaxVisible`)
- Core settings (`Theme`, `CompactMode`, `MaxIterations`, `LeaderKey`, etc.)

**Remediation:** Split into nested structs: `ThemeConfig`, `LayoutConfig`, `AnimationConfig`, `StatusBarConfig`, etc.

---

### M-5: Tool Interface Split Across Packages

**Location:** `internal/types/types.go:113-118` and `internal/tools/interface.go:46-48`

```go
// types package
type Tool interface {
    Name() string
    Description() string
    RiskLevel() RiskLevel
    Execute(ctx context.Context, input ToolInput) (ToolResult, error)
}

// tools package
type PermissionGate interface {
    RequestPermission(ctx context.Context, req PermissionRequest) (PermissionResponse, error)
}
```

The `Tool` interface lives in `internal/types` while `PermissionGate`, `PermissionRequest`, and `PermissionResponse` live in `internal/tools`. Tools need both packages to be fully described. The `SchemaProvider` optional interface is in `types` too.

**Remediation:** Consolidate tool-related interfaces into a single package (likely `internal/tools/`) and have `types` reference it, or move all to `types`.

---

### M-6: `ConfigWatchInterval` Polling Instead of File System Events

**Location:** `internal/config/loader.go:642-671`

```go
func WatchConfig(ctx context.Context, path string, ch chan<- ConfigReloadMsg) {
    ticker := time.NewTicker(types.ConfigWatchInterval) // 5 seconds
```

Config file watching uses polling (every 5 seconds via `os.Stat`) instead of file system notification (e.g., `fsnotify`). This wastes I/O and has up to 5-second latency.

**Remediation:** Use `fsnotify` or similar for event-driven file watching, with polling as a fallback.

---

### M-7: No Event Bus Between Layers

The architecture has no unified event system. Events flow through:
- `MsgEmitter.Emit(tea.Msg)` — workflow → TUI (via channel)
- `permListenerCmd` — tool permission requests (via channel)
- `questionListenerCmd` — tool question requests (via channel)
- `drainEmitterCmd` — TUI reads from emitter channel
- `channelEmitter{ch: make(chan tea.Msg, 128)}` — bridging layer

Each event path is ad-hoc with its own channel, goroutine, and message type. Adding new cross-layer events requires creating new channels and listener goroutines.

**Remediation:** Introduce a typed `EventBus` that the workflow, tools, and TUI subscribe to. This simplifies adding new event types and makes event flow testable.

---

## LOW — Minor Issues

### L-1: `normalizeToolName()` Is a 30-Line Switch Statement

**Location:** `internal/workflow/engine_parse.go:458-488`

Maps ~30 LLM-generated tool name variations to canonical names. This is a maintenance burden — every new tool requires adding all likely aliases.

**Remediation:** Use a `map[string]string` lookup table, or have each tool declare its own aliases.

---

### L-2: `Registry.List()` Decorates Output With " (active)"

**Location:** `internal/provider/registry.go:92-106`

```go
func (r *Registry) List() []string {
    // ...
    if name == r.active {
        names[i] = name + " (active)"
    }
}
```

Mixing presentation logic into a data-access method. Callers that need raw names must use `ListAll()` instead.

**Remediation:** `List()` should return raw names. Let the presentation layer (TUI) add decoration.

---

### L-3: `parseQuestions()` Has Three Fallback Strategies

**Location:** `internal/workflow/engine_parse.go:232-280`

Three cascading regex patterns for extracting questions from LLM output. The third fallback (`lines containing ?`) could match unrelated content. The function silently caps at 4 questions with no explanation.

**Remediation:** Document the cap reason. Consider structured output from the LLM instead of regex parsing.

---

### L-4: `skipDirsCache` Is Computed at `init()` Time

**Location:** `internal/types/constants.go:119-125`

```go
var skipDirsCache = func() map[string]bool { ... }()
```

This runs at package init time. If `SkipDirs` were ever made configurable (it's referenced by `ToolsConfig.SkipDirs`), the cache would be stale.

**Remediation:** Compute lazily on first access, or rebuild when config changes.

---

### L-5: Inconsistent Use of `io.ReadAll` vs `io.LimitReader`

While most response body reads use `io.LimitReader` (good), `HealthCheck` uses `io.Copy(io.Discard, io.LimitReader(...))` which is correct but inconsistent with the `bodyBytes, _ := io.ReadAll(io.LimitReader(...))` pattern used in `ChatCompletionStream`.

**Remediation:** Create a helper `readBodyLimited(resp, maxBytes)` and use it consistently.

---

## Dependency Graph Concerns

### Import Direction Violations

```
internal/workflow → github.com/charmbracelet/bubbletea  (C-3: business logic depends on UI framework)
internal/tui      → internal/workflow                    (expected)
internal/tui      → pkg/arbitrage                        (TUI depends on pkg — acceptable)
cmd/m31a          → internal/tui                         (expected)
cmd/m31a          → internal/provider/openrouter         (main directly constructs provider implementations)
cmd/m31a          → internal/provider/zen                (same)
```

The `cmd/m31a/main.go` directly constructs `openrouter.Client` and `zen.Client` concrete types. This couples the entry point to specific implementations rather than going through a factory or registry pattern.

### Circular Dependency Risk

Currently no circular imports exist, but the architecture is fragile:
- `internal/types` is imported by almost every package
- `internal/provider` is imported by `internal/workflow` and `internal/tui`
- `internal/tools` is imported by `internal/workflow` and `internal/tui`
- Adding a reference from `internal/types` to any `internal/*` package would create a cycle

---

## Positive Architectural Patterns Worth Preserving

1. **Phase transition guard** (`engine.go:243-251`) — explicit state machine for workflow phases
2. **Atomic file writes** (`fileutil/atomic.go`) — crash-safe persistence throughout
3. **Singleflight for cache refresh** (`cache.go:46-63`) — prevents thundering herd on model cache
4. **Token bucket rate limiter** (`dispatcher.go:56-73`) — protects against LLM-generated tool call floods
5. **Context window preflight check** (`engine.go:405-425`) — prevents OOM from oversized contexts
6. **Compile-time interface checks** (`var _ provider.LLMProvider = (*Client)(nil)`) — catches interface drift
7. **Stale cache fallback** — providers gracefully degrade when refresh fails
8. **Session cleanup on startup** — prevents unbounded disk accumulation
9. **API key masking** in error messages (`common.go:139-149`)
10. **Size-limited file reads** throughout session management

---

## Recommended Priority Order

| Priority | Issue | Effort | Impact |
|----------|-------|--------|--------|
| 1 | C-1: God Object AppState | High | Very High |
| 2 | C-2: Provider code duplication | Medium | High |
| 3 | C-3: Workflow leaks Bubble Tea | Medium | High |
| 4 | C-4: Engine too many responsibilities | High | High |
| 5 | H-1: Mutable global Version | Low | Medium |
| 6 | H-2: Duplicate message types | Low | Medium |
| 7 | H-3: No session schema versioning | Low | High (data loss risk) |
| 8 | H-6: Manual screen lifecycle | Medium | High |
| 9 | H-5: Dispatcher coupling | Medium | Medium |
| 10 | H-7: Session manager monolith | High | Medium |

---

## Appendix: File Reference Index

| File | Lines Read | Key Issues Found |
|------|-----------|-----------------|
| `cmd/m31a/main.go` | 235 | M-1: Manual wiring |
| `cmd/m31a/usage.go` | 59 | — |
| `internal/provider/interface.go` | 33 | — |
| `internal/provider/openrouter/client.go` | 330 | C-2: Duplication |
| `internal/provider/zen/client.go` | 269 | C-2: Duplication |
| `internal/provider/common.go` | 201 | — |
| `internal/provider/cache.go` | 129 | — |
| `internal/provider/registry.go` | 126 | L-2: List() decoration |
| `internal/tools/interface.go` | 48 | H-1: Global state |
| `internal/tools/dispatcher.go` | 302 | H-5: Mixed concerns |
| `internal/types/types.go` | 196 | M-5: Split interfaces |
| `internal/types/constants.go` | 131 | L-4: Init-time cache |
| `internal/workflow/engine.go` | 620 | C-3, C-4 |
| `internal/workflow/execute.go` | 447 | C-4: Too many responsibilities |
| `internal/workflow/engine_messages.go` | 154 | H-2: Duplicate types |
| `internal/workflow/engine_parse.go` | 613 | L-1, L-3: Parsing fragility |
| `internal/tui/app_state.go` | 321 | C-1: God Object |
| `internal/tui/app_update.go` | 1704 | C-1: Massive Update(), H-6 |
| `internal/tui/app.go` | 384 | C-1 |
| `internal/tui/types.go` | 358 | H-2: Duplicate types |
| `internal/tui/constants.go` | 16 | — |
| `internal/config/loader.go` | 727 | H-4: Reflection merge, M-6 |
| `internal/config/types.go` | 238 | M-4: UIConfig bloat |
| `internal/errors/errors.go` | 122 | — |
| `pkg/session/session.go` | 83 | H-3: No schema versioning |
| `pkg/session/manager.go` | 806 | H-7: Monolith |
| `go.mod` | 54 | — |
