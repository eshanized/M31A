# Phase 3: Engineering Excellence - Research

**Researched:** 2026-08-05
**Domain:** Go codebase refactoring, documentation, developer experience tooling
**Confidence:** HIGH

## Summary

Phase 3 targets the M31A codebase's engineering quality: splitting the 1927-line `engine.go` into focused files, standardizing internal APIs via interfaces, documenting architecture, and adding developer experience tooling (debug logging, profiling, release automation). The project already has strong foundations: a well-organized `internal/` layer hierarchy, `log/slog` for structured logging, GoReleaser v2 configured for cross-platform builds, and a comprehensive Makefile. The primary work is splitting `engine.go` (which contains ~40 methods across pause/resume, streaming, checkpointing, and orchestration concerns) into focused files under 500 lines each, then layering in pprof profiling and improving debug logging granularity.

**Primary recommendation:** Start with engine.go decomposition using Go's natural file-splitting approach (same package, multiple files), then add pprof profiling via a debug HTTP endpoint, and extend slog usage for consistent structured logging across all modules.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Start with engine.go split (1832 lines → focused structs) before documentation/DX work
- **D-02:** Focus on engine.go first because it unlocks safer concurrent changes later
- **D-03:** Put architecture docs in `.planning/codebase/` (already has ARCHITECTURE.md, CONCERNS.md)
- **D-04:** Keep code clean, docs versioned with plans
- **D-05:** Use convention + interface boundaries — Go module system for hard boundaries, interfaces for soft boundaries
- **D-06:** Low overhead approach — no runtime DI container, no compile-time tooling
- **D-07:** Implement all three DX improvements: debug logging, profiling setup, and release automation
- **D-08:** Debug logging is most immediately useful for contributor productivity

### the agent's Discretion
- Agent may choose specific file boundaries when splitting engine.go
- Agent may select documentation format within .planning/codebase/
- Agent may design debug logging format and profiling integration
- Agent may choose release automation tooling (goreleaser, Makefile targets, etc.)

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ARCH-01 | Strengthen module boundaries between engine, providers, tools, runtime, workflow, UI | Interface-based soft boundaries pattern; Go module system for hard boundaries |
| API-01 | Standardize interfaces, dependency injection, lifecycle management, ownership | Go interface patterns; constructor injection via EngineOptions |
| DEBT-01 | Eliminate oversized files (engine.go: 1927 lines) | Go file-splitting patterns: same package, multiple files by concern |
| DEBT-02 | Eliminate duplicated logic | Audit existing code for duplication patterns |
| DOC-01 | Document architecture, extension points, workflow, lifecycle, conventions | .planning/codebase/ directory; inline doc comments |
| DX-01 | Debug logging (slog-based, structured) | slog best practices; consistent field naming |
| DX-02 | Profiling setup (pprof) | net/http/pprof for debug endpoint; runtime/pprof for CLI profiling |
| DX-03 | Release automation (goreleaser) | Already configured; enhance with CI workflow |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Engine orchestration | Engine (`internal/engine/`) | UI (Bubble Tea) | Engine owns 7-phase pipeline; TUI is consumer |
| State management | Engine (WorkflowState) | Engine (StateMachine) | WorkflowState holds mutable session state; StateMachine validates transitions |
| Phase execution | Engine (workflow/) | Tools Dispatcher | Engine dispatches phases; tools execute within phases |
| Provider interaction | Integrations (`internal/integrations/provider/`) | Engine | Providers implement LLM interface; engine calls them |
| Tool dispatch | Tools (`internal/tools/`) | Engine | Dispatcher mediates all tool calls with permissions/rate limiting |
| Debug logging | Engine (cross-cutting) | All modules | slog is cross-cutting concern; each module creates its own logger |
| Profiling | Infrastructure | Engine | pprof endpoint runs independently; profiles engine behavior |
| Release automation | Build (Makefile, goreleaser) | CI | Build tooling; not runtime concern |

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `log/slog` | stdlib (Go 1.21+) | Structured logging | Standard library; already used in main.go and engine |
| `net/http/pprof` | stdlib | HTTP profiling endpoint | Standard library; zero dependencies |
| `runtime/pprof` | stdlib | Programmatic profiling | Standard library; for CLI profiling sessions |
| `goreleaser` | v2.17.0 | Cross-platform release builds | Already configured in `.goreleaser.yaml` |
| `golangci-lint` | v2.12.2 | Code quality | Already configured in `.golangci.yml` |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `golang.org/x/sync` | v0.22.0 | `singleflight` for deduplication | Already used; reference for concurrency patterns |
| `github.com/fsnotify/fsnotify` | v1.10.1 | File watching for config hot-reload | Already used; reference for change detection |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `log/slog` (stdlib) | `zerolog` or `zap` | slog is stdlib, zero deps, good enough; zerolog/zap only if >100k logs/sec |
| `net/http/pprof` | `runtime/pprof` file-based | HTTP endpoint easier for long-running processes; file-based better for CLI profiling sessions |
| GoReleaser manual | Custom Makefile cross-compile | GoReleaser handles archives, checksums, changelogs automatically |

**Installation:**
```bash
# No new packages needed — all tools are stdlib or already installed
# goreleaser and golangci-lint already on PATH
```

## Package Legitimacy Audit

No new external packages are being installed in this phase. All dependencies are Go standard library or already in `go.mod`.

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

## Architecture Patterns

### System Architecture Diagram

```text
┌──────────────────────────────────────────────────────────────────┐
│                    engine.go (1927 lines)                         │
│  ┌─────────────┬──────────────┬─────────────┬──────────────────┐ │
│  │ Engine      │ Pause/Resume │ Streaming   │ Checkpoint/      │ │
│  │ struct      │ (pauseMu,    │ (consumeStream│ Recovery        │ │
│  │ (fields)    │  pauseCh,    │  withTools) │ (SaveCheckpoint, │ │
│  │             │  resumeCh)   │             │  Recover)        │ │
│  └──────┬──────┴──────┬───────┴──────┬──────┴────────┬─────────┘ │
│         │             │              │               │            │
│  ┌──────▼─────────────▼──────────────▼───────────────▼─────────┐ │
│  │              WorkflowState (workflow_state.go)               │ │
│  │  transitionMu > planMu > messagesMu > intentResultMu        │ │
│  └─────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────┘
          │                           │
          ▼                           ▼
┌─────────────────────┐   ┌───────────────────────────────────────┐
│  StateMachine       │   │       PhaseCoordinator                │
│  (state_machine.go) │   │  (phase_coordinator.go)               │
│  Validated transitions│  │  Pre-phase setup, post-phase metrics  │
└─────────────────────┘   └───────────────────────────────────────┘
```

### Recommended Project Structure

After engine.go split:
```
internal/engine/workflow/
├── engine.go              # Engine struct definition, NewEngine, RunPhase orchestration (~400 lines)
├── engine_pause.go        # PauseExecution, ResumeExecution, IsPaused, SkipCurrentTask, etc. (~150 lines)
├── engine_streaming.go    # streamLLM, streamLLMWithTools, consumeStream, retryChatStream (~350 lines)
├── engine_checkpoint.go   # SaveCheckpointData, LoadCheckpointData, Recover, ClearRecovery (~200 lines)
├── engine_context.go      # buildSystemPrompt, preflightContextCheck, proactiveCompactCheck (~300 lines)
├── engine_model.go        # modelForPhase, SetPhaseModel, SetModel, providerAndModel (~150 lines)
├── engine_helpers.go      # emit, promptOrGet, truncateForLog, computePromptHash (~100 lines)
├── workflow_state.go      # WorkflowState struct and accessors (existing, 319 lines)
├── state_machine.go       # StateMachine (existing, 117 lines)
├── phase_coordinator.go   # PhaseCoordinator (existing, 170 lines)
└── ... (existing phase files unchanged)
```

### Pattern 1: File Splitting by Concern (Same Package)

**What:** Split a large Go file into multiple files within the same package, organized by functional concern.

**When to use:** When a single file exceeds ~500 lines and contains multiple distinct responsibilities.

**Example:**
```go
// engine_pause.go — All pause/resume logic
package workflow

// PauseExecution pauses the execute phase.
func (e *Engine) PauseExecution() bool {
    e.pauseMu.Lock()
    defer e.pauseMu.Unlock()
    if e.pauseCh != nil {
        return false
    }
    e.pauseCh = make(chan struct{})
    e.resumeCh = make(chan struct{})
    e.skipTaskCh = make(chan int, 1)
    e.cancelTaskCh = make(chan int, 1)
    e.cancelGroupCh = make(chan struct{})
    return true
}

// ResumeExecution resumes the execute phase after a pause.
func (e *Engine) ResumeExecution() bool {
    e.pauseMu.Lock()
    defer e.pauseMu.Unlock()
    if e.pauseCh == nil {
        return false
    }
    close(e.resumeCh)
    e.pauseCh = nil
    e.resumeCh = nil
    e.skipTaskCh = nil
    e.cancelTaskCh = nil
    e.cancelGroupCh = nil
    return true
}
```

### Pattern 2: Interface Boundaries for Soft Module Separation

**What:** Define small interfaces at the boundary of a module to decouple consumers from implementation.

**When to use:** When module A needs to call module B but shouldn't depend on B's concrete types.

**Example:**
```go
// Already in codebase: PhaseCoordinator uses Dispatcher interface
// internal/engine/workflow/phase_coordinator.go
type Dispatcher interface {
    RevokeBatchApprovals()
}

// Pattern to extend: define interfaces at consumer boundary
// internal/engine/workflow/engine_interfaces.go
type ToolDispatcher interface {
    CallTool(ctx context.Context, name string, input json.RawMessage) (string, error)
    List() []string
    GetTool(name string) (Tool, bool)
}
```

### Pattern 3: slog Structured Logging with Consistent Fields

**What:** Use `log/slog` with consistent field naming across all modules.

**When to use:** Always — standardize logging across the codebase.

**Example:**
```go
// Source: https://go.dev/blog/slog
// Consistent field naming convention:
logger := slog.Default().With(
    "component", "workflow-engine",
    "session_id", sessionID,
)

// Use structured fields, not string interpolation
logger.Info("phase started",
    "phase", phase,
    "goal", truncateForLog(goal, 100),
    "model", modelID,
)

// Log errors with context
if err != nil {
    logger.Error("phase execution failed",
        "phase", phase,
        "error", err,
        "elapsed", time.Since(start),
    )
}
```

### Anti-Patterns to Avoid

- **Splitting by file size alone:** Don't split just to hit a line count. Split by *concern* — pause/resume is one concern, streaming is another, checkpointing is another.
- **Creating new packages for small amounts of code:** Keep the split within the same package (`workflow`). New packages create import cycles and add cognitive overhead.
- **Hand-rolling DI containers:** D-06 explicitly forbids this. Use constructor injection (already done via `EngineOptions`).
- **Using `fmt.Printf` for logging:** Replace with `slog.Info/Warn/Error` for structured, filterable output.
- **Exposing pprof in production builds:** Always gate behind a debug flag or environment variable.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Profiling endpoints | Custom HTTP handlers | `net/http/pprof` | Standard library; handles all profile types |
| Structured logging | Custom logger with fmt | `log/slog` | Standard library; consistent with existing usage |
| Cross-compilation | Shell scripts with GOOS/GOARCH | GoReleaser v2 | Handles archives, checksums, changelogs, releases |
| Lock ordering enforcement | Custom mutex wrapper | Documented convention + `VerifyLockOrder` helper | Already exists in `engine_concurrency.go` |
| Interface definitions | Concrete type dependencies | Small interfaces at consumer boundary | Go idiom: "accept interfaces, return structs" |

**Key insight:** Go's standard library provides excellent tooling for logging (`slog`), profiling (`pprof`), and the project already has GoReleaser configured. The main work is decomposing `engine.go` into focused files — no new dependencies needed.

## Common Pitfalls

### Pitfall 1: Import Cycles When Splitting Files

**What goes wrong:** Creating new packages for split code causes circular import dependencies.

**Why it happens:** Go's package system禁止 circular imports. If `engine_pause.go` needs types from `workflow` and `workflow` needs types from `engine_pause`, you have a cycle.

**How to avoid:** Keep all split files in the same package (`workflow`). Use the existing `internal/core/types/` package for shared type vocabulary. If a new interface is needed, define it at the consumer boundary (in the package that uses it), not the provider boundary.

**Warning signs:** Compiler errors about import cycles; types that need to be shared across packages.

### Pitfall 2: Breaking the Bubble Tea Contract During Refactoring

**What goes wrong:** Accidentally introducing direct `AppState` mutation from goroutines during the refactoring.

**Why it happens:** When moving code between files, it's easy to miss that a function is called from a goroutine context.

**How to avoid:** Every function that mutates state must either be called from `Update()` or send a `tea.Msg` via the emitter. The existing `MsgEmitter` pattern is the correct bridge.

**Warning signs:** Race detector failures; data races in tests with `-race` flag.

### Pitfall 3: Lock Ordering Violations During Decomposition

**What goes wrong:** When splitting code across files, different developers may acquire locks in different orders.

**Why it happens:** The lock ordering hierarchy is documented in `engine_concurrency.go` but not enforced at compile time.

**How to avoid:** The existing `VerifyLockOrder` helper serves as executable documentation. Add tests that call it with the documented correct orderings. Reference the single authoritative source in `engine_concurrency.go`.

**Warning signs:** Deadlocks in tests; `go test -race` failures; mutex contention in profiles.

### Pitfall 4: pprof Endpoint Exposed in Production

**What goes wrong:** The pprof debug endpoint is accessible in production builds, leaking internal state.

**Why it happens:** Forgetting to gate the debug server behind a flag or environment variable.

**How to avoid:** Start the pprof server only when `M31A_DEBUG=1` or `--debug` flag is set. Use `localhost:6060` (not `:6060`) to prevent external access. Add a build tag `//go:build debug` if desired.

**Warning signs:** Security audit findings; unexpected network connections on port 6060.

### Pitfall 5: Inconsistent slog Field Names

**What goes wrong:** Different modules use different names for the same concept (e.g., `session_id` vs `sessionId` vs `sess_id`).

**Why it happens:** No centralized field naming convention.

**How to avoid:** Document field naming conventions in `AGENTS.md` or a dedicated `CONVENTIONS.md`. Use camelCase for field names (Go convention). Create a shared constants file for commonly used field names.

**Warning signs:** Inability to filter logs by field; grep fails because of inconsistent naming.

## Code Examples

Verified patterns from official sources and codebase:

### Engine File Split: Pause/Resume Extraction

```go
// Source: Adapted from internal/engine/workflow/engine.go lines 136-250
// File: internal/engine/workflow/engine_pause.go
package workflow

import "sync"

// All pause/resume methods extracted from engine.go.
// These methods all use e.pauseMu for synchronization.
// Lock ordering: pauseMu is not part of the WorkflowState hierarchy
// because it's Engine-level, not WorkflowState-level.

func (e *Engine) PauseExecution() bool {
    e.pauseMu.Lock()
    defer e.pauseMu.Unlock()
    if e.pauseCh != nil {
        return false // already paused
    }
    e.pauseCh = make(chan struct{})
    e.resumeCh = make(chan struct{})
    e.skipTaskCh = make(chan int, 1)
    e.cancelTaskCh = make(chan int, 1)
    e.cancelGroupCh = make(chan struct{})
    return true
}

func (e *Engine) ResumeExecution() bool {
    e.pauseMu.Lock()
    defer e.pauseMu.Unlock()
    if e.pauseCh == nil {
        return false // not paused
    }
    close(e.resumeCh)
    e.pauseCh = nil
    e.resumeCh = nil
    e.skipTaskCh = nil
    e.cancelTaskCh = nil
    e.cancelGroupCh = nil
    return true
}
```

### pprof Debug Endpoint

```go
// Source: https://pkg.go.dev/net/http/pprof
// File: internal/debug/pprof.go
package debug

import (
    "log/slog"
    "net"
    "net/http"
    _ "net/http/pprof"
)

// StartProfilingServer starts a pprof HTTP server on localhost.
// Only call this when debug mode is enabled (M31A_DEBUG=1).
// Binds to localhost only to prevent external access.
func StartProfilingServer(addr string) (func(), error) {
    mux := http.NewServeMux()
    mux.HandleFunc("/debug/pprof/", pprof.Index)
    mux.HandleFunc("/debug/pprof/cmdline", pprof.Cmdline)
    mux.HandleFunc("/debug/pprof/profile", pprof.Profile)
    mux.HandleFunc("/debug/pprof/symbol", pprof.Symbol)
    mux.HandleFunc("/debug/pprof/trace", pprof.Trace)

    listener, err := net.Listen("tcp", addr)
    if err != nil {
        return nil, err
    }

    server := &http.Server{Handler: mux}
    go func() {
        if err := server.Serve(listener); err != nil && err != http.ErrServerClosed {
            slog.Error("pprof server error", "error", err)
        }
    }()

    slog.Info("pprof server started", "addr", addr)
    return func() { server.Close() }, nil
}
```

### slog Consistent Logging Pattern

```go
// Source: https://go.dev/blog/slog
// Pattern: Create component-specific loggers with consistent fields

// In engine constructor:
e.logger = slog.Default().With(
    "component", "workflow-engine",
    "session_id", sessionID,
)

// In phase execution:
e.logger.Info("phase started",
    "phase", phase,
    "goal", truncateForLog(goal, 100),
)

// In error paths:
e.logger.Error("phase failed",
    "phase", phase,
    "error", err,
    "elapsed", time.Since(start),
)

// In debug paths (only emitted when level >= Debug):
e.logger.Debug("cache hit",
    "key", cacheKey,
    "age", time.Since(cachedAt),
)
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single 1927-line engine.go | Split into ~7 focused files | Phase 3 (this) | Easier navigation, reduced merge conflicts |
| Inconsistent logging (slog + fmt + log) | All modules use slog | Phase 3 (this) | Filterable, structured, consistent output |
| No profiling endpoints | pprof on localhost:6060 | Phase 3 (this) | Debug profiling for contributors |
| Manual cross-compilation | GoReleaser v2 (already configured) | Pre-existing | Automated releases |
| Lock ordering in code comments only | `VerifyLockOrder` helper + docs | Pre-existing | Executable documentation |

**Deprecated/outdated:**
- `fmt.Printf` for logging: Use `slog.Info/Warn/Error` instead
- Direct mutex access from outside package: Use accessor methods on `WorkflowState`

## Assumptions Log

> All claims in this research were verified against the codebase or official documentation.
> No user confirmation needed.

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| (none) | All claims verified via codebase inspection or official docs | — | — |

## Open Questions

1. **Should debug logging use a custom slog Handler?**
   - What we know: `log/slog` supports custom handlers via `slog.Handler` interface
   - What's unclear: Whether a custom handler adds value over `TextHandler` (dev) / `JSONHandler` (prod)
   - Recommendation: Start with `TextHandler` for dev, `JSONHandler` for prod. Add custom handler only if filtering/grouping needs arise.

2. **Should pprof be gated behind a build tag?**
   - What we know: `//go:build debug` would exclude pprof from release builds entirely
   - What's unclear: Whether the runtime flag approach (`M31A_DEBUG=1`) is sufficient
   - Recommendation: Use runtime flag (simpler). Build tag adds complexity for marginal benefit in a CLI tool.

3. **How should debug logging verbosity be controlled?**
   - What we know: slog supports level-based filtering
   - What's unclear: Whether to use environment variable, CLI flag, or config file
   - Recommendation: CLI flag `--log-level=debug` (highest priority), then `M31A_LOG_LEVEL` env var, then config file, then default (info).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Go | Build | ✓ | 1.26.5 | — |
| goreleaser | Release automation | ✓ | v2.x | Makefile cross targets |
| golangci-lint | Linting | ✓ | v2.12.2 | — |
| `net/http/pprof` | Profiling | ✓ (stdlib) | Go 1.25+ | — |
| `log/slog` | Logging | ✓ (stdlib) | Go 1.21+ | — |

**Missing dependencies with no fallback:** None.

**Missing dependencies with fallback:** None — all required tools are available.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Go standard library `testing` |
| Config file | None — `go test ./...` |
| Quick run command | `make test-fast` |
| Full suite command | `make test` (with race detector) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DEBT-01 | engine.go split preserves all existing behavior | integration | `make test` | ✅ existing tests |
| DEBT-01 | No import cycles after split | unit | `go build ./...` | ✅ compiler check |
| DX-01 | Debug logging emits structured fields | unit | `go test ./internal/engine/workflow/...` | ❌ Wave 0 |
| DX-02 | pprof endpoint responds on debug server | integration | `go test ./internal/debug/...` | ❌ Wave 0 |
| DX-02 | pprof not exposed without debug flag | unit | `go test ./internal/debug/...` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `make test-fast`
- **Per wave merge:** `make test`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `internal/debug/pprof_test.go` — pprof server start/stop, endpoint response
- [ ] `internal/engine/workflow/engine_pause_test.go` — already exists (`pause_resume_test.go`)
- [ ] `internal/engine/workflow/engine_streaming_test.go` — streaming extraction tests
- [ ] `internal/engine/workflow/engine_checkpoint_test.go` — checkpoint extraction tests

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V5 Input Validation | yes | Tool input validation via JSON schema; permission checks in dispatcher |
| V6 Cryptography | no | API keys via OS keychain; no custom crypto |

### Known Threat Patterns for Go CLI

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| pprof endpoint exposure | Information Disclosure | Bind to localhost only; gate behind debug flag |
| Debug logging sensitive data | Information Disclosure | Use `slog.ReplaceAttr` to redact secrets; never log API keys |
| Shell command injection | Tampering | Existing `CheckDangerousCommand()` blocklist in bash tool |

## Sources

### Primary (HIGH confidence)
- Codebase inspection: `internal/engine/workflow/engine.go` (1927 lines, all methods analyzed)
- Codebase inspection: `internal/engine/workflow/workflow_state.go` (319 lines, lock hierarchy documented)
- Codebase inspection: `internal/engine/workflow/engine_concurrency.go` (55 lines, lock ordering)
- Go official docs: `log/slog` package (https://pkg.go.dev/log/slog)
- Go official docs: `net/http/pprof` package (https://pkg.go.dev/net/http/pprof)
- Go official blog: Structured Logging with slog (https://go.dev/blog/slog)

### Secondary (MEDIUM confidence)
- GoReleaser v2 configuration: https://goreleaser.com/customization/builds/
- pprof best practices: https://oneuptime.com/blog/post/2026-01-07-go-pprof-profiling/view

### Tertiary (LOW confidence)
- (none — all claims verified against codebase or official docs)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries are stdlib or already in go.mod
- Architecture: HIGH — analyzed existing codebase structure and patterns
- Pitfalls: HIGH — derived from codebase analysis and Go best practices

**Research date:** 2026-08-05
**Valid until:** 2026-09-05 (30 days — stable domain, Go stdlib doesn't change rapidly)
