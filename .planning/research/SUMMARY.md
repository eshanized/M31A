# Project Research Summary

**Project:** M31A UI Refactor
**Domain:** Terminal-based AI coding assistant (Go/Bubble Tea TUI)
**Researched:** 2025-07-31
**Confidence:** HIGH

## Executive Summary

M31A is a mature Go/Bubble Tea terminal AI coding assistant with 180+ TUI files, three provider integrations (OpenRouter, Zen, NVIDIA), and a sophisticated workflow engine. The two critical issues—a blank REPL screen and hardcoded model names—are **code-level bugs** in an otherwise well-structured codebase, not architectural problems. The existing stack (Bubble Tea v1.3.0, Bubbles v0.20.0, Lipgloss v1.1.0) is correct and requires no new dependencies.

The recommended approach is a focused two-phase refactor: first fix the REPL rendering pipeline (viewport initialization, textarea focus, message display), then clean up hardcoded model references and wire dynamic model selection to the REPL. Both fixes are bounded to 4-6 files each, with the core infrastructure for dynamic fetching already in place. The primary risk is Bubble Tea's single-threaded constraint—goroutine state mutation must be avoided at all costs, and all state changes must flow through `Update()` via `tea.Msg` channels.

Research across all four domains (stack, features, architecture, pitfalls) converges on the same conclusion: **the foundation is solid, the wiring is broken**. This is good news—it means the fix is surgical, not a rewrite. The main pitfalls to avoid are viewport not receiving dimensions before first render, missing `tea.Cmd` returns breaking the message loop, and hardcoded model names in tests masking real API failures.

## Key Findings

### Recommended Stack

The existing stack is appropriate and requires no changes. No new dependencies are needed.

**Core technologies (already in place):**
- **Bubble Tea v1.3.0**: TUI framework (Elm architecture) — standard for Go TUIs, correct version
- **Bubbles v0.20.0**: TUI components (viewport, textarea) — compatible with Bubble Tea v1.3.0
- **Lipgloss v1.1.0**: Terminal styling — standard, no issues
- **Glamour v0.6.0**: Markdown rendering — appropriate for chat messages

**Supporting libraries:**
- `BurntSushi/toml v1.6.0` — Config parsing
- `godbus/dbus/v5 v5.2.2` — OS keychain integration (Linux)
- `pkoukk/tiktoken-go v0.1.8` — Token estimation

**What NOT to use:**
- Bubble Tea v2 (alpha/beta, not stable)
- Custom viewport implementations (Bubbles viewport is battle-tested)
- Hardcoded model lists (use `provider.Registry.ActiveProvider().FetchModels()`)

### Expected Features

**Must have (table stakes) — P1:**
- REPL viewport displays messages — users cannot interact without seeing AI responses
- REPL textarea accepts keyboard input — users cannot type without working input
- Dynamic model fetching from providers — hardcoded names break when providers update APIs
- Model selection updates active model — selecting a model must change what the LLM uses
- Streaming responses display in viewport — users expect real-time token output
- Error messages visible in REPL — users must see when things fail
- Welcome screen when no messages — first-run experience must not be blank
- Status bar shows current model — users need to know which model is active

**Should have (competitive) — P2:**
- Context window usage meter — already partially implemented
- Model pricing in status bar — already implemented, enable via config flag
- Provider health indicators in status bar — already wired via health ticker

**Defer to v2+:**
- Custom themes — theme system exists; customization adds config complexity
- Plugin system — premature until core is stable
- Multi-provider failover — complex; single provider works for now
- Real-time collaboration — massive infrastructure change; orthogonal to REPL fix

### Architecture Approach

The system follows Elm architecture (Model-View-Update) with strict separation: AppState owns screen routing and provider registry, ReplModel owns viewport/textarea/messages, ModelSelector owns model picker UI. Communication is via `tea.Cmd`/`tea.Msg` channels. The data flow is: user input → ReplModel.handleKeyMsg() → AppState.Update() → provider.ChatCompletionStream() → StreamMsg → ReplModel.handleStreamMsg() → viewport.SetContent().

**Major components:**
1. **AppState** — Screen routing, provider registry, session manager, workflow engine
2. **ReplModel** — Chat interface: viewport, textarea, messages, streaming state
3. **ModelSelector** — Model list display, search, provider filtering
4. **Provider Layer** — API calls, model caching, health checks (OpenRouter, Zen, NVIDIA)
5. **Tool System** — 18 built-in tools, dispatcher with permissions/rate limiting/concurrency

### Critical Pitfalls

1. **Goroutine state mutation** — Bubble Tea enforces single-threaded access; any goroutine directly mutating AppState or ReplModel causes data races, blank screens, or panics. Always use `tea.Msg` via channels.
2. **Viewport not re-initialized after screen switch** — Screen switches don't re-trigger `WindowSizeMsg`; viewport retains stale dimensions. Must explicitly resize on screen entry.
3. **Missing `tea.Cmd` returns in `Update()`** — If `Update()` doesn't return necessary continuation commands (e.g., `StreamTickCmd()`), the message loop stops silently. No error, just no activity.
4. **Hardcoded model names in tests** — 100+ test files use static model names (`gpt-4`, `claude-3`), masking real API failures. Tests pass but production breaks.
5. **Channel buffer overflow** — Emitter channel drops messages silently when workflow engine produces faster than TUI consumes; critical events may be lost.

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: Fix REPL Screen Rendering
**Rationale:** The blank REPL screen is the most critical issue — users cannot interact with the product at all without a working viewport. This phase must come first because everything else (streaming, model selection, session persistence) depends on the REPL being visible.

**Delivers:**
- Viewport displays messages on startup (no more blank screen)
- Textarea accepts keyboard input and has focus
- Welcome screen renders when no messages exist
- Streaming tokens display in real-time
- Error messages visible in REPL
- Status bar shows current model name

**Addresses:** All P1 table-stakes features from FEATURES.md

**Avoids:**
- Goroutine state mutation (verify all goroutines use message passing)
- Viewport not sized (add initialization on screen entry)
- Missing tea.Cmd returns (audit all Update() return paths)
- Channel overflow (monitor DroppedMessages counter)

**Files to modify:**
- `internal/ui/tui/repl_model.go` — Viewport initialization
- `internal/ui/tui/repl.go` — Init() call order
- `internal/ui/tui/repl_state.go` — renderMessages() guard
- `internal/ui/tui/repl_view.go` — Focus restoration
- `internal/ui/tui/app_screens.go` — syncReplSize() timing

### Phase 2: Fix Dynamic Model Selection
**Rationale:** Hardcoded model names break when providers update their APIs. This phase must come after Phase 1 because model selection UI depends on a working REPL viewport. The infrastructure for dynamic fetching already exists — this phase wires it correctly.

**Delivers:**
- Model selector shows real models from provider APIs
- Model selection persists across sessions
- Provider abbreviations are dynamic (not hardcoded)
- Tests use dynamic model fixtures instead of hardcoded names
- Context window meter populated from real model data

**Addresses:** Remaining P1 features + P2 competitive features

**Avoids:**
- Hardcoded model names masking real API failures
- Provider abbreviation hardcoding
- Circular dependencies in TUI package (extract sub-packages)

**Files to modify:**
- `internal/ui/tui/helpers_string.go` — ProviderShortName() registry
- `internal/ui/tui/modelselector_model.go` — Verify dynamic fetch
- `internal/ui/tui/repl_state.go` — activeModel enrichment
- Test files — Replace hardcoded model names with constants

### Phase Ordering Rationale

- **Phase 1 before Phase 2:** Model selection UI depends on a working REPL viewport; fixing the screen first ensures the model selector has a canvas to render on.
- **Phase 1 is surgical:** Both fixes are bounded to 4-6 files each, with core infrastructure already in place. No new dependencies needed.
- **Phase 2 builds on Phase 1:** Once the REPL renders correctly, dynamic model selection can be validated end-to-end.
- **This avoids the "rewrite" trap:** The research shows the foundation is solid; a full rewrite would be scope creep and risk introducing new bugs.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 1:** Low risk — well-documented Bubble Tea patterns, root causes identified in specific files
- **Phase 2:** Low risk — infrastructure exists; need to verify wiring across AppState, ReplModel, and Provider layers

Phases with standard patterns (skip research-phase):
- **Phase 1:** Viewport initialization is a standard Bubble Tea pattern (set defaults in constructor, let WindowSizeMsg correct them)
- **Phase 2:** Dynamic model fetching is already implemented in ModelSelector; need to wire to REPL

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Existing dependencies are correct and current; no changes needed |
| Features | HIGH | Table stakes clearly identified; competitive features well-scoped |
| Architecture | HIGH | Elm architecture is standard; component boundaries are clear |
| Pitfalls | HIGH | Root causes identified in specific files; prevention strategies documented |

**Overall confidence:** HIGH

### Gaps to Address

- **Exact viewport initialization bug location:** Research identifies the pattern (viewport has zero dimensions) but the exact line-by-line fix needs validation during implementation. The recommended approach (set defaults in `NewReplModel()`) is standard but must be tested with actual terminal resize events.
- **Model selection persistence path:** The flow from `ModelSelectedMsg` → `AppState.Update()` → config save needs verification. Research shows the infrastructure exists but the wiring may have gaps.
- **Test fixture modernization scope:** 100+ test files with hardcoded model names — need to assess how many are critical path tests vs. edge cases before prioritizing fixes.

## Sources

### Primary (HIGH confidence)
- Codebase analysis: `internal/ui/tui/` — 180+ files, main TUI implementation
- Codebase analysis: `internal/integrations/provider/` — 3 providers with dynamic model fetching
- Codebase analysis: `internal/core/types/` — Shared type definitions
- Bubble Tea documentation: Elm architecture, tea.Cmd, tea.Msg patterns

### Secondary (MEDIUM confidence)
- `CONCERNS.md`: TUI package size, provider abstraction duplication
- `PROJECT.md`: Blank REPL screen, hardcoded models issues
- Competitor analysis: Claude Code, Aider, Cursor terminal UIs

### Tertiary (LOW confidence)
- grep analysis: 100+ hardcoded model name references in test files (needs validation of actual impact)

---
*Research completed: 2025-07-31*
*Ready for roadmap: yes*
