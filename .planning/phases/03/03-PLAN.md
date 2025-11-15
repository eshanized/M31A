---
phase: 03
phase_name: Message Rendering Pipeline
plan: 03-01
type: PLAN
wave: 1
depends_on: []
files_modified:
  - internal/tui/app_channel.go
  - internal/tui/backup.go
  - internal/tui/backup_test.go
  - internal/tui/repl_quickactions.go
  - internal/tui/repl_thinking.go
  - internal/tui/repl_stream.go
  - internal/tui/repl.go
  - internal/tui/repl_welcome.go
  - internal/tui/app_view.go
autonomous: true
requirements:
  - "#5"
  - "#6"
  - "#7"
  - "#14"
created_at: 2026-06-08
---

# Phase 3 — Message Rendering Pipeline

## Objective

Clean up unused functions discovered by staticcheck (53 U1000 warnings) and fix wiring issues in the TUI rendering pipeline. Remove dead code, delete orphaned files, and verify all existing rendering features (streaming, thinking blocks, tool cards, header, status bar) still work correctly.

## Background

Phase 3 was previously listed as completed in STATE.md but was never tracked in the GSD planning system. The rendering pipeline was built incrementally across multiple phases. As a result, 53 unused functions accumulated — mostly view helpers that were replaced by component-based rendering but never removed.

## Tasks (Wave 1 — Core Cleanup)

### Task 03-01-01: Delete orphaned files
**Description:** Delete entire files that have zero callers
**Files:**
- `internal/tui/app_channel.go` — entire file is dead (channelCloser, channelEmitter)
- `internal/tui/backup.go` — entire file is dead (backupCurrentSessionAsync, copyDir)
- `internal/tui/backup_test.go` — test for deleted code
- `internal/tui/repl_quickactions.go` — entire file is dead (renderQuickActionsPanel)
**Action:** `rm` files
**Verification:** `go build ./internal/tui/...` passes

### Task 03-01-02: Remove unused functions from repl_*.go
**Description:** Remove dead functions from REPL-related files
**Files & Functions:**
- `repl_thinking.go:13` — `renderThinkingToggleHint`
- `repl_stream.go:215` — `streamTickCmds`
- `repl.go` — `renderQuickActions`
- `repl_welcome.go` — `renderBottomBar`
**Action:** Delete each function definition
**Verification:** `go vet ./internal/tui/...` passes

### Task 03-01-03: Remove unused functions from app_view.go
**Description:** Remove dead rendering functions
**Files & Functions:**
- `app_view.go` — `renderHeader`, `errorf`, `simpleError` type
**Action:** Delete each function/type
**Verification:** `go vet ./internal/tui/...` passes

## Tasks (Wave 2 — Extended Cleanup)

### Task 03-02-01: Remove unused helpers
**Files & Functions:**
- `helpers.go` — `ensureSidebarModel`, `propagateSessionID`, `applySessionRestored`, `renderSectionHeader`
- `sidebar.go` — `refreshCmd`, `fileStatusIcon`
- `header.go` — `formatDurationMs`
**Action:** Delete each function
**Verification:** `go vet ./internal/tui/...` passes

### Task 03-02-02: Remove unused view functions
**Files & Functions:**
- `execute_view.go` — `renderProgressBar`, `animatedProgressBarWidth`, `renderTaskSpinner`
- `plan_view.go` — `renderPlanHeader`
- `ship_view.go` — `renderShipStatsGrid`
- `settings_view.go` — `renderSettingCard`, `maskedKey`
**Action:** Delete each function
**Verification:** `go vet ./internal/tui/...` passes

### Task 03-02-03: Remove unused component code
**Files & Functions:**
- `components/truncate.go` — `truncateMiddle`
- `transition.go` — `transitionOverlayWidth` constant
- `workflow/engine_verify.go` — `verifyTaskContext`
**Action:** Delete each function/constant
**Verification:** `go vet ./...` passes

## Tasks (Wave 3 — Verification)

### Task 03-03-01: Full build & lint verification
**Action:** Run `go build ./...` and `golangci-lint run ./...`
**Expected:** 0 U1000 warnings remaining (53 → 0)
**Verification:**
```bash
go build ./...
golangci-lint run ./... 2>&1 | grep -c "U1000" || echo "0 U1000"
```

### Task 03-03-02: Test suite
**Action:** Run `go test -race -count=1 ./...`
**Expected:** All existing tests pass
**Verification:**
```bash
go test -race -count=1 ./...
```

## Verification

### Per-Task Checks
1. Each `rm`/delete is followed by `go build ./...` to catch breakage
2. `go vet ./...` is clean at the end of each wave
3. No unused imports remain after deletions

### Final Acceptance Criteria
- [ ] `go build ./...` passes
- [ ] `go vet ./...` passes  
- [ ] `golangci-lint run ./...` shows zero U1000
- [ ] `go test -race -count=1 ./...` passes
- [ ] Streaming still renders token-by-token
- [ ] Thinking blocks collapse/expand on `T`
- [ ] Tool cards render with correct colors and status
- [ ] Header shows context meter and model badge
- [ ] Status bar shows cwd/git branch/hints

## Success Criteria

1. 53 U1000 warnings eliminated (zero remaining)
2. All 4 orphaned files successfully deleted
3. All tests pass with -race flag
4. TUI rendering features continue to work correctly
5. No new warnings introduced
