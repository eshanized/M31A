---
phase: 03
phase_name: Message Rendering Pipeline
type: RESEARCH
created_at: 2026-06-08
---

## RESEARCH COMPLETE

### Phase 3 Wire-In Analysis

**Goal:** Wire all 27 unused functions to their appropriate call sites. Do NOT delete — the user wants them used where they belong.

### Wiring Map

| # | Function | File | Where to Wire | Status |
|---|----------|------|---------------|--------|
| 1 | `ensureSidebarModel` | helpers.go:53 | `app_view.go:49` — call before sidebar access in View() and app.go Update sidebar toggle | WIRE |
| 2 | `propagateSessionID` | helpers.go:60 | `app.go` session switch path — call when session ID changes | WIRE |
| 3 | `applySessionRestored` | helpers.go:76 | `app.go Update()` — handle `sessionRestoredMsg` message type | WIRE |
| 4 | `renderBottomBar` | repl_welcome.go:131 | `repl_welcome.go:45` — append at end of `renderWelcome()` before Place() | WIRE |
| 5 | `renderQuickActions` | repl.go:309 | `repl_welcome.go` or `repl_view.go` — display in welcome when no messages | WIRE |
| 6 | `renderQuickActionsPanel` | repl_quickactions.go:13 | `repl_view.go` — render below viewport when messages exist but idle | WIRE |
| 7 | `maskedKey` | settings_view.go:34 | `settings_model.go:371-378` — replace inline key masking | WIRE |
| 8 | `renderShipStatsGrid` | ship_view.go:14 | `ship_model.go:76-140` — replace inline stats grid building | WIRE |
| 9 | `refreshCmd` | sidebar.go:81 | `sidebar.go Update() line 106` — return cmd after loading data | WIRE |
| 10 | `fileStatusIcon` | sidebar.go:288 | `sidebar.go:206-218` — replace hardcoded icon map in View() | WIRE |
| 11 | `verifyTaskContext` | engine_verify.go:109 | `workflow/verify.go` — call during verify phase task analysis | WIRE |
| 12 | `errorf` / `simpleError` | app_view.go:371/375 | `app.go` error paths — replace `fmt.Errorf` with `errorf` | WIRE |
| 13 | `formatDurationMs` | header.go:233 | `repl_view.go` status bar — show thinking duration | WIRE |
| 14 | `renderPlanHeader` | plan_view.go:15 | `plan_model.go:186-210` — replace inline header rendering | WIRE |
| 15 | `renderSettingCard` | settings_view.go:15 | `settings_model.go` — wrap content in card layout | WIRE |
| 16 | `renderProgressBar` | execute_view.go:15 | `execute_model.go:182-186` — use as static bar alongside animated | WIRE |
| 17 | `renderTaskSpinner` | execute_view.go:54 | `execute_model.go:215` — show spinner for running tasks | WIRE |
| 18 | `animatedProgressBarWidth` | execute_view.go:49 | `execute_model.go:182` — replace hardcoded `barWidth := 10` | WIRE |
| 19 | `renderSectionHeader` | helpers.go:154 | `settings_view.go:282` — use for section titles | WIRE |
| 20 | `renderHeader` | app_view.go:339 | `app_view.go:55` — render before active screen content | WIRE |
| 21 | `streamTickCmds` | repl_stream.go:215 | `repl_stream.go:73` — return alongside stream read cmd | WIRE |
| 22 | `renderThinkingToggleHint` | repl_thinking.go:13 | `repl_view.go` — show toggle hint below thinking blocks | WIRE |
| 23 | `truncateMiddle` | components/truncate.go:19 | DEAD — exported `TruncateMiddle` in `tui/truncate.go:39` already used | REMOVE |
| 24 | `transitionOverlayWidth` | transition.go | DEAD — constant never read; transitions use full width | REMOVE |
| 25 | `app_channel.go` | app_channel.go | DEAD — abandoned channel implementation; streaming.go covers this | DELETE FILE |
| 26 | `backup.go` | backup.go | DEAD — abandoned async backup; session manager handles backups | DELETE FILE |

### Key Design Decisions

1. **No functional changes**: wiring must preserve existing behavior. The functions are being called where they were INTENDED to be called but never wired.
2. **No test breakage**: all existing tests must pass after wiring.
3. **Prefer reuse over rewrite**: when a function duplicates existing inline code, use the function and remove the inline code.
4. **`truncateMiddle` vs `TruncateMiddle`**: the unexported version in components/ is dead; the exported version in tui/truncate.go is used. Remove the dead one.
