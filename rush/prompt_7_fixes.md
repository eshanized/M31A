# Phase 7 Fix Prompt

You are fixing issues identified in the Phase 7 implementation audit. All Phase 7 code compiles and most tests pass, but there are 6 specific issues to resolve.

## Context

Phase 7 added 7 signature features to M31A (a Go terminal AI coding assistant). The code exists and compiles. All existing tests from Phases 0-6 still pass. This is a targeted fix pass — do NOT rewrite any files from scratch. Only make the specific changes described below.

## Existing Code

The following Phase 7 files already exist and are functional:

- `internal/tui/commands.go` (699 lines) — slash command system with 16 handlers
- `internal/tui/commands_test.go` (943 lines) — 26 tests
- `pkg/rollback/rollback.go` (298 lines) — git rollback chain
- `pkg/rollback/rollback_test.go` (503 lines) — 18 tests
- `pkg/arbitrage/arbitrage.go` (287 lines) — model cost arbitrage
- `pkg/arbitrage/arbitrage_test.go` (355 lines) — 16 tests
- `pkg/autodream/autodream.go` (338 lines) — context consolidation
- `pkg/autodream/autodream_test.go` (499 lines) — 19 tests
- `pkg/ledger/ledger.go` (512 lines) — session ledger viewer
- `pkg/ledger/ledger_test.go` (659 lines) — 23 tests
- `internal/tui/modelselector.go` (448 lines) — model selector screen
- `internal/tui/modelselector_test.go` — **MISSING**
- `internal/tui/settings.go` (898 lines) — updated with 6 tabs + inline editing
- `internal/tui/settings_test.go` (448 lines) — 17 tests
- `internal/tui/types.go` (89 lines) — added FallbackEventMsg, RefreshCacheMsg, etc.
- `internal/provider/cache.go` (143 lines) — added ModelCacheRefreshTicker
- `internal/tui/components/thinking.go` (117 lines) — has Toggle() method
- `internal/tui/app.go` (476 lines) — AppState with CacheRefreshTicker scheduled

## Issues to Fix

### Fix 1: Duplicate `fallback` Command Registration

**File:** `internal/tui/commands.go`

In `DefaultCommands()`, the `fallback` command is registered twice (around lines 153-154). Remove the duplicate registration. The command should only be registered once.

### Fix 2: Create `modelselector_test.go`

**File:** `internal/tui/modelselector_test.go` — **NEW FILE**

The model selector screen has zero test coverage. Read `internal/tui/modelselector.go` to understand the `ModelSelector` struct and its methods, then write comprehensive tests.

The ModelSelector uses:
- `NewModelSelector(registry *provider.Registry, theme theme.Theme)` — constructor
- `Init() tea.Cmd` — triggers model fetching
- `Update(msg tea.Msg) (tea.Model, tea.Cmd)` — handles key events, search, provider filter, model selection
- `View() string` — renders the screen
- `SelectedModel() *types.ModelInfo` — returns selected model
- Key bindings: Enter (select), Esc (cancel), `/` (search), `j/k` (navigate), `p` (provider filter)
- Messages: `modelFetchCompleteMsg` (models loaded), error messages

Write these tests (minimum 15):

1. `TestModelSelector_New` — constructor initializes with loading state, empty model list
2. `TestModelSelector_Init` — Init returns non-nil cmd (triggers fetch)
3. `TestModelSelector_ModelsLoaded` — send modelFetchCompleteMsg, verify models populated, list updated
4. `TestModelSelector_SelectModel` — navigate to model with `j`, press Enter, verify SelectedModel returns it
5. `TestModelSelector_Cancel` — press Esc, verify screen transitions back (returns AppMsg with previous screen)
6. `TestModelSelector_Search` — type in search input, verify filtered list narrows
7. `TestModelSelector_SearchCaseInsensitive` — search matches regardless of case
8. `TestModelSelector_SearchClear` — clear search, verify full list restored
9. `TestModelSelector_ProviderFilter` — press `p`, verify filtered to specific provider
10. `TestModelSelector_ProviderFilterCycle` — press `p` multiple times, cycles through providers + "All"
11. `TestModelSelector_Navigation` — j/k or up/down navigate the list, wrap around at boundaries
12. `TestModelSelector_View_Loading` — View shows loading state before models loaded
13. `TestModelSelector_View_Loaded` — View shows model list after models loaded
14. `TestModelSelector_View_SearchActive` — View shows search input when search is focused
15. `TestModelSelector_EmptyModels` — handles empty model list gracefully (shows "no models found")
16. `TestModelSelector_ErrorState` — handles fetch error, shows error message with retry option
17. `TestModelSelector_DetailView` — Enter on selected model or detail key shows expanded view

Use `github.com/eshanized/M31A/internal/provider` for a mock registry. Create test models with `types.ModelInfo{}` structs. Use `tea.KeyMsg` for key events.

### Fix 3: Handle `FallbackEventMsg` in `app.go`

**File:** `internal/tui/app.go`

`AppMsg` has a `FallbackEvent *FallbackEventMsg` field (defined in `types.go`), but there is no case handler for it in `AppState.Update()`.

Add a case in the `Update()` method's `AppMsg` handler (where `ProviderSwitchMsg` and `HealthUpdateMsg` are already handled):

```go
case m31types.FallbackEventMsg:
    // Log the fallback
    slog.Info("provider fallback",
        "from", msg.FallbackEvent.From,
        "to", msg.FallbackEvent.To,
        "reason", msg.FallbackEvent.Reason,
    )
    // Update active provider
    m.activeProvider = msg.FallbackEvent.To
    m.fallbackNotification = &FallbackNotification{
        Event:   msg.FallbackEvent,
        ShownAt: time.Now(),
    }
    // Show notification to user
    return m, nil
```

Also add the `FallbackNotification` type to `app.go`:

```go
type FallbackNotification struct {
    Event     m31types.FallbackEventMsg
    Dismissed bool
    ShownAt   time.Time
}
```

Add an `x` key handler in the REPL (or globally in AppState.Update) that dismisses the notification by setting `m.fallbackNotification.Dismissed = true`.

### Fix 4: Wire Thinking Toggle Key Binding

**File:** `internal/tui/repl.go` (or wherever the REPL Update method is)

The `ThinkingBlock` component in `internal/tui/components/thinking.go` has a `Toggle()` method, but there is no `T` key binding that calls it.

In the REPL model's `Update()` method, add a case for the `t` or `T` key that toggles the thinking block:

```go
case "t", "T":
    m.thinkingBlock.Toggle()
    return m, nil
```

Also update the footer/help text to include "T: toggle thinking" in the key bindings display.

If the REPL model doesn't have a `thinkingBlock` field, add one:

```go
thinkingBlock *components.ThinkingBlock
```

And initialize it in the REPL constructor.

### Fix 5: Add Tests for `cache.go` Refresh Ticker

**File:** `internal/provider/cache_test.go` — **NEW FILE or UPDATE EXISTING**

`ModelCacheRefreshTicker` was added to `cache.go` but has no tests. Write tests for:

1. `TestModelCacheRefreshTicker_Interval` — verify ticker fires at the configured interval
2. `TestModelCacheRefreshTicker_Stop` — verify Stop prevents further ticks
3. `TestModelCacheRefreshTicker_Tick` — verify Tick returns appropriate message
4. `TestModelCache_IsExpired` — cache expires after TTL
5. `TestModelCache_IsStale` — cache becomes stale after staleTTL
6. `TestModelCache_Get_Set` — set models, get returns them
7. `TestModelCache_Get_Missing` — get non-existent model returns false

If `cache_test.go` already exists, add to it. If not, create it.

### Fix 6: Add Tests for `thinking.go` Toggle

**File:** `internal/tui/components/thinking_test.go` — **UPDATE EXISTING**

The `ThinkingBlock` component has a `Toggle()` method. Verify tests cover:

1. `TestThinkingBlock_Toggle` — Toggle() flips IsExpanded()
2. `TestThinkingBlock_InitialState` — verify default expanded state
3. `TestThinkingBlock_AfterToggle` — verify state after toggle
4. `TestThinkingBlock_Header_Expanded` — header shows "▼" when expanded
5. `TestThinkingBlock_Header_Collapsed` — header shows "▶" when collapsed

If `thinking_test.go` already exists (it does, with 143 lines), verify these cases are covered. If any are missing, add them.

---

## Implementation Order

1. **Fix 1** (duplicate registration) — 1 line change, fastest
2. **Fix 3** (fallback handler in app.go) — ~20 lines
3. **Fix 4** (thinking toggle key) — ~10 lines
4. **Fix 2** (modelselector tests) — ~400 lines of test code
5. **Fix 5** (cache tests) — ~150 lines
6. **Fix 6** (thinking tests) — verify existing, add missing

## Verification

After all fixes, run:

```bash
cd /home/snigdha/Desktop/Helix/M31A

# 1. Tidy modules
go mod tidy

# 2. Build (must succeed)
CGO_ENABLED=0 go build -o m31a ./cmd/m31a

# 3. Vet (must be clean)
go vet ./...

# 4. All tests must pass with race detection
go test -race -count=1 ./...

# 5. Verify test count increased (was ~464 before Phase 7)
go test -v -count=1 ./internal/tui/ ./internal/provider/ ./internal/tui/components/ 2>&1 | grep -c "=== RUN"
```

## Absolute Rules

1. **Do NOT rewrite existing files** — only make targeted fixes
2. **Do NOT change any test that already passes** — only add new tests or fix broken ones
3. **Do NOT modify Phase 0-6 files** — only touch Phase 7 files and app.go for the fallback handler
4. **All tests must pass** — no broken tests in the codebase
5. **CGO_ENABLED=0 build must succeed** — no CGO dependencies
6. **No new dependencies** — use only existing imports
7. **Bubble Tea single-threaded** — no goroutine state mutations in TUI code
8. **Follow existing patterns** — match the coding style already in the files
