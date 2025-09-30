---
phase: 11-code-review
reviewed: 2026-06-06T21:00:00Z
depth: deep
files_reviewed: 24
files_reviewed_list:
  - pkg/taskrunner/runner.go
  - pkg/arbitrage/arbitrage.go
  - pkg/bisect/bisect.go
  - pkg/bisect/exec.go
  - pkg/autodream/autodream.go
  - pkg/ledger/ledger.go
  - pkg/rollback/rollback.go
  - pkg/session/session.go
  - pkg/session/manager.go
  - pkg/session/planning.go
  - pkg/session/checkpoint.go
  - pkg/session/session_info.go
  - pkg/keychain/keychain.go
  - pkg/keychain/keychain_linux.go
  - pkg/keychain/keychain_darwin.go
  - pkg/keychain/keychain_windows.go
  - pkg/keychain/errors.go
  - internal/config/types.go
  - internal/config/loader.go
  - internal/git/git.go
  - internal/log/log.go
  - internal/tokens/estimator.go
  - cmd/m31a/main.go
  - cmd/m31a/usage.go
findings:
  critical: 3
  warning: 9
  info: 4
  total: 16
status: issues_found
---

# Phase 11: Code Review Report — Public Packages & Config Wiring

**Reviewed:** 2026-06-06T21:00:00Z
**Depth:** deep (cross-file import graph, call chain tracing, integration verification)
**Files Reviewed:** 24 (7 pkg/, 4 internal/config+git+log+tokens, 2 cmd/m31a, 11 cross-referenced internal/tui + internal/workflow)
**Status:** issues_found

## Summary

This review traces how public packages (`pkg/taskrunner`, `pkg/arbitrage`, `pkg/bisect`, `pkg/autodream`, `pkg/ledger`, `pkg/rollback`, `pkg/session`, `pkg/keychain`) and config/git/tokens infrastructure are wired into the application. The packages themselves are well-implemented with good test coverage and defensive coding. However, **three critical wiring failures** mean that major subsystems are declared but never actually activated: token estimation is ignored, ledger context injection is missing, and auto-consolidation never triggers automatically. Several secondary issues involve silent error swallowing and stale-state writes.

## Critical Issues

### CR-01: Token Estimation Stored but Never Used — Context Window Protection Disabled

**File:** `internal/workflow/engine.go:75` (declaration), `internal/workflow/engine.go:476-503` (streamLLM), `internal/workflow/engine.go:509-519` (streamLLMStreaming)
**Issue:** The `Engine` stores a `*tokens.Estimator` (line 75) passed via `NewEngine` (line 131), but `e.tokens` is **never referenced** in any execution path. Both `streamLLM` and `streamLLMStreaming` send requests to the provider without ever checking estimated token count against the model's context window. This means the 80% context warning banner and the 95% hard block (`ErrContextExceeded`) described in the architecture docs are completely non-functional in the workflow engine. A workflow consuming a long-running conversation can silently exceed the context window, causing provider errors or truncated responses.

**Fix:**
```go
// Before ChatCompletionStream in streamLLM:
estimatedTokens := e.tokens.estimateTotalMessages(messages)
model, _ := e.provider.GetModel(e.modelForPhase(e.activePhase))
if model != nil && model.ContextLength > 0 {
    if float64(estimatedTokens) > float64(model.ContextLength)*0.95 {
        return "", fmt.Errorf("context window exceeded: %d estimated / %d total: %w",
            estimatedTokens, model.ContextLength, m31errors.ErrContextExceeded)
    }
}
// Also emit context warning banner if > 80%
```

### CR-02: `substituteVars` Silently Replaces Missing Env Vars with Empty String

**File:** `internal/config/loader.go:520-531`
**Issue:** The docstring on `applyVarSubstitution` (line 489) states: "If the variable is not set, the original ${VAR} pattern is preserved as-is." However, `substituteVars` at line 529 returns `""` for missing env vars, not the original `${VAR}` pattern. This means a config value like `api_key = "${M31A_SOME_KEY}"` where the env var is not set silently becomes an empty string instead of preserving the literal `${M31A_SOME_KEY}`. Combined with `ResolveAPIKeys`, this can cause API key resolution to fail silently because an explicitly-set config value gets blanked out during variable substitution before the keychain fallback is attempted.

**Fix:**
```go
func substituteVars(s string) string {
    if s == "" || !strings.Contains(s, "${") {
        return s
    }
    return varRe.ReplaceAllStringFunc(s, func(match string) string {
        name := match[2 : len(match)-1]
        if val, ok := os.LookupEnv(name); ok {
            return val
        }
        return match // preserve original ${VAR} pattern
    })
}
```

### CR-03: Ship Phase Writes STATE.md and Checkpoint AFTER Archiving Session

**File:** `internal/workflow/ship.go:110-126`
**Issue:** The ship phase calls `ArchiveSession` (line 111) which moves the entire session directory to `archived/<id>/`, then writes `STATE.md` (line 116) and `SaveCheckpoint` (line 121) to the **archived location**. If the process crashes between the archive and the state/checkpoint writes, the session has no final STATE.md or checkpoint, making post-ship inspection impossible. More critically, if a concurrent `ListSessions` call happens during this window, the session disappears from the active list but has no final state recorded. The state/checkpoint writes should happen **before** archiving.

**Fix:** Reorder ship.go steps 5, 6, and 7 so that `SaveState` and `SaveCheckpoint` happen before `ArchiveSession`:
```go
// Write STATE.md BEFORE archive
e.sessionMgr.SaveState(...)
e.sessionMgr.SaveCheckpoint(...)
// THEN archive
e.sessionMgr.ArchiveSession(e.sessionID)
```

## Warnings

### WR-01: Config Loaded Twice — Double Initialization of Keychain and Providers

**File:** `cmd/m31a/main.go:78-158` (first load), `internal/tui/app.go:164-184` (second load)
**Issue:** `main.go` loads config (line 78), initializes keychain (line 88), resolves API keys (line 93), and creates the provider registry (line 99). Then `tui.NewApp` is called (line 171) which independently loads config again (app.go:164), initializes keychain again (app.go:174), and resolves API keys again (app.go:183). The config object from `main.go` is discarded. While functionally equivalent, this doubles startup latency (two file reads, two keychain lookups) and means any config modifications made between the two loads would be lost.

**Fix:** Pass the already-loaded `*config.Config` from `main.go` into `NewApp` instead of having `NewApp` re-load it.

### WR-02: Ledger Not Queried During Initialize Phase — Cross-Session Context Injection Missing

**File:** `internal/workflow/initialize.go` (no ledger import)
**Issue:** The architecture (Phase 7, P7.3) specifies: "Context injection during Initialize phase" — the ledger should query relevant past sessions and inject their context into the new project. However, `initialize.go` never imports or uses the `ledger` package. The only ledger interaction is appending an entry after Ship (ship.go:100). This means cross-session learning is write-only; no past session insights are ever fed back to the LLM.

**Fix:** In `runInitialize`, load the ledger, query for relevant past sessions by project type/goal keywords, and inject a summary into `planning/PROJECT.md` or as context for the Discuss phase.

### WR-03: AutoDream Never Triggers Automatically — Manual-Only Consolidation

**File:** `internal/tui/app_update.go:318-319` (sync only), `internal/tui/commands_ai.go:47-97` (manual trigger only)
**Issue:** The `autoDream.Consolidator` is synced with messages on every slash command (app_update.go:319) and triggered manually via `/compress` (commands_ai.go:97), but there is no automatic check against `types.AutoDreamThreshold` (0.60) during streaming or message submission. The architecture specifies: "Trigger: context > 60% full, or 15 minutes of active conversation. Never runs during tool execution." No periodic or post-message check invokes `CanConsolidate()` + `Consolidate()` automatically.

**Fix:** After each user message is submitted (or after each assistant response completes), check:
```go
if m.autoDream != nil && m.autoDream.CanConsolidate() {
    m.autoDream.Consolidate()
    // Show [DREAM] tag in status bar
}
```

### WR-04: Rollback Does Not Reset Task States in TASKS.md

**File:** `internal/tui/commands_git.go:74-139`
**Issue:** The `handleRollback` handler performs git soft/hard/safe resets via `ctx.Rollback.SoftReset(hash, nil)` — passing `nil` for the `onReset` callback. The `Rollback.SoftReset` method (rollback.go:116) accepts an `onReset func(newHead string) error` callback specifically designed for syncing task states (M-28). By passing `nil`, rolled-back commits don't cause their corresponding tasks to revert to `StatusPending` in TASKS.md. The user sees old task statuses even though the code was reverted.

**Fix:** Wire the onReset callback to load tasks, find tasks with commit hashes in the rolled-back range, and reset their status:
```go
ctx.Rollback.SoftReset(hash, func(newHead string) error {
    tasks, err := ctx.SessionManager.LoadTasks(ctx.SessionID)
    // ... find tasks with commit hashes after newHead and reset to Pending
    return ctx.SessionManager.SaveTasks(ctx.SessionID, tasks)
})
```

### WR-05: `Config.Save` Errors Silently Discarded Across Command Handlers

**File:** `internal/tui/commands_config.go:98,104,109,116,123,130,137,144,151` and `internal/tui/commands_core.go:123`
**Issue:** At least 10 calls to `ctx.Config.Save(ctx.ConfigPath)` discard the returned error. If the save fails (disk full, permissions issue), the in-memory config has the new value but the file on disk does not. The next restart reverts to the old config, with no indication to the user that the change was lost.

**Fix:** Check the error and surface it:
```go
if err := ctx.Config.Save(ctx.ConfigPath); err != nil {
    return CommandResult{Success: false, Message: fmt.Sprintf("Failed to save config: %v", err)}
}
```

### WR-06: `ListSessions` Does Not Populate `ParentID`/`ChildrenIDs` in SessionInfo

**File:** `pkg/session/manager.go:380-384`
**Issue:** When building `SessionInfo` from the parsed `Session` struct, `ListSessions` copies `Model`, `Provider`, `StartedAt`, `MessageCount`, and `WorkflowPhase` but omits `ParentID` and `ChildrenIDs` (lines 380-384). The `SessionInfo` struct has these fields (session_info.go:12-13), but they're always empty in listing results. This means the resume screen and session browser cannot display fork relationships, and `SiblingSessions` must load every session individually to determine hierarchy.

**Fix:** Add the missing field copies:
```go
info.ParentID = s.ParentID
info.ChildrenIDs = s.ChildrenIDs
```

### WR-07: Bisect Loop Has No Iteration Limit — Potential Infinite Loop on Corrupt Git State

**File:** `pkg/bisect/bisect.go:89-116`
**Issue:** The bisect loop at line 89 runs `b.run("bisect", "log")` to check for "first bad commit", but the error is silently discarded (`logOut, _ := b.run(...)`). If git is in a corrupt state (e.g., bisect database corrupted, detached HEAD), the log may never contain "first bad commit" and the loop runs indefinitely. There is no maximum iteration guard. A typical `git bisect` on N commits completes in O(log N) steps, so a safety limit of 100 iterations would be sufficient.

**Fix:** Add an iteration counter:
```go
const maxBisectSteps = 100
for step := 0; step < maxBisectSteps; step++ {
    // ... existing loop body
}
return nil, fmt.Errorf("bisect did not converge after %d steps", maxBisectSteps)
```

### WR-08: `validateService` in macOS Keychain Returns Wrong Error Type

**File:** `pkg/keychain/keychain_darwin.go:29-33`
**Issue:** The `validateService` function on macOS returns `ErrNotImplemented` when the service name is invalid (line 32). This is semantically wrong — `ErrNotImplemented` means the platform doesn't support keychain operations, not that the input is invalid. Linux's `validateService` correctly returns `ErrKeychainUnavailable`. The macOS version should return a dedicated `ErrInvalidServiceName` or at minimum `ErrKeychainUnavailable`, so callers can distinguish "platform unsupported" from "bad input."

**Fix:**
```go
func validateService(service string) error {
    if !validServiceName.MatchString(service) {
        return ErrKeychainUnavailable
    }
    return nil
}
```

### WR-09: `pkg/` Packages Import `internal/git` — Architectural Boundary Blur

**File:** `pkg/rollback/rollback.go:8`, `pkg/bisect/bisect.go:8`
**Issue:** Two public packages (`pkg/rollback`, `pkg/bisect`) directly import `internal/git`. While Go permits this (both are within the module root), it creates a coupling where `pkg/` packages depend on `internal/` implementation details. If `internal/git` changes its API, both `pkg/` packages break. The `bisect` package partially addresses this with its `GitRunner` interface (bisect.go:20-22), but `rollback` takes a concrete `*git.Git` (rollback.go:34). For consistency and testability, `rollback` should also accept an interface.

**Fix:** Define a `GitRunner` interface in `pkg/rollback` similar to `pkg/bisect`:
```go
type GitRunner interface {
    HeadHash() (string, error)
    Log(oneline bool, since string) ([]git.CommitInfo, error)
    Diff(ref1, ref2 string) (string, error)
    // ... other methods used by rollback
}
```

## Info

### IN-01: Arbitrage Integration Is Manual-Only — Plan Phase Does Not Auto-Suggest

**File:** `internal/workflow/plan.go` (no arbitrage import)
**Issue:** The arbitrage module is only invoked by the manual `/optimize` slash command (commands_config.go:321) and displayed in the Plan TUI screen (plan.go:78). The Plan workflow phase itself does not call `arbitrage.Recommend()` to suggest cheaper models for individual tasks. Per the architecture (P7.2), "Arbiter.SuggestAll(tasks): batch suggestions for Plan screen" should be called during plan generation.

**Fix:** After task generation in `runPlan`, call `arbitrage.SuggestAll` with the generated tasks and the model catalog, and attach recommendations to the `PhaseResult`.

### IN-02: `pkg/keychain` Uses Global `newFunc` Variable — Not Thread-Safe for Initialization

**File:** `pkg/keychain/keychain.go:31`
**Issue:** The `newFunc` variable is set by platform-specific `init()` functions. While Go guarantees `init()` runs before `main()`, the package-level mutable variable pattern is fragile for concurrent initialization scenarios (e.g., tests running in parallel with different build constraints). This is low-risk since `init()` is deterministic, but a more explicit approach would be a switch on `runtime.GOOS`.

### IN-03: `SessionIDLength` Config Is Divided by 2 for Bytes — Fragile Coupling

**File:** `internal/tui/app_workflow.go:216-218`
**Issue:** `SessionIDLength` from config is divided by 2 to get the byte count for `generateID`. The minimum clamp is 2 bytes (4 hex chars). If a user sets `session_id_length = 5` (odd number), the division truncates to `2` bytes (4 hex chars), silently ignoring their intent. The config validation (loader.go:422) allows 4-16, but doesn't enforce even numbers.

**Fix:** Add validation that `SessionIDLength` must be even:
```go
if cfg.Features.SessionIDLength != 0 && cfg.Features.SessionIDLength%2 != 0 {
    errs = append(errs, ValidationError{...})
}
```

### IN-04: Ledger Append Uses `ErrTaskFailed` as Dedup Guard — Semantic Mismatch

**File:** `pkg/ledger/ledger.go:136`
**Issue:** `Append` returns `m31errors.ErrTaskFailed` when a duplicate SessionID is found. `ErrTaskFailed` means "a task completed with failure status" — semantically unrelated to "ledger entry already exists." This could confuse callers that check `errors.Is(err, ErrTaskFailed)` and interpret it as a task failure. A dedicated `ErrDuplicateEntry` would be clearer.

---

_Reviewed: 2026-06-06T21:00:00Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: deep_
