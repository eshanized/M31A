---
phase: 03
phase_name: Message Rendering Pipeline
plan: 03-01
type: PLAN
wave: 1
depends_on: []
files_modified:
  - internal/tui/helpers.go
  - internal/tui/app.go
  - internal/tui/app_view.go
  - internal/tui/repl_welcome.go
  - internal/tui/repl_view.go
  - internal/tui/repl_stream.go
  - internal/tui/repl_thinking.go
  - internal/tui/plan_view.go
  - internal/tui/plan_model.go
  - internal/tui/ship_view.go
  - internal/tui/ship_model.go
  - internal/tui/settings_view.go
  - internal/tui/settings_model.go
  - internal/tui/sidebar.go
  - internal/tui/execute_view.go
  - internal/tui/execute_model.go
  - internal/tui/header.go
  - internal/tui/components/truncate.go
  - internal/workflow/engine_verify.go
  - internal/workflow/verify.go
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

Wire 27 unused functions to their intended call sites. Each function has a logical home where it should be called — wire it there. Remove 3 truly dead code items (duplicate `truncateMiddle`, unused constant `transitionOverlayWidth`, abandoned `app_channel.go` and `backup.go` files).

## Wave 1 — REPL & Welcome Screen Wiring

### Task 03-01-01: Wire `renderBottomBar` into welcome screen
**Files:** `repl_welcome.go`
**Wiring:** Append `m.renderBottomBar()` result to welcome content before `lipgloss.Place()` (after hints line at line 45).
**Acceptance:** Welcome screen shows cwd and version at bottom.

### Task 03-01-02: Wire `renderQuickActions` into welcome screen
**Files:** `repl_welcome.go`, `repl.go`
**Wiring:** Call `m.renderQuickActions()` from `renderWelcome()` when no messages and not streaming. Shows "/help /settings /models /workflow" hints.
**Acceptance:** Quick action hints appear in welcome screen.

### Task 03-01-03: Wire `renderQuickActionsPanel` into repl view
**Files:** `repl_view.go`, `repl_quickactions.go`
**Wiring:** Call `m.renderQuickActionsPanel(rw)` in `View()` below viewport content when messages exist but no workflow is active.
**Acceptance:** Quick action panel appears below last message when idle.

### Task 03-01-04: Wire `streamTickCmds` into stream continuation
**Files:** `repl_stream.go`
**Wiring:** In `handleStreamMsg()`, append `StreamTickCmd()` alongside the continuation read cmd to drive 10fps rendering.
**Acceptance:** Streaming tick fires at 10fps during active stream.

### Task 03-01-05: Wire `renderThinkingToggleHint` into message rendering
**Files:** `repl_thinking.go`, `repl_view.go`
**Wiring:** After rendering thinking blocks, append `renderThinkingToggleHint(index, durationMs)` as a footer.
**Acceptance:** "[T] expand/collapse" hint shows below thinking blocks.

## Wave 2 — Screen View Wiring

### Task 03-02-01: Wire `renderPlanHeader` into plan view
**Files:** `plan_view.go`, `plan_model.go`
**Wiring:** Replace inline header (lines 186-210) with `renderPlanHeader(t, len(pm.tasks), pm.estCost, pm.modelName, pm.provider)`.
**Acceptance:** Plan screen header matches current appearance but uses shared function.

### Task 03-02-02: Wire `renderShipStatsGrid` into ship view
**Files:** `ship_view.go`, `ship_model.go`
**Wiring:** Replace inline grid building (lines 76-140) with `renderShipStatsGrid(t, rows, colWidth)`.
**Acceptance:** Ship screen stats display identically.

### Task 03-02-03: Wire `maskedKey` into settings keys tab
**Files:** `settings_view.go`, `settings_model.go`
**Wiring:** Replace inline key masking (line 373-377) with `maskedKey(key)`.
**Acceptance:** API keys show "●●●●●●●" masking in settings.

### Task 03-02-04: Wire `renderSettingCard` into settings view
**Files:** `settings_view.go`, `settings_model.go`
**Wiring:** Use `renderSettingCard(t, title, content, width)` for settings tab content sections.
**Acceptance:** Settings sections wrapped in card styling.

### Task 03-02-05: Wire `renderProgressBar`, `renderTaskSpinner`, `animatedProgressBarWidth` into execute view
**Files:** `execute_view.go`, `execute_model.go`
**Wiring:**
- Replace `barWidth := 10` with `barWidth := animatedProgressBarWidth(em)`
- Show `renderTaskSpinner(em)` next to running task lines
- Render `renderProgressBar(t, done, total, barWidth)` alongside animated bar
**Acceptance:** Execute screen shows progress bar and spinner.

### Task 03-02-06: Wire `renderSectionHeader` into settings view
**Files:** `helpers.go`, `settings_model.go`
**Wiring:** Replace inline section title rendering (line 282, 325, 345, 392) with `renderSectionHeader(title, w)`.
**Acceptance:** Settings section dividers use shared format.

### Task 03-02-07: Wire `renderHeader` into app view
**Files:** `app_view.go`
**Wiring:** Call `m.renderHeader("")` at the top of each non-REPL screen render method (settings, plan, execute, verify, ship).
**Acceptance:** Non-REPL screens show header bar with brand, model badge, git branch.

## Wave 3 — Sidebar & Workflow Wiring

### Task 03-03-01: Wire `ensureSidebarModel` before sidebar access
**Files:** `helpers.go`, `app_view.go`
**Wiring:** Call `m.ensureSidebarModel()` before `m.sidebarModel.View()` at line 51 and before any sidebar access.
**Acceptance:** Sidebar model initialized before first access; no nil pointer panic.

### Task 03-03-02: Wire `refreshCmd` into sidebar update
**Files:** `sidebar.go`
**Wiring:** In `Update()`, after processing `SidebarRefreshMsg`, return `s.refreshCmd()` to chain periodic refreshes. Also return `refreshCmd()` when sidebar first becomes visible.
**Acceptance:** Sidebar git data refreshes periodically.

### Task 03-03-03: Wire `fileStatusIcon` into sidebar view
**Files:** `sidebar.go`
**Wiring:** Replace hardcoded group icon/color map (lines 207-218) with calls to `fileStatusIcon("M", t)` etc.
**Acceptance:** File status icons render identically.

### Task 03-03-04: Wire `propagateSessionID` into session switch
**Files:** `helpers.go`, `app.go`
**Wiring:** Call `m.propagateSessionID(id)` in the session switch/restore Update message handler.
**Acceptance:** Workflow sub-models receive correct session ID on restore.

### Task 03-03-05: Wire `applySessionRestored` into app update loop
**Files:** `helpers.go`, `app.go`
**Wiring:** In `AppState.Update()`, add case for `sessionRestoredMsg` that calls `m.applySessionRestored(msg)`.
**Acceptance:** Session restore flows through to screen transition.

### Task 03-03-06: Wire `verifyTaskContext` into verify phase
**Files:** `workflow/engine_verify.go`, `workflow/verify.go`
**Wiring:** Call `engine.verifyTaskContext(ctx)` at the start of the verify phase.
**Acceptance:** Verify phase propagates task context correctly.

### Task 03-03-07: Wire `errorf` into error paths
**Files:** `app_view.go`, `app.go`
**Wiring:** Replace `fmt.Errorf` in error creation paths with `errorf(msg)` where appropriate.
**Acceptance:** Error creation uses shared helper.

### Task 03-03-08: Wire `formatDurationMs` into status bar
**Files:** `header.go`, `repl_view.go`
**Wiring:** Use `formatDurationMs(ms)` in status bar thinking duration display.
**Acceptance:** Duration formatting in status bar.

## Wave 4 — Dead Code Removal

### Task 03-04-01: Remove `truncateMiddle` from components/truncate.go
**Reason:** Unexported duplicate of `TruncateMiddle` in `tui/truncate.go:39` which is already used.
**Action:** Delete the function, not the file (truncateEnd is still used).

### Task 03-04-02: Remove `transitionOverlayWidth` constant
**Reason:** Never referenced; transitions use computed width.
**Action:** Delete the constant.

### Task 03-04-03: Remove `backup.go`
**Reason:** Entire file is dead — `backupCurrentSessionAsync` and `copyDir` never called. Session manager handles backups.
**Action:** Delete the file.

## Verification

### Per-Wave Checks
- `go build ./internal/tui/...` passes after each wave
- `go vet ./internal/tui/...` is clean

### Final Acceptance
- [ ] `go build ./...` passes
- [ ] `go vet ./...` passes
- [ ] `go test -race -count=1 ./...` passes
- [ ] Welcome screen shows quick actions + bottom bar
- [ ] Quick action panel appears when idle with messages
- [ ] Plan screen header uses shared function
- [ ] Ship stats grid uses shared function
- [ ] Settings shows masked API keys
- [ ] Sidebar refreshes and shows file status icons
- [ ] Session restore flows through all sub-models
