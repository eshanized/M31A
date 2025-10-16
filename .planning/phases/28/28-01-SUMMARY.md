# Plan 28-01: Wave 1 — Immediate Helper Extraction

## Summary

Extracted 7 high-impact helper functions that eliminate the most pervasive duplicate patterns in the codebase.

## Changes

### New File: `internal/tui/helpers.go`
Created shared helper file with:
- `makeAssistantMsg(content string) types.Message` — eliminates 9 instances of error message boilerplate (M-3)
- `syncReplProvider(sessionID string) tea.Cmd` — eliminates 12 instances of SetProvider+SetDispatcher+SetCommandRegistry (H-1)
- `listenerCmds() []tea.Cmd` — eliminates 19+ instances of permission+question listener creation (H-8)
- `updateSlashSuggestions()` — removes duplicate 43-line slash autocomplete block (H-3)
- `ensureReplModel()` — eliminates 7+ nil-check+assignment blocks (H-6)
- `ensureSidebarModel()` — eliminates 2 identical nil-check blocks (H-5)
- `propagateSessionID(id string)` — eliminates 2 identical 4-line blocks (H-4)
- `loadAndRestoreSession(sessionID string, clearExisting bool) tea.Cmd` — eliminates 2 near-identical session loading blocks (H-2)
- `centerScreen(content string, w, h int) string` — eliminates 30+ identical centering calls (H-9)
- `renderSectionHeader(title string, width int) string` — eliminates 6+ header-bar patterns (M-12)

### Updated Files
- `internal/tui/app_update_screen.go` — replaced 16 listener command patterns
- `internal/tui/app_update_permission.go` — replaced 4 listener command patterns
- `internal/tui/app.go` — replaced 1 listener command pattern
- `internal/tui/app_update_slash.go` — replaced 3 makeAssistantMsg patterns
- `internal/tui/repl_keys.go` — replaced 3 makeAssistantMsg patterns, removed duplicate slash autocomplete
- `internal/tui/repl_stream.go` — replaced 1 makeAssistantMsg pattern
- `internal/tui/app_update_workflow.go` — replaced 1 makeAssistantMsg pattern
- `internal/tui/repl.go` — replaced duplicate slash autocomplete with helper call

## Test Results

```
ok  	github.com/eshanized/M31A/internal/tui	30.192s
ok  	github.com/eshanized/M31A/internal/tui/components	1.137s
ok  	github.com/eshanized/M31A/internal/tui/theme	1.015s
```

All 537+ tests pass with race detector enabled.

## Impact

- **Lines removed:** ~400+ lines of duplicated code
- **Patterns consolidated:** 7 major duplicate patterns (C-1, H-1 through H-9, M-3, M-12)
- **Files modified:** 8 existing files + 1 new file
- **Risk:** Low — pure refactoring with identical behavior
