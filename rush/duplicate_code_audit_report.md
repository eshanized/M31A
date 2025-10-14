# M31A — Duplicate Code Audit Report

> **Date:** 2026-06-07
> **Scope:** All `internal/` and `pkg/` Go source files
> **Method:** Automated pattern analysis + manual verification

---

## Executive Summary

| Severity | Count | Description |
|----------|-------|-------------|
| **CRITICAL** | 1 | Hand-rolled progress bars ignore existing `components.ProgressBar` |
| **HIGH** | 9 | Pervasive structural duplicates (provider setup, session loading, slash autocomplete, etc.) |
| **MEDIUM** | 12 | Duplicate constants, hardcoded literals, rendering patterns |
| **LOW** | 5 | Naming collisions, minor rendering variations |

**Total unique duplicate patterns found: 27**

---

## CRITICAL

### C-1: Hand-Rolled Progress Bars Ignore `components.ProgressBar`

**9+ locations** manually construct `strings.Repeat("█", filled) + strings.Repeat("░", empty)` instead of using the existing `components.ProgressBar` component.

| File | Line |
|------|------|
| `internal/tui/execute_view.go` | 154-155 |
| `internal/tui/discuss.go` | 203-204 |
| `internal/tui/plan_view.go` | 272-273 |
| `internal/tui/metrics.go` | 304-305, 333-334 |
| `internal/tui/firstrun_view.go` | 283 |
| `internal/tui/resume_view.go` | 329-330 |
| `internal/tui/components/special_renderers.go` | 47 |
| `internal/tui/components/permission.go` | 180-181 |

**Fix:** Replace all with `components.ProgressBar(width, filled, brandColor, borderColor)`.

---

## HIGH SEVERITY

### H-1: REPL Provider Setup Triple-Call (12 locations)

The sequence `SetProvider() + SetDispatcher() + SetCommandRegistry()` is repeated verbatim in 12 places.

| File | Line | Context |
|------|------|---------|
| `app_update_screen.go` | 110-112 | First-run completion |
| `app_update_screen.go` | 186-188 | Resume session |
| `app_update_workflow.go` | 445-447 | Workflow→REPL |
| `app_update_workflow.go` | 421 | Model change sync |
| `app_update_workflow.go` | 487-488 | Settings: provider changed |
| `app_update_workflow.go` | 508-509 | Settings: model changed |
| `app_update_slash.go` | 196-198 | Session switch (/fork) |
| `app.go` | 304-305 | Model shortcut |
| `app_state.go` | 386-388 | NewApp: offline |
| `app_state.go` | 404-406 | NewApp: no providers |
| `app_state.go` | 416-418 | NewApp: normal |
| `app_update.go` | 330-331 | Auto-fallback |

**Fix:** Extract `m.syncReplProvider(sessionID string) tea.Cmd`.

---

### H-2: Session Loading Sequence (2 near-identical copies)

Full session loading (load session → create ReplModel → SetProvider/SetDispatcher/SetSessionID → add messages → propagate to workflow/sub-models) is duplicated between:

| File | Line | Context |
|------|------|---------|
| `app_update_screen.go` | 179-213 | ScreenResume handler |
| `app_update_slash.go` | 191-222 | /fork, /prev, /next handler |

**Differences:** Location B adds `ClearMessages()` and `SetCommandRegistry`.

**Fix:** Extract `m.loadAndRestoreSession(sessionID string, clearExisting bool) tea.Cmd`.

---

### H-3: Slash Command Autocomplete (2 identical 43-line blocks)

The entire slash suggestion algorithm (get all commands → filter by partial → cap at 8 → set slashVisible) is duplicated:

| File | Line | Context |
|------|------|---------|
| `repl.go` | 157-199 | Runs on every `Update()` call |
| `repl_keys.go` | 121-158 | Runs on every key press |

Both run on every keystroke — the second overwrites the first with the same result.

**Fix:** Extract `m.updateSlashSuggestions()` and call it from one place only.

---

### H-4: Session ID Propagation to Sub-Models (2 identical blocks)

The 4-line block propagating sessionID to planModel/executeModel/verifyModel/shipModel is duplicated:

| File | Line |
|------|------|
| `app_update_screen.go` | 196-207 |
| `app_update_slash.go` | 211-222 |

**Fix:** Extract `m.propagateSessionID(id string)`.

---

### H-5: NewSidebarModel Lazy Init (2 identical blocks)

```go
if m.sidebarModel == nil {
    m.sidebarModel = NewSidebarModel(m.git, m.themeManager.Current())
}
```

| File | Line |
|------|------|
| `app_update_screen.go` | 115-117 |
| `app_update_workflow.go` | 433-435 |

**Fix:** Extract `m.ensureSidebarModel()`.

---

### H-6: NewReplModel Nil Check + Assignment (5+ locations, 3 in one function)

```go
if m.replModel == nil {
    rp := NewReplModel(m.themeManager.Current(), m.version)
    m.replModel = &rp
}
```

| File | Line |
|------|------|
| `app_update_screen.go` | 86-88, 182-184 |
| `app_update_slash.go` | 192-194 |
| `app_update_workflow.go` | 429-431 |
| `app_state.go` | 383-385, 401-403, 413-415 |

The 3 copies in `app_state.go` are in the same function.

**Fix:** Extract `m.ensureReplModel()`.

---

### H-7: DoubleBorder Literal (4 identical copies)

```go
doubleBorder := lipgloss.Border{
    Top: "═", Bottom: "═", Left: "║", Right: "║",
    TopLeft: "╔", TopRight: "╗", BottomLeft: "╚", BottomRight: "╝",
}
```

| File | Line |
|------|------|
| `execute_view.go` | 164-173 |
| `plan_view.go` | 115-124 |
| `verify.go` | 347-356 |
| `components/toolcard.go` | 235-244 |

**Fix:** Define as `theme.DoubleBorder` or package-level `var`.

---

### H-8: Permission + Question Listener Commands (20+ locations)

```go
cmds := []tea.Cmd{permissionListenerCmd(m.shutdownCtx, m.dispatcher), questionListenerCmd(m.shutdownCtx, m.dispatcher)}
```

| File | Count |
|------|-------|
| `app_update_screen.go` | 16 |
| `app_update_permission.go` | 4 |
| `app.go` | 1 |

**Fix:** Extract `m.listenerCmds() []tea.Cmd`.

---

### H-9: `lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, ...)` (30 occurrences)

Every modal/overlay/fullscreen view uses the identical centering call.

**Fix:** Extract `CenterScreen(content string, w, h int) string`.

---

## MEDIUM SEVERITY

### M-1: Duplicate Constants (`types/constants.go` vs `tools/constants.go`)

7 constant pairs duplicated across packages (acknowledged in code comments as "to avoid import cycle"):

| `types/constants.go` | `tools/constants.go` | Value |
|---|---|---|
| `DirPermission` | `DirPermission` | `0755` |
| `FilePermission` | `FilePermission` | `0644` |
| `DefaultMaxGlobResults` | `MaxGlobResults` | `1000` |
| `DefaultMaxGrepResults` | `DefaultMaxGrepResults` | `100` |
| `DefaultBashKillGraceSecs` | `BashKillGracePeriod` | `5` / `5s` |
| `DefaultMaxBackupsPerFile` | `MaxBackupsPerFile` | `10` |
| `DefaultWebfetchMaxRedirects` | `MaxRedirects` | `5` |

**Fix:** Move shared constants to a leaf package or accept the import cycle workaround.

---

### M-2: Hardcoded Literals That Should Reference Constants

| Hardcoded Value | File:Line | Should Reference |
|---|---|---|
| `300` | `tui/discuss.go:48,79,107` | New `DiscussDefaultTimeoutSecs` constant |
| `300` | `tui/app_workflow.go:163` | Config value |
| `300` | `tools/question.go:106` | `types.DefaultPermissionTimeout` |
| `0.80` | `tui/header.go:68` | `types.ContextWarningThreshold` |
| `0.3` | `config/loader.go:46` | `types.EMACorrectionAlpha` |
| `120 * time.Second` | `tui/app_update.go:229` | `types.MaxRetryAfterWait` |
| `"https://openrouter.ai/api/v1"` | `tui/firstrun_model.go:288` | `types.DefaultOpenRouterBaseURL` |
| `"https://opencode.ai/zen/v1"` | `tui/firstrun_model.go:294` | `types.DefaultZenBaseURL` |
| `"https://github.com/eshanized/M31A"` | `tui/firstrun_model.go:310` | `types.DefaultReferer` |
| `"M31A/dev"` | `tui/firstrun_model.go:306` | New constant |
| `"M31A"` | `firstrun_model.go:314`, `openrouter/client.go:63` | New `DefaultXTitle` |
| `"2006-01-02"` | `resume_view.go:151`, `metrics.go:82`, `log/log.go:82` | New `DateFormat` constant |
| `5 * time.Minute` (7 test files) | Various test files | `types.ModelCacheTTL` |
| `0755` (28 test files) | Various test files | `types.DirPermission` |
| `0644` (~100 test locations) | Various test files | `types.FilePermission` |

---

### M-3: Error Message Boilerplate (9 instances)

Every error message duplicates `Content` into both `Message.Content` and `MessageSegment.Content`:

```go
types.Message{
    Role:    "assistant",
    Content: "...",
    Segments: []types.MessageSegment{{
        Type: "content", Content: "...", Visible: true,
    }},
    CreatedAt: time.Now(),
}
```

| File | Line |
|------|------|
| `app_update_slash.go` | 39-48, 104-113, 298-307 |
| `repl_keys.go` | 206-215, 256-265, 315-324 |
| `repl_stream.go` | 223-232 |
| `app_update_workflow.go` | 71-78 |
| `repl_commands.go` | 25-35 |

**Fix:** Extract `makeAssistantMsg(content string) types.Message`.

---

### M-4: `replWidth` Calculation (3 identical blocks)

```go
replWidth := m.width - m.sidebarWidth
if replWidth < 20 { replWidth = 20 }
```

| File | Line |
|------|------|
| `repl_state.go` | 33-36 |
| `repl.go` | 94-97 |
| `repl_view.go` | 38-41 |

**Fix:** Extract to `ReplModel.replWidth()` method.

---

### M-5: `contentWidth` Calculation in `components/message.go` (3 identical blocks)

```go
contentWidth := width - GutterWidth
if contentWidth < 20 { contentWidth = 20 }
```

| Line |
|------|
| 97-100 |
| 138-141 |
| 218-221 |

**Fix:** Extract to `calcContentWidth(width int) int`.

---

### M-6: Logo Rendering (2 functions, same ASCII art)

| File | Function | Difference |
|------|----------|------------|
| `repl_welcome.go:116` | `renderLogo()` | No bold |
| `firstrun_view.go:77` | `renderWelcomeLogo()` | `.Bold(true)` |

**Fix:** Extract shared logo to `components` package with optional bold param.

---

### M-7: `renderSearchBar` (2 near-identical functions)

| File | Line | Label |
|------|------|-------|
| `resume_view.go` | 338-352 | `"> "` |
| `ledger.go` | 325-339 | `"🔍 "` |

**Fix:** Extract to shared helper with configurable label.

---

### M-8: Theme Computed Styles Duplicated in Dark() and Light()

`theme/colors.go` — ~50 lines of identical computed style definitions in both `Dark()` (lines 41-89) and `Light()` (lines 128-177). Since these use `lipgloss.Color(t.Brand)` etc., they're structurally identical.

**Fix:** Extract `applyThemeStyles(t *Theme)` called after base colors are set.

---

### M-9: Skip Dirs List (2 different data structures, same values)

| File | Type | Values |
|------|------|--------|
| `config/loader.go:67` | `[]string` | node_modules, vendor, .next, dist, build, target, .venv, venv, __pycache__ |
| `workflow/engine_verify.go:32-36` | `map[string]bool` | Same 9 dirs |

**Fix:** Define once, derive the other.

---

### M-10: Git Config Defaults (2 identical struct literals)

| File | Line |
|------|------|
| `config/loader.go` | 70-75 |
| `workflow/engine.go` | 91-95 |

Both define: `CommitPrefix: "feat", FixPrefix: "fix", ShipPrefix: "chore", UserName: "M31A", UserEmail: "m31a@local"`.

**Fix:** Reference `config.DefaultGitConfig()`.

---

### M-11: `formatDuration` (2 functions, different output formats)

| File | Line | Format |
|------|------|--------|
| `resume_view.go` | 444-455 | `"2h 30m"` |
| `components/permission.go` | 242-247 | `"150:00"` |

Plus `formatDurationMs` in `app.go:244`. Three duration formatters with confusingly similar names.

**Fix:** Rename to `formatDurationHuman()`, `formatDurationClock()`, `formatDurationMs()`.

---

### M-12: Section Header Bar Rendering (`"── Title "` + dash-fill, 6+ locations)

The pattern `fmt.Sprintf("── %s ", title) + strings.Repeat("─", remaining)` appears in:

| File | Line |
|------|------|
| `ship_view.go` | 94-103, 137-146, 196-205 |
| `plan_view.go` | 258-259 |
| `metrics.go` | 188-189 |
| `execute_view.go` | 25 |

**Fix:** Extract `RenderSectionHeader(title string, width int) string`.

---

## LOW SEVERITY

### L-1: `renderProviderCard` Naming Collision

Two functions named `renderProviderCard` exist in different models:
- `ReplModel.renderProviderCard()` — shows current provider status
- `FirstRunModel.renderProviderCard(p, isActive)` — shows provider selection cards

Different signatures, different semantics. Not a code duplicate but a naming collision risk.

---

### L-2: Footer/Hint Rendering Patterns (4 functions)

| File | Function |
|------|----------|
| `repl_welcome.go` | `renderKeyboardHints()` |
| `firstrun_view.go` | `renderLaunchpadFooter()` |
| `repl_welcome.go` | `renderBottomBar()` |
| `settings_view.go` | `renderFooter()` |

All use `Foreground(theme.TextMuted/TextSecondary)` with bold brand keys. Not identical but same pattern.

---

### L-3: Workflow Phase Routing Guards (3 identical cases)

```go
case ScreenXXX:
    if m.workflowEngine == nil { return m, nil }
    m.setWorkflowPhase(types.PhaseXXX)
    return m, RunPhaseCmd(m, types.PhaseXXX, m.workflowGoal)
```

| File | Line | Phase |
|------|------|-------|
| `app_update_workflow.go` | 387-393 | Execute |
| `app_update_workflow.go` | 394-400 | Verify |
| `app_update_workflow.go` | 401-407 | Ship |

**Fix:** Use mapping table or combined case.

---

### L-4: Diff Line-Style Switch (2 identical blocks in `diff_view.go`)

The switch statement mapping `DiffAdded/Deleted/Hunk/Header/Context` to styles is duplicated between `renderSplitView` (lines 64-83) and `renderUnifiedView` (lines 160-190).

**Fix:** Extract `diffLineStyle(lineType, theme) (lipgloss.Style, string)`.

---

### L-5: Offline Mode Error String (2 identical locations in `app_state.go`)

```go
"No providers available — offline mode. History is readable but no new messages."
```

Used 4 times (2 for `healthStatus.Error`, 2 for `currentOperation`) across 2 code paths.

**Fix:** Extract as `const offlineModeMsg`.

---

## Recommendations Summary

### Immediate (1-2 hours)
1. Extract `makeAssistantMsg()` helper — eliminates 9 instances of boilerplate
2. Extract `m.syncReplProvider()` — eliminates 12 instances of triple-call
3. Extract `m.listenerCmds()` — eliminates 20+ instances of boilerplate
4. Remove duplicate slash autocomplete in `repl_keys.go:121-158` (already covered by `repl.go:157-199`)
5. Replace hand-rolled progress bars with `components.ProgressBar`

### Short-term (half day)
6. Extract `m.loadAndRestoreSession()` — eliminates 2 near-identical session loading blocks
7. Extract `m.propagateSessionID()` — eliminates 2 identical 4-line blocks
8. Extract `m.ensureReplModel()` and `m.ensureSidebarModel()` — eliminates 7+ nil-check blocks
9. Define `doubleBorder` as shared constant — eliminates 4 identical struct literals
10. Replace hardcoded `300`, `0.80`, `0.3`, URLs with constant references

### Medium-term (1 day)
11. Extract `CenterScreen()` helper — eliminates 30 identical centering calls
12. Extract `replWidth()` method — eliminates 3 identical calculation blocks
13. Extract `applyThemeStyles()` — eliminates ~50 duplicated lines in Dark()/Light()
14. Consolidate `formatDuration` variants with distinct names
15. Extract `RenderSectionHeader()` — eliminates 6+ header-bar patterns

### Architectural
16. Move shared constants from `tools/constants.go` to a leaf package or accept the duplication
17. Create a base `dimensions` struct to reduce WindowSizeMsg boilerplate across 12+ models
18. Consider an embedded `spinnerable` struct to reduce spinner tick boilerplate across 9 models
