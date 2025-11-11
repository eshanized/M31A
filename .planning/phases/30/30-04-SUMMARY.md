---
phase: 30
plan: 30-04
title: "Wave 4 — REPL Screen Polish"
subsystem: "internal/tui"
tags: ["repl", "tool-card", "thinking-block", "welcome-screen", "opencode-style"]
requires: []
provides: []
affects: ["repl_view.go", "repl_model.go", "repl_state.go", "repl_welcome.go"]
tech-stack:
  added: []
  patterns: ["ThinBorder for all card-style components", "Panel-style thinking blocks with header/footer borders"]
key-files:
  created: []
  modified:
    - internal/tui/repl_view.go
    - internal/tui/repl_model.go
    - internal/tui/repl_state.go
    - internal/tui/components/message.go
    - internal/tui/components/toolcard.go
    - internal/tui/components/thinking.go
    - internal/tui/repl_welcome.go
decisions:
  - "Remove bottom border — status bar sits flush below textarea (opencode style)"
  - "Remove textarea border — inherits terminal background"
  - "User label: lowercase 'user', muted, no bold"
  - "Tool cards: ThinBorder (┌─┐) replaces double-border (╔═╗)"
  - "Thinking blocks: panel-style with header/footer ThinBorder borders"
  - "Welcome: remove starfield, compact layout, thin borders, muted hint list"
metrics:
  duration: "~20 min"
  completed_date: "2026-06-08"
---

# Phase 30 Plan 30-04: REPL Screen Polish

## One-Liner

Remove bottom border and textarea border, lowercase user labels, thin tool card borders, panel-style thinking blocks, and compact welcome screen — all consistent with the opencode-inspired visual redesign.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Remove bottom border and textarea border from REPL | `a431b31` | repl_view.go, repl_model.go |
| 2 | Refine message rendering | `f667be9`, `d3595c6` | message.go, repl_state.go |
| 3 | Refine tool cards | `0276bb3` | toolcard.go |
| 4 | Refine thinking block | `26a89a6` | thinking.go |
| 5 | Refine welcome screen | `daa08d3` | repl_welcome.go |

## Key Changes

### Task 1 — Remove borders
- **Bottom border removed**: The `╹▀▀▀` line below textarea is deleted. The status bar now sits flush below the textarea.
- **Viewport chrome adjusted**: `viewportBottomChrome()` returns `inputHeight + 3` (was `+4`).
- **Textarea border disabled**: `textarea.FocusedStyle.Base` and `textarea.BlurredStyle.Base` set to `lipgloss.NewStyle()`.

### Task 2 — Message rendering
- **User label**: Changed from uppercase `USER` to lowercase `user` with muted foreground (`t.TextMuted`), no bold.
- **Timestamp bars**: Added `Faint(true)` for reduced visual weight.
- **Turn spacing**: Extra blank line inserted between messages when role changes (user↔assistant).

### Task 3 — Tool cards
- **Border style**: Replaced double-border (`╔═╗`) with `theme.ThinBorder` (`┌─┐`).
- **Header**: Single-line format — tool badge + input snippet on left, status icon + timing on right.
- **Output**: 2-char left padding, no inner border.
- **Collapsed state**: Status icon + tool name + input + `[+N lines]` format.

### Task 4 — Thinking blocks
- **Panel style**: Rewrote rendering to use ThinBorder with header panel, body content, and footer panel.
- **Header**: `▸/▾ Thinking · duration` with toggle hint.
- **Body**: Italic thinking content (blue `#8AB4F8`).
- **Footer**: Duration text.
- **Collapsed**: Single-line ThinBorder panel with header only.
- **Scroll**: Scroll indicators preserved for content >20 lines.

### Task 5 — Welcome screen
- **Starfield removed**: `RenderStarfield()` call deleted.
- **Provider card**: Uses `theme.ThinBorder` instead of `RoundedBorder`.
- **Keyboard hints**: Changed from `FilterChips` pills to muted single-line list separated by `·`.
- **Layout**: Compact vertical stack without optional decorative row.

## Deviations from Plan

None — plan executed as written.

## Verification Results

| Check | Status |
|-------|--------|
| `CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a` | ✅ PASS |
| `go vet ./internal/tui/...` | ✅ PASS |
| `go test -race -count=1 -cover ./internal/tui/...` | ✅ PASS |

## Self-Check

- [x] File `internal/tui/repl_view.go` — modified, exists, builds
- [x] File `internal/tui/repl_model.go` — modified, exists, builds
- [x] File `internal/tui/repl_state.go` — modified, exists, builds
- [x] File `internal/tui/components/message.go` — modified, exists, builds
- [x] File `internal/tui/components/toolcard.go` — modified, exists, builds
- [x] File `internal/tui/components/thinking.go` — modified, exists, builds
- [x] File `internal/tui/repl_welcome.go` — modified, exists, builds
- [x] Commit `a431b31` exists
- [x] Commit `f667be9` exists
- [x] Commit `d3595c6` exists
- [x] Commit `0276bb3` exists
- [x] Commit `26a89a6` exists
- [x] Commit `daa08d3` exists

## Self-Check: PASSED
