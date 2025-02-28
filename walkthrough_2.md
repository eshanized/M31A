# Walkthrough 2 — TUI Foundation

## Completed Plans

### Plan 01 — Theme System & Shared Types (Wave 1)
- [x] `internal/tui/types.go`: `Screen` enum (ScreenREPL, ScreenFirstRun, ScreenHelp, ScreenExit), `AppMsg` with generic payload, `HealthUpdateMsg`, `HealthCheckTickMsg`, `ProviderSwitchMsg`, `ErrorMsg`
- [x] `internal/tui/theme/theme.go`: `Theme` struct with 12 color fields + 18 lipgloss styles, Dark/Light/Auto/Default palettes, `Manager` with `Current()`, `Set()`, `Cycle(auto→light→dark)`
- [x] Color parity across palettes (same tool labels map to different hex values)
- [x] `internal/tui/theme/theme_test.go` — 8 tests (manager init, dark/light specific colors, auto fallback, cycle, tool label keys, Default)

### Plan 02 — Layout Components + Health Ticker (Wave 2)
- [x] `internal/tui/header.go`: `RenderHeader` — brand name in accent, provider badge ([OP]/[ZE]), model name dim, context bar with color shift at 80%/95%, health status (green/yellow/red dots + label)
- [x] `internal/tui/header_test.go` — 16 tests (brand and provider badge, model display, context bar at 0%/80%/95%, health states green/yellow/red/unhealthy, truncation)
- [x] `internal/tui/statusbar.go`: `RenderStatusBar` — operation text left-aligned, timestamp right-aligned, smart truncation with ellipsis
- [x] `internal/tui/statusbar_test.go` — 10 tests (ready state, operation display, timestamp alignment, truncation)
- [x] `internal/tui/health.go`: `HealthCheckTicker` struct + `NextHealthTick` — `tea.Tick`-based scheduling with configurable interval (default 60s), context extraction from AppState
- [x] `internal/tui/health_test.go` — 11 tests (nil context, zero interval, next interval calculation types)

### Plan 03 — REPL Screen (Wave 2)
- [x] `internal/tui/repl.go`: `ReplModel` — 4-region layout (viewport for conversation history, textarea for input, spinner for loading state, status line), input history with up/down navigation, `NewReplModel`/`Init`/`Update`/`View` implementing `tea.Model`
- [x] Textarea disabled during processing, re-enabled when idle
- [x] Spinner runs at 1ms tick (configurable)
- [x] `internal/tui/repl_test.go` — 22 tests (enter sends message, resize, history up/down, scroll, empty input, status text, spinner states)

### Plan 04 — First-Run Wizard (Wave 2)
- [x] `internal/tui/firstrun.go`: `FirstRunModel` — 6-state wizard: Welcome→ProviderSelect→KeyInput→Validating→KeychainPrompt→Complete, masked API key input, provider toggle (OpenRouter/Zen via Enter key), emits `AppMsg{ScreenREPL}` on completion
- [x] Skip flow: pressing `ctrl+w` at Welcome jumps to Complete → REPL
- [x] `internal/tui/firstrun_test.go` — 23 tests (state transitions, provider selection, key input masking, validation states, keychain prompt, skip flow, view rendering)

### Plan 05 — AppState & Screen Routing (Wave 3)
- [x] `internal/tui/app.go`: `AppState` — full `tea.Model` implementation, screen routing (FirstRun → REPL), health lifecycle (60s tick, provider health check on tick), full-view composition (header + body + statusbar), error state display, terminal-too-small guard (80×24 threshold)
- [x] `NewApp` accepting `AppConfig` with theme manager reference
- [x] `calculateNextInterval` — exponential backoff with jitter (60s base, 5 min max, uniform)
- [x] `internal/tui/app_test.go` — 20 tests (init, new app, ctrl+c quit, first-run→REPL transition, health tick reschedule, terminal-too-small, error state, view composition)
- [x] Binary not wired — prints version and exits (no main.go changes)
- [x] All state mutations through `Update()` only — no goroutine mutations

## Build Verification
- [x] `CGO_ENABLED=0 go build ./cmd/m31a` produces static binary
- [x] `go build ./...` passes
- [x] `go vet ./...` passes (zero warnings)
- [x] `go test -race -count=1 ./...` passes (110+ tests)

### go build ./...
```
(no output = clean)
```

### go vet ./...
```
(no output = clean)
```

### go test -race -count=1 ./internal/tui/...
```
ok  	github.com/eshanized/M31A/internal/tui	1.043s
ok  	github.com/eshanized/M31A/internal/tui/theme	1.016s
```

### Binary verification
```
m31a: ELF 64-bit LSB executable, x86-64, version 1 (SYSV), statically linked, BuildID=43e4... with debug_info, not stripped
```

### All tests (with race detection)
```
ok  	github.com/eshanized/M31A/internal/provider	1.012s
ok  	github.com/eshanized/M31A/internal/provider/openrouter	1.286s
ok  	github.com/eshanized/M31A/internal/provider/zen	1.019s
ok  	github.com/eshanized/M31A/internal/tui	1.043s
ok  	github.com/eshanized/M31A/internal/tui/theme	1.016s
```

## Deviations from Spec
- **Provider badge format**: Uses first 2 chars uppercase `[OP]`/`[ZE]` instead of planned `[OR]` (follows literal provider name prefix rule)
- **HealthCheckInterval**: Already defined in `internal/types/constants.go` — reused instead of redeclared; `health.go` keeps local const for its own use
- **First-run completion**: `AppMsg{ScreenREPL}` emitted immediately from `selectOption()` and `updateKeychainPrompt()`, not deferred to `updateComplete()`
- **REPL history**: `historyPos = len(inputHistory)`, first up arrow gives most recent entry (index `len-1`)
- **Go version**: Auto-upgraded from 1.22 to 1.24.2 by `go get` — no compatibility issues

## Open Questions / Blockers
None. Phase 2 is complete and verified.

## Deliverable Summary
Phase 2 implemented the complete Bubble Tea TUI Foundation for M31A across three waves and five plans. The theme system provides Dark/Light/Auto palettes with 18 ready-to-use lipgloss styles. Layout components (header with context/health bar, status bar with timestamp) compose into a consistent shell. The REPL screen delivers a functional conversation interface with viewport, input, spinner, and history navigation. The first-run wizard guides users through provider API key setup in 6 states. The top-level AppState ties everything together with screen routing, 60s health tick lifecycle, and terminal-size guard. All 110+ tests pass with race detection enabled, binary builds as static without CGO, and zero vet warnings.
