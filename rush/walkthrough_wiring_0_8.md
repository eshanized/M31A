# M31A Wiring Audit — v0.8

> **Date:** 2026-05-29
> **Scope:** Connection audit — not whether components exist, but whether they are **wired together**
> **Method:** Trace every `tea.Msg` lifecycle, every screen transition, every registry instantiation, every cross-package call

---

## 1. Executive Summary

**Overall Wiring Health: PARTIALLY_PLUGGED**

The M31A codebase has a solid TUI shell with working screen transitions for the interactive screens (FirstRun, REPL, Settings, Resume, ModelSelector, Permission). However, **three major subsystems are fully implemented but disconnected from the TUI at runtime**:

1. **Command Registry** — 16 slash commands registered but **never instantiated** in AppState. All commands in the REPL are handled by inline `case` string checks for 3 commands (`/settings`, `/resume`, `/models`) and the remaining 13+ commands simply pass through as user text to an LLM that doesn't exist yet.
2. **Workflow Engine** — Full 6-phase engine with 6 screen models, but **`NewEngine()` is never called outside `_test.go`** and **no code ever transitions to `ScreenPlan`, `ScreenExecute`, `ScreenVerify`, or `ScreenShip`**.
3. **Streaming Pipeline** — `StartStreamCmd` is fully implemented but **never called in production code**. When the user presses Enter in REPL, the message is appended to history and `sent = true` is returned, but no LLM request is made.

Additionally, **two critical message types are emitted but never consumed** (`SettingsSavedMsg` drops at AppState level), and **the entire provider fallback path is defined but never triggered** (`FindFallbackProvider` is never called outside tests).

| Area | Status | Summary |
|------|--------|---------|
| Message Bus | PARTIALLY_PLUGGED | 8/12 Msg types fully wired; 3 defined but never emitted; 1 emitted but handler missing |
| Screen Transitions | PARTIALLY_PLUGGED | 6/10 screens reachable; 4 screens unreachable (Plan, Execute, Verify, Ship) |
| Command Registry | UNPLUGGED | Built but never instantiated; 3 commands handled inline; 13 dead |
| Workflow Engine | UNPLUGGED | Never instantiated; never called; 4 screens unreachable |
| Provider Fallback | UNPLUGGED | `FindFallbackProvider` never called; `FallbackEventMsg` never emitted |
| Streaming Pipeline | UNPLUGGED | `StartStreamCmd` never called; REPL returns without LLM request |
| Settings SavedMsg | BROKEN | Emitted by SettingsModel but AppState has no handler |

---

## 2. Wiring Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        cmd/m31a/main.go                         │
│  Creates: Registry, config, keychain                            │
│  Launches: tea.NewProgram(tui.NewApp(...))                      │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                     internal/tui/app.go (AppState)               │
│                                                                  │
│  INITIALIZED fields:                                            │
│    registry          ✅ from main.go                            │
│    config            ✅ loaded from disk                        │
│    keychain          ✅ (may be nil)                            │
│    sessionManager    ✅                                         │
│    dispatcher        ✅ DefaultDispatcher()                     │
│    settingsModel     ✅ NewSettingsModel()                      │
│    resumeModel       ✅ NewResumeModel()                        │
│    modelSelector     ✅ NewModelSelector() (if registry!=nil)   │
│    firstRunModel     ✅ (conditional — no apiKey)               │
│    replModel         ✅ (conditional — has apiKey)              │
│    ledger            ✅                                         │
│                                                                  │
│  NEVER INITIALIZED in AppState:                                 │
│    workflow.Engine   ❌ never called                            │
│    CommandRegistry   ❌ never called                            │
│                                                                  │
│  SCREEN ROUTING (inline string checks in Update):               │
│    /settings → ScreenSettings     ✅ wired                      │
│    /resume   → ScreenResume       ✅ wired                      │
│    /models   → ScreenModelSelector ✅ wired                     │
│    ALL OTHER /commands → fall through, no handling              │
│                                                                  │
│  MESSAGE HANDLERS:                                              │
│    tea.WindowSizeMsg         ✅ → all sub-models                │
│    tea.KeyMsg                ✅ → dispatch to current screen    │
│    HealthCheckTickMsg        ✅ → health poll                   │
│    RefreshCacheMsg           ✅ → cache refresh                 │
│    AppMsg                    ✅ → screen transitions            │
│    FallbackEventMsg          ✅ → updates activeProvider        │
│    ErrorMsg                  ✅ → sets currentOperation         │
│    PermissionRequestMsg      ✅ → ScreenPermission              │
│    PermissionResponseMsg     ✅ → back to prevScreen            │
│    SettingsSavedMsg          ❌ NO HANDLER in AppState          │
└────────┬──────────────────────────────┬────────────────────────┘
         │                              │
         ▼                              ▼
┌────────────────────┐     ┌────────────────────────────────────┐
│   REPL Flow        │     │   Workflow Flow (UNPLUGGED)         │
│                    │     │                                     │
│  Enter pressed     │     │  Engine.NewEngine()    ❌ never     │
│  → messages append │     │  Engine.RunPhase()     ❌ never     │
│  → sent = true     │     │  streamLLM()           ❌ never     │
│  → NO LLM call     │     │  StartStreamCmd()      ❌ never     │
│  → NO streaming    │     │                                     │
│                    │     │  ScreenPlan            ❌ no path   │
│  StartStreamCmd    │     │  ScreenExecute         ❌ no path   │
│  exists but never  │     │  ScreenVerify          ❌ no path   │
│  called            │     │  ScreenShip            ❌ no path   │
└────────────────────┘     └────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│  CommandRegistry (DEAD CODE)                                    │
│                                                                  │
│  DefaultCommands() registers 16 commands in commands.go         │
│  BUT: never instantiated in AppState                            │
│  BUT: registry.Execute() never called in app.go or repl.go      │
│  BUT: only handleHelp() calls DefaultCommands() internally      │
│                                                                  │
│  Inline handlers (3 commands):                                  │
│    /settings → m.screen = ScreenSettings    (app.go:203-205)   │
│    /resume   → m.screen = ScreenResume      (app.go:206-211)   │
│    /models   → m.screen = ScreenModelSelector (app.go:212-218) │
│                                                                  │
│  Dead commands (13 registered, never reachable):                │
│    /help /clear /status /model /provider /reset /quit           │
│    /undo /compress /ledger /rollback /sessions /goal            │
│    /phase /config /fallback /models                             │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│  Provider Fallback (UNPLUGGED)                                  │
│                                                                  │
│  FindFallbackProvider()  exists in fallback.go                  │
│  BUT: never called outside _test.go files                       │
│  BUT: no code detects 429/503 errors from provider calls        │
│  BUT: FallbackEventMsg never emitted                            │
│                                                                  │
│  FallbackEventMsg handler EXISTS in AppState (app.go:294-301)   │
│  FallbackEventMsg handler EXISTS in ReplModel (repl.go:241-246) │
│  BUT: nothing ever constructs FallbackEventMsg{}                │
└─────────────────────────────────────────────────────────────────┘
```

---

## 3. Message Bus Audit

| Msg Type | Defined | Emitted By | Handled By | Status |
|----------|---------|------------|------------|--------|
| `AppMsg` | types.go:25 | FirstRunModel, ResumeModel, ModelSelector, SettingsModel | AppState.Update | ✅ |
| `HealthCheckTickMsg` | types.go:44 | HealthCheckTicker (app.go:146) | AppState.Update:222 | ✅ |
| `RefreshCacheMsg` | types.go:77 | CacheRefreshTicker (app.go:147) | AppState.Update:240 | ✅ |
| `PermissionRequestMsg` | types.go:56 | permissionListenerCmd (app.go:157) | AppState.Update:307 | ✅ |
| `PermissionResponseMsg` | types.go:60 | inline in AppState.Update:194-196 | AppState.Update:315 | ✅ |
| `FallbackEventMsg` | types.go:65 | **NOWHERE** — never constructed | AppState:294, ReplModel:241 | ⚠️ |
| `ThinkingToggleMsg` | types.go:72 | ReplModel.Update (T key, repl.go:208) | ReplModel.Update:248 | ✅ |
| `ModelSelectedMsg` | types.go:82 | ModelSelector (modelselector.go:237) | AppState.Update:264 | ✅ |
| `SettingsSavedMsg` | types.go:88 | SettingsModel.saveCmd (settings.go:411), ctrl+s handler (settings.go:570) | SettingsModel.Update:495 (self), **NOT in AppState** | ⚠️ |
| `StreamMsg` | streaming.go:14 | StartStreamCmd goroutine (streaming.go:95) | ReplModel.Update:107 | ⚠️ |
| `StreamDoneMsg` | streaming.go:20 | StartStreamCmd goroutine (streaming.go:78) | ReplModel.Update:110 | ⚠️ |
| `StreamErrorMsg` | streaming.go:27 | StartStreamCmd goroutine (streaming.go:44,87) | ReplModel.Update:113 | ⚠️ |
| `HealthUpdateMsg` | types.go:37 | (defined but never used) | **NOWHERE** | ❌ |
| `ProviderSwitchMsg` | types.go:48 | (defined but never used) | **NOWHERE** | ❌ |
| `TickMsg` | streaming.go:32 | StreamTickCmd (streaming.go:134) | ReplModel.Update:116 | ✅ |
| `ErrorMsg` | types.go:52 | SettingsModel.saveCmd (settings.go:409), various | AppState.Update:303, SettingsModel.Update:500 | ✅ |

**Key findings:**
- `HealthUpdateMsg` and `ProviderSwitchMsg` are **dead types** — defined in types.go, part of AppMsg struct, but never constructed anywhere.
- `StreamMsg/StreamDoneMsg/StreamErrorMsg` are wired to ReplModel handlers but `StartStreamCmd` is never invoked, so they never fire.
- `FallbackEventMsg` has handlers in both AppState and ReplModel but is never emitted.
- `SettingsSavedMsg` is handled by SettingsModel itself (for dirty flag reset) but **AppState has no handler**, so if settings save triggers a provider change, AppState doesn't react.

---

## 4. Screen Transition Graph

### Reachable Screens

```
ScreenFirstRun ──AppMsg{ScreenREPL}──→ ScreenREPL
     ↑                                      │
     │ AppMsg{ScreenFirstRun}               │ inline string check
     │                                      ├ /settings → ScreenSettings
     │                                      ├ /resume   → ScreenResume
     │                                      └ /models   → ScreenModelSelector
     │                                              │
ScreenResume ←─AppMsg─────────────────────────┘      │
     │                                               │ Esc
     │ AppMsg{ScreenREPL}                            ▼
     │                                        (previous screen)
     ▼
ScreenREPL

ScreenSettings ──AppMsg{ScreenREPL} (Esc key)──→ ScreenREPL

ScreenPermission ──PermissionResponseMsg──→ m.prevScreen
```

### Unreachable Screens

| Screen | Status | Why |
|--------|--------|-----|
| `ScreenPlan` | **UNREACHABLE** | No code sets `m.screen = ScreenPlan`. PlanModel.Update returns `AppMsg{Screen: ScreenExecute}` on 'A' key, but nothing ever creates a PlanModel or transitions to it. |
| `ScreenExecute` | **UNREACHABLE** | No code sets `m.screen = ScreenExecute`. Only reachable from PlanModel's 'A' key, which is itself unreachable. |
| `ScreenVerify` | **UNREACHABLE** | No code sets `m.screen = ScreenVerify`. VerifyModel exists but nothing transitions to it. |
| `ScreenShip` | **UNREACHABLE** | No code sets `m.screen = ScreenShip`. ShipModel exists but nothing transitions to it. |

**All 4 workflow screens render placeholder text only:**
```go
// app.go:498-512
case ScreenPlan:
    return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center,
        "Plan screen — driven by workflow engine")
```

### Transition Completeness

| Transition | Path | Status |
|------------|------|--------|
| FirstRun → REPL | `AppMsg{Screen: ScreenREPL}` from firstrun.go:158,216,219,227 | ✅ |
| REPL → Settings | inline `case "/settings"` at app.go:203-205 | ✅ |
| REPL → Resume | inline `case "/resume"` at app.go:206-211 | ✅ |
| REPL → ModelSelector | inline `case "/models"` at app.go:212-218 | ✅ |
| Settings → REPL | `AppMsg{Screen: ScreenREPL}` from settings.go:515 (Esc key) | ✅ |
| Resume → REPL | `AppMsg{Screen: ScreenREPL}` from resume.go:186 (Esc key) | ✅ |
| Resume → FirstRun | `AppMsg{Screen: ScreenFirstRun}` from resume.go:175 ('N' key) | ✅ |
| ModelSelector → prevScreen | Esc key at app.go:393-395, ModelSelectedMsg at app.go:270 | ✅ |
| Permission → prevScreen | PermissionResponseMsg at app.go:317 | ✅ |
| **ANY → Plan** | No code path | ❌ |
| **ANY → Execute** | Only from PlanModel (unreachable) | ❌ |
| **ANY → Verify** | No code path | ❌ |
| **ANY → Ship** | No code path | ❌ |

---

## 5. Command Registry Status

The `CommandRegistry` in `commands.go` registers **16 slash commands** via `DefaultCommands()`. However:

### Instantiation
- **Never instantiated** in `AppState` or anywhere in production code.
- `DefaultCommands()` is only called from within `handleHelp()` (commands.go:167), which itself is never reachable because the registry is never created.

### Execution
- **`registry.Execute()` is never called** in `app.go`, `repl.go`, or any production file.
- The grep for `registry.Execute(` only finds references in planning documents and test files.

### Command Routing

| Command | Handler | Routing Method | Status |
|---------|---------|----------------|--------|
| `/settings` | inline string check | `app.go:203` `case "/settings"` | **INLINE** (works) |
| `/resume` | inline string check | `app.go:206` `case "/resume"` | **INLINE** (works) |
| `/models` | inline string check | `app.go:212` `case "/models"` | **INLINE** (works) |
| `/help` | `handleHelp` | registered but never invoked | **DEAD** |
| `/clear` | `handleClear` | registered but never invoked | **DEAD** |
| `/status` | `handleStatus` | registered but never invoked | **DEAD** |
| `/model` | `handleModel` | registered but never invoked | **DEAD** |
| `/provider` | `handleProvider` | registered but never invoked | **DEAD** |
| `/reset` | `handleReset` | registered but never invoked | **DEAD** |
| `/quit` | `handleQuit` | registered but never invoked | **DEAD** |
| `/undo` | `handleUndo` | registered but never invoked | **DEAD** |
| `/compress` | `handleCompress` | registered but never invoked | **DEAD** |
| `/ledger` | `handleLedger` | registered but never invoked | **DEAD** |
| `/rollback` | `handleRollback` | registered but never invoked | **DEAD** |
| `/sessions` | `handleSessions` | registered but never invoked | **DEAD** |
| `/goal` | `handleGoal` | registered but never invoked | **DEAD** |
| `/phase` | `handlePhase` | registered but never invoked | **DEAD** |
| `/config` | `handleConfig` | registered but never invoked | **DEAD** |
| `/fallback` | `handleFallback` | registered but never invoked | **DEAD** |

**Result:** 3 commands work via inline checks. 16 commands are registered but **zero** are reachable at runtime.

---

## 6. Workflow Engine Connectivity

### Engine Instantiation
- `workflow.NewEngine()` is **never called** outside `_test.go` files.
- Grep for `NewEngine` in non-test files: only the declaration in `engine.go:89`.

### Phase Execution
- `Engine.RunPhase()` is **never called** in production code.
- Grep for `RunPhase` in non-test files: only `engine.go:114` (declaration) and documentation references.

### Engine-Message Integration
- The engine's `streamLLM()` method (engine.go:213) uses `ChatCompletionStream()` + `consumeStream()` internally, producing a **plain `string`** return value.
- This does **NOT** connect to the TUI's streaming pipeline (`StreamMsg`, `StreamDoneMsg`). The engine consumes the stream synchronously; the TUI expects token-by-token `tea.Cmd` events.
- **Pattern mismatch:** Engine returns `(*PhaseResult, error)`; TUI expects `tea.Msg` through the event loop. No bridge exists.

### Screen Model Fields
- `AppState` does **NOT** have fields for `PlanModel`, `ExecuteModel`, `VerifyModel`, or `ShipModel`.
- These models exist as standalone types in their respective files but are only instantiated in `_test.go` files.
- The `View()` method for these screens in `AppState` renders placeholder text via `lipgloss.Place`.

### Phase Result Consumption
- `PhaseResult` (engine.go:79) is returned by `RunPhase()` but **never consumed** anywhere.
- No code receives a `PhaseResult` and triggers a screen transition or emits a `tea.Msg`.

### What Would Be Needed to Wire the Engine

```
1. Add engine field to AppState:
     engine *workflow.Engine

2. Instantiate engine when workflow begins:
     planningDir := filepath.Join(sessionDir, "planning")
     m.engine = workflow.NewEngine(sessionID, cwd, backupDir, planningDir,
         m.registry.ActiveProvider(), m.activeModel.ID,
         m.dispatcher, tokenEstimator, m.sessionManager)

3. Bridge engine output to TUI:
     result, err := m.engine.RunPhase(ctx, phase, goal)
     // Emit tea.Msg based on result:
     return m, func() tea.Msg {
         return AppMsg{Screen: ScreenPlan} // or ScreenExecute, etc.
     }

4. Wire each phase's result to the appropriate screen transition
```

---

## 7. Streaming Pipeline Status

### Current State: UNPLUGGED

The streaming pipeline has all its components implemented but **nothing connects user input to LLM streaming**.

### Component Inventory

| Component | Status | Used In Production? |
|-----------|--------|---------------------|
| `StartStreamCmd()` | Implemented (streaming.go:36) | ❌ Never called |
| `StreamMsg` | Emitted by StartStreamCmd | ⚠️ Handler exists, never fires |
| `StreamDoneMsg` | Emitted by StartStreamCmd | ⚠️ Handler exists, never fires |
| `StreamErrorMsg` | Emitted by StartStreamCmd | ⚠️ Handler exists, never fires |
| `StreamTickCmd()` | Implemented (streaming.go:133) | ⚠️ Called by ReplModel but only during streaming |
| `ReplModel.handleStreamMsg()` | Implemented (repl.go:273) | ✅ Handler ready |
| `ReplModel.handleStreamDoneMsg()` | Implemented (repl.go:313) | ✅ Handler ready |
| `ReplModel.handleStreamErrorMsg()` | Implemented (repl.go:345) | ✅ Handler ready |

### The Gap

When user presses Enter in REPL (`repl.go:142-168`):
```go
case "enter":
    input := strings.TrimSpace(m.textarea.Value())
    // ... validation ...
    userMsg := types.Message{Role: "user", Content: input, ...}
    m.messages = append(m.messages, userMsg)
    m.renderMessages()
    m.viewport.GotoBottom()
    m.textarea.Reset()

    var cmds []tea.Cmd
    return cmds, true   // ← NO LLM CALL, NO STREAMING
```

The `sent = true` return value signals to AppState that a message was sent, but:
1. No `provider.ChatRequest` is constructed
2. No `StartStreamCmd` is returned
3. No LLM call is made
4. The REPL just sits there with the user message rendered

### What's Missing

```go
// After appending user message, need:
ctx, cancel := context.WithCancel(context.Background())
m.streamCancel = cancel

req := provider.ChatRequest{
    Model:    activeModel.ID,
    Messages: m.messages,
    Stream:   true,
}

return cmds, StartStreamCmd(ctx, registry.ActiveProvider(), req, sessionID)
```

But `ReplModel` has **no access to the provider registry or active model**. These are `AppState` fields. The ReplModel would need either:
- A provider reference passed in at construction, or
- A message-based handshake where AppState receives the user message and initiates streaming

---

## 8. Provider Fallback Path

### Current State: UNPLUGGED

The fallback system is fully implemented at the provider layer but **never triggered**.

### Component Chain (Expected)

```
LLM call returns 429/503 error
  → ShouldFallback(statusCode) detects it
  → FindFallbackProvider(registry, currentProvider) finds healthy alternative
  → registry.SetActive(newProvider) switches
  → FallbackEvent constructed
  → FallbackEventMsg emitted into Bubble Tea loop
  → AppState.Update handles it (updates activeProvider, shows notification)
  → ReplModel.Update handles it (shows fallback banner)
```

### Where It Breaks

| Step | Component | Status |
|------|-----------|--------|
| Error detection | LLM provider clients | ❌ No code checks HTTP status codes from streaming errors |
| `ShouldFallback()` | `provider/fallback.go:16` | ✅ Implemented, never called |
| `FindFallbackProvider()` | `provider/fallback.go:20` | ✅ Implemented, only called in tests |
| `registry.SetActive()` | `provider/registry.go` | ✅ Available, called by settings/handlers |
| `FallbackEventMsg` construction | **NOWHERE** | ❌ Never constructed |
| `FallbackEventMsg` → AppState | `app.go:294-301` | ✅ Handler exists, updates `activeProvider` |
| `FallbackEventMsg` → ReplModel | `repl.go:241-246` | ✅ Handler exists, shows banner |

**The break is at step 6:** Nothing constructs `FallbackEventMsg{}`. The `provider.FallbackEvent` struct is returned by `FindFallbackProvider()`, but no code converts it to `tui.FallbackEventMsg` and feeds it into the Bubble Tea loop.

---

## 9. Deviation Register

| ID | Component | Type | Severity | Files | Description | Expected Connection | Actual State | Impact |
|----|-----------|------|----------|-------|-------------|---------------------|--------------|--------|
| W-01 | Command Registry | `dead_registry` | CRITICAL | commands.go, app.go, repl.go | Registry should handle all slash commands in REPL | Never instantiated; 13 commands unreachable | Users cannot use /help, /status, /quit, etc. |
| W-02 | Workflow Engine | `disconnected_subsystem` | CRITICAL | engine.go, app.go, plan.go, execute.go, verify.go, ship.go | Engine should execute phases and transition screens | Never instantiated; RunPhase never called | Full workflow is non-functional |
| W-03 | Streaming Pipeline | `missing_emission` | CRITICAL | streaming.go, repl.go | User input should trigger LLM streaming | StartStreamCmd never called; Enter just appends message | AI never responds to user input |
| W-04 | Provider Fallback | `missing_emission` | HIGH | fallback.go, app.go, registry.go | 429/503 errors should trigger fallback | FindFallbackProvider never called; FallbackEventMsg never emitted | No automatic provider failover |
| W-05 | SettingsSavedMsg | `dropped_message` | HIGH | settings.go, app.go, types.go | Settings save should notify AppState to restart health check, refresh model | SettingsSavedMsg emitted by SettingsModel but AppState has no handler | Health ticker not restarted on provider change; model badge not refreshed |
| W-06 | ScreenPlan | `unreachable_screen` | HIGH | plan.go, app.go | Workflow should transition to Plan screen | No code sets m.screen = ScreenPlan | Plan screen never shown |
| W-07 | ScreenExecute | `unreachable_screen` | HIGH | execute.go, app.go | Plan acceptance should transition to Execute | Only reachable from PlanModel (itself unreachable) | Execute screen never shown |
| W-08 | ScreenVerify | `unreachable_screen` | HIGH | verify.go, app.go | Execute completion should transition to Verify | No code sets m.screen = ScreenVerify | Verify screen never shown |
| W-09 | ScreenShip | `unreachable_screen` | HIGH | ship.go, app.go | Verify completion should transition to Ship | No code sets m.screen = ScreenShip | Ship screen never shown |
| W-10 | HealthUpdateMsg | `missing_emission` | MEDIUM | types.go | Health check should emit status updates | Defined but never constructed | Health badge cannot update dynamically |
| W-11 | ProviderSwitchMsg | `missing_emission` | MEDIUM | types.go | Provider switch should notify TUI | Defined but never constructed | Provider badge cannot update via message |
| W-12 | Command Bypass | `bypassed_registry` | MEDIUM | app.go:201-219 | Commands should go through CommandRegistry | 3 commands handled via inline string comparisons | Inconsistent command handling; no access to CommandContext |
| W-13 | ReplModel Provider Access | `incompatible_patterns` | HIGH | repl.go, app.go | ReplModel needs provider to make LLM calls | ReplModel has no provider/model reference | Cannot initiate streaming even if StartStreamCmd were wired |

---

## 10. Recommendations

### Priority 1: Wire the Streaming Pipeline (Effort: ~2 hours)

The REPL must actually call the LLM. This is the most critical gap.

**Steps:**
1. Pass a provider reference and model ID to `ReplModel` at construction (or add them as fields on AppState that the REPL can access via message)
2. In `ReplModel.Update()`, after the user presses Enter and `sent = true`:
   - Construct a `provider.ChatRequest` from `m.messages`
   - Return `StartStreamCmd(ctx, provider, req, sessionID)` as a tea.Cmd
3. Ensure `StreamMsg/StreamDoneMsg/StreamErrorMsg` handlers are already wired (they are)

**Files to modify:** `internal/tui/repl.go`, `internal/tui/app.go`

### Priority 2: Wire CommandRegistry into AppState (Effort: ~1.5 hours)

Replace inline string checks with the registry.

**Steps:**
1. Add `cmdRegistry *CommandRegistry` to `AppState`
2. In `NewApp()`, initialize with `DefaultCommands()`
3. In `AppState.Update()` for `ScreenREPL`, before the inline string checks, call:
   ```go
   if result, handled := m.cmdRegistry.Execute(input, ctx); handled {
       // Handle result.Message, result.Screen, etc.
   }
   ```
4. Remove inline `/settings`, `/resume`, `/models` cases (they'll be handled by registry)
5. Populate `CommandContext` with actual registry, session manager, config, etc.

**Files to modify:** `internal/tui/app.go`, `internal/tui/commands.go`

### Priority 3: Wire SettingsSavedMsg to AppState (Effort: ~30 minutes)

**Steps:**
1. Add `case SettingsSavedMsg:` handler in `AppState.Update()`
2. On receive: restart health check ticker if provider changed, refresh model badge, reload settings into all sub-models

**Files to modify:** `internal/tui/app.go`

### Priority 4: Wire Workflow Engine to TUI (Effort: ~4-6 hours)

This is the largest wiring task.

**Steps:**
1. Add `engine *workflow.Engine` to `AppState`
2. Create a trigger mechanism (e.g., `/workflow` command or initial goal input) that instantiates the engine
3. Create a `PhaseStartMsg` / `PhaseCompleteMsg` tea.Msg pair to bridge engine output to TUI
4. In `AppState.Update()`, handle phase transitions:
   - Initialize → create planning dir, instantiate engine
   - Discuss → show questions in REPL or dedicated screen
   - Plan → set `m.screen = ScreenPlan`, populate PlanModel
   - Execute → set `m.screen = ScreenExecute`, populate ExecuteModel
   - Verify → set `m.screen = ScreenVerify`, populate VerifyModel
   - Ship → set `m.screen = ScreenShip`, populate ShipModel
5. Wire each screen model as a field in `AppState`
6. Wire each screen's View() to render the model instead of placeholder text

**Files to modify:** `internal/tui/app.go`, `internal/tui/plan.go`, `internal/tui/execute.go`, `internal/tui/verify.go`, `internal/tui/ship.go`

### Priority 5: Wire Provider Fallback (Effort: ~1 hour)

**Steps:**
1. In the streaming goroutine (`StartStreamCmd`), check for 429/503 errors
2. On detection, call `provider.FindFallbackProvider(registry, currentProvider)`
3. If fallback succeeds, emit `FallbackEventMsg{From: ..., To: ..., Reason: ...}` into the Bubble Tea loop
4. Alternatively: wrap the provider call in a cmd that checks errors post-return

**Files to modify:** `internal/tui/streaming.go`, `internal/tui/app.go`

### Priority 6: Clean Up Dead Message Types (Effort: ~20 minutes)

Remove or wire `HealthUpdateMsg` and `ProviderSwitchMsg` from `types.go` and `AppMsg`.

**Decision:** If these are planned for future use, keep them. If not, remove them to reduce cognitive load.

---

## Appendix: Build Verification

```
$ CGO_ENABLED=0 go build -o /dev/null ./cmd/m31a
# Clean build — no compilation errors
```

The codebase compiles cleanly. All wiring issues are **runtime gaps**, not compile-time errors. This is why they survived previous audits — every component type-checks correctly in isolation.
