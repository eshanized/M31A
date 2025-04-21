# Wire Workflow Engine + Settings Emission + End-to-End Streaming

## Context

The wiring audit (W-01 through W-13) has been fixed. All 13 issues are resolved:
- Streaming pipeline wired (ReplModel Enter handler calls StartStreamCmd)
- CommandRegistry instantiated and called in REPL
- Workflow screens (Plan/Execute/Verify/Ship) have model fields and Update/View wired
- Provider fallback emits FallbackEventMsg on 429/503
- SettingsSavedMsg handler exists in AppState.Update()
- Dead types (HealthUpdateMsg, ProviderSwitchMsg) removed

All builds clean, all 22 packages pass tests, binary is statically linked.

## Remaining Gaps to Fix

There are three critical gaps that remain after the wiring fixes:

### Gap 1: Workflow Engine Not Instantiated in AppState

The `workflow.Engine` exists in `internal/workflow/engine.go` with `NewEngine()`, `RunPhase()`, `Transition()`, and `buildSystemPrompt()`. It is fully implemented and tested. But it is **never instantiated in production code** — only in test files.

The TUI needs the engine so that when a user enters a goal or triggers `/phase`, the workflow phases (Initialize → Discuss → Plan → Execute → Verify → Ship) can run and their results bridge to screen transitions.

**What to do:**
1. Add `workflowEngine *workflow.Engine` field to `AppState` struct
2. Instantiate the engine in `NewApp()` after registry and config are ready — pass the registry, dispatcher, git instance, workDir, and session manager
3. Add a message type `PhaseResultMsg` that carries workflow phase completion data (tasks, errors, next phase) from the engine to the TUI
4. Wire a command or handler that starts the workflow — e.g., when user types a goal message in REPL, or uses `/phase initialize`, the engine's `RunPhase()` is called via a `tea.Cmd` (goroutine) and emits `PhaseResultMsg` on completion
5. Add a `PhaseResultMsg` handler in `AppState.Update()` that transitions to the appropriate screen (Plan, Execute, Verify, Ship) based on the phase result

### Gap 2: SettingsSavedMsg Never Emitted

`SettingsSavedMsg` is defined in `types.go` and handled in `AppState.Update()`. But the settings screen's save action does **not** emit this message. The settings model returns `SettingsSavedMsg` from its Update method or it should be emitted after config save.

**What to do:**
1. Read `internal/tui/settings.go` to understand how settings saves work
2. Ensure that when the settings model saves config (after inline editing on any of the 6 tabs), it returns a `tea.Cmd` that emits `SettingsSavedMsg`
3. The `AppState.Update()` handler for `ScreenSettings` should propagate this message up — either the settings model returns `SettingsSavedMsg` directly as a tea.Cmd, or the AppState detects a save and emits it
4. The handler should restart health check and cache refresh tickers if provider/model changed

### Gap 3: End-to-End Streaming Verification

`StartStreamCmd` is now called in the ReplModel Enter handler. But several things need verification:
- The `StreamMsg` / `StreamDoneMsg` / `StreamErrorMsg` flow works correctly when returned as tea.Cmd from a goroutine
- The `TickMsg` handler keeps the viewport at bottom during streaming
- The `streamCancel` context is properly used on Ctrl+C
- Error handling: if `ChatCompletionStream` fails immediately (before any chunks), the error displays correctly

**What to do:**
1. Read `internal/tui/streaming.go` — verify `StartStreamCmd` returns a proper `tea.Cmd` (single message per invocation pattern)
2. Read `internal/tui/repl.go` — verify the Enter handler properly initializes streaming state before returning the cmd
3. Add a test file `internal/tui/streaming_test.go` with tests for:
   - `StartStreamCmd` returns a function that blocks until a message arrives
   - `StreamMsg` handler appends content correctly
   - `StreamDoneMsg` handler finalizes the message and resets streaming state
   - `StreamErrorMsg` handler displays error and resets streaming state
4. Consider adding a mock provider test that simulates a streaming response

## Instructions

Fix all three gaps above. For each:

1. **Read the relevant source files** first to understand current state
2. **Make minimal changes** — don't refactor, just wire what's missing
3. **After each fix**, run `CGO_ENABLED=0 go build -o m31a ./cmd/m31a` and `go vet ./...` to verify
4. **After all fixes**, run `go test -race -count=1 ./...` and verify all 22 packages pass

### Files to Read First

- `internal/workflow/engine.go` — understand Engine struct, NewEngine, RunPhase, Transition
- `internal/tui/settings.go` — understand how settings save works and where to emit SettingsSavedMsg
- `internal/tui/streaming.go` — verify StartStreamCmd pattern
- `internal/tui/repl.go` — verify Enter handler streaming state initialization
- `internal/tui/app.go` — understand current AppState structure for adding workflowEngine field
- `internal/provider/interface.go` — understand ChatRequest struct for building requests

### Expected Deliverables

1. `internal/tui/app.go` — workflowEngine field added, instantiated in NewApp, PhaseResultMsg handler wired
2. New message type(s) if needed (e.g., `PhaseResultMsg`) in `internal/tui/types.go`
3. `internal/tui/settings.go` — SettingsSavedMsg emitted on save
4. `internal/tui/streaming_test.go` — tests for streaming pipeline
5. All builds clean, all tests pass

### Constraints

- Follow AGENTS.md rules: Bubble Tea single-threaded (state mutations through Update only), use tea.Cmd/tea.Msg for async work
- No new dependencies beyond what's already in go.mod
- No CGO
- No telemetry or analytics
- Keep changes minimal — wire what's missing, don't redesign
