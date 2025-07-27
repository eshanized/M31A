---
phase: 19
plan: 04
subsystem: session, config, testing, dependencies
tags:
  - session-cleanup
  - config-hot-reload
  - test-coverage
  - dependency-pinning
dependency_graph:
  requires:
    - 19-01
    - 19-02
  provides:
    - session auto-cleanup
    - config hot-reload
    - critical path test coverage
  affects:
    - pkg/session/manager.go
    - internal/config/loader.go
    - internal/tui/app.go
    - internal/tui/app_update.go
    - go.mod
tech_stack:
  added:
    - time.Ticker for config polling
  patterns:
    - polling-based file watcher
    - startup cleanup hook
key_files:
  created:
    - internal/tui/app_update_test.go
    - internal/tui/repl_stream_test.go
  modified:
    - pkg/session/manager.go
    - pkg/session/manager_test.go
    - internal/config/loader.go
    - internal/tui/app.go
    - internal/tui/app_update.go
    - internal/workflow/ship_test.go
    - internal/workflow/execute_test.go
    - go.mod
decisions:
  - "Session cleanup uses os.RemoveAll with 30-day retention, non-blocking on failure"
  - "Config watcher uses polling (5s) for cross-platform compatibility"
  - "Config reload only updates non-provider fields (UI, Permissions, Features, Ledger)"
  - "DEP comments added as plain Go comments in go.mod require block syntax"
metrics:
  duration: "task 1: ~15min, task 2: ~20min"
  completed: "2026-06-04"
  tasks_completed: 2
  files_created: 2
  files_modified: 8
---

# Phase 19 Plan 04: Session Cleanup, Config Hot-Reload & Test Coverage Summary

Session auto-cleanup on startup, config hot-reload via polling watcher, critical message handler and stream segment tests, git error path tests, and dependency version pinning comments.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] Fixed duplicate test function names**
- **Found during:** Task 2
- **Issue:** `TestReplModel_AppendStreamChunk`, `TestReplModel_HandleStreamDoneMsg`, and `TestReplModel_HandleStreamErrorMsg` conflicted with existing functions in `app_test.go` and `streaming_test.go`
- **Fix:** Renamed to `_Basic` variants to avoid redeclaration errors
- **Files modified:** `internal/tui/repl_stream_test.go`
- **Commit:** edad5c0

**2. [Rule 1 - Bug] Fixed missing `m31errors` import in repl_stream_test.go**
- **Found during:** Task 2
- **Issue:** Used `m31errors.ErrContextExceeded` etc. in `TestRenderErrorBanner` and `TestTypedErrorName` but didn't import the package
- **Fix:** Added `m31errors "github.com/eshanized/M31A/internal/errors"` import
- **Files modified:** `internal/tui/repl_stream_test.go`
- **Commit:** edad5c0

**3. [Rule 1 - Bug] Fixed `AutodreamEnabled` field reference that doesn't exist**
- **Found during:** Task 2
- **Issue:** `config.FeaturesConfig` has no `AutodreamEnabled` field; used `AutoBackup` instead
- **Fix:** Changed test to use `AutoBackup: true`
- **Files modified:** `internal/tui/app_update_test.go`
- **Commit:** edad5c0

### Plan Adjustments

**TEST-5 and TEST-6 deferred per plan specification:**
- TEST-5 (repl_quickactions.go tests): Skipped — low priority, UI convenience only
- TEST-6 (OS-specific keychain tests): Skipped — compile-tag guards prevent cross-platform testing

## Implementation Details

### Task 1: FEAT-2 Session Cleanup + FEAT-3 Config Hot-Reload + DEP Pinning

**FEAT-2 — Session auto-cleanup:**
- Added `Cleanup(maxAge time.Duration) (int, error)` to `pkg/session/manager.go`
- Removes session directories older than maxAge using `os.RemoveAll`
- Skips `archived/` directory and non-directory entries
- Logs warnings on removal failures (non-blocking)
- Called on startup from `NewApp` with 30-day retention

**FEAT-3 — Config hot-reload:**
- Added `ConfigReloadMsg` type and `WatchConfig` function to `internal/config/loader.go`
- Polls config file every 5 seconds using `time.Ticker`
- Emits `ConfigReloadMsg` when file modtime changes
- Handler in `app_update.go` updates only non-provider config fields (UI, Permissions, Features, Ledger)

**DEP-1/2/3 — Version pinning:**
- Added comments to `go.mod` for `golang.org/x/sync` (DEP-1), `doublestar` (DEP-2), `BurntSushi/toml` (DEP-3)

### Task 2: TEST-1 through TEST-4, TEST-7 Test Coverage

**TEST-1/2/3 — app_update_test.go (12 tests):**
- `TestApp_Update_ErrorMsg` — error sets currentOperation
- `TestApp_Update_ErrorMsg_SentinelError` — sentinel errors produce user-friendly messages
- `TestApp_Update_HealthCheckResultMsg` — health status stored, next tick scheduled
- `TestApp_Update_HealthCheckResultMsg_InvalidatedHeaderCache` — header cache invalidation
- `TestApp_Update_PermissionRequestMsg` — permission handler invoked
- `TestApp_Update_ConfigReloadMsg_Success` — config fields updated on reload
- `TestApp_Update_ConfigReloadMsg_Error` — reload errors leave config unchanged
- `TestApp_Update_ConfigReloadMsg_NilConfig` — nil config doesn't panic
- `TestApp_Update_FallbackEventMsg` — fallback banner set
- `TestApp_Update_ToastMsg` — toast scheduling
- `TestApp_Update_ToastExpiryMsg` — toast cleared on expiry
- `TestApp_Update_StreamChunkMsg_NilReplModel` — no panic with nil replModel
- `TestApp_Update_ThemeChangedMsg` — theme change handling

**TEST-4 — repl_stream_test.go (18 tests):**
- Stream chunk appending (basic, nil, empty delta)
- Segment building (thinking, content, empty, multiple)
- Stream done message (basic, usage, thinking blocks)
- Stream error message (basic, state reset)
- Stream message handling (content chunk, thinking chunk, segment transitions, nil chunk, done chunk)
- Error banner rendering and sentinel error name extraction
- Tool call extraction from segments

**TEST-7 — workflow git error paths:**
- `TestEngine_RunShip_NoGit` — ship succeeds without git
- `TestEngine_RunShip_WithFailedTasks` — ship with failed tasks returns error but succeeds
- `TestEngine_RunShip_WithSkippedTasks` — ship with skipped tasks succeeds
- `TestEngine_BuildSummary_Empty` — summary with no tasks
- `TestEngine_RunExecute_ContextCancellation` — context cancellation handled gracefully
- `TestEngine_ExecuteTaskWithTools_EmptyResponse` — empty LLM response doesn't crash
- `TestEngine_ExecuteTaskWithTools_MultipleToolCalls` — multiple tool calls handled

## Verification

- `go build ./...` — passes
- `go vet ./...` — passes
- `go test -race -count=1 ./...` — full suite passes (22 packages)

## Self-Check: PASSED

All files created/modified exist. Both commits verified in git log.
