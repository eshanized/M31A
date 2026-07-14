---
phase: 02-code-review-command
reviewed: 2026-07-14T00:00:00Z
depth: standard
files_reviewed: 7
files_reviewed_list:
  - internal/workflow/engine_verify.go
  - internal/tools/persistent_permissions.go
  - internal/tools/permissions.go
  - internal/workflow/engine.go
  - internal/tui/sidebar_model.go
  - internal/tools/defaults.go
  - internal/tui/app_update_phase.go
findings:
  critical: 1
  warning: 5
  info: 3
  total: 9
status: issues_found
---

# Phase 02: Code Review Report

**Reviewed:** 2026-07-14T00:00:00Z
**Depth:** standard
**Files Reviewed:** 7
**Status:** issues_found

## Summary

Reviewed 7 files covering verification checks, persistent permissions, running cost/time display, sidebar cost display, tool registration, and workflow mode determination. Found 1 critical issue (duplicate persistent permission loading causing rule duplication), 5 warnings (including a dead-code method that undermines smart truncation, missing mutex on engine field access, silently swallowed JSON unmarshal error, and incomplete workflow mode overridability), and 3 informational items.

## Critical Issues

### CR-01: Duplicate persistent permissions loading appends rules twice

**File:** `internal/tools/defaults.go:15-46`
**Issue:** Persistent permissions are loaded twice during `DefaultDispatcher` initialization. First at lines 15-21 via `d.persistentPerms.Load(workDir)` which appends to `d.rules`. Then again at lines 41-46 via a *second* `NewPersistentPermissions()` instance that creates a new `pp` variable, calls `pp.Load(workDir)`, and appends the same rules again under the mutex. This causes every persisted permission rule to appear twice in the rules slice, potentially causing duplicate auto-approvals or double-matching in the last-match-wins evaluation. The second load also acquires `d.mu.Lock()` while the first does not, creating an inconsistency.
**Fix:**
```go
// Remove the first load block (lines 15-21):
// if d.persistentPerms != nil {
//     persistentRules := d.persistentPerms.Load(workDir)
//     if len(persistentRules) > 0 {
//         d.rules = append(d.rules, persistentRules...)
//     }
// }

// Keep only the second load block (lines 41-46), which properly uses the mutex:
pp := NewPersistentPermissions()
if saved := pp.Load(workDir); len(saved) > 0 {
    d.mu.Lock()
    d.rules = append(d.rules, saved...)
    d.mu.Unlock()
}
```
Alternatively, since `d.persistentPerms` is already initialized in `NewDispatcher`, consolidate to a single load using `d.persistentPerms` with proper locking.

## Warnings

### WR-01: `descriptionKeywords()` always returns nil — smart truncation is a no-op

**File:** `internal/workflow/engine_verify.go:85-89`
**Issue:** The `descriptionKeywords()` method unconditionally returns `nil`. It is called at line 57 and the result is passed to `findRelevantFunction`, but the keywords parameter is never used for matching inside `findRelevantFunction` (lines 94-115) — the function simply finds the first function declaration after the header. This makes the "smart truncation" feature misleading: the doc comment says it "attempts to find the function most relevant to the task description" but it just finds the *first* function, not the *relevant* one. The entire keywords parameter chain is dead code.
**Fix:** Either implement actual keyword matching in `findRelevantFunction` (e.g., check if the function name contains keywords from the task description), or simplify the code by removing the keywords parameter and documenting that the heuristic finds the first function declaration.

### WR-02: `engine.workflowMode` read without mutex in `RunPhase`

**File:** `internal/workflow/engine.go:876`
**Issue:** `e.workflowMode` is read directly at line 876 (`result.WorkflowMode = e.workflowMode`) without acquiring `e.workflowModeMu.RLock()`. Meanwhile, `SetWorkflowMode` (line 364-368) writes to `e.workflowMode` under `e.workflowModeMu.Lock()`. In the Bubble Tea architecture, `SetWorkflowMode` is called from `runWorkflowFromGoal` (TUI goroutine) while `RunPhase` executes in the workflow goroutine. These can race if the TUI sets the mode while a phase is running. The field is also read at line 35 (`e.workflowMode` in execute.go) without the lock.
**Fix:** Wrap reads of `e.workflowMode` with `e.workflowModeMu.RLock()` / `e.workflowModeMu.RUnlock()`, or use a snapshot value captured before the phase starts.

### WR-03: Persistent permissions Save silently discards corrupt JSON on re-read

**File:** `internal/tools/persistent_permissions.go:68-71`
**Issue:** When `Save` reads existing data, the `json.Unmarshal` error is silently discarded (`_ = json.Unmarshal(data, &pd)`). If the permissions file becomes corrupt (partial write, disk error), the unmarshal fails, `pd` remains at its zero value (nil Projects map), and the subsequent write at line 87 overwrites the file with only the new project's rules — **silently deleting all other projects' permissions**. This is data loss risk on file corruption.
**Fix:**
```go
data, err := os.ReadFile(p.path)
if err == nil {
    if unmarshalErr := json.Unmarshal(data, &pd); unmarshalErr != nil {
        slog.Warn("corrupt permissions file, starting fresh", "error", unmarshalErr, "path", p.path)
        // pd is zero-valued; existing data is lost but at least we log it
    }
}
```
Consider backing up the corrupt file before overwriting.

### WR-04: Workflow mode not fully overridable via UI

**File:** `internal/tui/app_session.go:88-129`
**Issue:** The workflow mode can be overridden via config (`config.Features.WorkflowMode`) and the `/direct` command, but there is no `/mode` command or UI toggle to switch between `full`, `fast`, and `direct` modes interactively. The user sees a toast notification about the adaptive mode (line 68-69) but has no way to override it mid-session without restarting the workflow. The config override is also only checked once at `resolveWorkflowMode` call time — changing config at runtime has no effect.
**Fix:** Consider adding a `/mode full|fast|direct` slash command that calls `m.workflowEngine.SetWorkflowMode(mode)` and `m.workflowMode = mode` directly, similar to how `/direct` works.

### WR-05: `nextPhaseForMode` silently terminates on unknown mode

**File:** `internal/tui/app_update_phase.go:17-53`
**Issue:** The `nextPhaseForMode` function has a fallback at line 52: `return types.PhaseIdle, false`. If `ModeAuto` is passed (which is a valid `WorkflowMode` constant defined in types.go:45), the function falls through all switch cases without matching and returns `PhaseIdle` with `false`, silently terminating the workflow. `ModeAuto` is the default value and could be passed if `resolveWorkflowMode` returns it before classification completes.
**Fix:** Add an explicit case for `ModeAuto` that either classifies at runtime or defaults to `ModeFull`:
```go
case types.PhaseInitialize:
    if mode == types.ModeDirect || mode == types.ModeAuto {
        return types.PhaseExecute, true
    }
    return types.PhaseDiscuss, true
```
Or treat `ModeAuto` as equivalent to `ModeFull` at this level.

## Info

### IN-01: `persistent_permissions.go` Load silently returns nil on missing file

**File:** `internal/tools/persistent_permissions.go:36-51`
**Issue:** `Load` returns `nil` for any error (file not found, permission denied, corrupt JSON) without logging. While `nil` is handled gracefully by callers (they check `len(rules) > 0`), the silent failure makes debugging harder when permissions aren't loading as expected.
**Fix:** Log warnings for unexpected errors (e.g., permission denied) while continuing to return nil for "file not found" which is the expected case on first run.

### IN-02: `isConfigFile` has an incomplete allowlist

**File:** `internal/workflow/engine_verify.go:506-517`
**Issue:** The `isConfigFile` function exempts common config files from the "suspiciously small file" check (50-byte threshold). The list is incomplete — it doesn't include `.toml`, `.yaml`, `.json`, `.env.example`, or other common small config files. A 30-byte `.env.example` would trigger the "file appears incomplete" warning.
**Fix:** Consider adding `.env.example`, `Makefile`, `.toml`, `.yaml`, `.yml`, and `.json` to the config file list, or increase the threshold for files with recognized config extensions.

### IN-03: Sidebar `UpdateTaskProgress` fallback uses `task.ID` as total count

**File:** `internal/tui/sidebar_model.go:289-291`
**Issue:** When no todo items exist yet, `UpdateTaskProgress` falls back to `s.taskProgress.Total = task.ID` (line 290). This uses the task's numeric ID (1-based) as the total task count, which is semantically incorrect — a task with ID=3 would set total to 3 even if there are only 2 tasks. This fallback is only reached when `s.taskProgress.Total == 0` (no todo items initialized yet).
**Fix:** Remove this fallback or use the task count from the task list instead of the task ID. If the intent is to show progress before todo items are populated, the caller should initialize `InitTaskProgress` with the correct total first.

---

_Reviewed: 2026-07-14T00:00:00Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_
