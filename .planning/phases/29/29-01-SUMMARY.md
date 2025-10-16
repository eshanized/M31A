# Plan 29-01 Summary: Foundation — Types, Theme, Constants

## Status: COMPLETE ✅

## What Was Done
- Deleted all 122 existing TUI Go files across internal/tui/, internal/tui/components/, internal/tui/theme/
- Created internal/tui/types.go: 16 Screen enum values (iota 0-15), 25+ message types, ShipSummary, PendingPermissionRequest, FallbackEvent structs
- Created internal/tui/theme/colors.go: Dark/Light palette color constants (DarkBackground through DarkSyntaxType, LightBackground through LightSyntaxType, BrandColor)
- Created internal/tui/theme/theme.go: Theme struct with ~50 Lipgloss Style/Color fields, Dark() and Light() constructors, initStyles() for all styled elements, ThemeManager with Cycle()/SetMode()/Current()
- Created internal/tui/constants.go: Layout (MinTerminalWidth=80, DefaultSidebarWidth=40), Timing (PermissionCountdownDefault=300, ToastDuration=10s, DiscussTimeout=5m), Rendering (MaxToolOutputLines=20, MaxMessageHistory=200), Spinner, String constants

## Files Created
- internal/tui/types.go
- internal/tui/constants.go
- internal/tui/theme/colors.go
- internal/tui/theme/theme.go

## Verification
- `go build ./internal/tui/...` — PASS
- `go build ./internal/tui/theme/...` — PASS
- `go vet ./internal/tui/...` — PASS
- `go vet ./internal/tui/theme/...` — PASS
