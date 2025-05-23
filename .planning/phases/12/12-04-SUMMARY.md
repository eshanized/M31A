---
phase: 12-ux-editor-experience-adaptations
plan: 04
type: execute
subsystem: tui
tags: [diff, git, viewer, screen, lipgloss]
dependency:
  requires: [01]
  provides: [DiffModel, ScreenDiff, enhanced /diff command]
  affects: [commands.go, types.go]
tech-stack:
  added: [bubbletea/diff-model]
  patterns: [tea.Model screen transition via CommandResult.Screen + Cmd]
key-files:
  created:
    - internal/tui/diff.go
  modified:
    - internal/tui/types.go (added ScreenDiff = 10)
    - internal/tui/commands.go (enhanced handleDiff)
    - internal/tui/commands_test.go (13 new tests)
    - internal/git/git.go (empty-ref fix)
key-decisions:
  - "Use theme colors (DiffAdded/DiffRemoved) instead of hardcoded lipgloss colors — adapts to dark/light mode"
  - "Screen transition via CommandResult.Screen + Cmd(func() tea.Msg { return DiffScreenMsg{} }) — matches existing ModelSelector pattern"
  - "--stat outputs formatted inline (no screen transition) using StatusPorcelain() for file-by-file counts"
  - "DiffCloseMsg{} emitted on esc/q/enter — parent AppState handles screen switch back to REPL"
metrics:
  duration: 4m
  completed: 2026-06-01T06:03+0530
  commits: 3
  files-changed: 5
  tests-added: 13
---

# Phase 12 Plan 04: Diff Viewer Screen — Summary

**One-liner:** Interactive diff viewer screen (DiffModel) with lipgloss-styled additions/deletions, scroll/split, plus enhanced `/diff` command supporting `--staged`, `--stat`, and `<commit>` arguments.

## Overview

Created a dedicated diff viewer screen (`ScreenDiff`) with DiffModel implementing `tea.Model` for interactive git diff browsing. The `/diff` command was enhanced beyond the old 4096-char truncation approach — it now routes full diffs to the interactive viewer, supports `--staged` for staged changes, `--stat` for inline diffstat summaries, and `<commit>` for comparing against a specific commit.

## Tasks

### Task 1 — Create DiffModel with tea.Model interface and diff rendering

- **Files:** `internal/tui/diff.go` (new), `internal/tui/types.go` (modified)
- **Commit:** `9e07839`
- **Details:**
  - Defined `DiffLineType` enum (`DiffContext`, `DiffAdded`, `DiffDeleted`, `DiffHeader`, `DiffHunk`)
  - Defined `DiffLine` struct with `Type` and `Content` fields
  - Defined `DiffScreenMsg` struct for carrying diff content/title to the screen
  - Defined `DiffModel` implementing `tea.Model` (`Init`/`Update`/`View`)
  - `parseDiff` method classifies lines by prefix and colors accordingly
  - Key handlers: `s` toggles split/unified, arrows/`j`/`k` scroll, `g`/`G` top/bottom, `pgup`/`pgdown` half-page, `esc`/`q`/`enter` exits
  - Added `ScreenDiff Screen = 10` to the Screen enum in `types.go`

### Task 2 — Enhance /diff command with --staged, --stat, <commit> args and screen route

- **Files:** `internal/tui/commands.go` (modified)
- **Commit:** `4f6e9a1`
- **Details:**
  - `--staged`: routes to `ctx.Git.DiffStaged()` → full-screen diff viewer
  - `--stat`: runs `git status --porcelain` and formats inline summary (no screen transition)
  - `<commit>`: runs `git diff <commit>..HEAD` → full-screen diff viewer
  - No args: routes to full-screen diff viewer (no more 4096-char truncation)
  - Uses `CommandResult.Screen = &ScreenDiff` + `CommandResult.Cmd` pattern

### Task 3 — Write tests for diff viewer and enhanced /diff command (including fixes)

- **Files:** `internal/tui/commands_test.go` (modified), `internal/git/git.go` (fix), `internal/tui/diff.go` (fix)
- **Commit:** `d52a8f3`
- **Details:**
  - 13 test functions:
    - `TestDiffCommand_NoArgs` — verifies screen transition + DiffScreenMsg
    - `TestDiffCommand_Staged` — verifies staged changes route
    - `TestDiffCommand_Stat` — verifies inline diffstat (no screen transition)
    - `TestDiffCommand_NoChanges` — verifies clean-repo message
    - `TestDiffCommand_NoGit` — verifies no-git error message
    - `TestDiffModel_ParseDiff` — verifies all 5 line type classifications
    - `TestDiffModel_Scroll` — verifies scrollPos up/down/home/end
    - `TestDiffModel_EscReturnsCloseMsg` — verifies exit behavior
    - `TestDiffModel_HomeEndKeys` — verifies home/end navigation
    - `TestDiffModel_ToggleSplit` — verifies `s` key toggles split view
    - `TestDiffModel_EmptyDiff` — verifies empty diff placeholder
    - `TestDiffModel_DiffScreenMsg` — verifies DiffModel receives and parses diff
    - `TestDiffModel_ViewContainsColoredContent` — verifies view output includes styled content
  - Fixes applied:
    - `Git.Diff("", "")` now runs plain `git diff` instead of `git diff ..` (pre-existing bug)
    - `parseDiff` now classifies `index` lines as `DiffHeader` (pre-existing gap)

## Verification

```bash
# Build
CGO_ENABLED=0 go build ./cmd/m31a/    # ✓ PASS

# Vet
go vet ./internal/tui/...             # ✓ PASS
go vet ./internal/git/...             # ✓ PASS

# Diff tests
go test ./internal/tui/... -run "TestDiffCommand|TestDiffModel" -count=1 -v  # ✓ ALL 13 PASS

# Full package tests
go test -count=1 ./internal/tui/...   # ✓ PASS (including sub-packages)
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed Git.Diff("", "") producing `git diff ..`**
- **Found during:** Task 3 (TestDiffCommand_NoArgs)
- **Issue:** When both refs are empty, `Git.Diff` concatenated an empty string with ".." producing `git diff ..` which fails
- **Fix:** Changed `Diff(ref1, ref2)` to run plain `git diff` when both refs are empty
- **File:** `internal/git/git.go`
- **Commit:** `d52a8f3`

**2. [Rule 2 - Missing] Added "index" line to DiffHeader classification**
- **Found during:** Task 3 (TestDiffModel_ParseDiff)
- **Issue:** `parseDiff` didn't handle `index abc..def 100644` lines — they fell through to `DiffContext`
- **Fix:** Added `case strings.HasPrefix(line, "index ")` to the header classification switch
- **File:** `internal/tui/diff.go`
- **Commit:** `d52a8f3`

## Feature Verification

| Criterion | Status |
|-----------|--------|
| DiffModel type defined | ✅ |
| DiffScreenMsg type defined | ✅ |
| ScreenDiff = 10 added to Screen enum | ✅ |
| parseDiff classifies 5 line types correctly | ✅ |
| Added lines rendered in green, deleted in red | ✅ |
| `s` key toggles split/unified view | ✅ |
| Arrow/j/k keys scroll | ✅ |
| g/G jumps top/bottom | ✅ |
| esc/q/enter exits diff view | ✅ |
| /diff --staged routes to diff viewer | ✅ |
| /diff --stat outputs inline diffstat | ✅ |
| /diff <commit> compares against commit | ✅ |
| /diff (no args) routes to diff viewer | ✅ |
| No changes shows "No uncommitted changes." | ✅ |
| No git repo shows error | ✅ |
| go build passes | ✅ |
| go vet passes | ✅ |
| All 13 tests pass | ✅ |

## Self-Check: PASSED

All 3 commits verified in git log:
- `9e07839` — feat(12-04): create DiffModel with tea.Model interface and diff rendering
- `4f6e9a1` — feat(12-04): enhance /diff command with --staged, --stat, <commit> args and screen route
- `d52a8f3` — test(12-04): add diff command/model tests

All modified files verified:
- `internal/tui/diff.go` — ✅ 180 lines
- `internal/tui/types.go` — ✅ contains `ScreenDiff Screen = 10`
- `internal/tui/commands.go` — ✅ enhanced handleDiff with 51 insertions
- `internal/tui/commands_test.go` — ✅ 13 test functions appended
- `internal/git/git.go` — ✅ empty-ref diff fix
