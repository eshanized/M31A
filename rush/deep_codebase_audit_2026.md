# M31A Deep Codebase Audit — Unused Components & Flaws

> Generated: 2026-06-04
> Scope: Full codebase analysis of github.com/eshanized/M31A

---

## Table of Contents

1. [Unused Components](#1-unused-components)
2. [Dead Code](#2-dead-code)
3. [Critical Bugs](#3-critical-bugs)
4. [Logic Flaws](#4-logic-flaws)
5. [Design Issues](#5-design-issues)
6. [Configuration Inconsistencies](#6-configuration-inconsistencies)
7. [Thread Safety Concerns](#7-thread-safety-concerns)
8. [Summary Statistics](#8-summary-statistics)

---

## 1. Unused Components

### 1.1 `components/sparkline.go` — Entirely Unused

**Location:** `internal/tui/components/sparkline.go` (161 lines)

The `Sparkline` struct and its `Render()` method are defined but **never instantiated or referenced** from any TUI screen, view, or component. Zero references exist outside the file itself.

**Recommendation:** Remove the file or integrate into a metrics dashboard view.

---

### 1.2 `components/filterchips.go` — Entirely Unused

**Location:** `internal/tui/components/filterchips.go` (128 lines)

The `FilterChips` struct, `Render()`, `ActiveCount()`, and `ToggleChip()` are defined but **never used** outside the file. The `FilterChipsModel` wrapper (lines 97-128) is also unused.

**Recommendation:** Remove or integrate into a list/filter view.

---

### 1.3 `AutoDreamThreshold` Constant — Defined but Never Used

**Location:** `internal/types/constants.go:13`

```go
AutoDreamThreshold = 0.60
```

This constant is declared but **never referenced** anywhere in the codebase. The autodream `Consolidate()` method has no threshold check — it consolidates the oldest 50% of candidates unconditionally. The config type doesn't even have a field for this threshold.

**Recommendation:** Either wire it into the autodream logic (e.g., trigger consolidation when context usage exceeds this ratio) or remove it.

---

### 1.4 `ShowThinkingByDefault` — Configurable but Not Wired

**Location:** `internal/config/types.go:37`

The field is:
- Defined in `ModelConfig`
- Validated in `loader.go`
- Editable in the Settings UI (`settings.go:126, 371`)

But it is **never read** by any rendering logic. The thinking visibility is controlled per-message via the `ToolCard` collapsed state and the REPL view, not by this config flag.

**Recommendation:** Wire it into the REPL or thinking component rendering, or remove the setting.

---

### 1.5 `AutoBackup` — Configurable but Not Functional

**Location:** `internal/config/types.go:86`

The `AutoBackup` boolean is:
- Defined in `FeaturesConfig`
- Editable in the Settings UI (`settings.go:146, 389`)

But **no backup logic** exists anywhere in the codebase. Toggling this setting has no effect.

**Recommendation:** Implement backup functionality or remove the config option.

---

### 1.6 `ResumeOnStartup` — Configurable but Not Functional

**Location:** `internal/config/types.go:87`

The `ResumeOnStartup` boolean is:
- Defined in `FeaturesConfig`
- Editable in the Settings UI (`settings.go:147, 391`)

But **no startup resume logic** reads this flag. The resume screen (`resume.go`) is only accessible via the `/resume` command, not automatically on startup.

**Recommendation:** Wire it into app initialization or remove the setting.

---

## 2. Dead Code

### 2.1 `FindFallbackWithRetryAfter` — Never Called

**Location:** `internal/provider/fallback.go:62-74`

```go
func FindFallbackWithRetryAfter(registry *Registry, currentProvider string, retryAfterHeader string) ...
```

This exported function is **never called** from any production code or test. It calls `time.Sleep()` which would block the Bubble Tea event loop if used in the TUI. It also wraps `FindFallbackProvider` which IS used.

**Recommendation:** Remove or wire into the provider error handling path in `app_update.go`.

---

### 2.2 `IsRateLimited` and `GetRetryAfter` — Test-Only

**Location:** `internal/provider/fallback.go:77-87`

Both exported functions are only referenced in `resilience_test.go`. No production code calls them. They are designed to work with `FindFallbackWithRetryAfter` which is itself unused.

**Recommendation:** Remove these or integrate them into the provider error handling flow.

---

### 2.3 `Rollback.SafeReset` — Test-Only Method

**Location:** `pkg/rollback/rollback.go:168-195`

`SafeReset` performs a hard reset followed by a stash pop to preserve uncommitted changes. It is **only called in tests** (`rollback_test.go`). The TUI `/rollback` command uses `HardReset` and `SoftReset` but never `SafeReset`.

**Recommendation:** Expose via the `/rollback` command (e.g., `/rollback --safe <hash>`) or remove.

---

### 2.4 `Rollback.Preview` — Test-Only Method

**Location:** `pkg/rollback/rollback.go:94-105`

`Preview` returns a diff between a commit and HEAD. Only called in `rollback_test.go`. The TUI never shows a rollback preview.

**Recommendation:** Integrate into `/rollback` command (e.g., `/rollback --preview <hash>`) or remove.

---

### 2.5 `Rollback.CurrentHead` — Test-Only Method

**Location:** `pkg/rollback/rollback.go:88-90`

Only called in tests. Production code uses `git.HeadHash()` directly.

---

### 2.6 Autodream Methods — Test-Only

The following `Consolidator` methods are **only called from tests**, never from production code:

| Method | Location |
|--------|----------|
| `Pause()` | `autodream.go:208` |
| `Resume()` | `autodream.go:215` |
| `IsPaused()` | `autodream.go:222` |
| `CanConsolidate()` | `autodream.go:99` |
| `Stats()` | `autodream.go:244` |

Production code only calls `Consolidate()`, `SetMessages()`, and `Messages()`. The pause/resume and stats reporting features are fully implemented but inaccessible to users.

**Recommendation:** Wire `CanConsolidate()` into the REPL `/compress` command to check before consolidating. Expose `Stats()` via a status command.

---

## 3. Critical Bugs

### 3.1 Control Flow Bug in `verify.go` — Bisect Runs Inside Wrong Block

**Location:** `internal/workflow/verify.go:55-103`

**Severity: CRITICAL**

The bisect logic (lines 60-102) is incorrectly placed **inside** the `if task.HealsAttempted >= MaxHealAttempts` block instead of in an `else` block. The indentation on line 59-60 reveals the intent (a comment says "Trigger bisect") but the braces don't match.

**Current structure:**
```
if HealsAttempted >= MaxHealAttempts {
    tasks[i].Status = Unrecoverable     // Line 56
    // [line 58 closes some inner block]
    // Bisect code runs here (lines 60-102)
    continue                            // Line 102
}
// Self-heal code (lines 106-137)
```

**Impact:**
1. When a task has **exhausted** heal attempts: It's marked Unrecoverable, then bisect runs unnecessarily (bisect's heal will also fail since heals are exhausted), then `continue` skips to next task.
2. The `StatusUnrecoverable` assignment on line 56 is inside the same block as bisect — if bisect succeeds, the task gets overwritten back to `StatusDone` on line 91. This creates contradictory state transitions.
3. Line 56 sets `StatusUnrecoverable` but the subsequent code on lines 71-102 may set it back to `StatusDone`, making the Unrecoverable assignment dead code in the success path.

**Fix:** Move lines 59-102 into an `else` block, or restructure the logic so bisect only runs when heals remain.

---

### 3.2 Session ID Length Mismatch — Config vs Validation

**Location:** `pkg/session/session.go:70` vs `internal/tui/app.go:188-193`

**Severity: HIGH**

`validateSessionID()` uses the hardcoded constant `types.SessionIDLength` (8) to validate session IDs:

```go
// session.go:70
if len(id) != types.SessionIDLength { // Always checks for 8
```

But session ID generation in `app.go` uses the configurable `cfg.Features.SessionIDLength`:

```go
// app.go:190
sessionIDBytes = cfg.Features.SessionIDLength / 2  // Config / 2 bytes
```

If a user sets `session_id_length = 10`:
- Generation: 5 bytes → 10 hex chars
- Validation: Expects exactly 8 chars → **all sessions fail validation**

**Fix:** `validateSessionID` should accept the configured length, not the hardcoded constant.

---

## 4. Logic Flaws

### 4.1 `FindFallbackWithRetryAfter` Blocks Event Loop

**Location:** `internal/provider/fallback.go:69`

```go
time.Sleep(wait) // Up to 60 seconds
```

If this function were wired into the TUI, `time.Sleep` would block the Bubble Tea single-threaded event loop for up to 60 seconds. The AGENTS.md explicitly states: "Bubble Tea is single-threaded. ALL state mutations go through Update() only."

**Fix:** Convert to a `tea.Cmd` that returns after the wait, or use a timer channel.

---

### 4.2 Bisect Bypasses Git Wrapper

**Location:** `pkg/bisect/bisect.go:39` and `internal/workflow/verify.go:189`

Both `bisect.Run()` and `findRootCommit()` use `exec.Command("git", ...)` directly instead of the project's `internal/git` wrapper. This bypasses:
- Error wrapping and typed errors
- Logging
- The testability benefits of the wrapper

The project has a dedicated `internal/git/git.go` wrapper but these two functions circumvent it.

---

### 4.3 Autodream `Consolidate()` Result Not Applied to REPL

**Location:** `internal/tui/commands_ai.go:56`

```go
result := ctx.AutoDream.Consolidate()
```

The `Consolidate()` method modifies the Consolidator's internal message list (removes old messages, adds summary). But the REPL model (`ReplModel`) maintains its **own** separate message list. After consolidation, `SetMessages(nil)` is called (line 32) which clears the Consolidator's messages, but the REPL's messages remain unchanged.

The consolidation result (removed messages, saved tokens) is reported to the user but the actual REPL message history is never modified.

**Fix:** After consolidation, update the REPL's message list with the Consolidator's modified messages.

---

### 4.4 `FindFallbackProvider` Health Check Doesn't Respect Context

**Location:** `internal/provider/fallback.go:34-36`

```go
ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
status := p.HealthCheck(ctx)
cancel()
```

The context is created with a 10-second timeout, but `cancel()` is called immediately after `HealthCheck` returns, not deferred. If `HealthCheck` is synchronous, this is fine. But if the provider's `HealthCheck` spawns goroutines that check the context, the premature cancel could cause incorrect "unhealthy" results for otherwise healthy providers.

---

### 4.5 Rollback `Chain` Ignores Diff Errors

**Location:** `pkg/rollback/rollback.go:74`

```go
diff, _ = r.git.Diff(c.Hash, "HEAD")
```

The diff error is silently discarded. If the diff fails (e.g., commit no longer exists), the `RollbackEntry.Diff` field will be empty with no indication of failure.

---

## 5. Design Issues

### 5.1 Duplicate Type Definitions

Both `internal/types` and `pkg/bisect` define `CommitInfo`:

| Type | Location |
|------|----------|
| `git.CommitInfo` | `internal/git/git.go` |
| `bisect.CommitInfo` | `pkg/bisect/bisect.go:13-18` |

The bisect package defines its own `CommitInfo` with the same fields but doesn't reuse the git package's type. This creates unnecessary type conversion at the boundary.

---

### 5.2 Consolidator `consolidatingInt` Field Naming

**Location:** `pkg/autodream/autodream.go:41`

```go
consolidatingInt int32 // atomic: 0=idle, 1=consolidating
```

The field is documented as atomic and accessed via `atomic.CompareAndSwapInt32`, but the name `consolidatingInt` is confusing. Standard Go convention would be `consolidating` with a `// atomic` comment or using `atomic.Bool` (Go 1.19+).

---

### 5.3 No Config Default for `ContextWarningThreshold` in Loader

**Location:** `internal/config/loader.go`

The `DefaultConfig()` function sets defaults for `SidebarWidthThreshold` (120) but not for `ContextWarningThreshold` (0.80) or `MaxIterations`. These rely on Go's zero-value behavior, meaning:
- `ContextWarningThreshold` defaults to 0.0 (not 0.80)
- `MaxIterations` defaults to 0 (not any meaningful limit)

The constants in `types/constants.go` define `ContextWarningThreshold = 0.80` but this constant is never used as a fallback in the loader.

---

### 5.4 `parseQuestions` Not Defined in Discuss Phase

**Location:** `internal/workflow/discuss.go:71`

```go
questions := parseQuestions(fullContent)
```

The `parseQuestions` function is called but not visible in the discuss.go file. If it's in another file, the parsing logic for extracting numbered questions from LLM output is a fragile text-processing operation that should be well-tested.

---

### 5.5 Hardcoded `sessionIDLength` in Validation

The constant `types.SessionIDLength = 8` is used for validation in `session.go` but the config allows values 4-16. The constant name implies it's a configurable length but it's actually a hardcoded validation value. This is a naming/semantics mismatch.

---

## 6. Configuration Inconsistencies

| Config Field | Editable in Settings | Used in Production | Status |
|-------------|---------------------|-------------------|--------|
| `model.show_thinking_by_default` | Yes | No | **Dead config** |
| `features.auto_backup` | Yes | No | **Dead config** |
| `features.resume_on_startup` | Yes | No | **Dead config** |
| `model.token_ema_alpha` | No | Yes | OK (config only) |
| `types.AutoDreamThreshold` | No | No | **Dead constant** |
| `features.session_id_length` | No | Partially | **Bug** (validation mismatch) |

---

## 7. Thread Safety Concerns

### 7.1 Autodream Consolidator Cross-Goroutine Access

**Location:** `internal/tui/app_update.go:311`

```go
m.autoDream.SetMessages(m.replModel.Messages())
```

The Consolidator uses `sync.RWMutex` and `atomic` operations, suggesting it's designed for concurrent access. However, Bubble Tea is single-threaded. The mutex overhead is unnecessary overhead in the TUI context, but the atomic CAS for reentrancy guard is appropriate for the `/compress` command.

### 7.2 `FindFallbackProvider` Creates Unmanaged Contexts

Each call to `FindFallbackProvider` creates a new `context.WithTimeout` without proper cleanup in the error path. If `HealthCheck` panics, `cancel()` is never called, leaking the context.

**Fix:** Use `defer cancel()`.

---

## 8. Summary Statistics

### Unused Components: 6

| Component | Lines of Dead Code |
|-----------|-------------------|
| `components/sparkline.go` | 161 |
| `components/filterchips.go` | 128 |
| `AutoDreamThreshold` constant | 1 |
| `ShowThinkingByDefault` (logic) | ~0 (config exists, no logic) |
| `AutoBackup` (logic) | ~0 |
| `ResumeOnStartup` (logic) | ~0 |

### Dead Code (Test-Only Functions): 8

| Function | Location | Lines |
|----------|----------|-------|
| `FindFallbackWithRetryAfter` | `fallback.go:62` | 13 |
| `IsRateLimited` | `fallback.go:77` | 3 |
| `GetRetryAfter` | `fallback.go:82` | 6 |
| `Rollback.SafeReset` | `rollback.go:168` | 28 |
| `Rollback.Preview` | `rollback.go:94` | 12 |
| `Rollback.CurrentHead` | `rollback.go:88` | 3 |
| `Consolidator.Pause/Resume/IsPaused` | `autodream.go` | 18 |
| `Consolidator.CanConsolidate/Stats` | `autodream.go` | 26 |

### Critical Bugs: 2

1. `verify.go` control flow bug (bisect in wrong block)
2. Session ID length validation mismatch (hardcoded vs configurable)

### Logic Flaws: 5

1. `FindFallbackWithRetryAfter` blocks event loop
2. Bisect bypasses git wrapper
3. Autodream consolidation not applied to REPL
4. `FindFallbackProvider` context handling
5. Rollback `Chain` ignores diff errors

### Design Issues: 5

1. Duplicate `CommitInfo` types
2. Poor naming of `consolidatingInt`
3. Missing config defaults in loader
4. Fragile question parsing
5. Misleading constant name `SessionIDLength`

### Total Estimated Dead Code: ~290+ lines (excluding tests)
