# Screen Migration Report — Help & Home Screens

**Date:** 2026-07-11  
**Scope:** Migration of Help (ScreenHelp, 404 lines) and Home (ScreenHome, 248 lines) screens to new Screenable interface + Router architecture (strangler-fig pattern, following ConfirmQuit pilot)

---

## Summary

Successfully migrated **Help** and **Home** screens to the new Screenable interface + Router architecture. All existing tests pass, build is clean, vet and lint show no new issues (pre-existing emitter_stress_test.go govet warning unrelated). The old and new dispatch paths coexist — Help and Home now route through the router, while all other 31 screens still use the original `app_routing.go` / `app_view.go` switch statements.

---

## Changes Made

### 1. Screenable Interface Conformance (`help_model.go`, `home_model.go`)

**HelpModel** (`help_model.go:330`):
- Changed `Update(msg tea.Msg) (tea.Model, tea.Cmd)` → `Update(msg tea.Msg) (Screenable, tea.Cmd)`
- No internal logic rewritten — pure boundary adaptation

**HomeModel** (`home_model.go:197`):
- Changed `Update(msg tea.Msg) (tea.Model, tea.Cmd)` → `Update(msg tea.Msg) (Screenable, tea.Cmd)`
- No internal logic rewritten — pure boundary adaptation

Both models already implemented `Init()`, `View()`, `SetDimensions(w, h int)`, and `SetTheme(theme.Theme)` — satisfied interface requirements without changes.

### 2. Router Registration (`app_routing.go`)

**ScreenHelp** (`app_routing.go:151-163`):
```go
m.screenUpdaters[ScreenHelp] = func(msg tea.Msg) tea.Cmd {
    if m.helpModel == nil {
        m.helpModel = NewHelpModel(m.themeManager.Current())
        m.helpModel.SetKeyRegistry(m.keyRegistry)
        // Register with router
        m.router.Register(ScreenHelp, m.helpModel)
    }
    newModel, cmd := m.helpModel.Update(msg)
    if r, ok := newModel.(*HelpModel); ok {
        m.helpModel = r
    }
    return cmd
}
```

**ScreenHome** (`app_routing.go:352-366`):
```go
m.screenUpdaters[ScreenHome] = func(msg tea.Msg) tea.Cmd {
    if m.homeModel == nil {
        cw, ch := m.contentDimensions()
        m.homeModel = NewHomeModel(m.themeManager.Current(), cw, ch, m.version)
        m.homeModel.SetCommandRegistry(m.cmdRegistry)
        m.homeModel.SetConfig(m.config)
        // Register with router
        m.router.Register(ScreenHome, m.homeModel)
    }
    newModel, cmd := m.homeModel.Update(msg)
    if r, ok := newModel.(*HomeModel); ok {
        m.homeModel = r
    }
    return cmd
}
```

Follows ConfirmQuit pilot pattern: lazy registration on first message, router registration inline.

### 3. View Delegation to Router (`app_view.go`)

**renderHelpContent** (`app_view.go:687-696`):
```go
func (m *AppState) renderHelpContent(chrome layout.PageChrome) string {
    // Ensure Help is registered with router
    if m.helpModel == nil {
        m.helpModel = NewHelpModel(m.themeManager.Current())
        m.helpModel.SetKeyRegistry(m.keyRegistry)
        m.router.Register(ScreenHelp, m.helpModel)
    }
    m.helpModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
    return m.router.View()
}
```

**renderHomeContent** (`app_view.go:800-810`):
```go
func (m *AppState) renderHomeContent(chrome layout.PageChrome) string {
    // Ensure Home is registered with router
    if m.homeModel == nil {
        m.homeModel = NewHomeModel(m.themeManager.Current(), chrome.ContentWidth(), chrome.ContentHeight(), m.version)
        m.homeModel.SetCommandRegistry(m.cmdRegistry)
        m.homeModel.SetConfig(m.config)
        m.router.Register(ScreenHome, m.homeModel)
    }
    m.homeModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
    return m.router.View()
}
```

Dual registration (in Update updater + View renderer) mirrors ConfirmQuit pilot — safe due to router's idempotent Register.

### 4. Screen Switch via `switchScreen` (`app_nav.go`)

**navigateToScreen** (`app_nav.go:241-270`):
- Changed `m.screen = screen` → `m.switchScreen(screen)` on both transition and non-transition paths
- `switchScreen` (defined in `app_state.go:473`) sets `m.screen` AND calls `m.router.SwitchTo(s)` to synchronize router active screen
- This change benefits ALL screens — router stays in sync for any future migrated screen

---

## Verification

| Check | Result |
|-------|--------|
| `go build ./...` | ✅ |
| `go test ./internal/tui/...` | ✅ (all pass) |
| `go test -race ./internal/tui/...` | ✅ |
| `go vet ./internal/tui/...` | ✅ (pre-existing emitter_stress_test.go govet warning only) |
| `golangci-lint run ./internal/tui/...` | ✅ (same pre-existing warning only) |

### Test Coverage Specific to Migration

| Test | Status |
|------|--------|
| `TestViewHelp` | ✅ PASS |
| `TestEnsureSubModel_Help` | ✅ PASS |
| `TestEnsureSubModel_Home` | ✅ PASS |
| `TestHandleKeyAction_OpenHome` | ✅ PASS |
| `TestHandleHelp` | ✅ PASS |
| `TestHomeSubmitMsg` / `_Empty` / `_Fields` | ✅ PASS |

All Help/Home related tests pass, including navigation, view rendering, and key handling.

---

## Manual Verification (Behavioral Equivalence)

### Help Screen (opens with `?` or `ctrl+x h`)

**Expected (pre-migration):**
1. Press `?` → Help overlay appears with full keybinding list from `keyRegistry`
2. Sections: Global, REPL, Leader Key, Workflow, Session, AI & Model, Git & Diff, Settings & System
3. Scroll with `up/down`, `pgup/pgdn`, `g` (top), `G` (bottom)
4. Close with `esc`, `q`, or `?`
5. Footer hints: "up/down scroll", "g/G top/bottom", "esc close"

**Verified (post-migration):**
- All sections render from `keyRegistry` dynamically (no hardcoded fallbacks used when registry present)
- Viewport scrolling works identically
- Key handlers (`esc`, `q`, `?`, `g`, `G`) return `PopScreenMsg` → `popScreen()` → `switchScreen(prev)` → router synchronized
- No visual or behavioral regression

### Home Screen (landing screen at startup, or `ctrl+x h` from REPL)

**Expected (pre-migration):**
1. Shows M31A logo, tagline "Autonomous coding agent"
2. Prompt input with rotating placeholders (4s interval)
3. Slash command autocomplete via `cmdRegistry` (`/new`, `/model`, `/help`, etc.)
4. Categorized suggested prompts (Code, Explore, Debug)
5. Keyboard shortcut tips row (ctrl+p, ctrl+x, ctrl+b, ctrl+m, ctrl+h)
6. Enter submits `HomeSubmitMsg` → intent classification → workflow or chat
7. First-visit tour hint if no prior sessions

**Verified (post-migration):**
- Logo, tagline, prompt, placeholders, suggestions, tips all render identically
- Slash autocomplete populates from `cmdRegistry` correctly
- `HomeTickMsg` rotates placeholders when input empty
- `HomeSubmitMsg` emitted on Enter with non-empty input
- `SetFirstVisit` / `ShowTour` / `DismissTour` state preserved
- Footer hints: "enter submit", "ctrl+p cmds", "ctrl+m models"

---

## Remaining Screens Untouched

All other 31 screens still route through original `app_routing.go` / `app_view.go` switch statements. Router only handles `ScreenConfirmQuit`, `ScreenHelp`, `ScreenHome`. Strangler-fig pattern confirmed viable.

---

## Friction / Interface Issues for Next Batch

| Issue | Impact | Recommendation |
|-------|--------|----------------|
| `Screen` type alias conflict | `Screen` is `int` in `tuitypes.go`; interface is `Screenable` | Keep `Screenable` name; document clearly |
| `ScreenID = Screen` alias | Uses int-based `Screen` for map keys | Works but semantically odd; consider `type ScreenID int` in future |
| `router.Register` takes `ScreenID` but model creation is lazy | Registration happens at first render/Update, not at init | Acceptable for strangler-fig; consider eager registration in full migration |
| `SetTheme`/`SetDimensions` propagation | Router propagates to all registered screens, but old screens don't implement `Screenable` | Only ConfirmQuit/Help/Home receive propagation currently; safe |
| `navigateToScreen` now calls `switchScreen` globally | All screen transitions now sync router — no-op for unregistered screens | Correct behavior; router.SwitchTo returns nil for unregistered IDs |
| Test helper `testAppState()` missing router | Added `router: NewRouter()` to fix `TestViewHelp` nil panic | Update test helpers proactively when adding router-dependent screens |

---

## Next Candidates (Phase 1 from inventory)

Per `docs/audits/screen-inventory.md`:
1. **GoalInput** (159 lines, standalone)
2. **FileExplorer** (142 lines, only `SetRoot()` + `SetDimensions()`)
3. **GhostPicker / GhostOutput** (193+178 lines, isolated feature)
4. **PhaseModelPicker** (337 lines, 7 tests, clean dual-model selection UI)

---

## Conclusion

The Screenable + Router pattern works end-to-end for Help and Home screens. The strangler-fig migration is viable: old and new paths coexist, zero regressions, minimal friction. Ready to proceed with Phase 1 batch migration.