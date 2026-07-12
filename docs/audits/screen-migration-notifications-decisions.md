# Screen Migration: Notifications & Decisions

**Date:** 2026-07-12
**Status:** Complete

## Summary

Migrated two screens to the Screenable + Router architecture:
1. **NotificationModel** (ScreenNotifications) — updated existing model
2. **ScreenDecisions** — created new lightweight `DecisionScreen` wrapper (previously had no sub-model)

## Changes

### NotificationModel (`notification_model.go`)

- Changed `Update()` return type from `(tea.Model, tea.Cmd)` to `(Screenable, tea.Cmd)`
- Comment updated from "implements tea.Model" to "implements Screenable"
- `SetTheme()`, `SetDimensions()`, `Init()`, `View()` already satisfied Screenable interface

### DecisionScreen (`decision_screen.go`) — NEW FILE

- Created lightweight `DecisionScreen` struct implementing `Screenable`
- Encapsulates the decision log rendering logic previously inlined in `renderDecisionsContent`
- Holds `decisions []decision.DecisionReceipt` slice, updated via `SetDecisions()`
- Handles `esc`/`q` key events to `PopScreenMsg{}` for back navigation
- Nil-guard for `themeManager` in `ensureSubModel` and `routeToScreen` (test safety)

### AppState (`app_state.go`)

- Added `decisionScreen *DecisionScreen` field

### Routing (`app_routing.go`)

- **ScreenNotifications**: Added `m.router.Register(ScreenNotifications, m.notifModel)` on first message
- **ScreenDecisions**: Replaced empty no-op updater with full router registration pattern — creates `DecisionScreen` on first message, registers with router, delegates Update

### View (`app_view.go`)

- **renderNotificationsContent**: Added router registration + `m.router.View()` fallback to `m.notifModel.View()`
- **renderDecisionsContent**: Replaced 70-line inline rendering with router delegation — creates `DecisionScreen`, sets decisions, registers, returns `m.router.View()`
- Removed unused `"github.com/eshanized/M31A/internal/decision"` import

### Navigation (`app_nav.go`)

- **routeToScreen**: Added `ScreenDecisions` case — creates `DecisionScreen` if nil
- **ensureSubModel**: Replaced nil-return for `ScreenDecisions` with model creation + dimension setting (with `themeManager` nil guard)

## Verification

- `go build ./...` — clean
- `go vet ./internal/tui/...` — only pre-existing emitter_stress_test.go copylocks
- `golangci-lint run ./internal/tui/...` — only pre-existing emitter_stress_test.go copylocks
- `go test -race ./internal/tui/...` — all pass

## Data Flow (unchanged)

- **Toast system**: `addToastCmd()` → `addToast()` → `m.notifModel.AddNotification()` — direct call, bypasses router
- **Decisions data**: `handleDecisionsSnapshot()` → `m.cachedDecisions` → `SetDecisions()` on `DecisionScreen` → rendered via router
- **Back navigation**: Both models return `PopScreenMsg{}` via `tea.Cmd`, handled by standard update loop

## Migration Status Update

With these two screens migrated, the remaining legacy screens are:
- ScreenREPL (ReplModel) — Update returns `(tea.Model, tea.Cmd)`
- ScreenChatHistory (ChatHistoryModel) — Update returns `(tea.Model, tea.Cmd)`
- ScreenFirstRun (FirstRunModel) — already migrated to Screenable but uses nil-guard pattern
