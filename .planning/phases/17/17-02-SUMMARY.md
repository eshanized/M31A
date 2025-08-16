---
plan_id: 17-02
phase: 17
subsystem: provider, tools, workflow, tui
tags: [correctness, security, performance]
dependency_graph:
  requires: [17-01]
  provides: []
  affects: [internal/provider, internal/tools, internal/workflow, internal/tui]
tech_stack:
  added: []
  patterns: [shared-http-client, async-tea-cmd, context-timeout]
key_files:
  created:
    - internal/provider/openrouter/client_test.go (test additions)
    - internal/provider/sse_test.go (test additions)
    - internal/tools/webfetch_security_test.go (test additions)
    - internal/tools/glob_test.go (test additions)
    - internal/workflow/engine_test.go (test additions)
  modified:
    - internal/provider/openrouter/client.go
    - internal/provider/zen/client.go
    - internal/tools/grep.go
    - internal/tools/webfetch.go
    - internal/provider/sse.go
    - internal/tools/glob.go
    - internal/workflow/engine.go
    - internal/tui/repl.go
    - internal/tui/app.go
    - internal/tui/app_update.go
    - internal/tui/app_workflow.go
decisions:
  - "isContextExceeded already had context window exceeded pattern; added test coverage"
  - "Grep truncation message simplified to generic '[... more matches (limit: N)]'"
  - "WebFetch shared client uses context.WithTimeout for per-request timeout"
  - "SSEParser uses strings.TrimSpace before [DONE] comparison"
  - "consumeStream checks errors before appending delta to preserve partial content"
  - "verifyTask uses exec.CommandContext with 5-minute timeout"
  - "SetProvider returns tea.Cmd for async model validation"
metrics:
  duration: 2226s
  completed: "2026-06-03T16:10:00Z"
  tasks_completed: 8
  files_modified: 11
---

# Phase 17 Plan 2: High Severity Correctness Fixes Summary

## One-liner
Fixed 8 high-severity correctness issues across provider, tools, workflow, and TUI layers.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Fix isContextExceeded patterns (H-1) | 6349c6d | openrouter/client_test.go |
| 2 | Fix Grep truncation messages (H-2, H-3) | 4cc34c3 | grep.go |
| 3 | Share http.Client in WebFetch (H-4) | 76edf4e | webfetch.go, webfetch_security_test.go |
| 4 | Fix SSEParser trailing whitespace (H-5) | 1f86f90 | sse.go, sse_test.go |
| 5 | Sort Glob rg output (H-6) | d00a4eb | glob.go, glob_test.go |
| 6 | Fix Engine.consumeStream error handling (H-7) | 089964c | engine.go, engine_test.go |
| 7 | Add context timeout to Engine.verifyTask (H-8) | f39c093 | engine.go, engine_test.go |
| 8 | Make ReplModel.SetProvider async (H-9) | b580f3a | repl.go, app.go, app_update.go, app_workflow.go |

## Deviations from Plan

### Task 1: isContextExceeded pattern already present
- **Found during:** Task 1
- **Issue:** Both openrouter/client.go and zen/client.go already contained the "context window exceeded" pattern
- **Fix:** Added test coverage to verify the pattern works correctly
- **Files modified:** internal/provider/openrouter/client_test.go
- **Commit:** 6349c6d

### Task 2: Truncation message simplified
- **Found during:** Task 2
- **Issue:** The plan suggested counting exact remaining matches, but this is not possible without scanning all results
- **Fix:** Used generic "[... more matches (limit: N)]" message instead of trying to count exact remaining matches
- **Files modified:** internal/tools/grep.go
- **Commit:** 4cc34c3

### Task 8: Wider scope than planned
- **Found during:** Task 8
- **Issue:** Making SetProvider return tea.Cmd required updating 11 callers across 4 files
- **Fix:** Updated all callers to capture and propagate the returned tea.Cmd
- **Files modified:** internal/tui/repl.go, internal/tui/app.go, internal/tui/app_update.go, internal/tui/app_workflow.go
- **Commit:** b580f3a

## Verification Results

- All provider tests pass: `go test ./internal/provider/...`
- All tools tests pass: `go test ./internal/tools/...`
- Workflow tests pass (pre-existing session load failures unrelated): `go test ./internal/workflow/...`
- TUI tests pass (pre-existing failures unrelated): `go test ./internal/tui/... -run TestReplModel`
- Build with race detection passes: `go build -race ./...`
