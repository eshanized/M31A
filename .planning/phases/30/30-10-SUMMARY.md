---
phase: 30
plan: 30-10
title: "Wave 10 — Accessibility & Terminal Compatibility"
subsystem: tui
tags: [accessibility, terminal-compatibility, responsive-layout, color-profile]
requires: []
provides: [color-profile-detection, width-thresholds, responsive-layout]
affects: [internal/tui/theme/colors.go, internal/tui/theme/theme.go, internal/tui/app_view.go, internal/tui/header.go]
tech-stack:
  added: []
  patterns: [color-profile-detection, responsive-viewport-gating]
key-files:
  created:
    - internal/tui/constants.go
  modified:
    - internal/tui/theme/colors.go
    - internal/tui/theme/theme.go
    - internal/tui/theme/theme_test.go
    - internal/tui/header.go
    - internal/tui/app_view.go
decisions:
  - "16-color ANSI fallback overrides key color fields (Brand, Success, Error, Warning, Thinking) with ANSI color codes while preserving structure"
  - "Profile detection checks COLORTERM first, then TERM — no lipgloss API dependency for color capability"
  - "Width thresholds at 40/60/80 — sidebar, header chrome, and toast overlays gate at different levels"
  - "Sidebar width gating done at app_view.go layout level, not inside SidebarModel.View() — sidebar model has no terminal width context"
metrics:
  duration: "~15 min"
  completed_date: "2026-06-08"
  source_files_changed: 6
  test_files_changed: 1
---

# Phase 30 Plan 30-10: Accessibility & Terminal Compatibility Summary

## One-liner

Terminal color profile detection (truecolor→256→16 fallback) and responsive layout width thresholds (40/60/80) to ensure M31A works across all terminal capabilities.

## Task Results

### Task 1 — Color profile detection and fallback

Implemented `ColorProfile` type (`ProfileTrueColor`, `Profile256`, `Profile16`) with `DetectColorProfile()` using `COLORTERM` and `TERM` environment variables. Added `PaletteForProfile()` and `ansiPalette()` for 16-color terminal fallback. Wired profile detection into `ThemeManager.NewManager()` constructor; `resolve()` applies ANSI color overrides when `profile == Profile16`.

**Files modified:**
- `internal/tui/theme/colors.go` — Added ColorProfile type, DetectColorProfile(), PaletteForProfile(), ansiPalette()
- `internal/tui/theme/theme.go` — Added profile field to Manager, updated NewManager/resolve
- `internal/tui/theme/theme_test.go` — Added 8 new tests covering all detection paths and palette outputs

**Commit:** `f636e18`

### Task 2 — Minimum width enforcement

Created `internal/tui/constants.go` with `WidthUltraCompact` (40), `WidthCompact` (60), `WidthFull` (80). Updated `app_view.go` with responsive layout routing: < 40 renders active screen only (no chrome), < 80 auto-hides sidebar. Updated `header.go` to hide right-side chrome (context meter, model badge, provider badge) at < 60 width. Updated `syncReplSize` to account for width-gated sidebar. Toast overlay hidden below compact width.

**Files modified:**
- `internal/tui/constants.go` (new) — Width threshold constants
- `internal/tui/header.go` — Gated right-side chrome at < 60 width
- `internal/tui/app_view.go` — Responsive layout with 4-tier width routing

**Commit:** `dbd02b2`

## Verification

| Check | Result |
|-------|--------|
| `CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a` | ✅ Passes |
| `go vet ./internal/tui/...` | ✅ No warnings |
| `go test -race -count=1 -cover ./internal/tui/...` | ✅ All pass |
| `go vet ./...` | ✅ No warnings |

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None.

## Threat Flags

None.

## Self-Check: PASSED

- ✅ All 6 source files exist
- ✅ Both commit hashes verified (`f636e18`, `dbd02b2`)
- ✅ SUMMARY.md in correct location
