# M31A Regression Analysis Report

**Date:** 2026-07-15  
**Investigator:** Automated Analysis  
**Scope:** Full codebase regression investigation after audit-driven changes

---

## Executive Summary

This investigation uncovered **1 critical regression** (infinite loop in bisect tests), **2 resource leaks** (goroutine leaks in headless mode), and **several potential issues** related to concurrency, error handling, and lifecycle management. The codebase generally follows good practices but has specific gaps introduced during recent refactoring.

---

## Critical Findings

### 🔴 CRITICAL-01: Bisect Infinite Loop (pkg/bisect/bisect.go)

**Location:** `pkg/bisect/bisect.go`, lines 88-119  
**Impact:** Test hangs indefinitely, blocks CI pipeline  
**Root Cause:** Bisect completion check happens at the **START** of the loop, but `git bisect log` only contains "first bad commit" **AFTER** the final `git bisect good/bad` command completes the bisection.

**Current Logic (Broken):**
```go
for {
    // Check at START - log doesn't have "first bad commit" yet
    logOut, _ := b.run("bisect", "log")
    if strings.Contains(logOut, "first bad commit") {
        break  // Never true on first iteration
    }
    current, _ := b.run("rev-parse", "HEAD")
    if checkFn() {
        b.run("bisect", "good")
    } else {
        b.run("bisect", "bad")
    }
    // Loop repeats - check at START again
}
```

**Expected Flow:**
1. Start bisect, mark good/bad
2. Loop:
   a. Get current commit
   b. Run checkFn
   c. Mark good/bad
   d. **Check if bisect complete** (by examining output of good/bad command OR checking log)
3. Parse result

**Evidence:** Test `TestBisect_Successful` hangs checking the same commit hash repeatedly (observed 1000+ iterations/second).

**Fix:** Check completion **after** marking good/bad, or check the output of `git bisect good/bad` for completion message.

---

## High Severity Findings

### 🟠 HIGH-01: Dispatcher Goroutine Leak in Headless Mode

**Location:** `cmd/m31a/main.go`, `runHeadlessWorkflow()` function  
**Impact:** Two background goroutines (rate limiter tickers) leak on every headless workflow run  
**Root Cause:** `tools.DefaultDispatcher()` starts two `time.NewTicker` goroutines. The headless workflow calls `defer engine.Close()` but **never calls `dispatcher.Stop()`**.

**Code Path:**
```go
// main.go:125
defer engine.Close()  // Only closes decisionLog

// main.go:78
dispatcher, err := tools.DefaultDispatcher(...)
// dispatcher.Start() called internally, starts 2 ticker goroutines

// main.go:170 - returns without stopping dispatcher
return 0
```

**Dispatcher.Start() (internal/tools/dispatcher.go:94-134):**
- Starts `rateTicker` goroutine (line 95-112)
- Starts `dangerousRateTicker` goroutine (line 118-134)
- Both run until `rateDone`/`dangerousRateDone` channels closed

**Fix:** Add `defer dispatcher.Stop()` in `runHeadlessWorkflow()`.

---

### 🟠 HIGH-02: Engine.Shutdown() Doesn't Stop Dispatcher

**Location:** `internal/workflow/engine.go`, `Shutdown()` method (lines 520-540)  
**Impact:** If TUI app crashes during workflow, dispatcher tickers leak  
**Root Cause:** `Engine.Shutdown()` cancels context and waits for `e.done` channel but doesn't stop the dispatcher.

**Current Shutdown:**
```go
func (e *Engine) Shutdown(ctx context.Context) error {
    if e.cancel != nil { e.cancel() }
    if e.running() { select { case <-e.done: case <-ctx.Done(): } }
    e.cache = nil  // Only clears cache
    return nil
}
```

**Missing:** `e.dispatcher.Stop()` call before return.

---

## Medium Severity Findings

### 🟡 MED-01: Engine.Close() Doesn't Stop Dispatcher

**Location:** `internal/workflow/engine.go`, line 510-514  
**Impact:** Inconsistent cleanup between TUI and headless modes  
**Details:** `Engine.Close()` only closes `decisionLog`. The dispatcher is owned by the caller (TUI or main.go) but not documented.

**TUI Cleanup (correct):** `internal/tui/app.go:194-196`
```go
if m.dispatcher != nil {
    m.dispatcher.Stop()
}
```

**Headless Cleanup (incorrect):** Only `engine.Close()` called.

---

### 🟡 MED-02: Context Leak in Compacted Stream Handling

**Location:** `internal/workflow/engine.go`, lines 1047, 1183  
**Impact:** `context.WithTimeout(context.Background(), 60*time.Second)` creates context not tied to workflow lifecycle  
**Details:** Proactive compaction uses `context.Background()` instead of workflow context, meaning compaction continues even after workflow cancellation.

**Code:**
```go
compactCtx, compactCancel := context.WithTimeout(context.Background(), 60*time.Second)
// Should use: context.WithTimeout(ctx, 60*time.Second)
```

---

### 🟡 MED-03: SSE Parser Close Error Handling

**Location:** `internal/provider/sse.go`, lines 120-130  
**Impact:** Response body close errors ignored  
**Code:**
```go
func (p *SSEParser) Close() error {
    if p.resp != nil {
        closeErr = p.resp.Body.Close()  // Error returned but not logged
    }
    // ...
}
```
**Fix:** Log close errors for debugging connection issues.

---

### 🟡 MED-04: Config Watcher Potential Leak

**Location:** `internal/config/loader.go`, `watchConfigPolling()` (lines 1065-1087)  
**Impact:** Ticker goroutine leaks if context not cancelled  
**Status:** Has `defer ticker.Stop()` and `case <-ctx.Done(): return` - **CORRECT**

**However:** The fsnotify watcher path (`watchConfigFSNotify`) should be verified for proper cleanup.

---

### 🟡 MED-05: Subagent Manager Force Cleanup Leaves Goroutine Running

**Location:** `internal/tools/subagent/manager.go`, `forceCleanup()` (lines 348-350)  
**Impact:** Subagent goroutine may continue running after force cleanup  
**Code:**
```go
func (m *Manager) forceCleanup(sa *Subagent) {
    m.agents.Delete(sa.Info.ID)  // Removes from map but doesn't cancel goroutine
}
```
**Fix:** Should send cancellation signal or close channel to stop the subagent goroutine.

---

## Low Severity / Code Quality Findings

### 🟢 LOW-01: Inconsistent Error Wrapping

**Pattern:** Some errors use `fmt.Errorf("%w", err)` others use `fmt.Errorf(": %w", err)`  
**Files:** Various  
**Impact:** Inconsistent error chains, harder debugging

---

### 🟢 LOW-02: Missing Doc Comments on Exported Functions

**Files:** Several internal packages  
**Impact:** go doc / godoc output incomplete  
**AGENTS.md Requirement:** "Exported functions/types need doc comments"

---

### 🟢 LOW-03: Hardcoded Timeouts in Tests

**Files:** `internal/workflow/coverage_boost_test.go`, `internal/workflow/fixes_test.go`  
**Impact:** Flaky tests under load  
**Example:** `context.WithTimeout(context.Background(), 50*time.Millisecond)`

---

### 🟢 LOW-04: Magic Numbers in Rate Limiting

**Location:** `internal/tools/dispatcher.go`  
**Constants:** `ToolRateLimitPerSec`, `DangerousRateLimitPerSec` - not configurable via config.toml

---

## Architecture Compliance Check

| Requirement (AGENTS.md) | Status | Notes |
|------------------------|--------|-------|
| `pkg/` must NOT import `internal/` | ✅ PASS | Verified by `go build` |
| Bubble Tea single-threaded (channels only) | ✅ PASS | TUI uses `tea.Cmd`/`tea.Msg` |
| Provider model lists dynamic | ✅ PASS | Fetched from APIs at runtime |
| API keys via OS keychain | ✅ PASS | `pkg/keychain/` implementations |
| CGO_ENABLED=0 static binary | ✅ PASS | `make build` succeeds |
| Go 1.25+ | ✅ PASS | `go.mod` specifies 1.25.0 |

---

## Test Coverage Gaps

| Package | Coverage Target | Current Status |
|---------|----------------|----------------|
| pkg/taskrunner | 90% | Unknown (tests pass) |
| pkg/bisect | 90% | **BROKEN** - tests hang |
| pkg/rollback | 90% | Unknown (tests pass) |
| internal/tools | 75% | Tests pass |
| internal/workflow | 75% | Tests pass |

**Note:** Full `go test -race ./...` hangs due to bisect test infinite loop.

---

## Recommendations Priority Order

1. **IMMEDIATE:** Fix bisect infinite loop (CRITICAL-01) - blocks CI
2. **IMMEDIATE:** Add `dispatcher.Stop()` to headless workflow (HIGH-01)
3. **HIGH:** Add `dispatcher.Stop()` to `Engine.Shutdown()` (HIGH-02)
4. **HIGH:** Use workflow context for compaction timeouts (MED-02)
5. **MEDIUM:** Fix subagent force cleanup (MED-05)
6. **MEDIUM:** Add SSE parser close error logging (MED-03)
7. **LOW:** Standardize error wrapping
8. **LOW:** Add missing doc comments
9. **LOW:** Make rate limits configurable
10. **LOW:** Fix test timeout flakiness

---

## Verification Commands

```bash
# Build verification
make build

# Quick test (excludes hanging bisect)
go test ./internal/... ./cmd/... ./pkg/taskrunner/... ./pkg/rollback/...

# Race detection (after bisect fix)
go test -race ./...

# Lint
make lint

# Full check
make check
```

---

## Files Modified During Investigation (for reference)

No files were modified during this investigation. All findings are observational.

---

*End of Report*
