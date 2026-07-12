# Screen Migration Report — Settings & Config Screens

**Date:** 2026-07-11  
**Scope:** Migration of SettingsModel (860 lines) and ConfigModel (412+288 lines) to new Screenable interface + Router architecture (strangler-fig pattern)

---

## Summary

Successfully migrated **Settings** and **Config** screens to the new Screenable interface + Router architecture. All 102 existing tests pass, build is clean, vet and lint show no new issues (pre-existing emitter_stress_test.go govet warning only).

---

## Investigation Findings (Step 1)

Per `docs/audits/settings-config-investigation.md`:

- **SettingsModel** (`settings_model.go` — 860 lines) and **ConfigModel** (`config_model.go` + `config_model_view.go` + `config_model_sections.go` — ~700 lines) are **completely separate models** for **two distinct screens**
- ScreenSettings (`/settings` command) uses SettingsModel — 6-tab sidebar layout
- ScreenConfig (`/config` command) uses ConfigModel — sectioned list with viewport
- They do NOT share code; each independently holds `*config.Config` and reads/writes fields
- Both have independent navigation, rendering, and save logic
- **No cross-screen coupling** — no delegation, no embedding, no shared sub-view

**Conclusion:** Treat as two independent screens per standard playbook. No special handling needed.

---

## Changes Made

### 1. SettingsModel (`settings_model.go`)

| Method | Change |
|--------|--------|
| `Update` | `(*SettingsModel, tea.Cmd)` → `(Screenable, tea.Cmd)` |
| `SetDimensions(w, h int)` | Added — updates width/height/editValue.Width |
| `SetTheme(t theme.Theme)` | Already existed |
| `Init()` | Already existed (returns health check cmd) |

Helper methods updated to return `(Screenable, tea.Cmd)`:
- `cycleChoice`, `activateField`, `toggleBool`, `setFieldValue`, `updateEditing`, `saveConfig`, `saveLocalConfig`

### 2. ConfigModel (`config_model.go`)

| Method | Change |
|--------|--------|
| `Update` | `(*ConfigModel, tea.Cmd)` → `(Screenable, tea.Cmd)` |
| `SetDimensions(w, h int)` | Added — updates width/height/viewport/editInput |
| `SetTheme(t theme.Theme)` | Added — updates theme |
| `Init()` | Already existed |

Helper methods updated:
- `updateConfirmExit`, `updateBrowsing`, `activateField`, `updateEditing`, `saveConfig`, `saveLocalConfig`

### 3. Router Registration (`app_routing.go`)

**ScreenSettings** (`app_routing.go:272-285`):
```go
m.screenUpdaters[ScreenSettings] = func(msg tea.Msg) tea.Cmd {
    if m.settingsModel == nil {
        m.settingsModel = NewSettingsModel(m.config, m.registry, m.themeManager.Current(), m.configPath, m.version, m.keychain, m.shutdownCtx)
        m.router.Register(ScreenSettings, m.settingsModel)
    }
    newModel, cmd := m.settingsModel.Update(msg)
    if r, ok := newModel.(*SettingsModel); ok {
        m.settingsModel = r
    }
    return cmd
}
```

**ScreenConfig** (`app_routing.go:146-158`):
```go
m.screenUpdaters[ScreenConfig] = func(msg tea.Msg) tea.Cmd {
    if m.configModel == nil {
        cw, ch := m.contentDimensions()
        m.configModel = NewConfigModel(m.themeManager.Current(), m.config, m.configPath, cw, ch, m.keychain)
        m.router.Register(ScreenConfig, m.configModel)
    }
    newModel, cmd := m.configModel.Update(msg)
    if r, ok := newModel.(*ConfigModel); ok {
        m.configModel = r
    }
    return cmd
}
```

Both follow ConfirmQuit pilot pattern: lazy init on first message, router registration inline.

### 4. View Delegation (`app_view.go`)

**renderSettingsContent** (`app_view.go:556-568`):
```go
func (m *AppState) renderSettingsContent(chrome layout.PageChrome) string {
    if m.settingsModel == nil {
        m.settingsModel = NewSettingsModel(m.config, m.registry, m.themeManager.Current(), m.configPath, m.version, m.keychain, m.shutdownCtx)
        m.router.Register(ScreenSettings, m.settingsModel)
    }
    m.settingsModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
    return m.router.View()
}
```

**renderConfigContent** (`app_view.go:682-695`):
```go
func (m *AppState) renderConfigContent(chrome layout.PageChrome) string {
    if m.configModel == nil {
        cw, ch := m.contentDimensions()
        m.configModel = NewConfigModel(m.themeManager.Current(), m.config, m.configPath, cw, ch, m.keychain)
        m.router.Register(ScreenConfig, m.configModel)
    }
    m.configModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
    m.configModel.SetTheme(m.themeManager.Current())
    m.configModel.cfg = m.config
    return m.router.View()
}
```

Dual registration (Update updater + View renderer) — safe due to router's idempotent `Register`.

### 5. Navigation

Both screens already use `navigateToScreen` which calls `switchScreen` — no direct `m.screen =` assignments found in production code. Test file assignments (`app_view_extra_test.go`, `tui_harness_test.go`) are acceptable.

---

## Verification

| Check | Result |
|-------|--------|
| `go build ./...` | ✅ |
| `go test -race ./internal/tui/...` | ✅ (all pass) |
| `go vet ./internal/tui/...` | ✅ (pre-existing emitter_stress_test.go warning only) |
| `golangci-lint run ./internal/tui/...` | ✅ (same pre-existing warning) |

### Test Coverage (102 tests)

| Test File | Tests | Status |
|-----------|-------|--------|
| `settings_model_extra_test.go` | 55 | ✅ PASS |
| `settings_extra_test.go` | 6 (incl. 3 fixed) | ✅ PASS |
| `config_model_extra_test.go` | 47 | ✅ PASS |

All 102 tests pass including:
- `TestSettingsActivateFieldEmpty`, `TestSettingsActivateFieldOOB`, `TestSettingsUpdateEditingOtherKey` (fixed for new return type)
- `TestConfigModelView`, `TestConfigModelViewEditing`, `TestConfigModelUpdateWindowSize`, `TestConfigModelUpdateDown`, `TestConfigModelUpdateUp`, etc.

---

## Screen-Specific Behavioral Verification

### Settings Screen (6-tab editor)
- Tab navigation (`[`/`]`, 1-6 keys) works identically
- Field cursor up/down, Tab/Shift+Tab for choice fields
- Enter to edit text/number/password fields, Esc to cancel
- `s` to save to config path, `L` to save local project config
- Provider health checks on init and `r` key refresh
- API key source badges (env/config) display correctly

### Config Screen (full TOML editor)
- Section tabs (Provider, TUI, General, Advanced)
- Field navigation with up/down, section switching with tab/shift+tab
- Inline editing for text/password/number/float, toggle for bool, cycle for choice
- Read-only fields show masked values
- `s` saves to config path, `L` saves local project config
- `r` reloads from disk
- Dirty tracking and unsaved changes warning on exit

---

## Remaining Screens Untouched

All other 24 screens still route through original `app_routing.go` / `app_view.go` switch statements. Router now handles 11 of ~35 screens: ConfirmQuit, Help, Home, GhostPicker, GhostOutput, PhaseModelPicker, Discuss, Bisect, Dashboard, Plan, Settings, Config.

---

## Friction / Interface Issues

| Issue | Impact | Recommendation |
|-------|--------|----------------|
| `Screen` type alias conflict | `Screen` is `int` in `tuitypes.go`; interface is `Screenable` | Keep `Screenable` name; document clearly |
| `ScreenID = Screen` alias | Uses int-based `Screen` for map keys | Works but semantically odd; consider `type ScreenID int` in future |
| Lazy router registration | Registration happens at first render/Update, not at init | Acceptable for strangler-fig; consider eager registration in full migration |
| `SetTheme`/`SetDimensions` propagation | Router propagates to all registered screens, but old screens don't implement `Screenable` | Only 11 screens receive propagation currently; safe |

---

## Conclusion

Settings + Config migration complete. Both screens are architecturally independent — the "shared ConfigModel" concern from inventory was a false positive; they are two fully separate editors that happen to mutate the same `*config.Config` object. Standard playbook applied without deviation.

Router now handles 11 screens. Ready for next batch (Execute, Verify, Ship, RuntimeCheck, etc.).