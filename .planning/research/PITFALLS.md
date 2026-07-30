# Domain Pitfalls: M31A UI Refactor

**Domain:** Go/Bubble Tea terminal UI refactoring, dynamic model selection
**Researched:** 2025-07-31
**Confidence:** HIGH

## Critical Pitfalls

### Pitfall 1: Mutating AppState from Goroutines

**What goes wrong:**
Bubble Tea enforces single-threaded access to the model. If any goroutine (file watcher, config watcher, provider health check, subagent manager) directly mutates `AppState` or `ReplModel` fields, it causes data races, silent state corruption, or panics. The REPL may appear blank or unresponsive because the viewport's internal state was corrupted by a concurrent write.

**Why it happens:**
Developers accustomed to concurrent Go patterns assume "goroutine-safe" means they can update shared state. Bubble Tea's `Update()` is the only safe mutation point, but the codebase has many background goroutines (file watcher, config watcher, health ticker, emitter drain) that communicate via channels. It's tempting to "just set the field" instead of sending a message.

**How to avoid:**
- Never write to `AppState` or `ReplModel` fields from goroutines. Always send a `tea.Msg` via `tea.Program.Send()` or channel.
- The existing pattern in `repl_view.go:59-64` (resize debounce) is correct: the timer callback sends `resizeDebounceMsg{}` instead of mutating `resizePending` directly.
- Audit every goroutine spawned in `app.go`, `repl_stream.go`, and handler files for direct state writes.
- Use `m.program.Send(msg)` to bridge goroutines to the Bubble Tea loop.

**Warning signs:**
- Flaky test failures with `-race` flag
- Blank viewport that renders correctly on retry
- Intermittent `concurrent map read and map write` panics
- Changes in one screen affecting another screen's display

**Phase to address:** Phase 1 (Fix REPL Screen) — verify all goroutines use message passing

---

### Pitfall 2: Viewport Not Re-Initialized After Screen Switch

**What goes wrong:**
When switching between screens (REPL, Settings, Model Selector, etc.), the viewport dimensions may not be recalculated for the new screen layout. The viewport retains stale `Width` and `Height` from the previous screen, causing blank rendering or garbled output. This is the likely root cause of the blank REPL screen: the viewport was never properly sized for the REPL content area.

**Why it happens:**
Bubble Tea's `WindowSizeMsg` is only sent once at startup and on terminal resize. Screen switches don't re-trigger `WindowSizeMsg`. The viewport must be explicitly resized when entering a screen, but this is easy to miss if the resize logic is buried in `Update()`.

**How to avoid:**
- In `repl.go:31-68`, the viewport resize logic handles `WindowSizeMsg` but only initializes if `m.viewport.Width == 0`. After a screen switch, Width may be non-zero but wrong.
- Add explicit viewport initialization in the screen entry path (e.g., `ensureReplModel()` or `routeToScreen()`).
- The pattern should be: when entering REPL screen, always set `m.viewport.Width` and `m.viewport.Height` based on current content dimensions.
- Test by switching between screens and verifying viewport renders correctly each time.

**Warning signs:**
- Blank screen on first load but works after resize
- Viewport shows content from previous screen
- `viewport.View()` returns empty string despite messages being present

**Phase to address:** Phase 1 (Fix REPL Screen) — add viewport initialization on screen entry

---

### Pitfall 3: Missing tea.Cmd Returns in Update()

**What goes wrong:**
If `Update()` doesn't return the necessary `tea.Cmd` chain (e.g., `StreamTickCmd()`, `drainEmitterCmd()`, `permListenerCmd()`), the message loop stops processing. The REPL appears frozen — no streaming, no permission prompts, no sidebar updates. This is a silent failure: no error, just no activity.

**Why it happens:**
Bubble Tea relies on returned `tea.Cmd` to schedule the next message. If a code path returns `nil` instead of the required continuation command, the entire event chain breaks. This is especially dangerous in streaming: `handleStreamMsg()` must return both `nextCmd` and `StreamTickCmd()`. Missing either stops the stream.

**How to avoid:**
- Audit every `Update()` return path in `repl.go`, `app_update.go`, and handler files.
- The streaming chain must be: `StreamMsg` → `handleStreamMsg()` → returns `[nextCmd, StreamTickCmd()]` → next chunk or tick.
- The emitter drain chain must be: `drainEmitterCmd()` → message from channel → `drainEmitterCmd()` again.
- Use the existing `drainAdaptiveCmd()` pattern that switches between single and batch draining.
- Add a "heartbeat" mechanism: if no messages arrive within N seconds, log a warning.

**Warning signs:**
- Streaming starts but never completes
- Permission requests never appear
- Sidebar stops updating mid-session
- `slog.Debug` shows no messages arriving after initial load

**Phase to address:** Phase 1 (Fix REPL Screen) — verify all cmd chains are unbroken

---

### Pitfall 4: Hardcoded Model Names in Tests Masks Real API Failures

**What goes wrong:**
Tests use hardcoded model names like `gpt-4`, `claude-3`, `gpt-4o` (found in 100+ locations across test files). When the actual providers change their model catalogs (which they do frequently), the tests pass but the real application fails because it tries to use models that no longer exist. This creates false confidence: tests green, production broken.

**Why it happens:**
Test fixtures are easier to write with static strings than with dynamically fetched model lists. Over time, the test model names drift from reality. The codebase has `FetchModels()` in the provider interface, but tests bypass it entirely.

**How to avoid:**
- Create a `testmodels` package with a mock provider that returns realistic model lists.
- Replace hardcoded `"gpt-4"` in tests with `testmodels.DefaultModelID` constant.
- Add an integration test that actually calls `FetchModels()` against each provider and validates the response schema.
- The `filterChatModels()` function in `modelselector_model.go:235-243` should be tested with real provider responses, not synthetic data.
- Mark tests that use hardcoded model names with `// TODO: use dynamic model list` until they're fixed.

**Warning signs:**
- Tests pass but `/model` command shows empty list
- `Model not found` errors in production that don't appear in tests
- Adding a new provider breaks existing tests unexpectedly

**Phase to address:** Phase 2 (Fix Hardcoded Models) — update test infrastructure first

---

### Pitfall 5: Provider Abbreviation Hardcoding in helpers_string.go

**What goes wrong:**
`helpers_string.go:111-129` has a hardcoded `ProviderShortName()` function that maps provider names to display abbreviations. When new providers are added (or existing ones rename), this function returns incorrect or empty abbreviations. The UI shows raw provider IDs instead of clean short names.

**Why it happens:**
Display strings are often treated as "just UI" and hardcoded without considering extensibility. The switch statement doesn't have a default fallback that generates reasonable abbreviations from unknown provider names.

**How to avoid:**
- Replace the hardcoded switch with a registry-based approach: each provider registers its short name during `RegisterProvider()`.
- Or: use a config-driven approach where `config.toml` can override abbreviations.
- The `default` case currently returns `name[:4]` which is a reasonable fallback, but the explicit cases for `openai`, `anthropic` suggest the function is meant to be the source of truth.
- Add a test that verifies `ProviderShortName()` works for all registered providers.

**Warning signs:**
- New provider shows as raw ID in status bar
- Provider abbreviation changes between sessions
- `ProviderShortName("my-new-provider")` returns empty string

**Phase to address:** Phase 2 (Fix Hardcoded Models) — make provider metadata dynamic

---

### Pitfall 6: Channel Buffer Overflow Drops Messages Silently

**What goes wrong:**
The emitter channel (`emitterCh`) has a fixed capacity (`ChannelCap`). When the workflow engine produces messages faster than the TUI consumes them, messages are dropped with only a `slog.Warn` log. Critical workflow events (phase transitions, tool completions, errors) may be lost, causing the UI to show stale state.

**Why it happens:**
The `drainEmitterCmd()` reads one message per tick, but workflow phases can emit bursts of messages (multiple tool calls, rapid phase transitions). The adaptive drain (`drainAdaptiveCmd()`) helps but doesn't guarantee delivery under sustained load.

**How to avoid:**
- Monitor `globalDropCounter` — if it's non-zero at session end, investigate.
- The existing `EmitterDropLogTick()` logs drops, but consider making drops visible in the UI (toast notification).
- For critical messages (errors, phase completions), use a separate high-priority channel that bypasses the emitter.
- The bounded retry in `wireTodoWriteCallback()` (`app.go:533-543`) is correct: bounded retries with backoff, never unbounded goroutines.
- Consider increasing `ChannelCap` during workflow-heavy sessions.

**Warning signs:**
- `slog.Warn("TodoWrite update dropped: channel full")` in logs
- Phase completions not reflected in sidebar
- Tool cards disappear during rapid tool execution
- `DroppedMessages() > 0` at shutdown

**Phase to address:** Phase 1 (Fix REPL Screen) — verify message delivery under load

---

### Pitfall 7: Session State Lost on Ungraceful Shutdown

**What goes wrong:**
If the process is killed (SIGKILL, Ctrl+C in some terminals, OOM), `Shutdown()` never runs, and `saveSessionOnShutdown()` never persists messages, workflow state, or model selection. The next session starts fresh, losing all conversation history.

**Why it happens:**
`Shutdown()` relies on receiving a `tea.Quit` message, which may not arrive on ungraceful termination. The `shutdownCtx` cancellation is cooperative — goroutines must check it.

**How to avoid:**
- `saveSessionOnShutdown()` is already implemented (`app.go:217-256`) and handles messages, workflow state, and session metadata.
- Consider adding periodic session persistence (every N messages or every M minutes) so recent progress is saved incrementally.
- The file watcher and config watcher already respect `shutdownCtx.Done()`, which is correct.
- For the model selection specifically: persist `activeModel` and `activeProvider` to config on every change, not just on shutdown.

**Warning signs:**
- Session restore shows empty conversation
- Model selection reverts to default after restart
- Workflow phase progress lost on restart

**Phase to address:** Phase 1 (Fix REPL Screen) — add incremental session persistence

---

### Pitfall 8: Circular Dependencies in 180-File TUI Package

**What goes wrong:**
With ~180 files in `internal/ui/tui/`, it's easy to create import cycles. The package already has sub-packages (`components/`, `theme/`, `tuitypes/`, `streaming/`, `layout/`), but the main package still has too many responsibilities. Adding new message types or handlers risks circular imports.

**Why it happens:**
The Elm architecture encourages a single large model with many message types. As the app grows, related types end up in the same package even when they should be separate. The `types.go` file defines shared types, but new types proliferate across files.

**How to avoid:**
- Follow the existing pattern: extract to sub-packages when a group of types has no dependencies on the main model.
- `tuitypes/` already exists for shared type definitions — use it.
- `streaming/` already exists for streaming types — follow this pattern for other concerns.
- The `CONCERNS.md` already recommends extracting `tui/screens/` and `tui/handlers/` — do this early.
- Enforce: no file in `tui/` should import more than 3 other files from `tui/`.

**Warning signs:**
- `go build` fails with "import cycle"
- Adding a new message type requires changes in 5+ files
- `goimports` shows messy import groups

**Phase to address:** Phase 2 (Fix Hardcoded Models) — extract sub-packages while refactoring

---

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Hardcode model names in tests | Faster test writing | Tests pass but production fails | Never — use test constants |
| Direct state mutation from goroutines | Simpler code | Data races, blank screens | Never — always use tea.Msg |
| Skip viewport resize on screen entry | Less code | Blank screens, garbled display | Never — always initialize |
| Single emitter channel for all messages | Simple architecture | Message drops under load | Only if channel capacity is tuned |
| Store API keys in config file | Simpler than keychain | Security risk on shared systems | Only if keychain unavailable |
| Bypass FetchModels() in tests | Faster tests | Tests don't validate real APIs | Only for unit tests of UI logic |

## Integration Gotchas

Common mistakes when connecting to external services.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| OpenRouter API | Assume model list is static | Call FetchModels() on every session start, cache with TTL |
| Zen API | Ignore health check failures | Implement fallback to next provider |
| NVIDIA NIM API | Assume all models support chat | Filter by Capabilities.Chat in model selector |
| OS Keychain | Assume keychain always works | Graceful degradation to config file (existing pattern) |
| File Watcher | Watch too many directories | Only watch working directory, use debounce |
| Config Hot-Reload | Reload without validating | Validate config before applying, revert on error |

## Performance Traps

Patterns that work at small scale but fail as usage grows.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Re-render all messages on every tick | High CPU during streaming | Use incremental rendering cache (`cachedMessageContent`) | >50 messages in conversation |
| Viewport scroll to bottom on every message | Janky scroll during rapid output | Use `autoScrollConditionally()` with user scroll detection | >10 messages/second |
| Full model list in memory | Memory growth with many providers | Use lazy loading in model selector, filter on-demand | >1000 models across providers |
| Unbounded toast queue | Memory leak, rendering slowdown | Cap at `maxVisibleToasts+2`, evict oldest | >100 toasts without dismissal |
| Blocking FetchModels() in Init() | Slow startup, blank screen during fetch | Use async fetch with spinner, show results when ready | Any provider with >100ms latency |

## Security Mistakes

Domain-specific security issues beyond general web security.

| Mistake | Risk | Prevention |
|---------|------|------------|
| Log API keys in debug output | Key leakage in logs | Never log key values, only provider names |
| Store config file with plaintext keys | Key file exposure | Use OS keychain (existing pattern in `pkg/keychain/`) |
| Allow arbitrary model IDs from user input | Injection of malicious model strings | Validate model ID against provider's FetchModels() response |
| Skip permission checks for "trusted" tools | Accidental file deletion | Always go through Dispatcher permission system |
| Expose session files to other users | Conversation data leakage | Use restrictive file permissions on `.m31a/` directory |

## UX Pitfalls

Common user experience mistakes in this domain.

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Blank screen on startup | User thinks app is broken | Show loading spinner with "Connecting to provider..." message |
| No feedback during model fetch | User doesn't know what's happening | Show spinner in model selector with per-provider status |
| Streaming stops without error | User doesn't know response failed | Always show error banner on stream failure (existing pattern) |
| Model selection not persisted | User must re-select every session | Save to config on every change |
| Keyboard input unresponsive | User can't interact | Verify textarea has focus, show focus ring (existing pattern) |
| Permission modal blocks streaming | User can't see streaming progress | Show permission modal as overlay, not replacement |

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces.

- [ ] **REPL renders messages**: Verify viewport contains actual message content, not just empty borders
- [ ] **Keyboard input works**: Type text, press Enter, verify message appears in viewport
- [ ] **Model selector shows real models**: Open `/model`, verify list is fetched from providers, not hardcoded
- [ ] **Streaming works**: Send a message, verify tokens arrive and render progressively
- [ ] **Session persists**: Close app, reopen, verify conversation history is restored
- [ ] **Provider fallback works**: Disable one provider, verify app switches to another
- [ ] **Permission prompts appear**: Trigger a tool that needs permission, verify modal appears
- [ ] **Sidebar updates**: Make file changes, verify sidebar reflects them in real-time
- [ ] **Hot-reload config**: Change `config.toml`, verify app picks up changes without restart
- [ ] **Shutdown saves state**: Ctrl+C during streaming, reopen, verify session restored

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Goroutine state mutation | HIGH | Identify goroutine, add message passing, add race detector tests |
| Missing tea.Cmd return | MEDIUM | Add logging to Update() returns, trace cmd chain |
| Viewport not sized | LOW | Add viewport initialization in screen entry path |
| Hardcoded model names | LOW | Replace with test constants, add integration tests |
| Channel overflow | MEDIUM | Increase capacity, add batch draining, monitor drops |
| Session state lost | LOW | Add incremental persistence, verify Shutdown() runs |

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Goroutine state mutation | Phase 1 (Fix REPL) | Run tests with `-race` flag, verify no warnings |
| Viewport not sized | Phase 1 (Fix REPL) | Test screen switching, verify viewport renders |
| Missing tea.Cmd returns | Phase 1 (Fix REPL) | Add logging to all Update() returns, trace chains |
| Channel overflow | Phase 1 (Fix REPL) | Monitor `DroppedMessages()` counter in logs |
| Session state lost | Phase 1 (Fix REPL) | Kill process mid-session, verify restore |
| Hardcoded model names | Phase 2 (Fix Models) | Run tests with real provider responses |
| Provider abbreviation hardcoding | Phase 2 (Fix Models) | Test with unknown provider names |
| Circular dependencies | Phase 2 (Fix Models) | Extract sub-packages, verify no import cycles |
| Model not persisted | Phase 2 (Fix Models) | Change model, restart, verify selection kept |

## Sources

- Bubble Tea documentation: Elm architecture, tea.Cmd, tea.Msg patterns
- Codebase analysis: 180+ files in `internal/ui/tui/`, provider registration, streaming
- `CONCERNS.md`: TUI package size, provider abstraction duplication
- `PROJECT.md`: Blank REPL screen, hardcoded models issues
- grep analysis: 100+ hardcoded model name references in test files

---
*Pitfalls research for: M31A UI Refactor (Go/Bubble Tea)*
*Researched: 2025-07-31*
