---
phase: 15-comprehensive-audit-fixes
plan: 08
status: complete
commit_range: pending
files_modified:
  - internal/tui/types.go
  - internal/tui/commands.go
  - internal/tui/commands_config.go
  - internal/tui/commands_test.go
  - internal/tui/repl_stream.go
  - internal/tui/plan.go
  - internal/tui/components/toolcard.go
files_created:
  - internal/tui/typed_errors_test.go
---

# 15-08 Summary — Typed Errors & Dead Config

## Findings Addressed

### H-11 — Typed error banner for ErrContextExceeded
**Status:** Fixed
**Change:** Added `renderErrorBanner(err)` helper that uses `errors.Is` to match `ErrContextExceeded` (yellow), `ErrInvalidKey` (red), `ErrRateLimited` (orange), and falls back to generic red. Replaced the generic `"Error during streaming: %v"` message in `handleStreamErrorMsg`.
**Files:** `internal/tui/repl_stream.go`

### H-19 — /optimize wired to arbitrage
**Status:** Fixed
**Change:** Updated `handleOptimize` to read `config.Model.AutoArbitrage` (M-12) and `config.Model.ArbitrageThreshold` (M-14). Added "O"/"o" key to Plan screen that emits `SlashCommandMsg{Command: "/optimize"}`. Added `OptimizedMsg` type to `internal/tui/types.go`.
**Files:** `internal/tui/commands_config.go`, `internal/tui/plan.go`, `internal/tui/types.go`

### M-12 — AutoArbitrage config flag read
**Status:** Fixed
**Change:** `handleOptimize` checks `ctx.Config.Model.AutoArbitrage` and returns an error message when disabled.
**Files:** `internal/tui/commands_config.go`

### M-14 — ArbitrageThreshold config flag read
**Status:** Fixed
**Change:** `handleOptimize` reads `ctx.Config.Model.ArbitrageThreshold` (default 0.15) and passes it to `arbitrage.Recommend`.
**Files:** `internal/tui/commands_config.go`

### M-17 — AutoCollapseTools config flag honored
**Status:** Fixed
**Change:** Added `SetCollapsed(bool)` method to `ToolCard`. Tool cards created in `handleStreamDoneMsg` now call `card.SetCollapsed(true)` when `m.cfg.Model.AutoCollapseTools` is true.
**Files:** `internal/tui/components/toolcard.go`, `internal/tui/repl_stream.go`

### M-23 — /cost toggles ShowCostEstimate
**Status:** Fixed
**Change:** Added `handleCost` command handler that toggles `Config.UI.ShowCostEstimate` and persists via `Config.Save`. Registered as `/cost` in the command registry.
**Files:** `internal/tui/commands_config.go`, `internal/tui/commands.go`

### M-24 — Debug-mode log typed error name
**Status:** Fixed
**Change:** Added `typedErrorName(err)` helper that walks the error chain and returns the sentinel name. Called in `handleStreamErrorMsg` when `M31A_LOG_LEVEL=debug`.
**Files:** `internal/tui/repl_stream.go`

## Tests Added (12 tests, all pass with -race)

| Test | Finding | What it verifies |
|------|---------|-----------------|
| `TestOptimizeCommand_RespectsAutoArbitrageFlag` | M-12 | AutoArbitrage=false → failure |
| `TestOptimizeCommand_UsesThreshold` | M-14 | Threshold reads from config |
| `TestTypedErrorBanner_ContextExceeded` | H-11 | Yellow banner for context exceeded |
| `TestTypedErrorBanner_InvalidKey` | H-11 | Red banner for invalid key |
| `TestTypedErrorBanner_RateLimited` | H-11 | Orange banner for rate limit |
| `TestTypedErrorBanner_GenericError` | H-11 | Generic red for unknown errors |
| `TestTypedErrorName_ContextExceeded` | M-24 | Returns "ErrContextExceeded" |
| `TestTypedErrorName_ChainedWrapped` | M-24 | Wrapped error returns correct name |
| `TestTypedErrorName_Unknown` | M-24 | Unknown error returns "unknown" |
| `TestAutoCollapseTools_Respected` | M-17 | SetCollapsed works correctly |
| `TestCostCommand_TogglesFlag` | M-23 | Toggle + persistence to disk |
| `TestTypedErrorName_DebugLog` | M-24 | Error handler adds message to list |
