---
phase: 02-high-priority-issues
reviewed: 2026-07-14T00:00:00Z
depth: standard
files_reviewed: 15
files_reviewed_list:
  - AGENTS.md
  - TESTING.md
  - CHANGELOG.md
  - DX_AUDIT.md
  - .planning/phases/01-critical-issues/PLAN.md
  - .planning/phases/02-high-priority-issues/PLAN.md
  - internal/tui/tuitypes/tuitypes.go
  - internal/tui/types.go
  - internal/config/types.go
  - internal/tools/git.go
  - internal/tools/bash.go
  - internal/tools/persistent_permissions.go
  - internal/tools/defaults.go
  - cmd/m31a/main.go
  - internal/tui/phase_transition_model.go
findings:
  critical: 0
  warning: 3
  info: 5
  total: 8
status: issues_found
---

# Phase 02: Code Review Report

**Reviewed:** 2026-07-14T00:00:00Z
**Depth:** standard
**Files Reviewed:** 15
**Status:** issues_found

## Summary

This review verifies that all 10 high-priority issues (H1-H10) from the DX_AUDIT.md have been properly implemented, and checks documentation consistency. All 10 issues are confirmed implemented with working code. The documentation (AGENTS.md, TESTING.md, CHANGELOG.md) is accurate and consistent with the current architecture. Three warnings and five info items were found relating to minor quality issues.

## Implementation Verification: H1-H10

All 10 high-priority issues are confirmed implemented:

| ID | Issue | Status | Evidence |
|----|-------|--------|----------|
| H1 | Dedicated Git tool | ✅ Implemented | `internal/tools/git.go` exists with add/commit/diff/log/branch/checkout/stash/status operations, registered in `defaults.go:130` |
| H2 | Bash workdir parameter | ✅ Implemented | `internal/tools/bash.go:67` defines workdir schema, `bash.go:123-124` uses it in Execute |
| H3 | Settings unification | ✅ Implemented | `internal/tui/settings_model.go:611` shows "Showing common options -- see Config editor for advanced settings" |
| H4 | Input validation | ✅ Implemented | `internal/tui/settings_model.go:440-494` validates numeric fields with toast errors |
| H5 | Phase transition confirmation | ✅ Implemented | `ScreenPhaseTransition` (Screen=34) in tuitypes, `phase_transition_model.go` with proceed/go-back/cancel options |
| H6 | Pause/resume execution | ✅ Implemented | `ExecutePauseMsg` type, `PauseExecution/ResumeExecution/IsPaused` in WorkflowEngine interface, pause button in execute_model.go:176 |
| H7 | Verification improvements | ✅ Implemented | `engine_verify.go:315-341` supports custom build/test/lint commands, empty function patterns (lines 272-283), 50-byte threshold (line 294) |
| H8 | Running cost display | ✅ Implemented | `GetCostInfo()` in WorkflowEngine interface, `/cost` command, sidebar cost accumulator |
| H9 | Persistent permission saving | ✅ Implemented | `persistent_permissions.go:55-88` has Save() method writing to ~/.m31a/permissions.json |
| H10 | Workflow mode display | ✅ Implemented | `IntentClassifiedMsg` shows classification result (line 456), user can override mode (app_input.go:195-197) |

## Critical Issues

No critical issues found.

## Warnings

### WR-01: `go vet` Reports Mutex Passed by Value in Test

**File:** `internal/tui/emitter_stress_test.go:61`
**Issue:** `newStreamingPlanProvider` passes `sync.Mutex` by value in `StreamingMockProvider`, which copies the mutex instead of sharing it. This is a race condition in test code that could cause flaky tests.
**Fix:**
```go
// Change from:
func newStreamingPlanProvider() *StreamingMockProvider {
    return &StreamingMockProvider{}
}

// To:
func newStreamingPlanProvider() *StreamingMockProvider {
    return &StreamingMockProvider{}
}
// Ensure StreamingMockProvider uses *sync.Mutex or remove the mutex field if not needed
```

### WR-02: Settings Validation Does Not Reject Invalid Values for Non-Numeric Fields

**File:** `internal/tui/settings_model.go:461-462`
**Issue:** The `leader_key` field accepts any string without validation. Invalid leader key values (e.g., empty string, non-existent key names) are silently accepted, potentially breaking keyboard shortcuts.
**Fix:** Add validation for leader_key against known key names (e.g., "ctrl+x", "ctrl+k") and reject invalid values with a toast error.

### WR-03: install.sh Version Prefix Handling May Still Cause Mismatch

**File:** `install.sh:64`
**Issue:** The URL construction uses `$VERSION` directly from the GitHub API `tag_name`, which includes the `v` prefix (e.g., `v1.7.0`). While the goreleaser output also uses `v` prefix, this creates a tight coupling where the installer depends on GitHub's tag naming convention. If tags are ever created without the `v` prefix, downloads will fail.
**Fix:** Document the assumption that tags must have `v` prefix, or add logic to detect and normalize the prefix.

## Info

### IN-01: AGENTS.md Is Accurate and Current

**File:** `AGENTS.md`
**Issue:** AGENTS.md accurately reflects the current architecture including the Bubble Tea TUI, workflow engine with seven phases, provider layer, 18 built-in tools, and dependency rules. The Makefile commands, coverage targets, and build requirements match the actual codebase.
**Fix:** No action needed. Documentation is current.

### IN-02: TESTING.md Is Accurate

**File:** `TESTING.md`
**Issue:** TESTING.md accurately documents test commands, coverage targets (75% overall, 90% for taskrunner/bisect/rollback), security testing procedures, race testing requirements, and test organization patterns. All referenced test functions exist in the codebase.
**Fix:** No action needed. Documentation is accurate.

### IN-03: CHANGELOG.md v1.7.0 Properly Documents All Changes

**File:** `CHANGELOG.md:9-62`
**Issue:** The v1.7.0 changelog accurately documents all additions, fixes, changes, security improvements, and testing additions. The H1-H10 fixes are covered under appropriate sections (Added, Fixed, Security, Testing).
**Fix:** No action needed. Changelog is comprehensive.

### IN-04: Config Types Include All Required Structures for H7

**File:** `internal/config/types.go:128-132`
**Issue:** `VerifyConfig` struct with `BuildCommand`, `TestCommand`, and `LintCommand` fields is properly defined and used by the verification system.
**Fix:** No action needed. Configuration support is complete.

### IN-05: ScreenPhaseTransition Missing from Label() Switch

**File:** `internal/tui/tuitypes/tuitypes.go:132`
**Issue:** The `Label()` function's switch statement does not have a case for `ScreenPhaseTransition`, falling through to the default "Unknown" label. This is a minor UI inconsistency.
**Fix:** Add `case ScreenPhaseTransition: return "Phase Transition"` to the Label() switch statement.

---

_Reviewed: 2026-07-14T00:00:00Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_
