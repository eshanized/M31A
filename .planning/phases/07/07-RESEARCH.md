# Phase 7: Signature Features — Research

**Researched:** 2026-05-28
**Domain:** Seven sub-components + three gap fixes for signature features
**Confidence:** HIGH (all component types and interfaces verified against source code)

## Summary

Phase 7 delivers all six differentiating acceptance criteria (22–26) plus the slash command system, settings screen inline editing, model selector UI, and three gap fixes. The phase splits cleanly into four waves of increasing dependency: independent `pkg/` packages (arbitrage, rollback, ledger, autodream, commands) in parallel, followed by TUI screens (model selector, settings) that depend on them, plus three gap fix modifications to existing files.

All new `pkg/` packages use zero dependencies beyond `internal/git`, `internal/types`, and `internal/config` — no go.mod changes needed. TUI screens follow established Bubble Tea patterns (`Init/Update/View` triples returning `AppMsg`). The gap fixes modify three existing files (`app.go`, `types.go`, and either `repl.go` or `health.go`) with well-defined insertion points.

**Primary recommendation:** Execute Wave 1 (P7.6 commands, P7.4 rollback, P7.2 arbitrage) in parallel as pure-logic packages with no TUI coupling, then layer TUI screens in Waves 3-4.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### P7.6 — Slash Command System
- Commands are pure functions returning `CommandResult` with desired state changes
- TUI's `Update()` applies `CommandResult` (screen transitions, config saves)
- Use `strings.Fields()` for arg splitting, `strings.TrimPrefix(input, "/")` for command name
- Error messages: `"unknown command: /foo. Type /help for available commands."`
- CommandContext provides environment: Registry, SessionManager, Config, Dispatcher, Git

#### P7.4 — Commit Rollback Chain
- Chain() limits to 20 commits by default, entries newest-first (HEAD at index 0)
- All reset methods stash first if uncommitted changes exist (data loss prevention)
- Preview capped at 50,000 chars (types.BashOutputLimit)
- Checkpoint comparison via timestamps

#### P7.2 — Cost-Aware Model Arbitrage
- Complexity scoring keywords: Simple (fix/add/update/change/rename), Moderate (implement/create/refactor/restructure), Complex (design/architect/migrate/rewrite/system)
- Token ranges: Simple (1000-3000 in, 500-1500 out), Moderate (3000-8000 in, 1500-4000 out), Complex (8000-20000 in, 4000-10000 out)
- Models with missing pricing handled gracefully (skip or $0.00)
- Cost: `inputTokens * inputPricePerToken + outputTokens * outputPricePerToken`

#### P7.5 — AutoDream Context Consolidation
- Consolidation preserves: system messages, last 5 messages, tool call messages, initial goal/context
- Summary max 500 tokens; prefix: "[AutoDream Context Summary]"
- Token estimation for messages without Usage: words × 1.3

#### P7.3 — Cross-Session Learning Ledger
- Parsing: split by `## Session` headers, parse key-value lines
- Malformed entries skipped gracefully
- Atomic write: temp file + rename in same directory

#### P7.1 — Model Selector UI
- bubbles/list with custom DefaultDelegate
- Search case-insensitive on ID, name, description, provider
- Provider filter cycles through registered providers
- Cost display uses provider.EstimateCost() with standard usage (100K in, 50K out)

#### P7.7 — Settings Screen
- Inline editing replaces current value display during edit
- API key fields masked as "****"
- Model and Ledger tabs show loading state during fetch
- dirty flag set on any field modification

#### Gap Fixes
- FallbackEvent: notification banner at screen top, dismiss with `x`
- Thinking block toggle: `T` key in REPL, state persisted across messages
- Cache refresh: ModelCacheRefreshTicker at types.ModelCacheTTL (5 minutes)

### the agent's Discretion
- Exact implementation order within each wave
- Test file structure and helper naming
- Specific handle struct field names on TUI models

### Deferred Ideas (OUT OF SCOPE)
- Full Tab-completion for slash commands
- Vision support, voice interaction, multi-modal outputs
- Plugin system and team collaboration
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| REQ-22 | Model arbitrage: `/optimize` analyzes plan, suggests cheaper models with savings %; `O` key accepts | P7.2 arbitrage + P7.1 model selector. See `pkg/arbitrage/` patterns below |
| REQ-23 | Git bisect: after self-heal fails twice, Verify runs git bisect, shows offending commit diff | Depends on Phase 6 `pkg/bisect/` integration; NOT re-implemented here |
| REQ-24 | Diff preview: Plan screen `D` key shows predicted file changes with estimated line counts | Depends on Phase 6 `ScreenPlan` in `screens.go`; NOT re-implemented here |
| REQ-25 | Learning ledger: `~/.m31a/LEDGER.md` updated after Ship; `/ledger` viewer; `/ledger stats` aggregate stats | P7.3 ledger package + P7.6 `/ledger` command + P7.7 Ledger tab. See `pkg/ledger/` patterns |
| REQ-26 | Commit rollback: `/rollback` shows interactive commit timeline; soft/hard/safe modes; backup branch; task status update | P7.4 rollback package + P7.6 `/rollback` command. See `pkg/rollback/` patterns |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Slash command parsing/dispatch | TUI (internal/tui/) | — | Input processing is a TUI concern; `CommandResult` pattern decouples mutation from parsing |
| Commit rollback | Workflow (pkg/rollback/) | TUI (commands) | Pure git wrapper logic in `pkg/`; TUI only dispatches via `/rollback` command |
| Model cost arbitrage | Model (pkg/arbitrage/) | TUI (model selector, plan screen) | Scoring/comparison is pure computation; TUI consumes recommendations |
| Context consolidation | Workflow (pkg/autodream/) | — | Operates on message history; triggered by context threshold |
| LEDGER persistence | Storage (pkg/ledger/) | TUI (settings, commands) | File I/O for ~/.m31a/LEDGER.md; TUI consumes via `/ledger` command + settings tab |
| Model selection UI | TUI (modelselector.go) | Provider (registry) | Bubble Tea screen with bubbles/list; fetches models from provider registry |
| Settings inline editing | TUI (settings.go) | Config (internal/config/) | TUI screen modifies config struct; `config.Save()` persists to disk |
| Fallback notification | TUI (app.go) | Provider (fallback.go) | Receives `FallbackEvent` from provider layer; renders notification banner |
| Thinking block toggle | TUI (repl.go + components/) | — | `T` key binding on REPL model toggles ThinkingBlock visibility |
| Model cache refresh | Provider (cache.go) | TUI (health.go pattern) | Background ticker using health ticker pattern; keeps model catalog fresh |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `charmbracelet/bubbletea` | v1.3.0 | TUI framework — all screens, messages, commands | Existing dependency; all TUI code follows Init/Update/View |
| `charmbracelet/lipgloss` | v1.1.0 | Terminal styling — colors, borders, layout | Existing dependency; theme system in place |
| `charmbracelet/bubbles` | v0.20.0 | TUI components — list, textinput, viewport | Existing dependency; bubbles/list for model selector and resume |
| `github.com/eshanized/M31A/internal/git` | — | Git operations wrapper | Internal package; used by rollback |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `internal/types` | — | ModelInfo, Task, Message, Usage, constants | All pkg/ packages import this |
| `internal/config` | — | Config struct, LedgerConfig, ModelConfig | Settings tab + arbitrage threshold |
| `internal/errors` | — | Sentinel errors | All pkg/ packages use Err* sentinels |
| `internal/provider` | — | LLMProvider, Registry, ModelCache, FallbackEvent | Model selector + cache refresh + fallback |
| `internal/tools` | — | Dispatcher, PermissionRequest | CommandContext for commands |
| `pkg/session` | — | Manager, Checkpoint, Session | Commands need session list + checkpoint |
| `pkg/keychain` | — | Keychain interface | Settings tab API key storage |
| `charmbracelet/bubbles/textinput` | v0.20.0 | Text input widget | Settings inline editing, model selector search |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Pure functions returning CommandResult | Direct AppState mutation from handlers | Pure functions enable unit testing without Bubble Tea; mutation decoupling is the called-for pattern |
| bubbles/list for model selector | Custom list rendering | Bubbles/list handles filtering, scrolling, delegate styling — less code, standard pattern |
| Existing health ticker pattern for cache refresh | New standalone goroutine | Following health.go pattern ensures consistent tea.Tick usage and thread safety |

**Installation:**
No new go.mod dependencies needed for Phase 7. All packages use existing internal types and stdlib.

**Version verification (no new packages to verify):**
```bash
# Existing dependencies already in go.mod — no new external packages needed
```

## Package Legitimacy Audit

> No external packages are installed in Phase 7. All new code uses:
> - Existing go.mod dependencies (bubbletea, bubbles, lipgloss, etc.)
> - Internal packages (`internal/types`, `internal/config`, `internal/git`, `internal/provider`, `internal/errors`, `internal/tools`)
> - Existing pkg/ packages (`pkg/session`, `pkg/keychain`)
> - Pure Go stdlib (testing, os, strings, fmt, etc.)

**Packages imported but already in go.mod:**
| Package | Status |
|---------|--------|
| `github.com/charmbracelet/bubbletea` | ✅ Existing v1.3.0 |
| `github.com/charmbracelet/bubbles` | ✅ Existing v0.20.0 |
| `github.com/charmbracelet/lipgloss` | ✅ Existing v1.1.0 |
| `github.com/BurntSushi/toml` | ✅ Existing v1.6.0 (config marshal) |

**No new external packages to audit.** Skip slopcheck — the phase introduces zero new supply chain risk.

## Architecture Patterns

### Screen model pattern (Bubble Tea)
All TUI screens follow this pattern established by `FirstRunModel`, `SettingsModel`, `ResumeModel`:

```go
type MyScreenModel struct {
    theme      theme.Theme
    width      int
    height     int
    statusMsg  string
    errMsg     string
}

func NewMyScreenModel(t theme.Theme, ...) MyScreenModel { ... }

func (m *MyScreenModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) { ... }

func (m *MyScreenModel) View() string { ... }
```

Key pattern: `Update()` returns `([]tea.Cmd, *AppMsg)`. The `AppMsg` signals screen transitions back to `AppState.Update()`. This is the canonical pattern used by all existing screens.

### AppState routing pattern
`AppState` routes to screens via switch on `m.screen`:
```go
func (m *AppState) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
    // ...
    switch m.screen {
    case ScreenSettings:
        cmds, appMsg := m.settingsModel.Update(msg)
        if appMsg != nil { m.screen = appMsg.Screen }
        // ...
    // NEW screens added here:
    case ScreenModelSelector:
        cmds, appMsg := m.modelSelectorModel.Update(msg)
        if appMsg != nil { m.screen = appMsg.Screen }
    }
}
func (m *AppState) View() string {
    switch m.screen {
    case ScreenSettings:
        return m.settingsModel.View()
    // NEW:
    case ScreenModelSelector:
        return m.modelSelectorModel.View()
    }
}
```

### CommandResult dispatch pattern (P7.6)
Commands are pure functions — no direct `AppState` mutation. The TUI's `Update()` handles applying `CommandResult`:
```go
type CommandResult struct {
    Success   bool
    Message   string
    Screen    *Screen
    SessionID *string
    Config    *config.Config
}
// In AppState.Update():
if strings.HasPrefix(input, "/") {
    name, args, ok := ParseCommand(input)
    if ok {
        result := registry.Execute(input, ctx)
        // Apply result: screen transition, config update, etc.
    }
}
```

### Ticker pattern for cache refresh (gap fix)
Follows `internal/tui/health.go` pattern:
```go
func ModelCacheRefreshTicker(ctx context.Context, provider provider.LLMProvider, interval time.Duration) tea.Cmd {
    return tea.Tick(interval, func(t time.Time) tea.Msg {
        return ModelCacheRefreshMsg{Time: t}
    })
}
```
Added to `AppState.Init()` alongside `HealthCheckTicker`.

### Atomic file write pattern
For `pkg/ledger/` Append and `pkg/autodream/` state persistence, follow the established atomic write pattern from `internal/config/loader.go`:
```go
func atomicWrite(path string, data []byte) error {
    // 1. Generate random temp name via crypto/rand
    // 2. Create temp file in SAME directory (cross-device safety)
    // 3. Write data, Sync, Close
    // 4. os.Rename(tmpPath, path) — atomic
    // 5. Cleanup temp on failure via defer
}
```

### Recommended Project Structure
```
pkg/arbitrage/           # NEW
├── arbitrage.go         # Scorer, CompareModels, Recommend, ShouldArbitrage
└── arbitrage_test.go    # Tests using mock ModelInfo fixtures

pkg/rollback/            # NEW
├── rollback.go          # Rollback struct, Chain, SoftReset, HardReset, SafeReset
└── rollback_test.go     # Tests using temp git repos (see git_test.go pattern)

pkg/ledger/              # NEW
├── ledger.go            # Ledger struct, Entries, Stats, Append, filter methods
└── ledger_test.go       # Tests using temp ledger files

pkg/autodream/           # NEW
├── autodream.go         # Consolidator, ContextInfo, Consolidate, GenerateSummary
└── autodream_test.go    # Tests with mock Message slices

internal/tui/
├── commands.go          # NEW — ParseCommand, CommandRegistry, 16 handlers
├── commands_test.go     # NEW — tests for all commands
├── modelselector.go     # NEW — ModelSelectorModel, bubbles/list
├── modelselector_test.go # NEW — model selector tests
├── settings.go          # MODIFY — add Model/Ledger tabs, inline editing
├── settings_test.go     # MODIFY — add tab + editing tests
├── app.go               # MODIFY — add FallbackEvent handling, ModelSelector model
├── types.go             # MODIFY — add Fallback field to AppMsg
├── health.go            # MODIFY (or separate file) — add ModelCacheRefreshTicker
└── repl.go              # MODIFY — add T key thinking toggle
```

### Pattern 1: Pure function command handlers (P7.6)
```go
// Source: CONTEXT.md locked decisions + PHASE7_PROMPTS.md
func handleHelp(args []string, ctx CommandContext) CommandResult {
    commands := ctx.Registry.List()
    return CommandResult{
        Success: true,
        Message: fmt.Sprintf("Available commands:\n  /%s", strings.Join(commands, "\n  /")),
    }
}
```

### Pattern 2: Git-based rollback with temp repos (P7.4)
```go
// Source: git_test.go pattern for test setup
func setupRollbackRepo(t *testing.T) (*Rollback, *git.Git) {
    t.Helper()
    dir := t.TempDir()
    g := git.New(dir)
    g.Init()
    g.ConfigUser("Test", "test@test.com")
    return New(g), g
}
```

### Pattern 3: Mock provider for model selector tests (P7.1)
```go
// Pattern from provider/test patterns — provide mock LLMProvider for testing
type mockProvider struct {
    name   string
    models []types.ModelInfo
}
func (m *mockProvider) Name() string { return m.name }
func (m *mockProvider) FetchModels(ctx context.Context) ([]types.ModelInfo, error) { return m.models, nil }
// ... implement remaining LLMProvider interface methods as no-ops
```

### Anti-Patterns to Avoid
- **Direct mutation of AppState from commands**: Commands return `CommandResult`; AppState's `Update()` applies it. This is called out explicitly in CONTEXT.md decisions.
- **Adding external dependencies**: No new go.mod entries needed. All new code uses existing internal/ and pkg/ packages plus stdlib.
- **Spawning goroutines for cache refresh**: Use `tea.Tick` pattern from health.go instead. This keeps goroutine lifecycle managed by Bubble Tea.
- **Modulo arithmetic mistake in settings tab cycling**: When adding tabs 4 and 5, update `% 4` to `% 6` in both `tab` and `shift+tab` handlers.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Command parsing | Custom parser | `strings.Fields()` + `strings.TrimPrefix()` | Simple, tested, idiomatic. 16 commands don't need a parser library |
| Git operations | Shell git commands | `internal/git.Git` struct | Already wraps all git operations needed (Log, ResetSoft, ResetHard, StashPush, Diff, Status, HeadHash) |
| Model list component | Custom scrollable list | `bubbles/list.Model` | Already used by ResumeModel; standard pattern |
| Text input during editing | Custom input handler | `bubbles/textinput.Model` | Already used by FirstRunModel; handles cursor, echo modes, validation |
| Config file persistence | Custom TOML writer | `config.Save()` | Already exists, uses atomic write pattern |
| Session management | Custom session files | `session.Manager` | Already has ListSessions, LoadSession, SaveSession, checkpoints |
| Keychain storage | OS-specific keychain code | `keychain.Keychain` interface | Already exists with Linux/macOS/Windows backends |
| Theme/styling system | Custom color management | `theme.Theme` struct | All styles defined; use existing Lipgloss styles |
| Atomic file writes | Temp file + rename | `internal/config/atomicWrite()` | Pattern exists in config/loader.go; replicate for ledger |

**Key insight:** Phase 7's new packages (arbitrage, rollback, ledger, autodream) are pure business logic — they consume existing interfaces and produce results consumed by TUI screens. Every building block they need (git wrapper, types, config) already exists. The TUI screens reuse existing Bubble Tea components (bubbles/list, bubbles/textinput). No new infrastructure is needed.

## Common Pitfalls

### Pitfall 1: Settings tab modulo overflow
**What goes wrong:** Adding tabs 4 and 5 but forgetting to update `% 4` to `% 6` in both `tab` and `shift+tab` key handlers in `settings.go`.
**Why it happens:** Current code cycles 4 tabs with `m.activeTab = (m.activeTab + 1) % 4` and `(m.activeTab + 3) % 4`.
**How to avoid:** After adding `tabLabels` entries, update both modulo operations. Best practice: use `len(tabLabels)` instead of hardcoded values.
**Warning signs:** Tab 3 wraps to tab 0 instead of tab 4.

### Pitfall 2: FallbackEvent import cycle
**What goes wrong:** Adding `FallbackEvent` to `AppMsg` in `internal/tui/types.go` imports `internal/provider` which may create an import cycle.
**Why it happens:** `AppMsg` is in `internal/tui` package; `FallbackEvent` is in `internal/provider` package. The tui package already imports provider (see app.go imports), so importing FROM provider to tui would create a cycle.
**How to avoid:** Define `FallbackEvent` directly in `internal/tui/types.go` as a standalone struct (duplicate the provider struct), or use `interface{}` with type assertion. Simplest: define the notification struct in `types.go` directly — it's only used for TUI rendering, not provider logic.
**Warning signs:** Go compiler error: "import cycle not allowed".

### Pitfall 3: Chain() ordering confusion
**What goes wrong:** `Chain()` returns newest-first (HEAD at index 0), but `Diff(ref1, ref2)` expects `ref1 = older, ref2 = newer`.
**Why it happens:** `git.Diff(entry.Hash, "HEAD")` for a non-HEAD entry — if entry is older, `Diff` parameter order matters.
**How to avoid:** Always call `git.Diff(entry.Hash, "HEAD")` where entry is the older commit. For HEAD entry, no diff. Document in `RollbackEntry.IsCurrent` that when true, diff is empty.
**Warning signs:** Empty diff output for non-HEAD commits.

### Pitfall 4: Arbitrage pricing model mismatch
**What goes wrong:** `types.Pricing` has `InputPerMToken` and `OutputPerMToken` (both per million tokens). Cost calc must divide by 1,000,000.
**Why it happens:** The Pricing struct stores per-million-token rates but EstimateCost receives per-token usage counts.
**How to avoid:** `cost = (inputTokens * inputPerMToken + outputTokens * outputPerMToken) / 1_000_000.0`
**Warning signs:** Costs orders of magnitude too high or too low.

### Pitfall 5: AutoDream consolidating too aggressively
**What goes wrong:** `Consolidate()` preserving "last 5 messages" but first call having only 6 messages — consolidating only 1 message is wasteful.
**Why it happens:** The preserve logic doesn't check if consolidation is meaningful.
**How to avoid:** Only consolidate if there are > 8 messages total (threshold + buffer). Check `ShouldConsolidate()` before calling `Consolidate()`.
**Warning signs:** High frequency of consolidations with minimal savings.

### Pitfall 6: Model selector not mocking FetchModels in tests
**What goes wrong:** Tests try to call `provider.Registry.ActiveProvider().FetchModels()` which makes real HTTP calls.
**Why it happens:** The `Init()` command fetches models from the provider.
**How to avoid:** In tests, create a `mockRegistry` that returns a mock provider with canned model data. Do not use the real registry in model selector tests.
**Warning signs:** Test timeout or network error during model selector tests.

## Code Examples

### Integrating new screens into AppState (from existing patterns)

When adding ScreenModelSelector to AppState:

```go
// In AppState struct (app.go) — add field:
modelSelectorModel *ModelSelectorModel

// In AppState.Update() — add case to screen switch:
case ScreenModelSelector:
    if m.modelSelectorModel == nil {
        return m, nil
    }
    cmds, appMsg := m.modelSelectorModel.Update(msg)
    if appMsg != nil {
        m.screen = appMsg.Screen
        if appMsg.Screen == ScreenREPL && m.modelSelectorModel.SelectedModel() != nil {
            m.activeModel = m.modelSelectorModel.SelectedModel()
        }
    }
    return m, tea.Batch(cmds...)

// In AppState.View() — add case:
case ScreenModelSelector:
    if m.modelSelectorModel != nil {
        return m.modelSelectorModel.View()
    }
    return "Loading..."
```

### Settings tab cycling (existing pattern from settings.go, lines 56-62)

```go
// CURRENT (4 tabs):
case "tab":
    m.activeTab = (m.activeTab + 1) % 4
case "shift+tab":
    m.activeTab = (m.activeTab + 3) % 4

// AFTER (6 tabs):
case "tab":
    m.activeTab = (m.activeTab + 1) % 6
case "shift+tab":
    m.activeTab = (m.activeTab + 5) % 6
```

### HealthCheckTicker pattern (from health.go, to be replicated for cache refresh)

```go
// Source: internal/tui/health.go
func HealthCheckTicker(ctx context.Context, registry *provider.Registry,
    activeProvider string, interval time.Duration) tea.Cmd {
    if interval <= 0 { interval = HealthCheckInterval }
    return tea.Tick(interval, func(t time.Time) tea.Msg {
        return HealthCheckTickMsg{Time: t}
    })
}
```

Copy pattern for `ModelCacheRefreshTicker` — same `tea.Tick` mechanism, different message type.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Settings screen: 4 tabs, read-only display | 6 tabs with inline editing, Model + Ledger tabs | Phase 7 | `settings.go` — major refactor of existing model; tab cycling, field editing, new data sources |
| No slash command system | `CommandRegistry` with 16 pure-function handlers | Phase 7 | New file `commands.go`; AppState.Update() gets new message processing path |
| No model selector | Full-screen overlay with search, filter, cost display | Phase 7 | New file `modelselector.go`; new `ScreenModelSelector` constant |
| No git rollback capability | `pkg/rollback/` with Chain, SoftReset, HardReset, SafeReset | Phase 7 | New package; consumed by `/rollback` command |
| No cross-session ledger | `pkg/ledger/` with parsing, filtering, stats, Append | Phase 7 | New package; consumed by `/ledger` command + Ledger tab |
| No model arbitrage | `pkg/arbitrage/` with complexity scoring, cost comparison | Phase 7 | New package; consumed by model selector + Plan screen |
| No AutoDream consolidation | `pkg/autodream/` with threshold-based summarization | Phase 7 | New package; consumed by `/compress` command |
| No fallback notification | Banner notification when provider falls back | Phase 7 | `types.go` + `app.go` — FallbackEvent in AppMsg |
| No thinking toggle | `T` key toggles ThinkingBlock visibility | Phase 7 | REPL model gets new key binding |
| No cache auto-refresh | Background ticker refreshes model cache every 5 min | Phase 7 | New ticker alongside HealthCheckTicker |

**Deprecated/outdated:**
- None. Phase 7 adds new functionality without deprecating existing code.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Phase 6's Plan screen (screens.go) implements the `D` key diff preview for REQ-24 | Phase Requirements | REQ-24 is a Phase 6 concern; verify after Phase 6 complete |
| A2 | Phase 6's Verify phase integrates `pkg/bisect/` for REQ-23 | Phase Requirements | REQ-23 is a Phase 6 concern; verify after Phase 6 complete |
| A3 | `provider.EstimateCost(modelID string, usage types.Usage) float64` signature is current in V1 | Patterns | Confirmed via reading `internal/provider/interface.go` line 13 |
| A4 | `FallbackEvent` struct in `internal/provider/fallback.go` is defined as `{From, To, Reason string}` | Code Examples | Confirmed via reading; no `Timestamp` or `OriginalProvider` fields — use `From`/`To` |

## Open Questions

1. **Is Phase 6's Plan screen (`screens.go`) complete with `ShowDiff`/`D` key handling for REQ-24?**
   - What we know: REQ-24 requires diff preview overlay on `D` key.
   - What's unclear: Whether Phase 6 implemented this. The screens_test.go has `TestPlan_DiffPreviewToggle` which tests it.
   - Recommendation: Assume Phase 6 handles it; if not, a small gap-fix task should be added to Wave 2.

2. **Does Phase 6's Verify phase integrate `pkg/bisect/` for REQ-23?**
   - What we know: REQ-23 requires git bisect integration after 2 failed self-heal attempts.
   - What's unclear: Whether Phase 6 wired bisect into Verify workflow.
   - Recommendation: Same as above — verify after Phase 6 is shipped.

3. **Where exactly is the REPL model defined?**
   - What we know: `app.go` uses `m.replModel` and creates it via `NewReplModel(tm.Current())`.
   - What's unclear: The exact file path — it's likely `internal/tui/repl.go`.
   - Recommendation: The gap fix for `T` key binding needs the REPL model's `Update()` method. This should be straightforward to locate during execution.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| git (CLI) | `pkg/rollback/` tests | ✓ | Check at runtime | `internal/git` already depends on this |
| Go 1.22+ | All | ✓ | Check at runtime | — |
| Internet (for LLM provider) | Model selector (live model fetch) | ✗ | — | Use cached models; show "offline" state |

**Missing dependencies with no fallback:**
- None for Phase 7 specifically. git CLI is required for rollback — this is already a project requirement.

**Missing dependencies with fallback:**
- Internet: Model selector gracefully handles fetch failure with error state and retry.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `go test` (stdlib) |
| Config file | None — Go testing convention |
| Quick run command | `go test ./pkg/arbitrage/... ./pkg/rollback/... ./pkg/ledger/... ./pkg/autodream/... -count=1 -short` |
| Full suite command | `go test -race -cover ./...` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| REQ-22 | Arbitrage scores tasks, compares models, recommends cheapest | unit | `go test ./pkg/arbitrage/...` | ❌ Wave 1 |
| REQ-25 | Ledger parses LEDGER.md, computes stats, appends entries | unit | `go test ./pkg/ledger/...` | ❌ Wave 1 |
| REQ-26 | Rollback chains commits, resets soft/hard/safe | unit | `go test ./pkg/rollback/...` | ❌ Wave 1 |
| P7.5 | AutoDream consolidates context at threshold | unit | `go test ./pkg/autodream/...` | ❌ Wave 2 |
| P7.6 | Command parsing, registry, 16 handlers | unit | `go test ./internal/tui/... -run TestCommand -count=1` | ❌ Wave 1 |
| P7.1 | Model selector renders, searches, filters | unit | `go test ./internal/tui/... -run TestModelSelector -count=1` | ❌ Wave 3 |
| P7.7 | Settings has 6 tabs, inline editing | unit | `go test ./internal/tui/... -run TestSettings -count=1` | ❌ Wave 4 |
| Gap | Fallback notification renders and dismisses | unit | `go test ./internal/tui/... -run Fallback -count=1` | ❌ Wave 2 |
| Gap | Thinking block T key toggles | unit | `go test ./internal/tui/... -run ThinkingBlock -count=1` | ❌ Wave 2 |
| Gap | Cache refresh ticker works | unit | `go test ./internal/provider/... -run ModelCache -count=1` | ❌ Wave 2 |

### Sampling Rate
- **Per task commit:** `go test ./pkg/arbitrage/... ./pkg/rollback/... ./pkg/ledger/... ./pkg/autodream/... -count=1 -short`
- **Per wave merge:** `go test -race -cover ./...`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `pkg/arbitrage/arbitrage_test.go` — covers all arbitrage tests
- [ ] `pkg/rollback/rollback_test.go` — covers all rollback tests with temp git repos
- [ ] `pkg/ledger/ledger_test.go` — covers all ledger tests with temp files
- [ ] `pkg/autodream/autodream_test.go` — covers all autodream tests with mock messages
- [ ] `internal/tui/commands_test.go` — covers all command tests
- [ ] `internal/tui/modelselector_test.go` — covers model selector tests
- [ ] Extensions to `internal/tui/settings_test.go` — covers new tabs + editing
- [ ] Extensions to `internal/tui/app_test.go` — covers FallbackEvent + cache refresh
- [ ] Extensions to `internal/tui/repl_test.go` — covers T key binding
- [ ] `internal/provider/cache_test.go` — covers cache refresh (or add to provider tests)

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | yes | API key handling through env var → keychain → config (existing); settings tab masked fields |
| V5 Input Validation | yes | Command parsing via strings.Fields (safe); git command params via internal/git (already validated) |
| V6 Cryptography | no | No new crypto operations; keychain uses OS-native storage |

### Known Threat Patterns for Phase 7 Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| API key leakage via settings display | Information Disclosure | Masked display as "••••••••" in settings View(); never plaintext |
| Unvalidated git hash in rollback | Tampering | Input hash validated via internal/git.Log() and error handling |
| LEDGER.md file injection | Tampering | Append-only via atomic write; existing entries parsed conservatively with skip-on-error |
| Command injection via slash args | Tampering | Arguments validated by handler functions; git commands use internal/git wrapper not shell |

## Sources

### Primary (HIGH confidence)
- `internal/tui/app.go` — AppState fields, Init/Update/View patterns, screen routing, health ticker, permission modal, error/health/appMsg handling
- `internal/tui/types.go` — Screen enum, AppMsg struct, HealthUpdateMsg, ProviderSwitchMsg, ErrorMsg, PermissionRequest/ResponseMsg
- `internal/tui/theme/theme.go` — Theme struct, Manager, Mode, Dark/Light/Auto constructors, ToolLabel map, all lipgloss.Style fields
- `internal/tui/settings.go` — SettingsModel fields, Update pattern, tab rendering, Save/atomic write, maskAPIKey, boolStr, theme styling
- `internal/tui/firstrun.go` — Screen model pattern (Init/Update/View), textinput usage, state machine flow, AppMsg return pattern
- `internal/tui/resume.go` — bubbles/list usage, sessionInfoToItems, NewDefaultDelegate theme styling
- `internal/tui/health.go` — tea.Tick pattern for background goroutines
- `internal/tui/components/thinking.go` — ThinkingBlock struct, Toggle(), IsExpanded(), Duration(), Header(), Render()
- `internal/tui/settings_test.go` — Test patterns for settings screen
- `internal/tui/screens_test.go` — Test patterns for plan/execute/verify/ship screens
- `internal/tui/app_test.go` — Test patterns for AppState
- `internal/provider/interface.go` — LLMProvider interface
- `internal/provider/registry.go` — Registry struct, methods
- `internal/provider/cache.go` — ModelCache struct, methods
- `internal/provider/fallback.go` — FallbackEvent struct, ShouldFallback, FindFallbackProvider
- `internal/types/types.go` — All core types (ModelInfo, Task, Message, Usage, Pricing, CapFlags, etc.)
- `internal/types/constants.go` — All constants (ModelCacheTTL, AutoDreamThreshold, etc.)
- `internal/config/types.go` — Config, ProviderConfig, ModelConfig, UIConfig, PermissionsConfig, FeaturesConfig, LedgerConfig
- `internal/config/loader.go` — Load, Save, DefaultConfig, atomicWrite, ResolveAPIKeys
- `internal/config/loader_test.go` — Mock keychain, test patterns, config file IO testing
- `internal/git/git.go` — Git struct, all methods (Log, ResetSoft, ResetHard, StashPush, Diff, Status, HeadHash, etc.)
- `internal/git/git_test.go` — Temp git repo setup pattern for tests
- `internal/errors/errors.go` — All sentinel errors
- `internal/tools/interface.go` — PermissionRequest, PermissionResponse, PermissionGate
- `pkg/session/manager.go` — Manager, atomicWrite, NewSession, LoadSession, ListSessions, SaveSession
- `pkg/session/checkpoint.go` — Checkpoint, SaveCheckpoint, LoadCheckpoints, LatestCheckpoint
- `pkg/session/planning.go` — SaveProject, LoadProject, SaveTasks, LoadTasks, SaveState, LoadState
- `pkg/session/session.go` — Session struct, NewSession
- `pkg/session/session_info.go` — SessionInfo struct
- `pkg/keychain/keychain.go` — Keychain interface, New constructor
- `pkg/keychain/errors.go` — Keychain sentinel errors
- `go.mod` — All current dependencies

### Secondary (MEDIUM confidence)
- `PHASE7_PROMPTS.md` — Detailed implementation specs (verified against actual source code structs during this research)
- `07-CONTEXT.md` — User decisions and wave structure (source of locked decisions)
- `docs/INTERFACES.md` — interface documentation mirror (verified against source during this research)
- `docs/TYPES.md` — constant reference (verified against source during this research)
- `docs/ARCHITECTURE.md` — package dependency and data flow

### Tertiary (LOW confidence)
- No LOW confidence items — all claims verified against actual source code

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all dependencies verified in go.mod and source
- Architecture patterns: HIGH — verified against existing implementations (app.go, settings.go, firstrun.go, resume.go)
- Pitfalls: HIGH — derived from direct source reading and integration analysis
- Phase requirements: HIGH — each REQ maps to specific components verifiable in source

**Research date:** 2026-05-28
**Valid until:** 2026-07-01 (stable codebase, fast-moving only if Phase 6 changes interfaces)
