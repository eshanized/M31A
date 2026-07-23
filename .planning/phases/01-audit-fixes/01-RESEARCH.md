# Phase 1: Audit Bug Fixes - Research

**Researched:** 2026-07-23
**Domain:** Go concurrency, permission evaluation, session persistence, provider fallback, state machine correctness
**Confidence:** HIGH

## Summary

This phase resolves 30 confirmed logical bugs (B01-B30) organized into 4 severity-based batches. The bugs span concurrency safety (data races on shared state), permission logic (incorrect evaluation ordering), session persistence (data loss on save), provider fallback (missing error types), state machine correctness (transition validation bypass), and token estimation (systematic undercount).

The most critical finding is that B02 (RunPhase bypasses transition validation) interacts with B25 (duplicate history entry). Fixing B02 by routing RunPhase through Transition() will likely eliminate B25 as a side effect, per D-06. B06 (flock cross-process dual-lock) requires switching from `flock(2)` to `fcntl(F_SETLK)` or `O_EXCL` for true cross-process mutual exclusion.

**Primary recommendation:** Investigate B02 and B06 first (D-03), then fix in listed order within each batch (D-01), with one commit per fix (D-04).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Fix within each batch in the listed order (B01, B02, B03...). Matches audit priority and simplifies tracking.
- **D-02:** Class coverage tests — tests cover the bug class, not just the individual bug. E.g., all race conditions in a file get a shared stress test. Catches similar bugs.
- **D-03:** Investigate B02 and B06 before starting any Batch 1 fixes. Block Batch 1 on investigation results.
- **D-04:** One commit per fix (30 atomic commits). Cleanest bisect, most granular review.
- **D-05:** Use existing testutil from `tests/testutil/` (mocks, builders, env helpers). Consistent with codebase patterns.
- **D-06:** Fix B02 first, then verify if B25 (duplicate history) is resolved before writing a separate fix. B02 routing RunPhase through Transition() may eliminate the duplicate entry.
- **D-07:** Follow the spec exactly — use `toml.MetaData.IsDefined()` from BurntSushi/toml for B18, not more zero-value special-casing.
- **D-08:** Strict literal — only change exactly what's described in each bug. No adjacent cleanups, no opportunistic refactors.

### the agent's Discretion
- Test helper design (exact function signatures, table-driven vs individual)
- Whether a fix needs additional related tests beyond the mandatory regression test
- Exact lock type selection (sync.Mutex vs sync.RWMutex) per fix context

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| B01 | Fix proactive compaction result discard in execute.go:571 | Capture return value, assign back to messages variable |
| B02 | Fix RunPhase bypassing transition validation in engine.go:860 | Route through Transition() or validate before SetPhase |
| B03 | Fix permission last-match-wins logic in permissions.go:280-341 | Change evaluation to deny-wins or specificity-based |
| B04 | Add lock to LoadWorkflowState in manager.go:316 | Acquire m.lock before loadSessionMetadata |
| B05 | Fix saveSessionAtomic overwriting Messages in manager.go:328-333 | Marshal only workflow fields, not full Session |
| B06 | Fix flock cross-process dual-lock in fileutil.go:29-44 | Switch to fcntl(F_SETLK) or O_EXCL |
| B07 | Fix Shutdown nil cache race in engine.go:537 | Add synchronization or check before nil assignment |
| B08 | Add auth/credit/model-not-found to fallback triggers in handler_stream.go:57 | Extend error check to include ErrInvalidKey, ErrNoCredits, ErrModelNotFound |
| B09 | Add mid-stream SSE error fallback in streaming.go:135 | Map io.ErrUnexpectedEOF etc. to fallback-eligible errors |
| B10 | Fix permission response timeout race in permissions.go:506-515 | Ensure response channel survives timeout cleanup |
| B11 | Fix hardcoded param keys in matchAnyParamValue in permissions.go:384-396 | Iterate all string params, not hardcoded subset |
| B12 | Add planMu lock to SaveCheckpointData in engine.go:456 | Acquire planMu.RLock before reading planVersion |
| B13 | Add synchronization to GetCheckpointData in engine.go:565-567 | Add lock or atomic access |
| B14 | Add Goal and PlanVersion to transition checkpoint in phase_coordinator.go:124 | Copy fields from engine state |
| B15 | Reset discussPlanCycles on SetPhase in state_machine.go:101-105 | Add reset logic in SetPhase |
| B16 | Fix token estimation truncation in estimator.go:194 | Use math.Ceil instead of int() |
| B17 | Fix inconsistent token accounting in engine.go:1078-1089 | Use EstimateMessages consistently |
| B18 | Use toml.MetaData.IsDefined() for zero-value override in merge.go:40-51 | Pass defined map, check hasKey for int/float |
| B19 | Expand variable substitution coverage in config_validate.go:311 | Walk all string fields or extend hardcoded list |
| B20 | Add locking to checkpoint save in checkpoint.go:30-53 | Add mutex to Manager or use file lock |
| B21 | Add retry/TTL to keychain blacklist in keychain.go:61-63 | Add time-based expiry or retry counter |
| B22 | Handle "degraded" status in fallback.go:106 | Add degraded case alongside slow |
| B23 | Fix resizePending goroutine mutation in repl.go:64 | Send tea.Msg instead of direct mutation |
| B24 | Add lock to d.collector read in dispatcher.go:315 | Acquire d.mu.RLock before read |
| B25 | Verify if B02 fix eliminates duplicate history entry | Check after B02 fix |
| B26 | Cap history growth in state_machine.go:96,105 | Add max history size |
| B27 | Log doublestar.Match errors in permissions.go:365,401,417 | Check error return from doublestar.Match |
| B28 | Add comma-ok check to type assertion in dns_cache.go:134 | Use key, ok := key.(string) |
| B29 | Cache default model capabilities in capabilities.go:355 | Store default caps in cache |
| B30 | Drain requestCh/questionReqCh in Stop() in dispatcher.go:432 | Add drain loops for remaining channels |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Session persistence & locking | API / Backend | Database / Storage | SessionManager owns file I/O and locking |
| Permission rule evaluation | API / Backend | — | Dispatcher owns all permission logic |
| Provider fallback routing | API / Backend | — | Provider layer handles fallback decisions |
| Workflow state machine | API / Backend | — | Engine + StateMachine own phase transitions |
| Token estimation | API / Backend | — | Estimator is internal calculation |
| Config merging | API / Backend | — | Config loader owns merge logic |
| TUI resize handling | Browser / Client | — | Bubble Tea model owns UI state |
| Tool execution metrics | API / Backend | — | Dispatcher owns collector access |

## Batch Organization

### Batch 1: Critical + High (B01-B09) — 9 bugs

| ID | File | Line(s) | Severity | Category |
|----|------|---------|----------|----------|
| B01 | `internal/engine/workflow/execute.go` | 571 | Critical | Logic error — discarded return value |
| B02 | `internal/engine/workflow/engine.go` | 860 | High | State machine bypass |
| B03 | `internal/tools/permissions.go` | 280-341 | High | Logic error — wrong evaluation order |
| B04 | `internal/engine/session/manager.go` | 316 | High | Data race — missing lock |
| B05 | `internal/engine/session/manager.go` | 328-333 | High | Data corruption — overwrites messages |
| B06 | `internal/core/types/fileutil.go` | 29-44 | High | Incorrect locking primitive |
| B07 | `internal/engine/workflow/engine.go` | 537 | High | Data race — nil without sync |
| B08 | `internal/ui/tui/handler_stream.go` | 57 | High | Missing error classification |
| B09 | `internal/ui/tui/streaming/streaming.go` | 135 | High | Missing error classification |

### Batch 2: Medium — Group A (B10-B17) — 8 bugs

| ID | File | Line(s) | Severity | Category |
|----|------|---------|----------|----------|
| B10 | `internal/tools/permissions.go` | 506-515 | Medium | Timeout race |
| B11 | `internal/tools/permissions.go` | 384-396 | Medium | Hardcoded keys |
| B12 | `internal/engine/workflow/engine.go` | 456 | Medium | Data race — missing lock |
| B13 | `internal/engine/workflow/engine.go` | 565-567 | Medium | Data race — no sync |
| B14 | `internal/engine/workflow/phase_coordinator.go` | 124 | Medium | Missing checkpoint fields |
| B15 | `internal/engine/workflow/state_machine.go` | 101-105 | Medium | Missing state reset |
| B16 | `internal/engine/tokens/estimator.go` | 194 | Medium | Truncation error |
| B17 | `internal/engine/workflow/engine.go` | 1078-1089 | Medium | Inconsistent estimation |

### Batch 3: Medium — Group B (B18-B24) — 7 bugs

| ID | File | Line(s) | Severity | Category |
|----|------|---------|----------|----------|
| B18 | `internal/core/config/merge.go` | 40-51 | Medium | Zero-value override blocked |
| B19 | `internal/core/config/config_validate.go` | 311 | Medium | Incomplete substitution |
| B20 | `internal/engine/session/checkpoint.go` | 30-53 | Medium | R-M-W race |
| B21 | `internal/integrations/keychain/keychain.go` | 61-63 | Medium | Permanent blacklist |
| B22 | `internal/integrations/provider/fallback.go` | 106 | Medium | Missing status handling |
| B23 | `internal/ui/tui/repl.go` | 64 | Medium | Goroutine mutation |
| B24 | `internal/tools/dispatcher.go` | 315 | Medium | Read without lock |

### Batch 4: Low (B25-B30) — 6 bugs

| ID | File | Line(s) | Severity | Category |
|----|------|---------|----------|----------|
| B25 | `internal/engine/workflow/engine.go` + `state_machine.go` | 860+96 | Low | Duplicate history entry |
| B26 | `internal/engine/workflow/state_machine.go` | 96,105 | Low | Unbounded growth |
| B27 | `internal/tools/permissions.go` | 365,401,417 | Low | Swallowed errors |
| B28 | `internal/tools/search/dns_cache.go` | 134 | Low | Bare type assertion |
| B29 | `internal/integrations/provider/capabilities.go` | 355 | Low | Uncached defaults |
| B30 | `internal/tools/dispatcher.go` | 432 | Low | Incomplete channel drain |

## Investigation Findings: B02 and B06 (D-03)

### B02 Investigation: RunPhase bypasses transition validation

**Current code path (engine.go:860):**
```go
e.stateMachine.SetPhase(phase)  // Direct write, no validation
```

**Validated path (engine.go:901-911):**
```go
func (e *Engine) Transition(ctx context.Context, from, to m31types.WorkflowPhase) error {
    e.state.transitionMu.Lock()
    defer e.state.transitionMu.Unlock()
    if err := e.stateMachine.Transition(from, to); err != nil { ... }
    return e.phaseCoordinator.CoordinateTransition(ctx, from, to)
}
```

**State machine (state_machine.go:59-97):**
- `Transition()` validates against `validTransitions` map, checks `currentPhase == from`, enforces plan/discuss cycle limit
- `SetPhase()` directly writes `currentPhase` and appends to `history` with no validation

**Key finding:** `RunPhase` is the primary execution path used by both TUI and headless CLI. It calls `SetPhase()` which bypasses all transition validation. The `Transition()` method is only called from TUI's `handlePhaseTransitionDecision` and `main.go`.

**Fix approach:** Route RunPhase through Transition(). Specifically:
1. Before `e.stateMachine.SetPhase(phase)`, call `e.stateMachine.Transition(e.stateMachine.CurrentPhase(), phase)` to validate
2. Remove the redundant `SetPhase` call since `Transition()` already sets `currentPhase`
3. This also fixes B25 (duplicate history) because `Transition()` appends to history, and `SetPhase()` was adding a second entry

**Impact on B25:** Per D-06, B02 fix should eliminate B25. After fixing B02, verify that TUI phase transitions produce exactly one history entry. If so, B25 is resolved. If not, a separate fix may be needed.

**Risks:**
- `RunPhase` is called from multiple callers (TUI, headless CLI). Need to verify all callers pass correct `from` phase.
- The `from` phase must be `e.stateMachine.CurrentPhase()` to avoid false rejections.
- `CoordinateTransition()` has side effects (saves checkpoint, writes STATE.md). Need to verify these are appropriate for RunPhase's context.

### B06 Investigation: flock cross-process dual-lock

**Current code (fileutil.go:29-44):**
```go
func (fl *FileLock) Lock() error {
    f, err := os.OpenFile(fl.path, os.O_CREATE|os.O_RDWR, 0600)
    if err := syscall.Flock(int(f.Fd()), syscall.LOCK_EX); err != nil { ... }
    fl.mu.Lock()
    fl.file = f
    fl.mu.Unlock()
    return nil
}
```

**Problem:** `flock(2)` is per file-descriptor, not per file. Two processes opening the same lock file get separate FDs and can both acquire `LOCK_EX` simultaneously. This is a well-documented limitation of `flock(2)`.

**Fix options:**
1. **`fcntl(F_SETLK)`** — POSIX record locking, per-process per-file (not per-FD). Two processes cannot both hold `F_WRLCK` on the same file. This is the correct solution for cross-process mutual exclusion.
2. **`O_EXCL`** — Atomic file creation, fails if file exists. Simpler but doesn't release on crash.
3. **PID file** — Write PID to lock file, check for stale PIDs. More complex, needs stale detection.

**Recommendation:** Use `fcntl(F_SETLK)` with `F_WRLCK` (write lock). It's the standard POSIX mechanism for cross-process file locking. The `syscall` package in Go supports `fcntl` on Linux/Darwin.

**Platform considerations:**
- Linux: `syscall.Fcntl` with `F_SETLK` works
- Darwin: Same syscall interface
- Windows: Not applicable (build tag `//go:build !windows` already excludes Windows)
- Need to handle `EAGAIN`/`EACCES` for `TryLock` (non-blocking variant)

**Risks:**
- `fcntl` locks are per-process, not per-FD. If a process forks, the child inherits the lock. Not an issue for M31A (single process).
- Need to verify the existing `TryLock` implementation also uses `fcntl` for consistency.
- The `Unlock` method must also switch from `flock(LOCK_UN)` to `fcntl(F_SETLK, F_UNLCK)`.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `sync.Mutex` / `sync.RWMutex` | stdlib | Thread-safe state access | Go's standard concurrency primitive |
| `sync.Once` | stdlib | Idempotent initialization/shutdown | Already used in Dispatcher.Stop() |
| `atomic.Int64` | stdlib | Lock-free counters | Already used in pendingPermCount |
| `math.Ceil` | stdlib | Round-up for token estimation | Prevents systematic undercount |
| `syscall.Fcntl` | stdlib | POSIX file locking | Cross-process mutual exclusion |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `BurntSushi/toml` | v1 | TOML parsing with `MetaData.IsDefined()` | B18 zero-value override detection |
| `doublestar/v4` | v4 | Glob pattern matching | Already used in permissions |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `fcntl(F_SETLK)` | `O_EXCL` | O_EXCL doesn't release on crash; fcntl is more robust |
| `sync.Mutex` | `sync.Map` | Mutex is simpler for single-field protection |
| `math.Ceil` | `int(x + 0.999)` | math.Ceil is clearer and more idiomatic |

**Installation:** No new packages needed — all fixes use stdlib or existing dependencies.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cross-process file locking | Custom PID file + stale detection | `fcntl(F_SETLK)` | POSIX-standard, kernel-managed |
| Thread-safe state access | Channel-based message passing | `sync.Mutex` / `sync.RWMutex` | Simpler for protecting struct fields |
| Token estimation rounding | Custom rounding function | `math.Ceil` | Standard, well-tested |
| TOML key presence detection | Zero-value checks | `toml.MetaData.IsDefined()` | Correctly handles 0, false, empty string |

## Common Pitfalls

### Pitfall 1: B02 Transition Validation Bypass
**What goes wrong:** RunPhase sets phase without validation, allowing invalid transitions like Idle→Ship
**Why it happens:** SetPhase was designed for checkpoint restore (unconditional), but RunPhase reuses it for normal flow
**How to avoid:** Always use Transition() for normal phase changes; SetPhase only for restore
**Warning signs:** History entries showing impossible phase sequences

### Pitfall 2: B06 flock Limitations
**What goes wrong:** Two M31A instances corrupt session files despite both "holding the lock"
**Why it happens:** flock(2) is per-FD, not per-file — two processes get separate FDs
**How to avoid:** Use fcntl(F_SETLK) for cross-process locking
**Warning signs:** Corrupted session.json, lost checkpoint data in multi-instance scenarios

### Pitfall 3: B03 Permission Override
**What goes wrong:** Broad allow rule overrides specific deny rule
**Why it happens:** Last-match-wins evaluation; users write specific denies before broad allows
**How to avoid:** Change to deny-wins or specificity-based evaluation
**Warning signs:** Denied commands executing without permission prompt

### Pitfall 4: B07 Shutdown Race
**What goes wrong:** Nil pointer panic when accessing e.cache after Shutdown
**Why it happens:** e.cache set to nil without synchronization while goroutines may still read it
**How to avoid:** Use atomic pointer or mutex-protected access
**Warning signs:** Nil pointer dereference in logs during shutdown

### Pitfall 5: B16 Token Undercount
**What goes wrong:** Context window overflow because token estimates are systematically low
**Why it happens:** int() truncates toward zero; aggregate undercount across messages
**How to avoid:** Use math.Ceil for upper-bound estimates
**Warning signs:** Requests exceeding context window, unexpected truncation

## Code Examples

### B01 Fix: Capture Compaction Return Value
```go
// Source: BUGS.md B01, execute.go:571
// BEFORE:
e.proactiveCompactCheck(messages) //nolint:errcheck

// AFTER:
messages = e.proactiveCompactCheck(messages)
```

### B02 Fix: Route RunPhase Through Transition
```go
// Source: BUGS.md B02, engine.go:860
// BEFORE:
e.stateMachine.SetPhase(phase)

// AFTER:
if err := e.stateMachine.Transition(e.stateMachine.CurrentPhase(), phase); err != nil {
    return nil, fmt.Errorf("phase transition to %s: %w", phase, err)
}
```

### B03 Fix: Deny-Wins Evaluation
```go
// Source: BUGS.md B03, permissions.go:280-341
// BEFORE: last-match-wins
// AFTER: deny-wins (deny overrides allow regardless of order)
var lastAllow *struct{...}
for _, rule := range d.rules {
    // ... match logic ...
    switch rule.Action {
    case "allow":
        lastAllow = &struct{...}{true, pctx, nil}
    case "deny":
        // Deny immediately wins — no further rules needed
        return false, pctx, m31errors.ErrPermissionDenied
    case "ask":
        return false, pctx, nil
    }
}
if lastAllow != nil {
    return lastAllow.allowed, lastAllow.pctx, lastAllow.err
}
```

### B06 Fix: fcntl Locking
```go
// Source: BUGS.md B06, fileutil.go:29-44
// BEFORE: syscall.Flock(int(f.Fd()), syscall.LOCK_EX)

// AFTER: fcntl(F_SETLK) with F_WRLCK
lock := syscall.Flock_t{
    Type:   syscall.F_WRLCK,
    Whence: int16(os.SEEK_SET),
    Start:  0,
    Len:    0,
}
if err := syscall.Fcntl(int(f.Fd()), syscall.F_SETLK, &lock); err != nil {
    _ = f.Close()
    if err == syscall.EAGAIN || err == syscall.EACCES {
        return fmt.Errorf("lock held by another process: %w", err)
    }
    return fmt.Errorf("fcntl %s: %w", fl.path, err)
}
```

### B16 Fix: Use math.Ceil
```go
// Source: BUGS.md B16, estimator.go:194
// BEFORE: return int(float64(chars) / ratio)
// AFTER:
import "math"
return int(math.Ceil(float64(chars) / ratio))
```

### B18 Fix: Use MetaData.IsDefined
```go
// Source: BUGS.md B18, merge.go:40-51
// BEFORE:
func (m mergeHelper) intField(base, overlay *int, key string) {
    if *overlay != 0 {
        *base = *overlay
    }
}

// AFTER:
func (m mergeHelper) intField(base, overlay *int, key string) {
    if m.hasKey(key) {
        *base = *overlay
    }
}
```

### B23 Fix: Send Tea Message
```go
// Source: BUGS.md B23, repl.go:64
// BEFORE:
m.resizeTimer = time.AfterFunc(100*time.Millisecond, func() {
    m.resizePending = true  // goroutine mutation!
})

// AFTER:
m.resizeTimer = time.AfterFunc(100*time.Millisecond, func() {
    // Send a message through the program channel instead of mutating state directly
    if m.program != nil {
        m.program.Send(tea.Msg(resizeDebounceMsg{}))
    }
})
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `flock(2)` for cross-process locking | `fcntl(F_SETLK)` recommended | N/A (fixing now) | B06 — two instances can corrupt files |
| Last-match-wins permissions | Deny-wins recommended | N/A (fixing now) | B03 — security gap |
| `int()` truncation for tokens | `math.Ceil` recommended | N/A (fixing now) | B16 — systematic undercount |

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | B02 fix will eliminate B25 (duplicate history) | B02 Investigation | B25 needs separate fix; low risk, just extra work |
| A2 | `fcntl(F_SETLK)` works on Linux and Darwin | B06 Fix | Would need platform-specific implementation |
| A3 | Existing tests cover enough of the codebase to validate fixes | Test Strategy | May need additional test infrastructure |
| A4 | No new external packages are needed | Standard Stack | Would need to add and audit new deps |

## Open Questions

1. **B02: Should Transition() be called within RunPhase or should RunPhase accept a validated phase?**
   - What we know: RunPhase currently calls SetPhase directly; Transition() exists but is only called from TUI
   - What's unclear: Whether RunPhase should call Transition() internally or whether callers should validate first
   - Recommendation: Have RunPhase call Transition() internally — keeps validation close to the action

2. **B03: Should deny-wins be the default or should it be configurable?**
   - What we know: Current behavior is last-match-wins; deny-wins is safer
   - What's unclear: Whether any users depend on last-match-wins behavior
   - Recommendation: Change to deny-wins (safer default); document in CHANGELOG

3. **B19: Should variable substitution walk all string fields or extend the hardcoded list?**
   - What we know: Only ~10 fields are covered; many more string fields exist
   - What's unclear: Whether walking all fields is feasible without reflection
   - Recommendation: Extend hardcoded list for known missing fields; document limitation

4. **B21: What is the appropriate retry interval for keychain blacklist recovery?**
   - What we know: Current implementation permanently blacklists after first failure
   - What's unclear: Appropriate TTL (5 minutes? 30 minutes? configurable?)
   - Recommendation: 5-minute TTL with configurable option; conservative default

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Go stdlib `testing` with `-race` flag |
| Config file | `Makefile` targets |
| Quick run command | `make test-fast` |
| Full suite command | `make test` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| B01 | Compaction result applied to messages | unit | `go test -run TestProactiveCompact ./internal/engine/workflow/ -race` | Partial (coverage_boost_test.go) |
| B02 | RunPhase rejects invalid transitions | unit | `go test -run TestRunPhase_ValidateTransition ./internal/engine/workflow/ -race` | No (new) |
| B03 | Deny overrides allow regardless of order | unit | `go test -run TestCheckPermission_DenyWins ./internal/tools/ -race` | No (new) |
| B04 | LoadWorkflowState concurrent with Update | race | `go test -run TestLoadWorkflowState_Concurrent ./internal/engine/session/ -race` | No (new) |
| B05 | saveSessionAtomic preserves messages | unit | `go test -run TestSaveSessionAtomic_PreservesMessages ./internal/engine/session/ -race` | No (new) |
| B06 | fcntl prevents cross-process dual-lock | integration | `go test -run TestFileLock_CrossProcess ./internal/core/types/ -race` | No (new) |
| B07 | Shutdown nil cache doesn't panic | race | `go test -run TestShutdown_CacheRace ./internal/engine/workflow/ -race` | No (new) |
| B08 | Auth errors trigger fallback | unit | `go test -run TestHandleStreamErrorMsg_AuthFallback ./internal/ui/tui/ -race` | No (new) |
| B09 | Mid-stream errors trigger fallback | unit | `go test -run TestStreamError_Fallback ./internal/ui/tui/ -race` | No (new) |
| B10 | Permission response survives timeout | unit | `go test -run TestSendAndWaitForPermission_TimeoutRace ./internal/tools/ -race` | No (new) |
| B11 | matchAnyParamValue matches all string params | unit | `go test -run TestMatchAnyParamValue_AllKeys ./internal/tools/ -race` | No (new) |
| B12 | SaveCheckpointData uses planMu | race | `go test -run TestSaveCheckpointData_Concurrent ./internal/engine/workflow/ -race` | No (new) |
| B13 | GetCheckpointData synchronized | race | `go test -run TestGetCheckpointData_Concurrent ./internal/engine/workflow/ -race` | No (new) |
| B14 | Transition checkpoint has Goal/PlanVersion | unit | `go test -run TestCoordinateTransition_CheckpointFields ./internal/engine/workflow/ -race` | No (new) |
| B15 | SetPhase resets discussPlanCycles | unit | `go test -run TestSetPhase_ResetsCycles ./internal/engine/workflow/ -race` | No (new) |
| B16 | Token estimation uses ceiling | unit | `go test -run TestEstimate_Ceiling ./internal/engine/tokens/ -race` | No (new) |
| B17 | Preflight uses consistent estimation | unit | `go test -run TestTruncateMessages_ConsistentEstimation ./internal/engine/workflow/ -race` | No (new) |
| B18 | Zero-value int override works | unit | `go test -run TestMergeConfig_ZeroIntOverride ./internal/core/config/ -race` | No (new) |
| B19 | All string fields get variable substitution | unit | `go test -run TestApplyVarSubstitution_Fields ./internal/core/config/ -race` | No (new) |
| B20 | Concurrent checkpoint saves don't lose data | race | `go test -run TestSaveCheckpoint_Concurrent ./internal/engine/session/ -race` | No (new) |
| B21 | Keychain blacklist recovers after TTL | unit | `go test -run TestKeychainBlacklist_Recovers ./internal/integrations/keychain/ -race` | No (new) |
| B22 | Degraded status triggers fallback | unit | `go test -run TestFindFallbackProvider_Degraded ./internal/integrations/provider/ -race` | No (new) |
| B23 | Resize doesn't mutate from goroutine | race | `go test -run TestRepl_ResizeDebounce ./internal/ui/tui/ -race` | No (new) |
| B24 | collector read uses lock | race | `go test -run TestDispatcher_CollectorRace ./internal/tools/ -race` | No (new) |
| B25 | Phase transition produces one history entry | unit | `go test -run TestStateMachine_NoDuplicateHistory ./internal/engine/workflow/ -race` | No (new) |
| B26 | History capped at reasonable size | unit | `go test -run TestStateMachine_HistoryCap ./internal/engine/workflow/ -race` | No (new) |
| B27 | doublestar.Match errors logged | unit | `go test -run TestMatchToolName_ErrorHandling ./internal/tools/ -race` | No (new) |
| B28 | Type assertion uses comma-ok | unit | `go test -run TestDNSEvictOldest_TypeAssertion ./internal/tools/search/ -race` | No (new) |
| B29 | Default capabilities cached | unit | `go test -run TestDetectCapabilities_DefaultCached ./internal/integrations/provider/ -race` | No (new) |
| B30 | Stop drains all channels | unit | `go test -run TestStop_DrainsChannels ./internal/tools/ -race` | No (new) |

### Sampling Rate
- **Per task commit:** `make test-fast`
- **Per wave merge:** `make test`
- **Phase gate:** Full suite green (`make check`)

### Wave 0 Gaps
- [ ] B02: New test file `internal/engine/workflow/run_phase_test.go` — transition validation tests
- [ ] B03: New test cases in `internal/tools/permissions_test.go` — deny-wins evaluation
- [ ] B04-B05: New test file `internal/engine/session/manager_race_test.go` — concurrent access tests
- [ ] B06: New test file `internal/core/types/fileutil_fcntl_test.go` — cross-process lock tests
- [ ] B08-B09: New test cases in `internal/ui/tui/handler_stream_test.go` — fallback trigger tests
- [ ] B10-B11: New test cases in `internal/tools/permissions_test.go` — timeout and param matching tests
- [ ] B12-B13: New test cases in `internal/engine/workflow/engine_race_test.go` — checkpoint data race tests
- [ ] B14-B15: New test cases in `internal/engine/workflow/state_machine_test.go` — checkpoint restore tests
- [ ] B16-B17: New test cases in `internal/engine/tokens/estimator_test.go` — ceiling estimation tests
- [ ] B18-B19: New test cases in `internal/core/config/merge_test.go` — zero-value override tests
- [ ] B20: New test cases in `internal/engine/session/checkpoint_test.go` — concurrent save tests
- [ ] B21: New test cases in `internal/integrations/keychain/keychain_test.go` — retry recovery tests
- [ ] B22: New test cases in `internal/integrations/provider/fallback_test.go` — degraded status tests
- [ ] B23: New test cases in `internal/ui/tui/repl_test.go` — resize debounce tests
- [ ] B24: New test cases in `internal/tools/dispatcher_test.go` — collector race tests
- [ ] B25-B26: New test cases in `internal/engine/workflow/state_machine_test.go` — history tests
- [ ] B27: New test cases in `internal/tools/permissions_test.go` — error handling tests
- [ ] B28: New test cases in `internal/tools/search/dns_cache_test.go` — type assertion tests
- [ ] B29: New test cases in `internal/integrations/provider/capabilities_test.go` — caching tests
- [ ] B30: New test cases in `internal/tools/dispatcher_test.go` — channel drain tests

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V4 Access Control | yes | Permission rule evaluation (B03, B10, B11) |
| V5 Input Validation | yes | Config merge correctness (B18, B19) |
| V7 Data Protection | yes | Session persistence integrity (B04, B05, B20) |
| V9 Communication | yes | Provider fallback reliability (B08, B09, B22) |

### Known Threat Patterns for Go Backend

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Data race on shared state | Tampering | sync.Mutex/RWMutex (B04, B07, B12, B13, B24) |
| Permission bypass | Elevation of Privilege | Deny-wins evaluation (B03) |
| Session file corruption | Tampering | fcntl locking (B06), atomic writes (B20) |
| Silent error swallowing | Information Disclosure | Error logging (B27) |
| Unbounded resource growth | Denial of Service | History cap (B26), cache cleanup (B29) |

## Sources

### Primary (HIGH confidence)
- BUGS.md — Complete audit report with file locations, triggers, and impact analysis
- Code review of all 30 bug locations — exact line numbers and surrounding context verified
- `internal/engine/workflow/engine.go` — Engine struct, RunPhase, Transition, Shutdown, checkpoint methods
- `internal/tools/permissions.go` — Permission evaluation, matchAnyParamValue, sendAndWaitForPermission
- `internal/engine/session/manager.go` — LoadWorkflowState, saveSessionAtomic
- `internal/core/types/fileutil.go` — FileLock with flock(2)
- `internal/engine/workflow/state_machine.go` — SetPhase, Transition, history management
- `.planning/codebase/TESTING.md` — Test framework, patterns, mock conventions
- `.planning/codebase/CONVENTIONS.md` — Go coding conventions, concurrency rules

### Secondary (MEDIUM confidence)
- Go stdlib documentation for `fcntl(2)`, `math.Ceil`, `sync.Mutex`
- POSIX specification for record locking

### Tertiary (LOW confidence)
None — all findings verified against codebase

## Metadata

**Confidence breakdown:**
- Standard Stack: HIGH — all fixes use stdlib or existing deps, no new packages needed
- Architecture: HIGH — codebase patterns well understood from existing tests and conventions
- Pitfalls: HIGH — all pitfalls documented in BUGS.md with exact triggers and impacts

**Research date:** 2026-07-23
**Valid until:** 30 days — codebase is stable, Go stdlib APIs don't change
