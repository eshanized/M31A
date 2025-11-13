---
phase: 30
plan: 30-07
title: "Wave 7 — Sidebar & Diff Polish"
subsystem: TUI
tags: [sidebar, diff, polish, git, visual]
requires: [30-01]
provides: []
affects: [internal/tui/sidebar.go, internal/tui/diff_view.go, internal/tui/diff_model.go, internal/tui/rollback.go, internal/tui/types.go, internal/tui/keybindings_screens.go, internal/tui/app_update.go, internal/git/git.go]
tech-stack:
  added: []
  patterns: [lipgloss.JoinHorizontal for component assembly, file grouping by status category]
key-files:
  created: []
  modified:
    - internal/tui/sidebar.go
    - internal/tui/diff_view.go
    - internal/tui/rollback.go
    - internal/tui/types.go
    - internal/tui/keybindings_screens.go
    - internal/tui/app_update.go
    - internal/git/git.go
decisions:
  - "Swapped ctrl+x [/ctrl+x ] from model cycling to sidebar resize (plan-requested keys); model selection still available via ctrl+x m"
  - "RemoteTracking() returns empty string when no upstream configured (not an error)"
  - "Line numbers shown for added and context lines only; deleted lines show blank (avoids confusion about which line number is being displayed)"
metrics:
  duration: "~15 min"
  completed_date: "2026-06-08"
---

# Phase 30 Plan 07: Sidebar & Diff Polish

Refined the sidebar with cleaner section layout and git graph aesthetics. Enhanced the diff viewer with syntax highlighting, line numbers, and styled hunk headers.

## Must-Haves Achieved

- **Sidebar**: `ℹ M31A` brand title, git branch + remote tracking, files grouped by status, `✓ working tree clean` empty state, resizable (ctrl+x [/ctrl+x ])
- **Diff viewer**: Green bg for additions, red bg for deletions, dimmed context, line numbers (right-aligned, 4-char width), styled hunk headers in thinking color
- `CGO_ENABLED=0 go build` passes, tests pass, vet passes

## Tasks Completed

### Task 1: Sidebar visual refresh
- Changed title from `◈ M31A` to `ℹ M31A` in brand color
- Added `RemoteTracking()` method to `internal/git/git.go` — returns upstream branch (e.g. `origin/main`) or empty string if no tracking configured
- Remote tracking displayed below branch name in muted color
- Files now grouped by git status category (modified, added, deleted, renamed, untracked) with icon + label header per group
- Empty state changed from `clean` to `✓ working tree clean` in success color
- Added `IncreaseWidth()`/`DecreaseWidth()` methods with `sidebarMinWidth=20`, `sidebarMaxWidth=50`
- Added thin vertical right border `│` in muted color using `lipgloss.JoinHorizontal`
- Sidebar content width reserved to `w-1` to accommodate border column
- Keybindings: `ctrl+x [` → `sidebar_wider`, `ctrl+x ]` → `sidebar_narrower`
- Model cycling moved off bracket chords (still available via `ctrl+x m`)

**Commit:** `49db470`

### Task 2: Diff screen visual refresh
- Rewrote `colorizeDiff` in `internal/tui/rollback.go` with comprehensive styling:
  - Added lines: `DiffAddedBg` background + `DiffAdded` foreground + `+` prefix
  - Removed lines: `DiffRemovedBg` background + `DiffRemoved` foreground + `−` prefix
  - Context lines: `TextMuted` foreground + `Faint(true)` — dimmed, no prefix
  - Hunk headers (`@@ ... @@`): `Thinking` color + `Bold(true)` — styled prominently
  - File headers (`--- a/` and `+++ b/`): `TextMuted` foreground — clearly separated
  - Line numbers: right-aligned, 4-char width in muted — shown for added and context lines; blank for deleted lines
- Updated `diff_view.go` legend rendering with styled background previews

**Commit:** `5629b7c`

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None.

## Self-Check: PASSED

```
FOUND: internal/tui/sidebar.go
FOUND: internal/tui/diff_view.go
FOUND: internal/tui/rollback.go
FOUND: internal/tui/types.go
FOUND: internal/tui/keybindings_screens.go
FOUND: internal/tui/app_update.go
FOUND: internal/git/git.go

BUILD: PASS
VET:  PASS
TEST: PASS
```

## Verification

```bash
$ CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a   # exit 0
$ go vet ./internal/tui/...                          # exit 0
$ go test -race -count=1 -cover ./internal/tui/...   # ok
```
