---
phase: 16
plan: 01
subsystem: tui
tags:
  - commands
  - trust
  - safety
  - polish
type: tdd
requirements: [TRUST-01]
---

# Phase 16 Plan 1: Trust & Safety Fixes Summary

Implemented 12 trust/safety/command-honesty fixes across the TUI command layer, permission modal, settings, and task status display.

## Tasks Completed

| # | Task | Status | Commit |
|---|------|--------|--------|
| 1 | Fix `/clear` to actually clear conversation context | ✅ | `8cd53e5` |
| 2 | Make `/undo` honestly describe behavior | ✅ | `7e47a48` |
| 3 | Fix `/pause` to return truthful message | ✅ | `bd5d623` |
| 4 | Fix `/resume-task` to return truthful message | ✅ | `bd5d623` |
| 5 | Fix phase aliases to trigger actual transitions | ✅ | `a281b33` |
| 6 | Add confirmation to `/reset` | ✅ | `9cc96eb` |
| 7 | Add confirmation to `/rollback --hard` | ✅ | `e36dc56` |
| 8 | Fix settings re-mask logic | ✅ | `3e00d21` |
| 9 | De-emphasize `[E] Exit M31A` in permission modal | ✅ | `bbc2089` |
| 10 | Distinguish RiskDangerous from RiskDestructive | ✅ | `675615a` |
| 11 | Fix execute screen completion message accuracy | ✅ | `13968db` |
| 12 | Make plan task list reflect actual status | ✅ | `fe85ed4` |

## Files Modified

| File | Changes |
|------|---------|
| `internal/tui/commands_ai.go` | `/reset` requires `--confirm` flag |
| `internal/tui/commands_core.go` | `/clear` handler calls `ClearMessages` callback |
| `internal/tui/commands_session.go` | `/undo` shows honest "not yet implemented" message |
| `internal/tui/commands_workflow.go` | `/pause`, `/resume-task` truthful; phase aliases route via `handlePhase` |
| `internal/tui/commands.go` | Added `ClearMessages func()` to `CommandContext`; updated command descriptions |
| `internal/tui/commands_test.go` | Tests for all command fixes |
| `internal/tui/settings.go` | Changed `f.masked == false` to `!f.masked` |
| `internal/tui/components/permission.go` | De-emphasized exit hint; RiskDangerous uses Warning color |
| `internal/tui/components/permission_test.go` | Test for RiskDangerous Warning color |
| `internal/tui/app_update.go` | Wired `ClearMessages` callback in `CommandContext` construction |
| `internal/tui/execute.go` | Added completion summary (✓/⚠) |
| `internal/tui/plan.go` | Task list shows [x]/[>]/[!]/[-] for all statuses |
| `internal/tui/rollback.go` | `/rollback --hard` requires `--confirm` |

## Key Decisions

- `/clear` uses callback pattern (`ClearMessages func()`) for clean separation of concerns
- Phase aliases route through existing `handlePhase` with phase name detection
- `--confirm` flag required for destructive operations (`/reset`, `/rollback --hard`)
- RiskDangerous uses Warning (yellow) vs RiskDestructive uses Error (red) for visual distinction
- Exit option in permission modal rendered with `Faint(true)` for subordinate appearance
- Execute screen shows summary only after all tasks are Done/Failed (before Verify transition)

## Known Stubs

None — all fixes are functional and tested.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| None | — | No new security surface introduced |
