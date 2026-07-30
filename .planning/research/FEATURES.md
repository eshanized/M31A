# Feature Research

**Domain:** Terminal AI coding assistant (Bubble Tea TUI) — UI Refactor
**Researched:** 2025-07-31
**Confidence:** HIGH

## Feature Landscape

### Table Stakes (Users Expect These)

These are non-negotiable for the refactor. Missing any = product is broken.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| REPL viewport displays messages | Users cannot interact without seeing AI responses | LOW | Viewport exists but not rendering content — likely a state initialization or content-setting bug in `repl_model.go` / `repl_view.go` |
| REPL textarea accepts keyboard input | Users cannot type without working input | LOW | Textarea component exists but may not have focus or key handling wired correctly |
| Dynamic model fetching from providers | Hardcoded model names break when providers update APIs | MEDIUM | `FetchModels()` interface exists; `ModelSelector` already calls it; issue is wiring to REPL via `syncReplProvider()` |
| Model selection updates active model | Selecting a model must actually change what the LLM uses | MEDIUM | `ModelSelectedMsg` exists; handler in `AppState.Update()` needs to propagate to workflow engine and REPL |
| Streaming responses display in viewport | Users expect real-time token output during LLM calls | LOW | Streaming infrastructure exists (`repl_stream.go`); blank viewport prevents display |
| Error messages visible in REPL | Users must see when things fail | LOW | `MakeErrorBannerMsg()` exists; needs to be called on provider/LLM errors |
| Welcome screen when no messages | First-run experience must not be blank | LOW | `repl_welcome.go` exists; may not be triggering when viewport is empty |
| Status bar shows current model | Users need to know which model is active | LOW | Status bar exists with model info; data flow from `activeModel` needs verification |

### Differentiators (Competitive Advantage)

Features that set M31A apart from basic CLI tools. Not required for fix, but already partially implemented.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Model pricing in status bar | Users see cost implications before sending | LOW | `lastCost` and `ShowCost` fields exist in `StatusBarInfo`; config flag `UI.ShowCostEstimate` controls visibility |
| Context window usage meter | Users know when approaching limits | LOW | `ContextUsed` and `ContextMax` fields exist in status bar; needs `activeModel.ContextLength` populated from dynamic fetch |
| Auto-arbitrage (cheaper model switching) | Saves money on simple tasks | MEDIUM | Already implemented in `checkAutoArbitrage()`; requires dynamic model list to function properly |
| Provider health indicators | Users know if a provider is degraded | LOW | `HealthCheck()` interface exists; health ticker already runs via `NextHealthTick()` |
| Model capability badges | Users see if model supports chat/tools/reasoning | LOW | `ModelInfo.Capabilities` struct exists; `ModelSelector` already filters on `Capabilities.Chat` |
| @-mention file autocomplete | Faster file references in prompts | MEDIUM | Fully implemented in `repl_model.go` (`mentionVisible`, `MentionCompleter`) |
| Slash command autocomplete | Discoverable command interface | MEDIUM | Fully implemented (`slashVisible`, `CommandRegistry`) |
| Quick actions overlay (Ctrl+Q) | Rapid access to common actions | LOW | Fully implemented in `repl_quickactions.go` |
| Inline viewport search (Ctrl+F) | Find content in long conversations | LOW | Fully implemented in `repl_search.go` |
| Session persistence and resume | Work survives restarts | HIGH | Already implemented via `session.Manager` and `loadAndRestoreSession()` |
| Wave separator animation | Visual feedback during streaming | LOW | Fully implemented in `renderWaveSeparator()` |
| Smooth scroll during streaming | Polished reading experience | LOW | `smoothScrollTarget` field exists; auto-scroll logic in place |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems. Explicitly NOT in scope per PROJECT.md.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Full UI redesign | "Make it look modern" | Breaks existing patterns, massive scope creep, delays critical fixes | Fix existing components first; redesign only after core works |
| New slash commands | "Add more commands" | Distracts from fixing blank screen; adds untested code paths | Fix existing 18 tools and command registry first |
| Multi-window/split view | "Show code and chat side by side" | Bubble Tea layout complexity, not needed for core functionality | Use sidebar (already implemented) for context |
| Plugin/extension system | "Let users add features" | Premature abstraction; product not stable enough | Wait for v2+ when core is validated |
| Real-time collaboration | "Share sessions with team" | Massive infrastructure change; orthogonal to REPL fix | Not in scope; would require backend server |
| Custom themes | "Let users customize colors" | Theme system exists (`theme.Theme`); customization adds config complexity | Keep single dark theme per current design decision |
| Mouse-only interaction | "Make it clickable" | Terminal mouse support is fragile; accessibility concern | Keep keyboard-first; mouse is enhancement only |

## Feature Dependencies

```
[Dynamic Model Fetching]
    └──requires──> [Provider FetchModels() working]
                       └──requires──> [Provider registration in main.go]

[REPL Viewport Display]
    └──requires──> [AppState.Init() routing to correct screen]
    └──requires──> [ReplModel messages populated or welcome screen triggered]

[Model Selection Updates Active Model]
    └──requires──> [Dynamic Model Fetching]
    └──requires──> [ModelSelectedMsg handler in AppState.Update()]

[Streaming Display]
    └──requires──> [REPL Viewport Display]
    └──requires──> [Stream channel wired to REPL]

[Context Window Meter]
    └──requires──> [Dynamic Model Fetching] (needs ContextLength from ModelInfo)

[Auto-Arbitrage]
    └──requires──> [Dynamic Model Fetching] (needs pricing from ModelInfo)

[Session Resume]
    └──requires──> [REPL Viewport Display] (must show restored messages)
```

### Dependency Notes

- **Dynamic Model Fetching requires Provider Registration:** `main.go` already registers providers; the issue is that `syncReplProvider()` may not be completing before REPL renders
- **REPL Viewport requires Init() routing:** `app.go` sets `m.screen = ScreenHome` when `hasProvider=true`, then calls `ensureReplModel()` and `switchScreen()` — this chain must work for REPL to appear
- **Model Selection requires Dynamic Fetching:** `ModelSelector.Init()` calls `fetchModelsCmd()` for each provider; results come back via `modelSelectorLoadedMsg`; but the REPL's `activeModel` must be set from config default or user selection
- **Streaming requires Viewport:** Even if streaming works perfectly, blank viewport hides all output

## MVP Definition

### Launch With (v1) — Critical Fixes

- [ ] **REPL viewport displays messages** — Users cannot use the product without this
- [ ] **REPL textarea accepts keyboard input** — Users cannot type without this
- [ ] **Dynamic model fetching replaces hardcoded names** — Product breaks when providers update
- [ ] **Model selection propagates to LLM calls** — Selecting a model must actually work
- [ ] **Error messages visible in REPL** — Users must see failures
- [ ] **Welcome screen shows when empty** — First impression must not be blank

### Add After Validation (v1.x)

- [ ] **Context window usage meter** — Valuable but not blocking; needs `activeModel.ContextLength` from dynamic fetch
- [ ] **Model pricing display** — Already partially implemented; enable via config flag
- [ ] **Provider health status in status bar** — Already wired via health ticker; surface in UI

### Future Consideration (v2+)

- [ ] **Custom themes** — Theme system exists; customization adds config complexity
- [ ] **Plugin system** — Premature until core is stable
- [ ] **Multi-provider failover** — Complex; single provider works for now

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| REPL viewport displays messages | HIGH | LOW | P1 |
| REPL textarea accepts keyboard input | HIGH | LOW | P1 |
| Dynamic model fetching | HIGH | MEDIUM | P1 |
| Model selection propagation | HIGH | MEDIUM | P1 |
| Error messages in REPL | HIGH | LOW | P1 |
| Welcome screen when empty | MEDIUM | LOW | P1 |
| Context window meter | MEDIUM | LOW | P2 |
| Model pricing display | MEDIUM | LOW | P2 |
| Provider health in status bar | LOW | LOW | P2 |
| Custom themes | LOW | HIGH | P3 |

## Competitor Feature Analysis

| Feature | Claude Code | Aider | Cursor | M31A Approach |
|---------|-------------|-------|--------|---------------|
| Model selection | Config file only | CLI flag | GUI dropdown | Dynamic TUI selector with search + pricing |
| Streaming display | Terminal output | Terminal output | Editor panel | Bubble Tea viewport with wave animation |
| Context tracking | None visible | Token counter | Status bar | Status bar with context meter |
| Cost visibility | None | None | None | Auto-arbitrage + pricing display (differentiator) |
| Session persistence | None | Git-based | Auto-save | Checkpoint system with resume |
| Provider flexibility | Anthropic only | Multiple | Multiple | 3 providers with health checks |

## Implementation Notes

### Critical Bug: Blank REPL Screen

The blank screen is likely caused by one of:
1. **Viewport not receiving content:** `renderMessages()` not being called or viewport content being empty
2. **Init() not routing to REPL screen:** `m.screen = ScreenHome` but `switchScreen()` not working
3. **Textarea not focused:** `ta.Focus()` called in `NewReplModel()` but focus lost during initialization
4. **Welcome screen not rendering:** `repl_welcome.go` content not being set as viewport content

**Investigation path:** Check `app.go` Init() flow → `ensureReplModel()` → `switchScreen()` → REPL's `Init()` → `renderMessages()` / welcome content.

### Critical Bug: Hardcoded Models

Hardcoded models found in:
- `helpers_string.go` — `ProviderShortName()` has hardcoded provider names (but these are display names, not model IDs)
- Test files — `gpt-4`, `claude-3` used in test assertions
- Config fallback — `config.Model.Default` may be empty or hardcoded

**Fix:** Ensure `syncReplProvider()` completes and populates `activeModel` from `config.Model.Default` → provider's `GetModel()` → fallback to stub → async enrichment via `ProviderModelsFetchedMsg`.

## Sources

- Codebase analysis: `internal/ui/tui/repl_model.go`, `app.go`, `modelselector_model.go`
- Architecture review: `.planning/codebase/ARCHITECTURE.md`
- Project requirements: `.planning/PROJECT.md`
- Bubble Tea framework patterns (Elm architecture)
- Competitor analysis: Claude Code, Aider, Cursor terminal UIs

---
*Feature research for: M31A UI Refactor*
*Researched: 2025-07-31*
