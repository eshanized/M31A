---
phase: 30
plan: 08
name: OpenCode-Inspired TUI Visual Redesign — Animations & Transitions
subsystem: internal/tui/components, internal/tui
tags: [tui, animation, spinner, progress-bar, transition, visual-polish]
provides:
  - Custom spinner component (components.Spinner)
  - Animated progress bar with flash effects (components.AnimatedProgressBar)
  - Screen transition overlay (ScreenTransition)
requires: [30-03, 30-06, 30-07]
affects: [internal/tui/repl_model.go, internal/tui/execute_model.go, internal/tui/statusbar.go, internal/tui/verify.go, internal/tui/modelselector.go]
tech-stack:
  added:
    - components.Spinner — lightweight spinner with opencode frame set
    - components.AnimatedProgressBar — tick-driven animated progress bar
    - components.AnimatedProgress — ramp animation engine (500ms)
    - ScreenTransition — 200ms dim-and-reveal screen switching
  patterns:
    - Replace embedded bubbletea models with plain Go struct + TickMsg pattern
    - TickMsg from streaming.go reused across all animated components
key-files:
  created:
    - internal/tui/components/spinner.go
    - internal/tui/transition.go
  modified:
    - internal/tui/components/progress.go (added AnimatedProgress, AnimatedProgressBar)
    - internal/tui/execute_model.go (wired animated progress bar)
    - internal/tui/execute_view.go (animated progress bar rendering)
    - internal/tui/repl_model.go (swapped bubbles/spinner → components.Spinner)
    - internal/tui/repl.go (spinner init/tick)
    - internal/tui/repl_state.go (StreamTickCmd for spinner)
    - internal/tui/repl_view.go (spinner frame in status bar)
    - internal/tui/statusbar.go (SpinnerFrame field)
    - internal/tui/verify.go (custom spinner instead of inline frames)
    - internal/tui/modelselector.go (loading spinner)
    - internal/tui/modelselector_list.go (spinner in loading view)
    - internal/tui/app_state.go (transition field)
    - internal/tui/app_update.go (transition tick, ensureSubModel)
    - internal/tui/app_view.go (transition overlay rendering)
    - internal/tui/types.go (Screen.Label())
decisions:
  - Removed bubbles/spinner dependency — custom Spinner struct gives explicit control over the opencode frame set (⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ at 10fps) and avoids embedding a full tea.Model
  - TickMsg from streaming.go reused for all animated components (spinner, progress bar) to keep tick dispatch unified through AppState → sub-model routing
  - Peek()/Next() split for spinner: Peek() is View()-safe, Next() is Update()-only, preventing frame over-advance on re-render
  - Screen transitions are 200ms dim overlays; skipped for permission/diff overlays and screens with async init (resume)
  - Eager sub-model creation in navigateToScreen ensures models exist when transition completes
metrics:
  duration: ~35 min
  commits: 3
  files-changed: 19
  tests-passing: all (build, vet, race tests)
---

# Phase 30 Plan 08: OpenCode-Inspired TUI Visual Redesign — Animations & Transitions

## Overview

Adds three real-time animation systems to the M31A TUI:
- **Spinner component**: Replaces `bubbles/spinner.Model` with a lightweight custom `components.Spinner` using the 10-frame opencode set (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) at 10fps. Wired into REPL streaming/thinking status bar, verify heal attempts, execute task progress, and model selector loading state.
- **Animated progress bar**: `components.AnimatedProgress` smoothly ramps the displayed fill over 500ms when tasks complete. Green flash (300ms) on full completion, red flash on failure. Tick-driven via the shared TickMsg stream.
- **Screen transitions**: 200ms dim-and-reveal overlay on screen changes. Shows the target screen name centered on a dimmed background. Skipped for permission/diff overlays and async screens.
- **Removed `bubbles/spinner` dependency** from go.mod entirely.

---

## Task Results

| Task | Name                        | Type   | Status | Commit   |
|------|-----------------------------|--------|--------|----------|
| 1    | Custom Spinner component    | auto   | ✅     | `ffd19a5` |
| 2    | Animated progress bar       | auto   | ✅     | `cd2d019` |
| 3    | Screen transition effects   | auto   | ✅     | `fa24538` |

**3/3 tasks completed.**

---

## Deviations from Plan

### [Rule 2 — Missing Functionality] AnimatedProgress needs proper from/to tracking

- **Found during:** Task 2
- **Issue:** Plan pseudocode used `p.Displayed = int(float64(p.Current-p.Total) * pct) + p.Total` which doesn't properly track old→new transition. Added `fromVal`/`toVal` fields and `UpdateProgress(oldCurrent, newCurrent, total)` method for correct ramp interpolation.

### [Rule 2 — Missing Functionality] TickMsg not forwarded to execute screen

- **Found during:** Task 2
- **Issue:** The execute model's animated progress bar needs TickMsg to advance, but AppState's TickMsg handler only forwarded to the REPL model (when streaming/thinking).
- **Fix:** Added forwarding to `m.executeModel` when `m.screen == ScreenExecute` in `app_update.go`.

### [Rule 3 — Blocking] NewResumeModel loads sessions async

- **Found during:** Task 3
- **Issue:** `NewResumeModel` requires `[]session.SessionInfo` loaded via `openResumeScreen()` async command. Cannot eagerly create this model during transition setup.
- **Fix:** Skip transition for `ScreenResume` — falls through to immediate navigation.

### [Rule 3 — Blocking] Sub-model constructors have inconsistent signatures

- **Found during:** Task 3
- **Issue:** Some sub-models use `SetDimensions()` method, others use direct field assignment (`model.width = w`). The eagerly-created `ensureSubModel` function needed different patterns for different screens.
- **Fix:** Used direct field assignment for ExecuteModel, VerifyModel, ShipModel; used `SetDimensions()` for PlanModel; fell back to `openResumeScreen()` for resume.

---

## Verification Results

| Check | Status |
|-------|--------|
| `CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a` | ✅ PASS |
| `go vet ./internal/tui/...` | ✅ PASS |
| `go test -race -count=1 -cover ./internal/tui/...` | ✅ PASS |

---

## Commit Log

```
ffd19a5 feat(30-08): add custom Spinner component with opencode character set
cd2d019 feat(30-08): add animated progress bar with flash effects
fa24538 feat(30-08): add screen transition effect with dim overlay
```

---

## Self-Check

### Created files exist

- `internal/tui/components/spinner.go`: ✅
- `internal/tui/transition.go`: ✅

### Commits exist

- `ffd19a5`: ✅
- `cd2d019`: ✅
- `fa24538`: ✅

### All requirements met

1. ✅ Custom Spinner component with 10-frame opencode set at 10fps
2. ✅ Spinner wired into REPL, verify, execute, model-selector screens
3. ✅ `bubbles/spinner` dependency removed
4. ✅ Animated progress bar with 500ms fill ramp
5. ✅ Green flash on completion, red flash on failure
6. ✅ Screen transitions with 200ms dim overlay
7. ✅ Build, vet, and tests all pass

## Self-Check: PASSED
