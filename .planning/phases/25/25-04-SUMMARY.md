---
phase: 25
plan: 25-04
subsystem: tui
tags: [parity, warnings, docs, cleanup]
requires: [25-03]
provides: [WIRE-08]
affects: [internal/tui, internal/types, internal/provider, internal/workflow, docs]
tech-stack: [go, bubbletea, lipgloss]
key-files:
  created: []
  modified:
    - internal/tui/components/toolcard.go
    - internal/tui/app.go
    - internal/tui/execute.go
    - internal/workflow/ship.go
    - internal/types/types.go
    - internal/provider/sse.go
    - internal/tui/header.go
    - internal/tui/app_update.go
    - docs/INTERFACES.md
    - docs/ARCHITECTURE.md
decisions:
  - "Autodream/Subagent settings toggles deferred — FeaturesConfig doesn't expose these fields (V1.1 features)"
metrics:
  duration: ~1h
  tasks_completed: 15
  tasks_total: 15
  files_changed: 10
  commits: 9
---

# Phase 25 Plan 25-04: Code/Doc Parity — Warnings Summary

Fix 15 parity warnings covering tool icon mapping, ticker lifecycle, task metrics, commit log truncation, file action enums, interface doc freshness, permission token estimation, dead sentinel cleanup, header atomic reads, help text completeness, settings toggles, and ledger stats.

## Deviations from Plan

### Auto-fixed Issues

None — all tasks implemented as planned.

### Plan vs Implementation

**1. Task 14 (W-35): Autodream/Subagent Settings Toggles**
- **Found during:** Task 14
- **Issue:** Plan specified 4 Features toggles (Autodream, Subagent, Backup, ResumeOnStartup), but `FeaturesConfig` only defines `AutoBackup` and `ResumeOnStartup`. Autodream and Subagent are V1.1 features not yet in the config struct.
- **Resolution:** Current 2 toggles are present and functional. Autodream/Subagent toggles will be added when those features are implemented in Phase 9.
- **No code change needed** — existing toggles already work correctly.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | W-09 — Fix ToolIcons Map Key | df2f564 | toolcard.go |
| 2 | W-12 — Move Health Ticker Stop | 80873bd | app.go |
| 3 | W-14 — Move Task Metrics to Update | 4a5374c | execute.go |
| 4 | W-17 — Truncate Commit Log | 793bb9e | ship.go |
| 5 | W-18 — Add FileAction Constants | b5edccf | types.go |
| 6 | W-24 — Refresh INTERFACES.md | d39395d | INTERFACES.md |
| 7 | W-26 — Document Permission Coupling | b958949 | ARCHITECTURE.md |
| 8 | W-27 — Verify No DocKeychain | (no change) | INTERFACES.md |
| 9 | W-28 — Verify Session.ParentID | (no change) | types.go |
| 10 | W-29 — Wire ErrStreamTruncated | a17399c | sse.go |
| 11 | W-30 — Atomic Header Health | e09108b | header.go, app.go, app_update.go |
| 12 | W-31 — Verify SessionIDLength | (no change) | constants.go |
| 13 | W-34 — Verify Help Text | (no change) | commands_core.go |
| 14 | W-35 — Settings Toggles | (no change) | settings.go |
| 15 | W-36 — Verify /ledger stats | (no change) | commands_git.go |

## Known Stubs

None — no stubs were introduced.

## Threat Flags

None — no new security-relevant surface introduced.

## Self-Check

### Files Exist
- [x] internal/tui/components/toolcard.go — modified
- [x] internal/tui/app.go — modified
- [x] internal/tui/execute.go — modified
- [x] internal/workflow/ship.go — modified
- [x] internal/types/types.go — modified
- [x] internal/provider/sse.go — modified
- [x] internal/tui/header.go — modified
- [x] internal/tui/app_update.go — modified
- [x] docs/INTERFACES.md — modified
- [x] docs/ARCHITECTURE.md — modified

### Commits Exist
- [x] df2f564 — fix(25-04): fix ToolIcons map key for AskUserQuestion
- [x] 80873bd — fix(25-04): move health ticker stop to Shutdown
- [x] 4a5374c — fix(25-04): add recordTaskMetric helper for task metrics
- [x] 793bb9e — fix(25-04): truncate commit log to last 50 lines in ship phase
- [x] b5edccf — fix(25-04): add FileAction constants for FilePrediction.Action enum
- [x] d39395d — docs(25-04): refresh INTERFACES.md to match current types.go
- [x] b958949 — docs(25-04): document W-26 permission→tools coupling in ARCHITECTURE.md
- [x] a17399c — fix(25-04): wire ErrStreamTruncated sentinel in SSE parser
- [x] e09108b — fix(25-04): atomic read for header health status (W-30)

## Self-Check: PASSED
