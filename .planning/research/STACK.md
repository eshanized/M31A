# Stack Research: M31A UI Refactor

**Domain:** Terminal UI (Bubble Tea) - REPL rendering fixes and dynamic model selection
**Researched:** 2025-07-31
**Confidence:** HIGH

## Executive Summary

M31A is a mature Go/Bubble Tea application with 180+ TUI files. The two critical issues (blank REPL screen and hardcoded models) are **not stack problems** — they are **code-level bugs** in the existing, well-structured codebase. The stack is already correct; the implementation has defects.

**Blank REPL screen root cause:** The viewport is created in `Update()` on `tea.WindowSizeMsg`, but the initial `renderMessages()` call in `Init()` happens before the viewport receives its first size message. The viewport starts with width=0, height=0, so content is invisible.

**Hardcoded models root cause:** `ProviderShortName()` in `helpers_string.go` uses a hardcoded switch statement. The model selector infrastructure (`ModelSelector.Init()` → `fetchModelsCmd()`) already fetches models dynamically from providers — this is correct. The issue is in display/helper code that doesn't use the provider registry.

## Recommended Stack (No Changes Needed)

The existing stack is appropriate. No new dependencies are required.

### Core Framework (Already in Place)

| Technology | Version | Purpose | Why Correct |
|------------|---------|---------|-------------|
| `github.com/charmbracelet/bubbletea` | v1.3.0 | TUI framework (Elm architecture) | Standard for Go TUIs, correct version |
| `github.com/charmbracelet/bubbles` | v0.20.0 | TUI components (viewport, textarea) | Compatible with bubbletea v1.3.0 |
| `github.com/charmbracelet/lipgloss` | v1.1.0 | Terminal styling | Standard, no issues |
| `github.com/charmbracelet/glamour` | v0.6.0 | Markdown rendering | Appropriate for chat messages |

### Supporting Libraries (Already in Place)

| Library | Version | Purpose | Status |
|---------|---------|---------|--------|
| `github.com/BurntSushi/toml` | v1.6.0 | Config parsing | No issues |
| `github.com/godbus/dbus/v5` | v5.2.2 | Keychain integration | Linux-only, correct |
| `github.com/pkoukk/tiktoken-go` | v0.1.8 | Token estimation | No issues |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `charmbracelet/bubbletea/v2` | v2 is alpha/beta; v1.3.0 is stable and correct for this codebase | `bubbletea` v1.3.0 (already in use) |
| `donderom/bubblon` | Model stack library for new architectures; M31A already has screen routing | Existing `ScreenREPL`/`ScreenHome` routing |
| Custom viewport implementations | bubbles viewport is battle-tested; the bug is in initialization, not the component | Fix initialization order in `Init()` |
| Hardcoded model lists | Provider APIs change; models must be fetched dynamically | Use `provider.Registry.ActiveProvider().FetchModels()` |

## Fix Patterns (Not Stack Changes)

### Pattern 1: Viewport Initialization Fix

**What:** The viewport must receive its dimensions before content is rendered.

**When:** On startup, before `Init()` calls `renderMessages()`.

**Root cause in code:**
```go
// repl.go:Init() — BUG: viewport.Width == 0 here
func (m *ReplModel) Init() tea.Cmd {
    m.renderMessages()  // viewport has no dimensions yet!
    return StreamTickCmd()
}
```

**Fix:** Initialize viewport dimensions in `NewReplModel()` or ensure `tea.WindowSizeMsg` is processed before first render.

```go
// Option A: Set initial dimensions in NewReplModel
func NewReplModel(t theme.Theme, version string) ReplModel {
    // ... existing code ...
    // Initialize viewport with reasonable defaults
    m.viewport = viewport.New(80, 24)  // will be resized on first WindowSizeMsg
    m.viewport.SetContent(m.renderWelcome())
    return m
}

// Option B: Defer first render until dimensions are known
func (m *ReplModel) Init() tea.Cmd {
    // Don't render yet — wait for WindowSizeMsg
    return tea.Batch(StreamTickCmd(), func() tea.Msg {
        return tea.WindowSizeMsg{Width: 80, Height: 24}  // force initial size
    })
}
```

**Recommended:** Option A — set defaults in constructor, let `WindowSizeMsg` correct them.

### Pattern 2: Dynamic Provider Short Names

**What:** Replace hardcoded `ProviderShortName()` with registry-based lookup.

**When:** Always — provider list should come from the registry, not a switch statement.

**Root cause in code:**
```go
// helpers_string.go — HARDCODED
func ProviderShortName(name string) string {
    switch strings.ToLower(name) {
    case types.ProviderOpenRouter:
        return "OR"
    case types.ProviderZen, "zen-gateway":
        return "Zen"
    // ... more hardcoded cases
    }
}
```

**Fix:** Use a map or derive from provider metadata.

```go
// providerShortNames is a registry of known provider abbreviations.
// New providers register here or via the provider Registry.
var providerShortNames = map[string]string{
    types.ProviderOpenRouter: "OR",
    types.ProviderZen:        "Zen",
    types.ProviderNvidia:     "NV",
}

func ProviderShortName(name string) string {
    if abbr, ok := providerShortNames[strings.ToLower(name)]; ok {
        return abbr
    }
    if len(name) > 4 {
        return name[:4]
    }
    return name
}
```

### Pattern 3: Model Selection from Registry

**What:** Model selection UI should read from `provider.Registry`, not hardcoded lists.

**When:** In `ModelSelector.Init()` and any code that presents model choices.

**Status in code:** Already correct — `ModelSelector.Init()` calls `ms.registry.ListAll()` and `fetchModelsCmd()` for each provider. The infrastructure exists. Verify that `AppState.syncReplProvider()` properly wires the registry to the REPL model.

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| `bubbletea` v1.3.0 | `bubbles` v0.20.0 | Tested together, correct pairing |
| `bubbles` v0.20.0 | `lipgloss` v1.1.0 | Compatible |
| `glamour` v0.6.0 | `goldmark` v1.8.4 | Indirect dep, no conflicts |

## Source Analysis

### Key Files to Modify

| File | Issue | Fix |
|------|-------|-----|
| `internal/ui/tui/repl_model.go` | Viewport initialized with zero dimensions | Set defaults in `NewReplModel()` |
| `internal/ui/tui/repl.go` | `Init()` calls `renderMessages()` before viewport has size | Defer or set defaults |
| `internal/ui/tui/helpers_string.go` | `ProviderShortName()` hardcoded | Use registry-based map |
| `internal/ui/tui/repl_state.go` | `renderMessages()` writes to zero-size viewport | Guard against width==0 |

### Files That Are Already Correct

| File | Why Correct |
|------|-------------|
| `internal/ui/tui/modelselector_model.go` | Dynamically fetches models via `fetchModelsCmd()` |
| `internal/ui/tui/provider_registration.go` | Properly registers providers from config |
| `internal/integrations/provider/interface.go` | `FetchModels()` interface is correct |
| `internal/ui/tui/app.go` | `syncReplProvider()` triggers async model fetch |

## Confidence Assessment

| Area | Confidence | Reason |
|------|------------|--------|
| Stack | HIGH | Existing dependencies are correct and current |
| Viewport fix | HIGH | Root cause identified in initialization order |
| Model selection fix | HIGH | Infrastructure exists; just need to remove hardcoded values |
| No new deps needed | HIGH | All tools available in existing stack |

## What This Research Does NOT Cover

- **UI redesign** — out of scope per PROJECT.md
- **Performance optimization** — out of scope per PROJECT.md
- **New features** — out of scope per PROJECT.md
- **Bubble Tea v2 migration** — not recommended; v1.3.0 is stable

---
*Stack research for: M31A UI Refactor*
*Researched: 2025-07-31*
