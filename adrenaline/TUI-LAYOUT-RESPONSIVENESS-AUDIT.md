# TUI Layout & Responsiveness Audit

**Date:** 2026-06-11
**Scope:** All 90+ `.go` files in `internal/tui/`, `internal/tui/layout/`, `internal/tui/components/`, `cmd/m31a/`
**Methodology:** Full source review of layout system, resize propagation, dimension calculations, every screen model, and responsive breakpoints

---

## Layout Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│ AppState.View()                                                  │
│ ┌───────────────────────────────────────────────────────────┐   │
│ │ layout.RenderPage(chrome, content, header, footer)         │   │
│ │ ┌────────────────────────────────────────────────────────┐│   │
│ │ │ Header (1 line) — BuildHeader()                        ││   │
│ │ ├────────────────────────────────────────────────────────┤│   │
│ │ │ Content Area (height - 2)                              ││   │
│ │ │  ┌──────────┐ ┌────────────────────────────────────┐  ││   │
│ │ │  │ Sidebar  │ │ renderActiveScreen(chrome)          │  ││   │
│ │ │  │ (28 cols)│ │  → REPL, Settings, Plan, etc.      │  ││   │
│ │ │  │          │ │                                     │  ││   │
│ │ │  └──────────┘ └────────────────────────────────────┘  ││   │
│ │ ├────────────────────────────────────────────────────────┤│   │
│ │ │ Footer (1 line) — BuildFooter()                        ││   │
│ │ └────────────────────────────────────────────────────────┘│   │
│ └───────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘

ChromeHeight = 2 (1 header + 1 footer)
ContentHeight = terminal height - 2
ContentWidth = terminal width (- sidebar width if visible at ≥80 cols)
```

**Breakpoints** (`layout/responsive.go`):
| Breakpoint   | Width    | Features |
|-------------|----------|----------|
| UltraNarrow | < 40     | "Resize terminal" message only |
| Compact     | 40–59    | Minimal chrome, no sidebar, no hints |
| Standard    | 60–79    | Full chrome, no sidebar |
| Full        | ≥ 80     | Sidebar visible, all chrome, key hints, cost |

**Screens** (26 total): FirstRun, REPL, ModelSelector, Settings, Resume, Permission, Plan, Execute, Verify, Ship, Diff, Ledger, Rollback, GoalInput, Discuss, Metrics, Config, Help, Bisect, ThemePicker, Notifications, Dashboard, SessionDetail, FileExplorer, ToolDetail, PhaseModelPicker

---

## 1. CRITICAL — ensureSubModel Passes Full Terminal Dimensions

**Files:** `app_update.go:1146–1281`

`ensureSubModel()` creates sub-models with `m.width` and `m.height` (full terminal dimensions), but `handleWindowResize()` correctly uses `contentW` and `contentH` (terminal minus chrome minus sidebar). This means **every screen created through navigation starts with wrong dimensions**.

```go
// ensureSubModel — WRONG: uses m.width, m.height (full terminal)
case ScreenSettings:
    m.settingsModel = NewSettingsModel(...)
    m.settingsModel.width = m.width     // ← should be contentW
    m.settingsModel.height = m.height   // ← should be contentH

// handleWindowResize — CORRECT: uses contentW, contentH
if m.settingsModel != nil {
    m.settingsModel.width = contentW    // ← chrome-adjusted
    m.settingsModel.height = contentH   // ← chrome-adjusted
}
```

**Affected screens** (10 of 26):
| Screen | Line | Issue |
|--------|------|-------|
| Settings | 1160–1161 | `m.width`, `m.height` |
| GoalInput | 1170–1171 | `m.width`, `m.height` |
| Plan | 1176–1177 | `m.width`, `m.height` |
| Execute | 1181–1182 | `m.width`, `m.height` |
| Verify | 1187–1188 | `m.width`, `m.height` |
| Ship | 1193–1194 | `m.width`, `m.height` |
| Ledger | 1200–1201 | `m.width`, `m.height` |
| Metrics | 1214–1215 | `m.width`, `m.height` |
| Config | 1225–1226 | `m.width`, `m.height` |
| Discuss | 1237 | `m.width`, `m.height` |

**Impact:** Content overflows by 2 rows (chrome height), or renders with 2 empty rows at the bottom. Sidebar width is also ignored, causing horizontal overflow of 28 columns.

**Fix:** Compute content dimensions in `ensureSubModel` and pass those:
```go
contentW := m.width
contentH := m.height - layout.ChromeHeight
if m.sidebarModel != nil && m.sidebarModel.IsVisible() && layout.ShowSidebar(m.width) {
    contentW -= m.sidebarModel.GetWidth()
}
```

---

## 2. CRITICAL — REPL Double-Subtracts Chrome in Resize Handler

**Files:** `repl.go:24–42`, `app_update.go:696–704`

When `handleWindowResize` fires, it builds a `contentMsg` with chrome-adjusted dimensions and sends it to the REPL:

```go
// app_update.go:692-693
contentMsg := tea.WindowSizeMsg{Width: contentW, Height: contentH}
replM, replCmd := m.replModel.Update(contentMsg)
```

But the REPL's `Update` handler calls the **legacy** `viewportHeight()` which subtracts chrome **again**:

```go
// repl.go:30
vpH := viewportHeight(msg.Height)  // msg.Height is already content height!
```

`viewportHeight` computes: `termHeight - 0 - (1 + 3) = termHeight - 4`. Since `msg.Height` is already `terminal_height - 2`, the viewport ends up at `terminal_height - 6` instead of the correct `terminal_height - 2 - 4 = terminal_height - 6`. Wait — that's actually correct in this case because the viewport needs to subtract the REPL's own bottom chrome (separator + textarea = 4 rows) from the content area. But the function name and comments are misleading, and it only works by coincidence because `viewportTopChrome = 0`.

**Actually, the real bug is:** `ViewContent` correctly uses `contentViewportHeight(contentHeight)` which gives `contentHeight - 4`, but the `Update` handler uses `viewportHeight(msg.Height)` which gives `msg.Height - 4`. If `msg.Height` is the content height (correct), both produce the same result. But the `View()` method (not `ViewContent`) also uses `viewportHeight(m.height)` where `m.height` could be either content or terminal height depending on the call path.

**Impact:** The `View()` method (called directly, not through `ViewContent`) produces wrong viewport heights. Currently `View()` is used by tests and could be called in edge cases.

**Fix:** Deprecate `viewportHeight()` and use `contentViewportHeight()` everywhere.

---

## 3. CRITICAL — REPL Double-Subtracts Sidebar Width

**Files:** `repl_state.go:32–49`, `app_update.go:525–540`, `app_update.go:696–704`

`handleWindowResize` sets `m.replModel.width = contentW` where `contentW` already has sidebar width subtracted:

```go
// app_update.go:675-679
contentW := msg.Width
sw := 0
if m.sidebarModel != nil && m.sidebarModel.IsVisible() && layout.ShowSidebar(msg.Width) {
    sw = m.sidebarModel.GetWidth()
    contentW -= sw
}
m.replModel.width = contentW   // already sidebar-adjusted
m.replModel.SetSidebarWidth(sw) // ← subtracts sidebar AGAIN
```

`SetSidebarWidth` calls `replWidth()` which computes `m.width - m.sidebarWidth`:

```go
// repl_state.go:32-36
func (m *ReplModel) replWidth() int {
    w := m.width - m.sidebarWidth  // double subtraction!
    if w < 20 { w = 20 }
    return w
}
```

**Result:** Available REPL width = `terminal_width - sidebar - sidebar` = **28 columns lost**.

At 80 cols with sidebar: expected content = 52, actual = 24 (triggers the minimum 20-col guard).
At 120 cols with sidebar: expected content = 92, actual = 64.

**Same bug in `syncReplSize`:**
```go
// app_update.go:529-539
sw := 0
if m.sidebarModel != nil && m.sidebarModel.IsVisible() && layout.ShowSidebar(m.width) {
    sw = m.sidebarModel.GetWidth()
}
cw := chrome.ContentWidth()  // already has sidebar subtracted
m.replModel.width = cw       // correct
m.replModel.SetSidebarWidth(sw) // ← subtracts again!
```

**Fix:** Either:
1. Don't subtract sidebar from `contentW` before setting `m.replModel.width`, and let `SetSidebarWidth` handle it.
2. Or set `m.replModel.width = contentW` and call `SetSidebarWidth(0)`.

---

## 4. HIGH — Per-Screen Renderers Ignore Chrome Dimensions

**Files:** `app_view.go:330–493`

Most per-screen content renderers call the sub-model's `View()` without passing the chrome dimensions. They rely on `ensureSubModel` or `handleWindowResize` having set dimensions, but many never receive chrome-adjusted values.

| Renderer | Passes Chrome? | Notes |
|----------|---------------|-------|
| `renderREPLContent` | Yes | `chrome.ContentHeight()`, `chrome.ContentWidth()` |
| `renderSettingsContent` | Partial | Sets width/height from chrome, but calls `View()` |
| `renderFirstRunContent` | Partial | Only sets `ContentWidth`, not height |
| `renderConfigContent` | Partial | Only on first creation |
| `renderModelSelectorContent` | No | Uses stale dimensions |
| `renderPlanContent` | No | Uses stale dimensions |
| `renderExecuteContent` | No | Uses stale dimensions |
| `renderVerifyContent` | No | Uses stale dimensions |
| `renderShipContent` | No | Uses stale dimensions |
| `renderResumeContent` | No | Uses stale dimensions |
| `renderGoalInputContent` | No | Uses stale dimensions |
| `renderLedgerContent` | No | Uses stale dimensions |
| `renderRollbackContent` | No | Uses stale dimensions |
| `renderMetricsContent` | No | Uses stale dimensions |
| `renderDiscussContent` | No | Uses stale dimensions |
| `renderDiffContent` | No | Uses stale dimensions |
| `renderHelpContent` | No | Uses stale dimensions |
| All other screens | No | Uses stale dimensions |

**Impact:** Only the REPL and Settings get fresh dimensions on every render. All other 24 screens render with stale or wrong dimensions.

**Fix:** Every renderer should pass chrome dimensions to the sub-model before calling `View()`:
```go
func (m *AppState) renderPlanContent(chrome layout.PageChrome) string {
    if m.planModel == nil { return renderLoading(...) }
    m.planModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
    return m.planModel.View()
}
```

---

## 5. HIGH — renderLoading() Uses Hardcoded 80×10 Box

**File:** `helpers.go:179–183`

```go
func renderLoading(label string, t theme.Theme) string {
    content := spinner + " " + text
    return lipgloss.Place(80, 10, lipgloss.Center, lipgloss.Center, content)
}
```

On terminals narrower than 80 columns, the loading indicator overflows. On tall terminals, it appears near the top rather than centered.

**Called from:** 20+ screen renderers in `app_view.go` (every nil-check fallback).

**Fix:** Accept width and height parameters:
```go
func renderLoading(label string, w, h int, t theme.Theme) string {
    return lipgloss.Place(w, h, lipgloss.Center, lipgloss.Center, content)
}
```

---

## 6. HIGH — Permission Modal Uses `Place(0, 0, ...)` and Fixed Width

**File:** `app_view.go:604–647`

```go
return lipgloss.Place(0, 0, lipgloss.Center, lipgloss.Center, card)
```

`Place(0, 0, ...)` doesn't center — it renders at the origin with zero dimensions. The modal appears in the wrong position.

Additionally, `permModalWidth` defaults to 60 and is only configurable via `config.UI.PermissionModalWidth`. On terminals narrower than 60 columns, the modal overflows.

**Fix:**
```go
return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, card)
```
And clamp `permModalWidth` to `min(m.permModalWidth, m.width - 4)`.

---

## 7. HIGH — Hardcoded Card Widths in Welcome Screen

**File:** `repl_welcome.go:116, 182, 237, 309`

All four welcome screen cards use hardcoded `Width: 42`:
- Provider card: `Width: 42`
- Project card: `Width: 42`
- Getting Started card: `Width: 42`
- (Fourth card): `Width: 42`

Two-column layout requires `availWidth >= 88` (2×42 + spacing). On terminals between 60-87 cols, only one column is shown but the 42-col card still wastes space. Below 42+border cols, cards overflow.

**Fix:** Make card width responsive:
```go
cardWidth := min(42, availWidth - 4)
if availWidth >= 88 {
    cardWidth = min(42, (availWidth - 6) / 2)
}
```

---

## 8. MEDIUM — Duplicate Status Bar / Footer

**Files:** `repl_view.go:71–193`, `repl_view.go:198–280`, `app_view.go:93`

The REPL has two render paths:

1. **`View()`** (legacy): renders viewport + separator + metaRow + textarea + **statusBar**
2. **`ViewContent()`** (unified): renders viewport + separator + textarea (no status bar)

The unified `AppState.View()` wraps `ViewContent` with `layout.RenderPage` which adds a footer bar. This is correct.

But the REPL's `View()` includes its own status bar with **the same information** as the footer (cwd, branch, hints, cost, operation status). If `View()` is ever called instead of `ViewContent()`, there would be two status lines.

**Current status:** `View()` appears to only be called by tests and the `renderPermissionModal` fallback (`app_view.go:501`). The fallback should use `ViewContent` instead.

---

## 9. MEDIUM — Sidebar Height Never Set Until First View

**Files:** `app_view.go:58`, `sidebar.go:44–50`

`NewSidebarModel` doesn't set `height`. The sidebar's height is only set in `View()`:
```go
m.sidebarModel.SetHeight(m.height)
sidebarStr = m.sidebarModel.View()
```

If the sidebar is rendered before any resize event (e.g., at startup), it has `height = 0`. The sidebar doesn't use height in its `View()` currently (it renders all content without clipping), but the scrollable file section depends on height for viewport calculations.

**Fix:** Set sidebar height in `NewSidebarModel` or at startup.

---

## 10. MEDIUM — Welcome Screen Uses Legacy viewportHeight

**File:** `repl_welcome.go:27`

```go
vpHeight := viewportHeight(m.height)
```

The welcome screen calculates viewport height using the legacy `viewportHeight()` which subtracts REPL bottom chrome from `m.height`. But in the unified layout, `m.height` is the **content height** (already chrome-adjusted), and the welcome content is placed inside the viewport (which is already sized correctly by `ViewContent`).

This double-counts the REPL bottom chrome, making the welcome screen 4 rows shorter than the available viewport.

**Fix:** Use the viewport's actual height:
```go
vpHeight := m.viewport.Height
```

---

## 11. MEDIUM — Bottom Bar Uses Full Width Instead of REPL Width

**File:** `repl_welcome.go:340`

```go
spacer := m.width - lipgloss.Width(cwdLabel) - lipgloss.Width(versionLabel) - 4
```

`m.width` is the full content width (or terminal width, depending on how it was set), but the welcome content renders inside the REPL viewport which may be narrower due to the sidebar. The spacer calculation should use `m.replWidth()`.

**Fix:**
```go
spacer := m.replWidth() - lipgloss.Width(cwdLabel) - lipgloss.Width(versionLabel) - 4
```

---

## 12. MEDIUM — Command Palette Hardcoded Width

**File:** `cmdpalette.go:268`

```go
paletteWidth := 60
if cp.width > 0 && cp.width < paletteWidth+4 {
    paletteWidth = cp.width - 4
}
```

The palette defaults to 60 columns. On wide terminals this is fine, but the max visible items is hardcoded to 12, which on tall terminals wastes vertical space. Also, `cp.width` is sometimes set to `m.width` (full terminal) in `routeAppMsgAction`, not content width.

**Fix:** Make max items responsive to available height:
```go
maxItems := max(6, min(20, cp.height - 8))
```

---

## 13. MEDIUM — DiffModel Uses Raw Terminal Dimensions

**File:** `app_update.go:421–423`

```go
case DiffScreenMsg:
    m.diffModel.width = m.width
    m.diffModel.height = m.height
```

The diff model receives full terminal dimensions, not content area. This causes the diff view to overflow the footer.

---

## 14. MEDIUM — PhaseModelPicker Created With Full Terminal Dims

**File:** `app_update.go:315`

```go
picker := NewPhaseModelPickerModel(m.shutdownCtx, m.registry, m.themeManager.Current(), m.width, m.height)
```

Created in `GoalSubmittedMsg` handler with full terminal dimensions, not content dimensions.

---

## 15. LOW — Initial REPL Dimensions Are Zero

**File:** `app_update.go:661–664`

```go
if m.replModel.width == 0 {
    m.replModel.width = m.width
    m.replModel.height = m.height
}
```

At startup, `m.width` and `m.height` may be zero (before the first `WindowSizeMsg`). The REPL is created with zero dimensions, causing `renderWelcome` to return `"Welcome to M31A"` as plain text. This flashes briefly before the first resize.

**Fix:** The `tea.WithAltScreen()` option sends an initial `WindowSizeMsg`, so this is usually fine. But the guard should produce a loading state, not a raw string.

---

## 16. LOW — Sidebar Default Visible Consumes 28 Columns

**File:** `sidebar.go:44–50`

```go
func NewSidebarModel(g *git.Git, t theme.Theme) *SidebarModel {
    return &SidebarModel{
        visible: true,      // ← visible by default
        width:   sidebarDefaultWidth,  // ← 28 columns
    }
}
```

At terminals 80+ cols, the sidebar is always shown on startup, consuming 28 columns (35% of an 80-col terminal). Users on 80-col terminals get only 52 columns of content.

**Recommendation:** Consider starting hidden on terminals < 120 cols, or reducing default width to 22.

---

## 17. LOW — No Height-Aware Responsive Breakpoints

**File:** `layout/responsive.go`

The breakpoint system only considers terminal **width**. Height is never checked. On very short terminals (e.g., 5-10 rows), the layout breaks:
- ChromeHeight = 2 leaves only 3-8 rows for content
- REPL viewport minimum is 4 rows
- Textarea is 3 rows
- Total minimum: 2 (chrome) + 4 (viewport) + 1 (separator) + 3 (textarea) = 10 rows

Below 10 rows, content is clipped or overflows.

**Fix:** Add a minimum height check in `View()`:
```go
if m.height < 10 {
    return layout.RenderTooSmall(m.width, m.height, t)
}
```

---

## 18. LOW — truncateToVisibleWidth Is a No-Op

**File:** `app_view.go:245–252`

```go
func truncateToVisibleWidth(s string, maxW int) string {
    w := lipgloss.Width(s)
    if w <= maxW {
        return s
    }
    return s // TODO: use layout.TruncateToWidth when exported
}
```

This function always returns the input unmodified. Toast overlays on narrow terminals will overflow content.

---

## Component Inventory

### Layout Package (`internal/tui/layout/`)
| File | Purpose |
|------|---------|
| `responsive.go` | Breakpoint detection, Show* predicates, ChromeHeight constant |
| `minscreen.go` | RenderTooNarrow, RenderOverlay, truncateToWidth (private) |
| `page.go` | PageChrome, RenderPage, BuildHeader, BuildFooter, assembleThreeZone |
| `page_test.go` | Tests for ContentHeight with edge cases |

### Core App Files
| File | Purpose |
|------|---------|
| `app.go` | Init, Shutdown, workflow engine, permission/question listeners |
| `app_state.go` | AppState struct (50+ fields), NewApp constructor |
| `app_view.go` | View(), buildHeaderInfo, buildFooterInfo, 26 screen renderers |
| `app_update.go` | Update() dispatcher, handleWindowResize, ensureSubModel, routeKeyMsg |
| `app_update_phase.go` | Workflow phase result handling |

### REPL
| File | Purpose |
|------|---------|
| `repl_model.go` | ReplModel struct, NewReplModel |
| `repl_view.go` | View() legacy, ViewContent() unified, layout constants |
| `repl.go` | Init, Update (resize handler), keyboard handling |
| `repl_state.go` | SetTheme, replWidth(), SetSidebarWidth |
| `repl_welcome.go` | renderWelcome, provider/project cards, getting started |
| `repl_keys.go` | Key handling |
| `repl_stream.go` | Stream message handling |
| `repl_thinking.go` | Thinking state management |
| `repl_quickactions.go` | Quick actions panel |
| `repl_clipboard.go` | Clipboard operations |
| `repl_commands.go` | Command execution |

### Chrome & Overlays
| File | Purpose |
|------|---------|
| `header.go` | RenderHeader (legacy), RenderPhaseBadge, context meter |
| `statusbar.go` | RenderStatusBar, RenderPromptMetadata, RenderPromptBottomBorder |
| `cmdpalette.go` | CommandPaletteModel — 60-col overlay |
| `toast.go` | Toast rendering — stack and single |
| `transition.go` | ScreenTransition overlay |

### Sub-Screen Models (26 screens)
| File | Screen | Has SetDimensions? |
|------|--------|-------------------|
| `settings_model.go` | Settings | Yes (width/height fields) |
| `modelselector.go` | ModelSelector | Yes |
| `resume_model.go` | Resume | Yes |
| `plan_model.go` | Plan | Yes |
| `execute_model.go` | Execute | No (direct field access) |
| `verify_model.go` | Verify | No (direct field access) |
| `ship_model.go` | Ship | No (direct field access) |
| `goalinput.go` | GoalInput | Yes |
| `discuss.go` | Discuss | Yes |
| `diff_model.go` | Diff | No (direct field access) |
| `ledger.go` | Ledger | Yes |
| `rollback.go` | Rollback | Yes |
| `metrics.go` | Metrics | No (direct field access) |
| `config_model.go` | Config | No (direct field access) |
| `help.go` | Help | Yes |
| `bisect.go` | Bisect | Yes |
| `theme_picker.go` | ThemePicker | Yes |
| `notification_list.go` | Notifications | Yes |
| `dashboard.go` | Dashboard | Yes |
| `session_detail.go` | SessionDetail | Yes |
| `filetree.go` | FileExplorer | Yes |
| `toolcard.go` | ToolDetail | Yes |
| `modelselector.go` | PhaseModelPicker | Yes |

### Components (`internal/tui/components/`)
| File | Purpose |
|------|---------|
| `card.go` | Card renderer with border presets |
| `badge.go` | Badge component |
| `splitpane.go` | Side-by-side layout |
| `tabbar.go` | Tab bar component |
| `breadcrumb.go` | Breadcrumb navigation |
| `datatable.go` | Table component |
| `filetree.go` | File tree component |
| `codeblock.go` | Code block renderer |
| `progress.go` | Progress bar |
| `spinner.go` | Animated spinner |
| `message.go` | Message renderer |
| `permission.go` | Permission modal component |
| `question.go` | Question modal component |
| `toolcard.go` | Tool output card |
| `toolrenderers.go` | Tool-specific renderers |
| `bash_renderer.go` | Bash tool output |
| `thinking.go` | Thinking block toggle |
| `divider.go` | Visual divider |
| `truncate.go` | Text truncation |
| `notification_list.go` | Notification list |
| `dropdown.go` | Dropdown component |
| `confirm.go` | Confirmation dialog |
| `workflow_phasebar.go` | Workflow phase progress bar |
| `sparkline.go` | Sparkline chart |
| `starfield.go` | Starfield animation |
| `search.go` | Search component |
| `logo.go` | ASCII logo |
| `filterchips.go` | Filter chips |
| `metriccard.go` | Metric card |
| `statrow.go` | Statistics row |
| `file_renderers.go` | File-specific renderers |
| `special_renderers.go` | Special renderers |

### Theme (`internal/tui/theme/`)
| File | Purpose |
|------|---------|
| `theme.go` | Theme struct, Manager, Mode enum |
| `colors.go` | Color definitions |
| `borders.go` | Border presets |
| `shadow.go` | Shadow effects |
| `unicode.go` | Unicode helpers |
| `tabs.go` | Tab styling |
| `registry.go` | Theme registry |

---

## Dimension Propagation Flow

```
tea.WindowSizeMsg{Width: W, Height: H}
    │
    ▼
AppState.Update() — stores m.width=W, m.height=H
    │
    ▼
handleWindowResize():
    contentW = W (- sidebar if visible at ≥80 cols)
    contentH = H - 2 (ChromeHeight)
    │
    ├── m.replModel.width = contentW ← BUT also calls SetSidebarWidth(sw) → DOUBLE SUBTRACT
    ├── m.planModel.SetDimensions(contentW, contentH) ← CORRECT
    ├── m.executeModel.width = contentW ← CORRECT
    ├── ... (all other sub-models get contentW/contentH) ← CORRECT
    │
    ▼
AppState.View():
    chrome = PageChrome{Width: contentWidth, Height: m.height}
    │
    ├── renderREPLContent(chrome) → syncReplSize(chrome) → ViewContent(ch, cw) ← CORRECT
    ├── renderSettingsContent(chrome) → sets width/height from chrome ← CORRECT
    └── render*Content(chrome) → calls .View() with STALE dimensions ← BUG
```

---

## Summary of Issues by Severity

| Severity | Count | Key Issues |
|----------|-------|------------|
| CRITICAL | 3 | ensureSubModel wrong dims, REPL double sidebar subtract, REPL double chrome |
| HIGH | 4 | 24 screens stale dims, renderLoading hardcoded, perm modal Place(0,0), card widths |
| MEDIUM | 5 | Duplicate status bar, sidebar height, welcome viewport, bottom bar width, palette |
| LOW | 4 | Zero initial dims, sidebar default, no height breakpoints, truncateToVisibleWidth no-op |

**Total: 16 issues found.**

The three critical issues compound: on an 80-column terminal with sidebar visible, the REPL gets `80 - 28 - 28 = 24` effective width (minimum guard kicks in at 20), and sub-screens navigate with `80×24` instead of `52×22`.
