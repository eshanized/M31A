# M31A Broken TUI Screens & Nonfunctional Things Report

**Date:** 2026-06-13
**Scope:** Full TUI codebase — `internal/tui/`, components, theme, layout
**Method:** Deep static analysis, test coverage audit, code flow tracing

---

## Executive Summary

This report catalogs all broken, nonfunctional, or partially-implemented TUI screens and features in M31A. The TUI has **26 screens** defined in `internal/tui/types.go`. While all 26 have model + view implementations, several have critical functional gaps:

| Severity | Count | Description |
|----------|-------|-------------|
| **CRITICAL** | 5 | Features that are completely broken or unreachable |
| **HIGH** | 7 | Features that partially work but have missing handlers or wrong behavior |
| **MEDIUM** | 8 | Dead code, phantom shortcuts, or degraded functionality |
| **LOW** | 6 | Cosmetic issues, missing convenience features |

---

## 1. CRITICAL — Completely Broken or Unreachable Features

### 1.1 Bisect Screen — Empty Shell With No Data Path
- **Screen:** `ScreenBisect` (Screen=18)
- **Files:** `bisect_model.go`, `commands_git.go:107-117`
- **Issue:** The `BisectModel` is created in `ensureSubModel()` but `SetCommits()` is **never called** from any production code path. The `/bisect` command handler only returns static help text — it never navigates to the bisect screen or initializes the model. The screen is only reachable via `ctrl+x d` but always renders: `"No bisect range set. Use /bisect start <good> <bad> to begin."` — which is itself a dead instruction since no `/bisect start` command exists.
- **Impact:** Interactive git bisect is completely nonfunctional.

### 1.2 Session Detail Screen — Always Shows "No session selected"
- **Screen:** `ScreenSessionDetail` (Screen=22)
- **File:** `sessiondetail_model.go:68-74`
- **Issue:** `SetSession()` is **never called** from any production code path. The model always renders static text: `"No session selected."` There is no way for a user to reach this screen with actual data.
- **Impact:** Session detail preview is completely nonfunctional.

### 1.3 Notification Center — Never Receives Notifications
- **Screen:** `ScreenNotifications` (Screen=20)
- **File:** `notification_model.go:35`
- **Issue:** `AddNotification(text, ntype string)` is defined but **never called** in production code — only in test code. The notification screen is navigable but always shows an empty list.
- **Impact:** Notification center is an empty shell; users are never notified of anything.

### 1.4 File Explorer — Minimal Single-Node Tree
- **Screen:** `ScreenFileExplorer` (Screen=23)
- **File:** `fileexplorer_model.go:22, 75-81`
- **Issue:** The model is initialized with a single root node `"."` and `SetRoot()` is **never called** from production code. The view shows only the root directory dot without expanding into the actual project file tree.
- **Impact:** File explorer shows only "." — useless for browsing project files.

### 1.5 OptimizedMsg — Message Type Never Sent
- **File:** `types.go:315`, `app_update.go:554`
- **Issue:** The `OptimizedMsg` type is defined and handled in the Update loop, but it is **never constructed or sent** anywhere in production code. The only reference is in test code. The `handleOptimize` command does manual arbitrage but never emits this message.
- **Impact:** The optimize notification flow is dead — users never receive optimization completion notifications.

---

## 2. HIGH — Partially Broken Features

### 2.1 Bisect Footer Shows Wrong Key Bindings
- **File:** `app_view.go:230` vs `bisect_model.go:72-77`
- **Issue:** The global header footer hints say: `"y good", "n bad", "b skip", "esc back"` — but the actual key handler in `bisect_model.go` uses `g` for good, `b` for bad, `s` for skip. The footer in `app_view.go` is **wrong and contradicts** the model's own correct footer (`[g] Good [b] Bad [s] Skip [r] Reset [esc] Back`).
- **Impact:** Users following the footer hints will press wrong keys.

### 2.2 Dashboard Footer Advertises Enter Key But Handler Missing
- **File:** `dashboard_model.go:80-91, 137`
- **Issue:** The View footer renders: `[enter] Go to current phase   [esc] Back` — but the `Update()` method only handles `esc` and `q`. **No `enter` key handler exists.** Pressing Enter on the dashboard does nothing.
- **Impact:** Prominent UI affordance is a dead button.

### 2.3 Dashboard Missing j/k Scroll Despite Footer Advertisement
- **File:** `dashboard_model.go:80-91`, `app_view.go:236`
- **Issue:** The footer says `"j/k scroll"` but `DashboardModel.Update()` has no j/k key handling. The dashboard is a static view with no scrollable content, making this hint misleading.
- **Impact:** Users expect scrolling but get none.

### 2.4 `/bisect` Command Never Starts Interactive Bisect
- **File:** `commands_git.go:107-117`
- **Issue:** The `/bisect` command handler returns a static help string explaining how to use git bisect manually. It does **not** navigate to `ScreenBisect` or initialize the bisect model. The interactive bisect screen is unreachable from this command.
- **Impact:** The slash command and the screen are disconnected.

### 2.5 Discuss Screen — Timeout Never Activated
- **File:** `discuss.go:60`, `types.go:198`
- **Issue:** The `DiscussAnswerTimeoutMsg` message type is defined and handled, but `DiscussModel.SetTimeout()` is **never called** from production code. The discuss model is created at `app_update.go:1548` without setting a timeout. The timeout mechanism is dead code.
- **Impact:** Discuss questions have no timeout — users can stall indefinitely.

### 2.6 FirstRun Wizard — Cannot Be Escaped
- **File:** `firstrun_model.go`
- **Issue:** The first-run wizard does **not handle the `esc` key** to go back. If a user navigates to the first-run screen via `/reset`, there is no way to escape back to the REPL without completing the wizard or pressing `ctrl+c` twice.
- **Impact:** Users trapped in first-run wizard after `/reset` with no API key configured.

### 2.7 RouteToScreen() Only Handles 2 of 26 Screens
- **File:** `app_update.go:914-934`
- **Issue:** `routeToScreen()` only handles `ScreenFirstRun` and `ScreenREPL`. All other 24 screens return `nil` as their init command. This means non-REPL/non-FirstRun screens get no initialization command from this entry point, potentially leaving sub-models uninitialized.
- **Impact:** Edge-case rendering issues when navigating to screens via non-standard paths.

---

## 3. MEDIUM — Dead Code, Phantom Shortcuts, Degraded Features

### 3.1 `renderTransitionOverlay()` — Dead Code
- **File:** `transition.go:73`
- **Issue:** This method is defined on `*ScreenTransition` but is **never called** in production code. It is only referenced in test code. The screen transition system works via `TransitionTick()` but the overlay rendering is never invoked from `View()` or any update path.
- **Impact:** Transition overlay effect is defined but never rendered.

### 3.2 Command Palette — Phantom Shortcuts
- **File:** `cmdpalette.go:67-69`
- **Issue:** The command palette maps shortcuts that don't correspond to registered slash commands:
  - `"sidebar": "ctrl+b"` — no `/sidebar` command exists in `DefaultCommands()`
  - `"discuss": "ctrl+d"` — no `/discuss` command exists (discuss is workflow-triggered only)
  - `"new session": "ctrl+n"` — registered command is `/new`, not `/new session`
- **Impact:** Users searching the command palette for these will find no matching command.

### 3.3 `/memory revert` — Explicitly Not Implemented
- **File:** `commands_ai.go:45`
- **Issue:** Returns: `"Revert not yet implemented — use /clear and re-start the session."`
- **Impact:** Feature advertised but not available.

### 3.4 `/cost` — Informational Only, Cannot Toggle
- **File:** `commands_config.go:81-89`
- **Issue:** The `/cost` command only shows the current state (enabled/disabled) and tells the user to "Use settings to toggle." It does not actually toggle the cost display despite being registered like an actionable command.
- **Impact:** Misleading command behavior.

### 3.5 13 Models Have No-Op Init() Methods
- **Files:** `dashboard_model.go:77`, `metrics.go:121`, `fileexplorer_model.go:52`, `diff_model.go:89`, `bisect_model.go:61`, `help.go:168`, `config_model.go:613`, `sessiondetail_model.go:45`, `tooldetail_model.go:56`, `rollback.go:71`, `notification_model.go:54`, `themepicker_model.go:59`, `ledger.go:65`
- **Issue:** These models return `nil` from `Init()`, meaning they perform no setup when first navigated to. While this is partially mitigated by `ensureSubModel()`, it could cause a brief flash of uninitialized content.
- **Impact:** Potential rendering artifacts on first screen visit.

### 3.6 17 Silently Ignored Errors in TUI Code
- **Files:** `app.go:350`, `app_update.go:2094`, `app_update_phase.go:120`, `mention.go:61`, `repl.go:40`, `repl_state.go:46`, and others
- **Critical ignored errors:**
  - `_ = m.sessionManager.UpdateWorkflowState(...)` — workflow state save failure silently ignored
  - `tasks, _ = m.sessionManager.LoadTasks(m.sessionID)` — task loading failure means execute screen gets zero tasks
  - `_ = m.workflowEngine.SkipDiscuss()` — discuss skip failure silently ignored
- **Impact:** Users lose data or see empty screens without error messages.

### 3.7 FallbackEventMsg — Sent Manually, Not From Auto-Fallback
- **File:** `types.go:305`, `app_update.go:2109`
- **Issue:** The auto-fallback path (`attemptAutoFallback`) directly mutates state instead of emitting a `FallbackEventMsg`. The message is only sent by the `/fallback` command handler. This is an inconsistency — the auto-fallback path bypasses the message system.
- **Impact:** Auto-fallback events are not tracked/visible in the notification center.

### 3.8 No Keyboard Shortcuts for 3 Screens
- **Screens:** `ScreenConfig` (16), `ScreenSessionDetail` (22), `ScreenToolDetail` (24)
- **Issue:** No `handleKeyAction` entries exist for these screens. They are only reachable via specific commands or nested navigation. No leader chord exists for them.
- **Impact:** Power users cannot quickly access these screens via keyboard.

---

## 4. LOW — Cosmetic Issues & Missing Convenience

### 4.1 FallbackEventMsg Not Emitted From Auto-Fallback
- **File:** `app_update.go:2109`
- **Issue:** Auto-fallback directly mutates state instead of emitting `FallbackEventMsg`. Only the `/fallback` slash command emits this message type.
- **Impact:** Auto-fallback events are invisible to the notification system.

### 4.2 Workflow.test Binary in Repo Root
- **File:** `/home/snigdha/Desktop/Helix/M31A/workflow.test`
- **Issue:** A compiled ELF binary (`workflow.test`) is sitting in the repository root. This is a test artifact that should be `.gitignore`d and cleaned up.
- **Impact:** Cluttered repo, potential confusion.

### 4.3 Coverage Files in Repo Root
- **Files:** `coverage_final.out`, `cov2.out`, `cover.out`
- **Issue:** Build artifacts that should not be committed to the repository.
- **Impact:** Repo bloat, merge conflicts on coverage files.

### 4.4 Architecture Doc Drift
- **File:** `docs/ARCHITECTURE.md`
- **Issue:** Describes `internal/app/`, `internal/commands/`, `internal/style/`, `internal/ui/`, `internal/verify/`, `internal/version/` — but the actual codebase has these consolidated into `internal/tui/`, `internal/tools/`, and other packages. The doc has not been updated.
- **Impact:** New contributors will be confused by the documentation.

### 4.5 go.mod DEP Comments Indicate Tech Debt
- **File:** `go.mod:6-9`
- **Issues:**
  - `DEP-3`: BurntSushi/toml v1 — in maintenance mode; v2 has different API; migrate when ready
  - `DEP-2`: doublestar v4 — pin current version; check for breaking changes before upgrading
- **Impact:** Known dependency risks documented but not addressed.

### 4.6 QuestionModel Esc Handler Unclear
- **File:** `app_update.go:2016`
- **Issue:** When the question model is active (`m.questionRequest != nil`), the `handleQuestionKey` method delegates to `questionModel.Update()` but doesn't have an explicit esc handler — it depends on the `QuestionModel`'s own key handling which may or may not handle esc.
- **Impact:** Users may not be able to dismiss the question modal via esc.

---

## 5. Test Coverage Gaps (Related to Broken Screens)

### 5.1 Screens With No Update() Tests
- **ScreenFirstRun** — Only helpers tested; no `Update()` or `View()` test for the actual model
- **ScreenModelSelector** — Only struct helpers tested; no full model Update/View test

### 5.2 Tests With Zero Assertions (25+ Functions)
Key examples:
- `TestExecuteModelUpdateDown/Up` — calls Update but has zero assertions
- `TestLedgerModel` — calls Update with j/k/r keys but zero assertions
- `TestReplModelUpdateDown/Up/Tab/Esc` — all zero assertions
- `TestHelpModel` — g/G key tests have zero assertions
- 13 `SetTheme()` tests — call SetTheme but never verify it was applied

### 5.3 No End-to-End Screen Transition Tests
- No test verifies a complete screen transition (start → animate → land on new screen)
- No test for `TransitionTickMsg` processing that completes a transition
- No test for `AppMsg{Screen: ScreenX}` navigating and verifying final state

### 5.4 No Cross-Screen Keyboard Navigation Tests
- No test verifies Esc on a screen returns to the previous screen
- No test for leader key `ctrl+x` followed by a screen-change chord
- No test for command palette selecting and executing a screen navigation

### 5.5 Always-Pass Assertion Pattern
- `TestCountFileStatuses` (tui_test.go:439): Uses `if _, _, _, _ = countFileStatuses(nil); false {` — an **always-false condition** that can never fail
- `TestMessageTypes` (tui_test.go:1730): Assigns message types to `_` and never asserts — compilation-only test

---

## 6. Summary Table

| # | Severity | Issue | File(s) |
|---|----------|-------|---------|
| 1.1 | **CRITICAL** | Bisect screen empty, no data path | `bisect_model.go`, `commands_git.go:107` |
| 1.2 | **CRITICAL** | SessionDetail always shows "No session selected" | `sessiondetail_model.go:68` |
| 1.3 | **CRITICAL** | Notification center never receives notifications | `notification_model.go:35` |
| 1.4 | **CRITICAL** | File explorer shows only "." root | `fileexplorer_model.go:22` |
| 1.5 | **CRITICAL** | OptimizedMsg never sent in production | `types.go:315` |
| 2.1 | **HIGH** | Bisect footer shows wrong keys (y/n/b vs g/b/s) | `app_view.go:230` |
| 2.2 | **HIGH** | Dashboard Enter key advertised but not handled | `dashboard_model.go:80,137` |
| 2.3 | **HIGH** | Dashboard j/k scroll advertised but not handled | `dashboard_model.go:80`, `app_view.go:236` |
| 2.4 | **HIGH** | `/bisect` command never starts interactive bisect | `commands_git.go:107` |
| 2.5 | **HIGH** | Discuss timeout never activated | `discuss.go:60` |
| 2.6 | **HIGH** | FirstRun wizard cannot be escaped | `firstrun_model.go` |
| 2.7 | **HIGH** | `routeToScreen()` only handles 2/26 screens | `app_update.go:914` |
| 3.1 | **MEDIUM** | `renderTransitionOverlay()` dead code | `transition.go:73` |
| 3.2 | **MEDIUM** | Command palette phantom shortcuts | `cmdpalette.go:67` |
| 3.3 | **MEDIUM** | `/memory revert` not implemented | `commands_ai.go:45` |
| 3.4 | **MEDIUM** | `/cost` informational only, cannot toggle | `commands_config.go:81` |
| 3.5 | **MEDIUM** | 13 models with no-op Init() | Various |
| 3.6 | **MEDIUM** | 17 silently ignored errors in TUI | Various |
| 3.7 | **MEDIUM** | FallbackEventMsg bypass in auto-fallback | `app_update.go:2109` |
| 3.8 | **MEDIUM** | No keyboard shortcuts for 3 screens | `app_update.go` |
| 4.1 | **LOW** | Auto-fallback not emitting FallbackEventMsg | `app_update.go:2109` |
| 4.2 | **LOW** | workflow.test binary in repo root | Root directory |
| 4.3 | **LOW** | Coverage files in repo root | Root directory |
| 4.4 | **LOW** | Architecture doc drift | `docs/ARCHITECTURE.md` |
| 4.5 | **LOW** | go.mod DEP comments (tech debt) | `go.mod:6` |
| 4.6 | **LOW** | QuestionModel esc handler unclear | `app_update.go:2016` |

---

## 7. Recommendations

### Immediate Fixes (CRITICAL)
1. **Wire up `/bisect` command** to navigate to `ScreenBisect` and call `SetCommits()`
2. **Wire up SessionDetail** — call `SetSession()` from resume screen when user selects a session
3. **Wire up NotificationModel** — call `AddNotification()` from error handlers, phase transitions, and toast system
4. **Initialize FileExplorerModel** with actual project tree via `SetRoot()`
5. **Remove or implement `OptimizedMsg`** — either wire it up or delete the dead type

### High Priority
6. **Fix bisect footer** in `app_view.go:230` to show correct keys (g/b/s instead of y/n/b)
7. **Add Enter handler** to `DashboardModel.Update()` or remove the footer hint
8. **Add j/k scroll** to dashboard or remove the footer hint
9. **Activate discuss timeout** by calling `SetTimeout()` during discuss model creation
10. **Add esc handler** to FirstRun wizard
11. **Extend `routeToScreen()`** to handle all 26 screens
12. **Fix command palette** shortcuts to match actual registered commands

### Medium Priority
13. **Handle ignored errors** — especially `UpdateWorkflowState` and `LoadTasks`
14. **Add missing keyboard shortcuts** for Config, SessionDetail, and ToolDetail screens
15. **Implement `/memory revert`** or remove the command
16. **Clean up dead code** — `renderTransitionOverlay()`, `OptimizedMsg`

### Test Coverage
17. **Add Update() tests** for FirstRun and ModelSelector screens
18. **Add assertions** to the 25+ zero-assertion test functions
19. **Add end-to-end screen transition tests**
20. **Add cross-screen keyboard navigation tests**
21. **Fix always-pass assertion** in `TestCountFileStatuses`
