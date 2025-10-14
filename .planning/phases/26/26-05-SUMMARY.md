# Plan 26-05 — Diff/ModelSelector/GoalInput/Metrics Fixes: COMPLETE

## What Was Built

### WIR-04: Diff Screen Split View Toggle
- Added `splitView` bool field to `DiffModel`
- `V` key toggles between unified and split view
- Split view: file list (left panel) + diff content (right panel), separated by border
- Help bar updated to mention `V split view`

### WIR-05: ModelSelector Mock Latency
- Removed `generateMockUsageData()` — no more fabricated usage data
- Usage data now initialized as all-zeros (no real data source yet)
- List delegate shows sparkline only when real usage data exists (non-zero values)
- No misleading "Used Nx this week" labels when data is unavailable

### WIR-06: GoalInput PhaseInitialize Routing
- GoalInputModel now emits `goal_submitted` action on Ctrl+Enter
- `app_update.go` handles `goal_submitted` by routing to `PhaseInitialize` first
- Chain is now: GoalInput → PhaseInitialize → PhaseDiscuss → ScreenDiscuss
- Added `GoalSubmittedMsg` type to `types.go`

### WIR-07: Metrics Sparkline
- Added `sparklineData []float64` field to `MetricsModel`
- `LoadStats` populates sparkline data from daily session counts
- `renderDailyUsage` shows sparkline above the bar chart using `components.RenderSparkline`
- Sparkline uses brand color and renders 14-day trend

## Verification
- `go build ./...` — clean
- `go test ./internal/tui/...` — all pass
- `go vet ./...` — clean
