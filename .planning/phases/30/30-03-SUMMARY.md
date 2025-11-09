---
phase: 30
plan: 30-03
title: "Wave 3 — Status Bar Redesign"
subsystem: tui
tags: [statusbar, tui, layout]
requires: [30-01]
provides: []
affects: [internal/tui/statusbar.go, internal/tui/repl_view.go]
tech-stack:
  added: []
  patterns: [3-zone layout, compact mode]
key-files:
  created: []
  modified:
    - internal/tui/statusbar.go
    - internal/tui/repl_view.go
decisions:
  - "Status bar uses 3-zone layout: left (cwd+branch), center (operation), right (hints+cost)"
  - "No background fill — status bar inherits terminal background (transparent)"
  - "When idle and no operation, center zone shows nothing (no 'Ready' label)"
  - "Overflow policy: drop right zone first, then center, then truncate left with ellipsis"
  - "Compact mode: width < 80 hides hints/cost/shortens cwd; width < 60 hides git branch"
metrics:
  duration: 0h 10m
  completed: 2026-06-08
---

# Phase 30 Plan 30-03: Status Bar Redesign Summary

Refined the status bar from a left/right padding layout to a clean 3-zone layout with compact mode support and transparent background.

## Changes

### `internal/tui/statusbar.go` — Rewritten

- **New signature**: `RenderStatusBar(t theme.Theme, width int, info *StatusBarInfo) string` — removed `operation string` and `lastActivity time.Time` parameters (redundant — info carries IsStreaming, IsThinking, WorkflowPhase)
- **3-zone layout**:
  - **Left zone**: cwd basename with `⌂` prefix + git branch with `⎇` prefix
  - **Center zone**: operation status — leader mode (`ctrl+x ─ waiting ─`), thinking (`thinking...`), streaming (`responding...`), workflow phase, or empty when idle
  - **Right zone**: keyboard hints + token usage + cost estimate
- **Separator**: ` · ` (space-middle dot-space) between non-empty zones only
- **No background fill**: no `Background()` calls — inherits terminal background, only foreground colors used
- **Overflow handling**: drops right zone first, then center, then truncates left with ellipsis
- **Compact mode**:
  - Width < 80: hides keyboard hints, hides cost/tokens, shortens cwd to last path component
  - Width < 60: also hides git branch

### `internal/tui/repl_view.go` — Updated caller

- Updated `RenderStatusBar` call to match new signature (removed `m.GetStatusText()` and `m.lastActivity` args)

### StatusBarInfo struct — Unchanged

All needed fields were already present: `CwdName`, `GitBranch`, `IsStreaming`, `IsThinking`, `LeaderActive`, `WorkflowPhase`, `WhichKey`, `KeyboardHints`, `ShowCost`, `TotalTokens`, `Cost`.

## Deviations from Plan

None — plan executed exactly as written.

## Verification Results

- `CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a` — PASS
- `go vet ./internal/tui/...` — PASS
- `go test -race -count=1 -cover ./internal/tui/...` — PASS (tui: 0%, components: 40.0%, theme: 95.4%)

## Self-Check: PASSED

- [x] `internal/tui/statusbar.go` exists with new implementation
- [x] `internal/tui/repl_view.go` has updated caller
- [x] `⌂` prefix present for cwd (verified via grep)
- [x] `⎇` prefix present for git branch (verified via grep)
- [x] No `Background()` in status bar (verified via grep — zero matches)
- [x] ` · ` separator between zones (verified via grep)
- [x] Compact mode: width < 80 → hide hints/cost/shorten cwd (present at line 46)
- [x] Ultra-compact: width < 60 → hide git branch (present at line 57)
- [x] Commit `bb0ff35` exists in git log
- [x] Build, vet, and all tests pass
