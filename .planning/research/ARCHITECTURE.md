# Architecture Research: M31A UI Refactor

**Domain:** Terminal-based AI coding assistant (Bubble Tea / Elm architecture)
**Researched:** 2026-07-31
**Confidence:** HIGH

## Current System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Entry Point (cmd/m31a/main.go)            │
│  Flag parsing, config load, provider registration, TUI init  │
├─────────────────────────────────────────────────────────────┤
│                    Presentation Layer (internal/ui/tui/)     │
│  ┌──────────┐  ┌──────────────┐  ┌────────────────────┐    │
│  │ AppState │──│ ReplModel    │──│ ModelSelector      │    │
│  │ (screen  │  │ (chat UI,    │  │ (provider/model    │    │
│  │  routing)│  │  viewport,   │  │  picker)           │    │
│  │          │  │  textarea)   │  │                    │    │
│  └────┬─────┘  └──────┬───────┘  └────────────────────┘    │
│       │               │                                      │
├───────┴───────────────┴──────────────────────────────────────┤
│                    Business Logic (internal/engine/)          │
│  ┌──────────────┐  ┌──────────┐  ┌──────────┐               │
│  │ Workflow     │  │ Session  │  │ Tokens   │               │
│  │ Engine       │  │ Manager  │  │ Estimator│               │
│  │ (7 phases)   │  │          │  │          │               │
│  └──────┬───────┘  └──────────┘  └──────────┘               │
├─────────┴───────────────────────────────────────────────────┤
│                    Tool System (internal/tools/)             │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ Dispatcher (permissions, rate limiting, concurrency) │    │
│  └─────────────────────────────────────────────────────┘    │
├─────────────────────────────────────────────────────────────┤
│                    Provider Layer (internal/integrations/)   │
│  ┌────────────┐  ┌────────┐  ┌─────────┐                   │
│  │ OpenRouter │  │ Zen    │  │ NVIDIA  │                   │
│  │ Provider   │  │Provider│  │Provider │                   │
│  └────────────┘  └────────┘  └─────────┘                   │
├─────────────────────────────────────────────────────────────┤
│                    Foundation (internal/core/)               │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                   │
│  │ Types    │  │ Config   │  │ Errors   │                   │
│  │          │  │ (TOML)   │  │          │                   │
│  └──────────┘  └──────────┘  └──────────┘                   │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow: REPL Interaction

```
User types in textarea
    ↓
ReplModel.handleKeyMsg()
    ↓
handleEnterKey() → SlashCommandMsg
    ↓
AppState.Update() → handleSlashCommand()
    ↓
sendChatMessage() or startAgentLoop()
    ↓
provider.ChatCompletionStream() → StreamIterator
    ↓
goroutine reads chunks → StreamMsg → AppState.Update()
    ↓
ReplModel.handleStreamMsg() → renderMessages() → viewport.SetContent()
    ↓
AppState.View() → renderREPLContent() → ReplModel.ViewContent()
```

## Data Flow: Model Selection

```
Init() → syncReplProvider() → ReplModel.SetProvider()
    ↓
async FetchModels() → ProviderModelsFetchedMsg
    ↓
ReplModel.handleProviderModelsFetched()
    ↓
activeModel enriched (pricing, context length, capabilities)
```

## Component Boundaries

### AppState (Orchestrator)
- **Owns:** Screen routing, provider registry, session manager, workflow engine
- **Does NOT own:** REPL rendering details, streaming state
- **Communicates with:** ReplModel via tea.Cmd/tea.Msg channels

### ReplModel (Chat Interface)
- **Owns:** Viewport, textarea, messages, streaming state, render cache
- **Does NOT own:** Provider connections, session persistence
- **Communicates with:** AppState via tea.Cmd returning functions

### ModelSelector (Model Picker)
- **Owns:** Model list display, search, provider filtering
- **Does NOT own:** Active model selection (AppState handles persistence)
- **Communicates with:** AppState via AppMsg{ModelSelected}

### Provider Layer
- **Owns:** API calls, model caching, health checks
- **Does NOT own:** UI state, session management
- **Communicates with:** TUI via StreamIterator and ModelInfo

## Identified Root Causes

### Blank REPL Screen

**Root Cause:** The REPL viewport has no content when messages list is empty, and the textarea may not receive keyboard focus correctly.

**Evidence:**
1. `renderMessages()` in `repl_state.go:377` checks `if len(m.messages) == 0 && !m.streaming` → renders welcome screen
2. `renderWelcome()` exists but may not be rendering visible content
3. The textarea focus is set in `NewReplModel()` via `ta.Focus()` but may be lost during screen transitions
4. The viewport is only created when `m.viewport.Width == 0` in `repl.go:39`, and the height calculation depends on `contentViewportHeight()` which requires proper terminal dimensions

**Key Files:**
- `repl_state.go:377` - `renderMessages()` function
- `repl.go:27` - `ReplModel.Update()` function
- `repl_view.go:214` - `ViewContent()` function
- `app_screens.go:19` - `renderREPLContent()` function

### Hardcoded Models

**Root Cause:** Model names are hardcoded in test files and helper functions, but the actual provider layer already supports dynamic fetching.

**Evidence:**
1. `provider_registration.go` shows providers are registered dynamically
2. `FetchModels()` is already implemented in each provider
3. `ModelSelector` already fetches models asynchronously
4. `helpers_string.go:111` - `ProviderShortName()` has hardcoded provider names but this is just for display, not model selection
5. The real issue is that model selection is not persisted correctly across sessions

**Key Files:**
- `provider_registration.go` - Provider registration (already dynamic)
- `modelselector_model.go` - Model selector (already fetches dynamically)
- `repl_state.go:92` - `SetProvider()` (already validates models)
- `app_state.go:368` - `handleAppMsg()` (persists model selection)

## Recommended Architecture Modifications

### Phase 1: Fix REPL Screen Rendering

**Changes Required:**

1. **Ensure viewport initialization** in `repl.go`:
   - Verify `viewport.New(rw, vpH)` is called with correct dimensions
   - Add debug logging to `renderMessages()` to trace empty message case

2. **Fix textarea focus** in `repl_view.go`:
   - Ensure `textarea.Focus()` is called after screen transitions
   - Add focus restoration in `renderREPLContent()` when screen is REPL

3. **Welcome screen rendering** in `repl_state.go`:
   - Verify `renderWelcome()` returns visible content
   - Add fallback content if welcome screen is empty

4. **Dimension synchronization** in `app_screens.go`:
   - Ensure `syncReplSize()` is called before `ViewContent()`
   - Verify `contentViewportHeight()` calculates correctly

### Phase 2: Fix Model Selection Persistence

**Changes Required:**

1. **Config persistence** in `app_state.go`:
   - Verify `config.SaveWithKeychain()` is called after model selection
   - Add error handling for save failures

2. **Session restoration** in `app_session.go`:
   - Verify model info is restored from session on resume
   - Add fallback to config model if session model is missing

3. **Provider model cache** in provider layer:
   - Ensure `GetModel()` returns enriched model info
   - Add cache warming on startup

### Phase 3: Clean Up Hardcoded Values

**Changes Required:**

1. **Test files**: Replace hardcoded model names with dynamic test fixtures
2. **Helper functions**: Keep `ProviderShortName()` as-is (display only)
3. **Documentation**: Update comments to reflect dynamic model system

## Build Order Implications

### Dependencies Between Components

```
Foundation (types, config, errors)
    ↓
Provider Layer (needs types)
    ↓
Tool System (needs types, provider)
    ↓
Engine (needs types, provider, tools)
    ↓
TUI Layer (needs all above)
    ↓
Entry Point (wires everything)
```

### Recommended Fix Order

1. **Foundation first**: Verify types and config are correct
2. **Provider layer**: Ensure FetchModels works correctly
3. **TUI REPL**: Fix rendering and focus issues
4. **Integration**: Wire model selection persistence
5. **Tests**: Update test fixtures

## Anti-Patterns to Avoid

### Anti-Pattern 1: Goroutine State Mutation
**What people do:** Modify AppState from goroutines
**Why it's wrong:** Breaks Bubble Tea's single-threaded contract
**Do this instead:** Send tea.Msg through channels, handle in Update()

### Anti-Pattern 2: Hardcoded Model Names
**What people do:** Use test model names in production code
**Why it's wrong:** Breaks when providers update their model lists
**Do this instead:** Use provider.FetchModels() and cache results

### Anti-Pattern 3: Missing Nil Checks
**What people do:** Assume model/provider/session is non-nil
**Why it's wrong:** Causes blank screen or crashes when initialization fails
**Do this instead:** Check nil before every access, provide fallbacks

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| OpenRouter API | HTTP REST + SSE streaming | Requires API key, rate limiting |
| Zen API | HTTP REST + SSE streaming | Custom base URL support |
| NVIDIA NIM API | HTTP REST + SSE streaming | Custom model format |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| AppState ↔ ReplModel | tea.Cmd/tea.Msg channels | Strict Elm architecture |
| AppState ↔ Provider | Context + StreamIterator | Async with cancellation |
| ReplModel ↔ Viewport | Direct method calls | Synchronous rendering |

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| Single user | Current architecture is fine |
| Multiple sessions | Session manager handles isolation |
| Large model catalogs | Provider cache with TTL (already implemented) |
| High streaming throughput | Adaptive drain, batch messages (already implemented) |

### Scaling Priorities

1. **First bottleneck:** Viewport rendering during streaming (mitigated by incremental rendering)
2. **Second bottleneck:** Model catalog fetch on startup (mitigated by async fetch + cache)

## Sources

- `internal/ui/tui/` - 180+ files, main TUI implementation
- `internal/integrations/provider/` - 3 providers with dynamic model fetching
- `internal/core/types/` - Shared type definitions
- `internal/core/config/` - TOML config with keychain integration

---
*Architecture research for: M31A UI Refactor*
*Researched: 2026-07-31*
