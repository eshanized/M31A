# M31A TUI Comprehensive Audit Report

**Auditor:** Senior Go Engineer / Terminal UI Expert / Bubble Tea Architect
**Scope:** 39,133 lines of Go across 90+ source files in `internal/tui/`
**Status:** Read-only audit — no fixes applied

---

## 1. Executive Summary

M31A's TUI is a large, feature-rich Bubble Tea application with 33 screens, 11 themes, 45+ components, and a leader-key driven keyboard system. The architecture follows Elm principles (Model → Update → View) and the codebase demonstrates disciplined engineering with no TODO/FIXME markers and consistent patterns.

However, the system has accumulated significant complexity hotspots that would block a stable v1.0 release. The most critical issues are: a 3,500-line `Update()` method with 9 duplicated screen-routing switch blocks (~1,800 lines of duplication), a rendering pipeline that mutates state inside `View()`, several correctness bugs (UTF-8 misalignment, bisect convergence, search not scrolling to matches), and incomplete implementations (theme picker, quick actions, search highlighting).

**Production Readiness Score: 5.5/10**

---

## 2. Critical Issues

| # | Title | File:Line | Description | Confidence |
|---|-------|-----------|-------------|------------|
| C1 | **`Update()` is 3,544 lines with 50+ message types** | `app_update.go:27-1410` | Single dispatch point with 9 duplicated 30-screen switch blocks (~1,800 lines of duplicated routing). Adding a screen requires editing 9 places. | High |
| C2 | **`View()` mutates `m.screen`** | `app_view.go:35-38` | Transition rendering temporarily changes `m.screen` via `defer`. Violates Bubble Tea's Elm architecture contract (View must be pure). | High |
| C3 | **`sync.RWMutex` declared but effectively unused** | `app_state.go:47` | Mutex only acquired in `SetCwd()`. All 220+ field mutations are unprotected. Creates false thread-safety guarantee. | High |
| C4 | **Clipboard UTF-8 misalignment bug** | `repl_clipboard.go:47,51` | `content[2:]` does not correctly strip "✗ " (4 bytes, not 2). Produces corrupted text on clipboard copy. | High |
| C5 | **Flex rounding errors lose cells** | `layout/solver.go:80,170` | Integer division truncation means total child widths < container width. No remainder redistribution. | High |
| C6 | **Agent loop goroutine leak** | `streaming/agent_loop.go:317-331` | `defer close(progressDone)` only fires when outer goroutine exits. Ticker goroutines for earlier tools leak until agent loop completes. | High |

---

## 3. High Priority Issues

| # | Title | File:Line | Description | Confidence |
|---|-------|-----------|-------------|------------|
| H1 | **Channel emitter silently drops messages** | `app_channel.go:22-27` | When buffer is full, TaskStartMsg/ToolStartMsg are silently dropped. User sees no progress indicators. | High |
| H2 | **Nine duplicated screen-routing switch blocks** | `app_update.go`, `app_view.go` | Mouse forwarding, forwardMsg, routeKey, routeToScreen, ensureSubModel, handleWindowResize, applyTheme, renderActiveScreen, screenName — all 30+ case switches. | High |
| H3 | **`MaxScreenStack` constant (20) vs `screenCap` field (16)** | `constants.go:36`, `app_state.go:312` | Constant defined but never used. Field uses hardcoded 16. | High |
| H4 | **DashboardModel missing 3 phase→screen mappings** | `dashboard_model.go:98-109` | Initialize, Discuss, and Runtime phases have no navigation on Enter — user gets no feedback. | High |
| H5 | **GhostOutputModel scroll field is dead** | `ghostoutput_model.go:19,37,99` | `scroll` is set but never modified. Cursor can go off-screen when file list exceeds viewport. | High |
| H6 | **BisectModel binary search convergence issue** | `bisect_model.go:93-112` | Can get stuck on already-classified commits; no guard for `low+1==high` being the bug commit. | High |
| H7 | **`handleTokens` uses byte/4 heuristic** | `commands/commands_config.go:246-250` | Uses `len()` (bytes) not rune count. Codebase has proper tokenizer in `internal/tokens/`. | High |
| H8 | **`View()` and `ViewContent()` are 60% duplicated** | `repl_view.go:95-274` | Both build identical pipelines. Visual fixes must be applied in two places. | High |
| H9 | **Search doesn't scroll to matches or highlight them** | `repl_search.go:44-48` | Enter navigates to next match index but viewport doesn't scroll. No match highlighting. | High |
| H10 | **`ctrl+x s` shortcut conflict** | `keybindings_screens.go:18`, `cmdpalette.go:77` | "Open settings" (binding) vs "config" (palette shortcut) use same chord. | High |
| H11 | **`ctrl+x m` ambiguity** | `keybindings_screens.go:37`, `cmdpalette.go:78` | "Select model" in REPL vs "metrics" in palette. | High |
| H12 | **MetricsModel loads all sessions synchronously** | `metrics_model.go:58-111` | Single tea.Cmd loads every session from disk. No progress indication. | High |
| H13 | **`renderGradientSeparator` allocates style per character** | `repl_welcome.go:119,128` | 100+ style allocations per welcome render. | Medium |

---

## 4. Medium Priority Issues

| # | Title | File:Line | Description | Confidence |
|---|-------|-----------|-------------|------------|
| M1 | **Inconsistent sub-model Update() return types** | `app_update.go:59-312` | Some return `tea.Model` (need type assertion), others return concrete types. Silent failures if assertion fails. | High |
| M2 | **Phase handlers bypass screen transition** | `app_update_phase.go:64,119,161,181,245` | Direct `m.screen = ScreenXxx` instead of `navigateToScreen()`. No back-stack push, no animation. | High |
| M3 | **`component.Component` interface is dead code** | `components/base.go` | `Component`, `StatelessComponent`, `Context`, `RenderFunc` defined but zero components implement them. | High |
| M4 | **Inconsistent render method naming** | `components/*.go` | ~60% use `Render()`, ~30% use `View()`, ~10% are standalone functions. | High |
| M5 | **Three parallel badge systems** | `components/badge.go` | `Badge`, `SimpleBadge`, `EnhancedBadge` coexist with different APIs. | Medium |
| M6 | **`getFieldValue`/`setFieldValue` duplication** | `settings_model.go:371-489`, `config_model.go:240-615` | Adding a field requires updating 3 places. 374+ lines of switch statements. | High |
| M7 | **Theme picker only applies dark/light** | `themepicker_model.go:80-86` | 10 presets displayed but only 2 theme values can be applied. Per-palette theming not implemented. | Medium |
| M8 | **SidebarModel is a god object** | `sidebar_model.go:1453 lines` | 30+ state fields, 545-line View(). Combines git, tokens, phases, tools, costs, files, todos. | High |
| M9 | **`computeWaves()` duplicates Kahn's algorithm** | `plan_model.go:80-159` | Already exists in `pkg/taskrunner/runner.go`. | Medium |
| M10 | **DiscussModel timer doesn't tick** | `discuss_model.go:142-245` | Countdown in View() calculates from deadline but no periodic tick triggers re-render. Display doesn't update in real-time. | Medium |
| M11 | **No dirty-changes guard on ConfigModel exit** | `config_model.go` | `q` immediately pops screen without prompting to save unsaved changes. | Medium |
| M12 | **Quick actions overlay has no keyboard activation** | `repl_quickactions.go` | Display-only; no `handleQuickActionsKey` handler. No mouse hit-test in `handleLeftClick`. | Medium |
| M13 | **`mention.go` GetLineCount reads entire file** | `mention.go:100-111` | Reads entire file into memory just to count newlines. | Medium |
| M14 | **`a11y/announce.go` uses iTerm2-specific escapes** | `a11y/announce.go` | `\x1b]1337;` sequences only work in iTerm2. Silently fails on Linux/Windows terminals. | Medium |
| M15 | **`TabWidth` is 0 in 8 of 11 themes** | `theme/*.go` | Only Dark/Light/HighContrast set it. Community themes default to 0. | Medium |
| M16 | **Nord theme has reversed text contrast** | `theme/nord.go` | TextSecondary is brighter than TextPrimary. | Medium |
| M17 | **Monochrome theme has poor semantic differentiation** | `theme/monochrome.go` | Error/Success/Warning/Thinking differ by only 10-16 brightness levels. | Medium |
| M18 | **Dracula theme Border == SurfaceElevated** | `theme/dracula.go` | Both `#6272a4`. Elevated surfaces blend with borders. | Medium |
| M19 | **`renderSectionHeader()` duplicated** | `helpers.go:219`, `components/logo.go:121` | Functionally identical in two packages. | Medium |
| M20 | **MessageRenderer.toolCallCache grows unbounded** | `components/message.go` | No eviction policy. Accumulates in long conversations. | Medium |

---

## 5. Low Priority Issues

| # | Title | File:Line | Description | Confidence |
|---|-------|-----------|-------------|------------|
| L1 | **Ignored errors in production paths** | `app_update.go:1123,2374`, `app.go:388` | Session save, provider switch, tool registration failures silently ignored. | High |
| L2 | **Variable shadowing of `msg`** | `app_update.go:663` | `var msg string` shadows outer type-switched variable in 1,400-line function. | Medium |
| L3 | **Magic numbers throughout** | Multiple files | `permModalWidth: 60`, `screenCap: 16`, `maxVisibleToasts+2`, `modalW: width*4/5`, etc. | Medium |
| L4 | **`Init()` has ~30 lines of duplicated code** | `app.go:63-121` | Both `hasProvider` and `!hasProvider` branches set up identical listeners. | Medium |
| L5 | **`screenName()` should be `Screen.Name()`** | `app_view.go:891-946` | `Screen.Label()` already exists. `screenName()` is a 25-case switch duplicate. | Medium |
| L6 | **`joinStrings()` reimplements `strings.Join()`** | `runtime_model.go:169-178` | Standard library function exists. | Low |
| L7 | **Superscript digit conversion duplicated** | `components/message.go:496-521,553-577` | Identical switch statement in two methods. | Low |
| L8 | **`clampScroll` pattern duplicated across 7+ models** | Multiple files | Each list model implements its own with different chrome height constants. | Low |
| L9 | **`overlayStartY()` re-renders overlays just to count lines** | `repl_mouse.go:156,163,210` | Full overlay rendered on every mouse event just for height calculation. | Medium |
| L10 | **`padFrameLines()` allocates per call** | `transition.go:238-253` | New `[]string` of height entries on every 60fps frame. | Low |
| L11 | **`renderStreamingContent` allocates style per tick** | `repl_state.go:505-508` | New lipgloss style every 100ms during streaming. | Low |
| L12 | **`autoScrollConditionally` does `strings.Count` on full content** | `repl_state.go:341` | O(n) in content size on every message add. | Low |
| L13 | **`renderMessages()` is 103 lines mixing concerns** | `repl_state.go` | Separator rendering, line offset tracking, streaming append, cache management. | Medium |
| L14 | **12 `KeyContext` constants defined but have no bindings** | `keybindings.go:24-36` | `CtxPlan`, `CtxExecute`, `CtxVerify`, etc. have no registered bindings. | Low |
| L15 | **No global scroll keybindings** | `keybindings.go` | No PageUp/PageDown, ctrl+u/ctrl+d registered globally. | Low |
| L16 | **`renderErrorBanner`/`plainErrorBanner` 90% duplicated** | `repl_stream.go:198-252` | Only difference is lipgloss styling. | Low |
| L17 | **`compositeOverlays`/`compositeOverlaysTop` near-mirrors** | `repl_view.go:52-319` | Only loop offset differs. | Low |
| L18 | **`settings_edit.go` is a stub** | `settings_edit.go` | 3-line comment file. Dead code. | Low |
| L19 | **`completed` field written but never read** | `runtime_model.go` | Set in `SetSummary()`, never checked in Update/View. | Low |
| L20 | **PlanModel `timeEstimate`/`costEstimate` fields dead** | `plan_model.go` | Declared, never populated or displayed. | Low |
| L21 | **Multiple `sessionID` fields are dead state** | `execute_model.go`, `verify_model.go`, `ship_model.go` | Set in constructor, never used in methods. | Low |
| L22 | **`CompactMode` field in Theme is dead** | `theme/theme.go` | Defined but never read by any component. | Low |
| L23 | **`formatDurationMs` + "s" produces "1m 23ss"** | `repl_thinking.go:24,29` | Duration suffix appended unconditionally. | Medium |
| L24 | **Search backspace can split multi-byte runes** | `repl_search.go:59-63` | Byte-level truncation on UTF-8 query. | Low |
| L25 | **`CenterText()` does not truncate** | `layout/page.go:477` | Text wider than container overflows. | Low |
| L26 | **`fillW` in header clamped to min 1** | `layout/page.go:141-144` | Can cause overflow when zones exceed width. | Low |
| L27 | **Children with Width=0 AND Flex=0 silently skipped** | `layout/solver.go:89,183` | No diagnostic. | Low |
| L28 | **`handleReset` misleading — only deletes current session** | `commands/commands_core.go:73-104` | Prompt says "all data" but only current session deleted. | Medium |
| L29 | **`detectProjectLanguage` nondeterministic for ties** | `mention.go:206-256` | Go map iteration order is random. | Low |
| L30 | **macOS `du` fallback may report wrong units** | `commands/commands_config_diskusage_unix.go:19-42` | `du -s` returns 512-byte blocks, code returns raw value. | Medium |

---

## 6. UX Problems

| # | Title | Description | Severity |
|---|-------|-------------|----------|
| U1 | **Theme picker is misleading** | Shows 10 palettes with color swatches but only applies dark/light toggle. Users expect per-palette theming. | High |
| U2 | **Search doesn't highlight or scroll to matches** | Shows "3 matches" count but user can't see where they are or jump between them. | High |
| U3 | **Quick actions overlay is display-only** | Rendered but no way to activate items via keyboard or mouse. | Medium |
| U4 | **ConfigModel doesn't warn on unsaved changes** | Pressing `q` silently discards all edits. | Medium |
| U5 | **DiscussModel timer doesn't count down in real-time** | Countdown only updates when something triggers a re-render. | Medium |
| U6 | **No loading state for FileExplorerModel** | Tree built once at construction, never refreshed. Stale after file changes. | Medium |
| U7 | **MetricsModel shows "Loading..." with no progress** | Synchronous session loading blocks with no indication of progress. | Medium |
| U8 | **DashboardModel ignores 3 workflow phases** | Enter does nothing during Initialize, Discuss, or Runtime. | High |
| U9 | **Shell commands have hardcoded 30s timeout** | Long-running builds/tests get killed prematurely. No configuration. | Medium |
| U10 | **`/reset` prompt is misleading** | Says "deletes config, API keys, session data, ledger" but only deletes current session. | Medium |
| U11 | **Monochrome theme makes error/success/warning nearly indistinguishable** | Users relying on this theme can't tell status apart. | Medium |
| U12 | **No confirmation for dangerous actions in rollback** | Double-press exists but the error state loses all context (commit list disappears). | Low |

---

## 7. Accessibility Problems

| # | Title | Description | Severity |
|---|-------|-------------|----------|
| A1 | **a11y/announce.go uses iTerm2-only escapes** | `\x1b]1337;` sequences silently fail on 90%+ of terminals (Linux, Windows Terminal, Kitty, tmux). | High |
| A2 | **No screen reader announcements in practice** | The `Announce`, `DescribeElement`, `RegionStart/End` functions are only usable in iTerm2. | High |
| A3 | **Emoji in PermissionModal may be double-width** | "🔒" rendered in modal can be double-width on some terminals, breaking alignment. | Medium |
| A4 | **No focus visibility indicator** | Keyboard navigation has no visible focus ring on interactive elements. | Medium |
| A5 | **`DescribeElement` reverses label:role convention** | Formats as `role:label` instead of expected `label:role`. | Low |
| A6 | **Color-dependent status indicators** | Error/success/warning rely solely on color. No shape/text differentiation in badges. | Medium |

---

## 8. Performance Problems

| # | Title | Description | Severity |
|---|-------|-------------|----------|
| P1 | **10fps full message re-render during streaming** | `renderMessages()` re-renders ALL messages through glamour/markdown every 100ms. Scales linearly with message count. | High |
| P2 | **`overlayStartY()` re-renders overlays for height** | Full slash/mention dropdown rendered on every mouse event just to count lines. | Medium |
| P3 | **Agent loop goroutine leak** | Ticker goroutines per tool iteration leak until outer goroutine exits. Up to 50 goroutines. | High |
| P4 | **`parseTextToolCalls` O(n*m) worst case** | Creates `json.Decoder` at every `{` character in 64KB content. | Medium |
| P5 | **`padFrameLines()` allocates per frame** | New `[]string` slice on every 60fps transition frame. | Low |
| P6 | **`renderStreamingContent` allocates style per tick** | New lipgloss style every 100ms. | Low |
| P7 | **`autoScrollConditionally` does `strings.Count` on full content** | O(n) in viewport content size on every message add. | Low |
| P8 | **`MessageRenderer.toolCallCache` grows unbounded** | No eviction in long conversations. | Medium |
| P9 | **`renderGradientSeparator` allocates style per character** | 100+ allocations on welcome screen. | Low |
| P10 | **No debounce on resize** | Every `WindowSizeMsg` triggers full layout recomputation. | Low |

---

## 9. Code Smells

| # | Title | Files | Description |
|---|-------|-------|-------------|
| S1 | **Giant Update() method** | `app_update.go` | 3,544 lines, single method dispatching 50+ message types. |
| S2 | **9 duplicated 30-screen switch blocks** | `app_update.go`, `app_view.go` | ~1,800 lines of nearly identical routing code. |
| S3 | **God object SidebarModel** | `sidebar_model.go` | 30+ fields, 1,453 lines, 545-line View(). |
| S4 | **374-line switch statements** | `config_model.go` | `getFieldValue()` (160) + `setFieldValue()` (214) must be kept in sync. |
| S5 | **Two command palette implementations** | `cmdpalette.go`, `commandpalette_model.go` | Duplicate `filterCommands`, `renderHighlightedQuery`, `renderEntry`. |
| S6 | **Dead interfaces** | `components/base.go` | `Component`, `StatelessComponent`, `Context`, `RenderFunc` — zero implementations. |
| S7 | **Three badge APIs** | `components/badge.go` | `Badge`, `SimpleBadge`, `EnhancedBadge` — which to use? |
| S8 | **`renderSectionHeader()` duplicated** | `helpers.go`, `components/logo.go` | Identical in two packages. |
| S9 | **Cache invalidation is manual and error-prone** | `repl_state.go` | 4 cache fields must be cleared together; done correctly but fragile. |
| S10 | **`View()` and `ViewContent()` 60% duplicated** | `repl_view.go` | Same pipeline, two entry points. |

---

## 10. Architecture Problems

| # | Title | Description | Severity |
|---|-------|-------------|----------|
| AR1 | **No ScreenAdapter interface** | The same 30-screen switch pattern is repeated 9 times. A `ScreenAdapter` interface with `Update/View/Init/Resize/SetTheme/Name` would eliminate all duplication. | Critical |
| AR2 | **`View()` violates Elm purity** | Transition rendering mutates `m.screen`. Should pass target screen as parameter. | Critical |
| AR3 | **Mutex is theater** | `sync.RWMutex` declared with doc saying "all goroutines must acquire" but only 1 acquisition in 220+ fields. Either remove or actually use it. | High |
| AR4 | **Channel emitter has no backpressure** | Silently drops messages when full. Should use bounded queue or drop oldest with notification. | High |
| AR5 | **Screen routing is scattered** | `navigateToScreen()`, `routeToScreen()`, `ensureSubModel()`, direct assignment — four different ways to change screens. | Medium |
| AR6 | **Component interface never adopted** | `base.go` defines lifecycle but no component uses it. Dead abstraction. | Medium |
| AR7 | **Config merge uses reflection** | `internal/config/loader.go` uses `reflect.Value`/`reflect.Type` for merge. Fragile, hard to debug. | Medium |

---

## 11. Missing Features

| # | Title | Severity |
|---|-------|----------|
| F1 | **Search match highlighting** — user sees count but not locations | High |
| F2 | **Search match navigation with viewport scroll** — Enter should scroll to match | High |
| F3 | **Quick actions keyboard activation** — overlay exists but can't be used | Medium |
| F4 | **ConfigModel unsaved changes prompt** — warn before discarding | Medium |
| F5 | **DiscussModel real-time countdown timer** — needs periodic tick | Medium |
| F6 | **FileExplorerModel refresh** — rebuild tree on file changes | Medium |
| F7 | **MetricsModel incremental loading** — show progress during session load | Medium |
| F8 | **Per-palette theme application** — theme picker should apply full palettes | Medium |
| F9 | **Shell command timeout configuration** — allow user to set via config | Medium |
| F10 | **Global scroll keybindings** — PageUp/PageDown, ctrl+u/ctrl+d | Low |

---

## 12. Missing Tests

| Area | What's Missing | Priority |
|------|---------------|----------|
| **All 33 screen models** | Zero `_test.go` files for any screen model (Discuss, Plan, Execute, Verify, Runtime, Ship, Home, Settings, Config, Sidebar, Help, FirstRun, etc.) | Critical |
| **Update() dispatch** | No tests for the 50+ message type handlers in `app_update.go` | Critical |
| **View() rendering** | No snapshot/rendering tests for any screen | High |
| **Keyboard navigation** | No tests for key routing, leader key, search, history | High |
| **Mouse handling** | No tests for `repl_mouse.go` hit-testing, scrollbar drag | High |
| **Streaming** | No tests for stream message processing, segment splitting | High |
| **Search** | No tests for match counting, query handling | High |
| **Clipboard** | No tests for UTF-8 prefix stripping (would catch C4) | High |
| **Layout engine** | Only `layout/*_test.go` exist; no rendering output tests | Medium |
| **Theme system** | No contrast tests, no color profile detection tests | Medium |
| **Race conditions** | No `-race` tests for concurrent message routing | Medium |
| **Resize behavior** | No tests for narrow/ultrawide terminal layouts | Medium |

---

## 13. Suggested Refactors

| Priority | Refactor | Impact | Effort |
|----------|----------|--------|--------|
| **1** | **Extract `ScreenAdapter` interface** — one per screen, eliminates 9 switch blocks | Eliminates ~1,800 lines of duplication | High |
| **2** | **Decompose `Update()`** — per-concern handler functions or message handler registry | Reduces complexity from 3,544 to manageable chunks | High |
| **3** | **Fix `View()` purity** — pass target screen as parameter, don't mutate state | Elm architecture compliance | Low |
| **4** | **Extract `clampScroll`/`visibleRows`** — shared list helper used by 7+ models | Eliminates duplication, consistent behavior | Low |
| **5** | **Table-driven config** — replace `getFieldValue`/`setFieldValue` switches with struct tags or map | Eliminates 374-line DRY violation | Medium |
| **6** | **Merge command palette implementations** — single model with overlay/full-screen modes | Eliminates duplicated filtering, highlighting | Medium |
| **7** | **Decompose SidebarModel** — extract git, tokens, phases, tools into sub-models | Reduces god object | High |
| **8** | **Unify `View()`/`ViewContent()`** — single pipeline with mode parameter | Eliminates 60% duplication | Low |
| **9** | **Remove dead interfaces** — `Component`, `StatelessComponent`, `Context`, `RenderFunc` in `base.go` | Reduces confusion | Low |
| **10** | **Use `Screen.Label()` or add `Screen.Name()`** — eliminate `screenName()` switch | 25-case switch → method | Low |

---

## 14. Production Readiness Score: 5.5/10

**Strengths:**
- Well-organized file structure with clear separation of concerns
- Consistent patterns across models (constructor, SetTheme, SetDimensions, Update, View)
- Zero TODO/FIXME markers — disciplined maintenance
- 11 well-crafted themes with comprehensive style definitions
- Robust streaming pipeline with goroutine-per-stream and cancellation
- Good defensive defaults (zero-value theme fallbacks, width clamping)
- Comprehensive keyboard system with leader key and which-key overlay

**Weaknesses blocking v1.0:**
- `Update()` at 3,544 lines is unmaintainable
- 9 duplicated switch blocks (~1,800 lines) make adding screens error-prone
- `View()` violates Elm purity
- Zero unit tests for any screen model
- Several correctness bugs (clipboard UTF-8, bisect convergence, flex rounding)
- Theme picker is misleading (shows 10 palettes, applies 2)
- Search is incomplete (no highlighting, no scroll-to-match)
- a11y is iTerm2-only
- `sync.RWMutex` is theater
