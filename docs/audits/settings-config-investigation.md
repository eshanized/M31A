# Settings & Config Screen Investigation

**Date:** 2026-07-11  
**Purpose:** Investigate relationship between SettingsModel and ConfigModel before migration to Screenable + Router

---

## Findings

### 1. Are they the same model? **NO**

**SettingsModel** (`settings_model.go` — 860 lines) and **ConfigModel** (`config_model.go` + `config_model_view.go` + `config_model_sections.go` — ~700 lines) are **completely separate models** for **two distinct screens**:

| Aspect | ScreenSettings (SettingsModel) | ScreenConfig (ConfigModel) |
|--------|-------------------------------|---------------------------|
| Entry command | `/settings` | `/config` |
| UI paradigm | 6-tab sidebar (Provider, Model, UI, Keys, Workflow, About) | Sectioned list with tabs (Provider, TUI, General, Advanced) |
| Navigation | `[`/`]` or 1-6 keys, up/down for fields | Tab/`]` for sections, up/down for fields |
| Save action | `s` saves to config path, `L` saves local | `s` saves to config path, `L` saves local |
| Reload action | None | `r` reloads from disk |
| Keybind hints | Tab-based footer | Full footer with all keys |
| Complexity | Higher-level editor | Granular field editor (every TOML field) |

**They do not share code.** Each independently:
- Holds a `*config.Config` pointer
- Reads/writes fields via `getFieldValue`/`setFieldValue`
- Handles its own viewport/input/edit state
- Calls `config.SaveWithKeychain()` on save

### 2. Cross-Screen Coupling? **NONE**

- No navigation between them (no "open config from settings" or vice versa)
- No shared internal state
- `AppState` holds **both** `settingsModel` and `configModel` as independent fields
- Both receive the same `m.config` pointer, so edits in one reflect in the other on next render — but that's via the shared config object, not model coupling

### 3. Screenable Readiness

| Method | SettingsModel | ConfigModel |
|--------|---------------|-------------|
| `Init() tea.Cmd` | ✅ (health checks) | ✅ (returns nil) |
| `View() string` | ✅ | ✅ |
| `Update(msg) (Model, Cmd)` | ✅ (returns `*SettingsModel`) | ✅ (returns `*ConfigModel`) |
| `SetTheme(theme.Theme)` | ✅ | ❌ (needs adding) |
| `SetDimensions(w, h int)` | ❌ (uses `width`/`height` fields directly) | ❌ (uses `width`/`height` fields directly) |

Both need:
- `Update` return type: `(*Model, tea.Cmd)` → `(Screenable, tea.Cmd)`
- `SetDimensions(w, h int)` method
- `SetTheme(theme.Theme)` method (ConfigModel only)

### 4. Routing & Rendering (Current)

**app_routing.go:**
- `ScreenSettings` updater: uses `m.settingsModel`, returns nil if nil
- `ScreenConfig` updater: uses `m.configModel`, returns nil if nil

**app_view.go:**
- `renderSettingsContent`: creates `SettingsModel` if nil, sets width/height, returns `View()`
- `renderConfigContent`: creates `ConfigModel` if nil, syncs `cfg`/`width`/`height`/`theme`, returns `View()`

**app_nav.go:**
- `ScreenSettings`: creates if nil, calls `SetDimensions`
- `ScreenConfig`: creates if nil

---

## Proposed Migration Approach

**Treat as two independent screens** — same as Discuss/Bisect/Dashboard. Each gets:
1. `Update` → `(Screenable, tea.Cmd)`
2. `SetDimensions`, `SetTheme` added
3. Router registration in both updater and renderer (dual registration pattern)
4. `m.screen = ScreenX` → `m.switchScreen(ScreenX)` in navigation

No special wrapper/delegation needed. The "shared config" is just `*config.Config` passed to both — each model owns its own view state.

---

## Test Coverage (Existing — Will Validate Migration)

| File | Tests |
|------|-------|
| `settings_model_extra_test.go` | 55 tests |
| `config_model_extra_test.go` | 47 tests |
| `handler_config_test.go` | 2 tests |

**Total: 102 tests** — the best-tested pair in the inventory. Migration must preserve all.

---

## Verdict

**Proceed with standard playbook for BOTH screens independently.** No architectural deviation needed. They happen to edit the same config object but are architecturally independent screens.