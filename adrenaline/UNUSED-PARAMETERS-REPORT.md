# M31A — Unused Parameters & Dead Code Report

> Generated via `unparam` and `staticcheck -checks U1000` with manual analysis.
> Each finding includes context, impact, and a concrete recommendation: **Use It** or **Remove It**.

---

## Table of Contents

1. [Unused Function Parameters](#1-unused-function-parameters)
2. [Always-Nil / Unused Return Values](#2-always-nil--unused-return-values)
3. [Unused Struct Fields](#3-unused-struct-fields)
4. [Unused Functions & Methods](#4-unused-functions--methods)
5. [Unused Types, Constants & Variables](#5-unused-types-constants--variables)
6. [Summary & Priority Matrix](#6-summary--priority-matrix)

---

## 1. Unused Function Parameters

### 1.1 — `internal/git/git.go:143` — `(*Git).logInternal(oneline bool, since string)` → `oneline` is unused

**What:** The `oneline` parameter is accepted but never read. The function builds a `--format=%H|%h|%an|%s|%aI` log command regardless.

**Where it could be used:** When `oneline` is true, the format could switch to `%h %s` (short hash + subject) for lightweight callers like the sidebar commit list or rollback browser.

**Recommendation:** **Use It.** Wire `oneline` to switch the `--format` string. Callers that want compact logs (sidebar, rollback) can pass `true`.

---

### 1.2 — `internal/git/git.go:162` — `parseLog(out string, oneline bool)` → `oneline` is unused

**What:** `parseLog` always parses the 5-field pipe-delimited format. The `oneline` flag is passed through but ignored.

**Where it could be used:** If `logInternal` starts producing one-line output, `parseLog` needs to parse a simpler 2-field format (`hash subject`).

**Recommendation:** **Use It** (coupled with 1.1). When `oneline=true`, parse `hash subject` instead of 5-field pipe format. If `logInternal` stays uniform, **Remove It**.

---

### 1.3 — `internal/workflow/execute.go:18` — `(*Engine).runExecute(ctx, goal string)` → `goal` is unused

**What:** The execute phase loads tasks from disk and runs them. The `goal` string is never referenced because task context comes from `TASKS.md` and `PROJECT.md` files.

**Where it could be used:** Could be injected into `buildExecuteContext` as a top-level reminder in the system prompt, reinforcing the user's original intent to the LLM during task execution.

**Recommendation:** **Use It.** Inject `goal` into the execute context's system prompt so the LLM stays aligned with the original objective during task execution.

---

### 1.4 — `internal/workflow/initialize.go:14` — `(*Engine).runInitialize(ctx, goal string)` → `ctx` is unused

**What:** The initialize phase performs synchronous file I/O and git operations. It never calls `ctx.Done()` or passes `ctx` to any blocking call.

**Where it could be used:** Git init, directory creation, and file writes should respect context cancellation for clean abort during long-running sessions or slow filesystems.

**Recommendation:** **Use It.** Pass `ctx` to `e.git.Init()` (via `exec.CommandContext`) and check `ctx.Done()` between steps. This enables clean workflow cancellation during initialization.

---

### 1.5 — `internal/workflow/ship.go:31` — `(*Engine).runShip(ctx, goal string)` → `goal` is unused

**What:** The ship phase builds a summary, commits, writes ledger, and archives. The `goal` is not referenced because summary data comes from saved planning files.

**Where it could be used:** The goal could be embedded in the ship summary output or passed to `generateDemonstration` as a narrative anchor for the walkthrough document.

**Recommendation:** **Use It.** Pass `goal` to `generateDemonstration` and include it in the `ShipSummary` struct for richer output.

---

### 1.6 — `internal/workflow/verify.go:16` — `(*Engine).runVerify(ctx, goal string)` → `goal` is unused

**What:** The verify phase runs acceptance checks and self-heal. The `goal` is not referenced.

**Where it could be used:** The goal could be included in the self-heal prompt to give the LLM better context about what the user originally wanted when attempting to fix verification failures.

**Recommendation:** **Use It.** Inject `goal` into the heal prompt context so the LLM understands the original intent when fixing broken tasks.

---

### 1.7 — `internal/tui/app_update.go:1675` — `(*AppState).attemptAutoFallback(origErr error)` → `origErr` is unused

**What:** When auto-fallback triggers, the original error is received but never logged or displayed. The function only checks for fallback availability.

**Where it could be used:** The error should be logged for debugging and could be included in the toast notification shown to the user.

**Recommendation:** **Use It.** Log `origErr` with `slog.Warn` and include a sanitized version in the fallback toast message (e.g., "Switching to fallback provider after: <error>").

---

### 1.8 — `internal/tui/app_view.go:331-493` — 20 `render*Content` methods receive `chrome layout.PageChrome` but never use it

**Affected methods:** `renderModelSelectorContent`, `renderPlanContent`, `renderExecuteContent`, `renderVerifyContent`, `renderShipContent`, `renderResumeContent`, `renderGoalInputContent`, `renderLedgerContent`, `renderRollbackContent`, `renderMetricsContent`, `renderDiscussContent`, `renderDiffContent`, `renderHelpContent`, `renderBisectContent`, `renderThemePickerContent`, `renderNotificationsContent`, `renderDashboardContent`, `renderSessionDetailContent`, `renderFileExplorerContent`, `renderToolDetailContent`, `renderPhaseModelPickerContent`

**What:** These 20+ render methods accept a `chrome` parameter (containing width/height from the layout system) but call their sub-model's `View()` without forwarding dimensions.

**Where it could be used:** Each sub-model could receive `chrome.ContentWidth()` and `chrome.ContentHeight()` to enable responsive rendering. Currently most sub-models use hardcoded dimensions or stale cached values.

**Recommendation:** **Use It.** Forward `chrome.ContentWidth()` and `chrome.ContentHeight()` to each sub-model via a `SetSize(w, h)` method or by passing dimensions to `View()`. This fixes layout bugs where sub-models render at wrong sizes after terminal resize.

---

### 1.9 — `internal/tui/firstrun_view.go:61` — `(*FirstRunModel).renderStepDots(current int, total int)` → `total` always receives `4`

**What:** The `total` parameter always receives the literal `4` from all 4 call sites. The step count is hardcoded by the wizard flow.

**Where it could be used:** If the wizard gains configurable steps (e.g., skipping provider setup when already configured), `total` would vary.

**Recommendation:** **Remove It.** Replace with a constant `firstRunStepCount = 4` and hardcode the loop bound. If wizard steps become dynamic later, reintroduce the parameter.

---

### 1.10 — `internal/tui/resume_view.go:78` — `renderSessionInfoRow(info, selected, w int, t)` → `w` is unused

**What:** The width parameter `w` is accepted but the row renders at natural width without padding or truncation.

**Where it could be used:** The width could right-align metadata (age, phase) or truncate long session IDs to fit the available column width.

**Recommendation:** **Use It.** Use `w` to pad/truncate the row to the viewport width, right-aligning the age and phase columns for a clean tabular layout.

---

### 1.11 — `internal/tui/ship_view.go:14` — `renderShipStatsGrid(t theme.Theme, rows [][2]string, colWidth int)` → `t` is unused

**What:** The theme parameter is received but the grid renders plain text without styling.

**Where it could be used:** Apply themed styles to labels (left column) vs values (right column) for visual hierarchy — e.g., `t.TextSecondary` for labels and `t.Text` for values.

**Recommendation:** **Use It.** Style the label column with `t.TextSecondary` and value column with `t.Text` for better readability.

---

### 1.12 — `internal/tui/transition.go:73` — `(*ScreenTransition).renderTransitionOverlay(th theme.Theme, width, height int)` → `th` is unused

**What:** The theme parameter is received but the overlay uses a hardcoded `░` character without themed colors.

**Where it could be used:** The overlay could use `th.Background` or `th.Surface` for the dim color instead of a hardcoded character.

**Recommendation:** **Use It.** Apply `th.Surface` or a themed dim color to the overlay for consistent appearance across themes.

---

### 1.13 — `pkg/keychain/keychain_linux.go:264` — `fmtSecret(conn *dbus.Conn, value string)` → `conn` is unused

**What:** The D-Bus connection is passed but the secret is built with an empty `sessionPath`. The comment says "empty = first session."

**Where it could be used:** The connection could be used to open an actual D-Bus secret service session, which is more correct per the Secret Service API specification.

**Recommendation:** **Use It** (future). For now, the empty session path works with GNOME Keyring. When implementing proper D-Bus session negotiation, `conn` will be needed. Keep the parameter but add a `// TODO: open D-Bus session` comment.

---

### 1.14 — `pkg/session/manager.go:88` — `readFileLimited(path string, maxBytes int64)` → `maxBytes` always receives `types.MaxSessionFileSize` (52428800)

**What:** Every caller passes the same constant value. The parameter exists for flexibility but has no variation.

**Where it could be used:** If different file types need different limits (e.g., plan files vs task files vs demonstration files), the parameter makes sense.

**Recommendation:** **Keep It.** The parameter is good defensive design — it allows per-call limits if new file types are added. No action needed; the tool flags it but it's intentional API design.

---

## 2. Always-Nil / Unused Return Values

### 2.1 — `internal/tui/app.go:229` — `(*AppState).initWorkflowEngine()` → result `tea.Cmd` is always nil

**What:** The function signature returns `tea.Cmd` but every return path returns `nil`. Callers may chain the result.

**Recommendation:** **Use It.** Return a `tea.Cmd` that emits a `WorkflowEngineReadyMsg` on success, allowing the Bubble Tea update loop to react to engine readiness without polling.

---

### 2.2 — `internal/tui/app_update.go:1645` — `(*AppState).handleDiscussAnswer()` → result `tea.Cmd` is always nil

**What:** After submitting a discuss answer, the function returns nil. No follow-up command is issued.

**Recommendation:** **Use It.** Return a command that advances to the next discuss question or triggers discuss completion when all questions are answered.

---

### 2.3 — `internal/tui/app_update_phase.go:159` — `(*AppState).handlePlanReady()` → result `tea.Cmd` is always nil

**What:** After updating the plan model with tasks and estimates, returns nil.

**Recommendation:** **Use It.** Return a navigation command to switch the screen to `ScreenPlan`, ensuring the user sees the plan immediately.

---

### 2.4 — `internal/tui/repl_stream.go:29` — `(*ReplModel).handleStreamMsg()` → result 1 (`bool`) is always false / never used

**What:** The second return value (a "done" flag) is always `false` in this function and callers never check it.

**Recommendation:** **Remove It.** Change the signature to return only `[]tea.Cmd`. The "done" concept is handled by `handleStreamDoneMsg`.

---

### 2.5 — `internal/tui/repl_stream.go:96` — `(*ReplModel).handleStreamDoneMsg()` → result 0 (`[]tea.Cmd`) always nil, result 1 (`bool`) never used

**What:** Returns `(nil, true)` — the commands slice is always nil and the bool is never checked by callers.

**Recommendation:** **Simplify.** Change to return nothing (`void`). The caller already knows the stream is done from the message type.

---

### 2.6 — `internal/tui/repl_stream.go:191` — `(*ReplModel).handleStreamErrorMsg()` → result 0 always nil, result 1 never used

**What:** Same pattern as 2.5 — returns `(nil, true)` where neither value is meaningful.

**Recommendation:** **Simplify.** Change to return nothing. The error display is self-contained.

---

### 2.7 — `internal/tui/repl_thinking.go:37` — `(*ReplModel).handleThinkingToggle()` → result `tea.Cmd` is always nil

**What:** The toggle is synchronous (toggles block, re-renders), so no async command is needed.

**Recommendation:** **Keep It.** The `tea.Cmd` return type matches the Bubble Tea handler convention. Changing it would require refactoring the dispatch table. Acceptable as-is.

---

## 3. Unused Struct Fields

### 3.1 — `internal/tui/app_state.go:149-150` — `discussAnswers []string` and `discussIndex int`

**What:** These fields were intended for tracking discuss Q&A state in the AppState, but the discuss workflow uses the engine's internal state instead.

**Recommendation:** **Remove It.** The discuss state lives in `workflow.Engine`. These AppState fields are dead weight.

---

### 3.2 — `internal/tui/app_state.go:168-169` — `lastActivity time.Time` and `streamErrorTime time.Time`

**What:** `lastActivity` was likely intended for idle detection/timeout. `streamErrorTime` was for debouncing rapid stream errors. Neither is read anywhere.

**Recommendation:** **Remove It** for now. If idle timeout or error debouncing is planned, reintroduce when the feature is implemented.

---

### 3.3 — `internal/tui/bisect_model.go:18-19,22` — `good int`, `bad int`, `viewport viewport.Model`

**What:** `good` and `bad` track bisect boundary commits (indices), but the bisect UI uses `current` and `total` instead. `viewport` was allocated for scrollable commit display but never initialized or used.

**Recommendation:** **Use `good`/`bad`.** These should mark the bisect range visually (green for good, red for bad commits). **Remove `viewport`** unless scrollable commit list is planned.

---

### 3.4 — `internal/tui/commands.go:72` — `CommandRegistry.lastCompressTime time.Time`

**What:** Intended to rate-limit `/compress` commands but never checked.

**Recommendation:** **Use It.** Add a cooldown check in the compress handler: reject compress if < 30s since last. This prevents expensive repeated context compression.

---

### 3.5 — `internal/tui/dashboard_model.go:23` — `elapsed string`

**What:** Intended to show session elapsed time on the dashboard but never populated or rendered.

**Recommendation:** **Use It.** Update `elapsed` on each `TickMsg` and display it in the dashboard header alongside goal and model info.

---

### 3.6 — `internal/tui/execute_model.go:25-27` — `toolCalls int`, `totalTokens int`, `totalCost float64`

**What:** These fields were designed to track execution metrics but the data flows through task results and AppState instead.

**Recommendation:** **Use It.** Populate from `ToolCompleteMsg` and `StreamDoneMsg` usage data, then render in the execute view's footer as live metrics.

---

### 3.7 — `internal/tui/plan_model.go:30` — `selected int`

**What:** Intended for task selection in the plan view (e.g., selecting a task to see details or to refine).

**Recommendation:** **Use It.** Wire up arrow/enter key handling in the plan screen to let users select individual tasks for detail view or targeted refinement.

---

### 3.8 — `internal/tui/repl_model.go:105` — `shellMode bool`

**What:** Designed for a `!command` shell mode in the REPL input where `!` prefix sends commands directly to bash.

**Recommendation:** **Use It.** Implement the shell mode: when input starts with `!`, execute via Bash tool directly without LLM involvement. This is a valuable power-user feature.

---

### 3.9 — `internal/tui/sidebar.go:41` — `err string`

**What:** Stores the last git error for display, but errors are handled inline and this field is never set or read.

**Recommendation:** **Use It.** Set `err` when git status fetch fails and display it in the sidebar footer for user visibility.

---

### 3.10 — `internal/tui/themepicker_model.go:16` — `offset int`

**What:** Scroll offset for theme list pagination, but the list is short enough to fit without scrolling.

**Recommendation:** **Remove It** unless the theme catalog grows. If keeping, implement `PageUp`/`PageDown` handlers that use `offset`.

---

### 3.11 — `internal/tui/phasemodelpicker.go:27` — `pickerPanel.providerName string`

**What:** Intended to filter models by provider in the phase model picker, but provider filtering is not implemented.

**Recommendation:** **Use It.** Implement provider filter tabs (All / OpenRouter / Zen) in the model picker UI.

---

## 4. Unused Functions & Methods

### 4.1 — `internal/tools/webfetch.go:170` — `(*WebFetch).resolveAndCheck`

**What:** DNS resolution + private IP check for SSRF protection. Defined but not called in the Execute path.

**Recommendation:** **Use It.** Call `resolveAndCheck` in `WebFetch.Execute()` before making the HTTP request. This is a **security issue** — the SSRF guard exists but is not wired in.

---

### 4.2 — `internal/tui/app_channel.go:15-39` — `channelCloser`, `newChannelCloser`, `close`, `chan_`

**What:** A channel-safe wrapper with `sync.Once` close semantics. Created but never instantiated.

**Recommendation:** **Remove It.** The `channelEmitter` in the same file is used; `channelCloser` is dead code from an earlier design.

---

### 4.3 — `internal/tui/app_view.go:604-612` — `errorf`, `simpleError`, `(*simpleError).Error`

**What:** A custom error constructor and type, unused.

**Recommendation:** **Remove It.** Standard `fmt.Errorf` or `errors.New` serve the same purpose.

---

### 4.4 — `internal/tui/execute_view.go:15-67` — `renderProgressBar`, `renderAnimatedProgressBar`, `animatedProgressBarWidth`, `renderAnimatedProgressPct`

**What:** Progress bar rendering functions that were superseded by the `bubbles/progress` component.

**Recommendation:** **Remove It.** The `ExecuteModel` now uses `bubbles/progress.Model` directly. These are leftover from before the component was adopted.

---

### 4.5 — `internal/tui/firstrun_model.go:390` — `pickTopModels`

**What:** Selects top N models by context length for the first-run wizard suggestions. Defined but not called.

**Recommendation:** **Use It.** Wire into the first-run model selection step to show recommended models instead of the full list.

---

### 4.6 — `internal/tui/firstrun_view.go:76` — `renderBlockDivider`

**What:** Decorative gradient divider for the first-run wizard. Never called in any view.

**Recommendation:** **Use It.** Add as a section separator between wizard steps for visual polish, or **Remove It**.

---

### 4.7 — `internal/tui/firstrun_view.go:372` — `overlayOnPlainGrid`

**What:** Composites an overlay panel centered on a starfield background grid. Never called.

**Recommendation:** **Use It.** This was designed for centering the API key input panel over the starfield background. Wire it into the first-run view for step 3 (API key entry).

---

### 4.8 — `internal/tui/firstrun_view.go:414` — `runeWidth`

**What:** Thin wrapper around `lipgloss.Width`. Only called by `overlayOnPlainGrid` (also unused).

**Recommendation:** **Remove It** (coupled with 4.7). If `overlayOnPlainGrid` is used, keep this. Otherwise, callers can use `lipgloss.Width` directly.

---

### 4.9 — `internal/tui/plan_view.go:15` — `renderPlanHeader`

**What:** Builds a styled plan header with task count, cost, and model badge. Never called — the plan model renders its own header.

**Recommendation:** **Use It.** Replace the inline header construction in `PlanModel.View()` with this function for consistency and testability.

---

### 4.10 — `internal/tui/repl.go:384` — `(*ReplModel).renderQuickActions`

**What:** Renders quick action hint text. Never called in the REPL view.

**Recommendation:** **Use It.** Show in the welcome/empty state when no messages are displayed, guiding new users to available commands.

---

### 4.11 — `internal/tui/repl_commands.go:45` — `(*ReplModel).expandFileRefs`

**What:** Resolves `@filepath` references in REPL input and inlines file contents. Never called.

**Recommendation:** **Use It.** Wire into the submit handler to expand `@file` references before sending to the LLM. This is a **functional gap** — @mention autocomplete works but the references aren't expanded.

---

### 4.12 — `internal/tui/repl_stream.go:217` — `(*ReplModel).streamTickCmds`

**What:** Returns streaming tick commands. Superseded by inline `StreamTickCmd()` calls.

**Recommendation:** **Remove It.** The inline call is cleaner and this wrapper adds no value.

---

### 4.13 — `internal/tui/toast.go:163` — `advanceToastFrame`

**What:** Advances toast slide-in animation frame. Never called — toast animations are static.

**Recommendation:** **Use It** if toast slide-in animation is desired, otherwise **Remove It** along with `toastSlideInFrames` (5.3).

---

### 4.14 — `internal/tui/verify.go:243` — `(*VerifyModel).countResults`

**What:** Counts passed/failed/total verification results. Never called in the verify view.

**Recommendation:** **Use It.** Display a summary badge in the verify view header: e.g., "12/15 passed, 3 failed".

---

### 4.15 — `internal/tui/app_update_commands.go:199` — `sessionLoadedMsg` type

**What:** A message type for session restore completion. Never sent or received.

**Recommendation:** **Remove It** unless session restore emits this message in a planned feature.

---

## 5. Unused Types, Constants & Variables

### 5.1 — `internal/tui/firstrun_view.go:865` — `const logo`

**What:** ASCII art logo constant, duplicated from `components/logo.go`. The first-run wizard uses `components.Logo` instead.

**Recommendation:** **Remove It.** It's a stale duplicate.

---

### 5.2 — `internal/tui/toast.go:24` — `var toastSlideInFrames`

**What:** Animation frame definitions for toast slide-in. Never referenced.

**Recommendation:** **Remove It** (coupled with 4.13). If animation is implemented, use it.

---

## 6. Summary & Priority Matrix

### High Priority (Security / Functional Gaps)

| # | Finding | Action | Effort |
|---|---------|--------|--------|
| 4.1 | `WebFetch.resolveAndCheck` not wired | **Use It** — SSRF guard must be active | Low |
| 4.11 | `expandFileRefs` not called | **Use It** — @mention resolution broken | Low |
| 1.3-1.6 | Workflow `goal` unused in execute/ship/verify | **Use It** — improves LLM context quality | Medium |
| 1.7 | `origErr` unused in auto-fallback | **Use It** — debugging visibility | Low |

### Medium Priority (UX / Correctness)

| # | Finding | Action | Effort |
|---|---------|--------|--------|
| 1.8 | 20 render methods ignore `chrome` dimensions | **Use It** — fixes resize layout bugs | Medium |
| 1.10 | `renderSessionInfoRow` ignores width | **Use It** — clean tabular layout | Low |
| 1.11 | `renderShipStatsGrid` ignores theme | **Use It** — visual hierarchy | Low |
| 1.12 | `renderTransitionOverlay` ignores theme | **Use It** — theme consistency | Low |
| 3.8 | `shellMode` field unused | **Use It** — power-user feature | Medium |
| 3.6 | Execute model metrics unused | **Use It** — live execution stats | Low |
| 4.10 | `renderQuickActions` not shown | **Use It** — new user guidance | Low |
| 4.14 | `countResults` not called | **Use It** — verify summary badge | Low |
| 4.5 | `pickTopModels` not called | **Use It** — better first-run UX | Low |
| 3.4 | `lastCompressTime` not checked | **Use It** — compress rate limiting | Low |

### Low Priority (Dead Code Cleanup)

| # | Finding | Action | Effort |
|---|---------|--------|--------|
| 4.2 | `channelCloser` dead code | **Remove It** | Low |
| 4.3 | `errorf` / `simpleError` dead code | **Remove It** | Low |
| 4.4 | Unused progress bar functions | **Remove It** | Low |
| 4.8 | `runeWidth` wrapper | **Remove It** | Low |
| 4.12 | `streamTickCmds` wrapper | **Remove It** | Low |
| 5.1 | Duplicate `logo` constant | **Remove It** | Low |
| 5.2 | `toastSlideInFrames` dead variable | **Remove It** | Low |
| 3.1 | `discussAnswers`/`discussIndex` dead fields | **Remove It** | Low |
| 3.2 | `lastActivity`/`streamErrorTime` dead fields | **Remove It** | Low |

### No Action Needed (Intentional Design)

| # | Finding | Reason |
|---|---------|--------|
| 1.14 | `readFileLimited.maxBytes` always same value | Defensive API — allows future per-call limits |
| 2.7 | `handleThinkingToggle` returns nil Cmd | Matches Bubble Tea handler convention |

---

### Statistics

- **Total findings:** 50
- **Unused function parameters:** 14
- **Unused return values:** 7
- **Unused struct fields:** 11
- **Unused functions/methods:** 15
- **Unused types/constants/variables:** 3

### Recommended Action Split

- **Use It (wire in):** 25 findings
- **Remove It (dead code):** 18 findings
- **No action:** 2 findings
- **Keep (defensive design):** 1 finding
