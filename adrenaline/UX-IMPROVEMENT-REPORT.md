# M31A UX (User Experience) Improvement Report

**Date:** 2026-06-09
**Scope:** Deep analysis of all user-facing surfaces — REPL, workflow screens, first-run wizard, settings, sidebar, command palette, permission modals, error handling, streaming, toasts, status bar, keybindings, and navigation
**Method:** Line-by-line code review of ~90 source files across `internal/tui/`, `internal/workflow/`, `internal/tools/`, `internal/config/`, `internal/errors/`, `internal/provider/`, `cmd/m31a/`, and supporting packages

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [First-Run & Onboarding Experience](#2-first-run--onboarding-experience)
3. [REPL & Chat Interface](#3-repl--chat-interface)
4. [Error Handling & User Feedback](#4-error-handling--user-feedback)
5. [Workflow Phase Screens](#5-workflow-phase-screens)
6. [Navigation & Screen Transitions](#6-navigation--screen-transitions)
7. [Command Palette & Slash Commands](#7-command-palette--slash-commands)
8. [Settings & Configuration UX](#8-settings--configuration-ux)
9. [Sidebar & Git Integration](#9-sidebar--git-integration)
10. [Permission & Question Modals](#10-permission--question-modals)
11. [Streaming & Response Display](#11-streaming--response-display)
12. [Status Bar & Metadata](#12-status-bar--metadata)
13. [Keyboard & Accessibility](#13-keyboard--accessibility)
14. [Session Management UX](#14-session-management-ux)
15. [Toast & Notification System](#15-toast--notification-system)
16. [Responsive Design & Terminal Sizing](#16-responsive-design--terminal-sizing)
17. [Prioritized Action Plan](#17-prioritized-action-plan)

---

## 1. Executive Summary

This report identifies **52 UX improvement opportunities** across 16 categories. The most impactful findings are:

- **Onboarding friction**: First-run wizard has no validation feedback for API keys, no connectivity test before model selection, and the skip flow leads to a dead-end model picker
- **Silent failures**: Multiple operations (session creation, workflow init, provider registration) fail silently with only `slog.Warn` — the user sees nothing
- **Inconsistent loading states**: 9 screens show bare "Loading..." text with no spinner, progress indicator, or context about what's loading
- **Dead-end screens**: Metrics screen has no navigation hints rendered; several screens have no way to return to REPL except `esc`
- **Workflow opacity**: During the longest operations (plan, execute, verify), the user gets minimal real-time feedback about what the LLM is doing
- **Toast reliability**: Toasts use a simple FIFO timer that can expire the wrong toast when multiple arrive rapidly
- **Settings confusion**: API key editing in settings has no validation, no connectivity test, and the "Keys" tab allows editing keys that are resolved from env vars (changes are silently overridden)

---

## 2. First-Run & Onboarding Experience

### UX-01: No API Key Validation Before Model Selection

**File:** `internal/tui/firstrun_model.go:328-351`
**Severity:** High

When the user enters an API key and presses Enter, the wizard immediately advances to the model selection step without validating the key. If the key is invalid, the user discovers this only after completing the entire wizard, when the REPL fails to get a response.

**Current flow:**
```
Welcome → Provider Select → API Key (no validation) → Model Pick → Done → REPL (fails)
```

**Recommended flow:**
```
Welcome → Provider Select → API Key → [Validate key via health check] → Model Pick → Done → REPL
```

**Fix:** After the user enters the API key, run a quick `HealthCheck()` call in a goroutine. Show a spinner during validation. If the key is invalid, display the error inline and keep the user on the API key step.

---

### UX-02: Skip Flow Leads to Confusing Model Picker

**File:** `internal/tui/firstrun_model.go:317-324`
**Severity:** Medium

When the user presses `s` to skip provider selection, they land on the model pick step with `DefaultProvider = ""`. The view then shows:

> "No provider configured. Enter a model ID or leave blank."

This is confusing because:
1. The user doesn't know what model IDs are valid without a provider
2. Leaving it blank means the app starts with no LLM access at all
3. There's no explanation of how to configure a provider later

**Fix:** When skipping providers, show a clearer message: "You can configure providers later via `/settings`. Press Enter to start without an LLM, or type a model ID if you know what you want."

---

### UX-03: No Visual Indication of Which Provider Is Default When Multiple Are Selected

**File:** `internal/tui/firstrun_model.go:300-311`
**Severity:** Low

When the user selects multiple providers (e.g., OpenRouter + Zen), the first selected provider becomes the default. However, there's no visual indicator showing which provider will be the default. The user must know that selection order determines the default.

**Fix:** Add a "(default)" badge next to the first selected provider, or add a dedicated "set as default" action.

---

### UX-04: Keychain Toggle Uses Tab Key Without Visual Focus

**File:** `internal/tui/firstrun_view.go:574-580`
**Severity:** Low

The "Save to system keychain" checkbox is toggled with Tab, but the checkbox itself doesn't look like a focusable element. Users might not discover this option since there's no visual cursor or highlight on it.

**Fix:** Add a visible focus ring or cursor when the checkbox is the active element, or move it to a dedicated step.

---

### UX-05: Welcome Screen Starfield May Cause Flicker on Slow Terminals

**File:** `internal/tui/firstrun_view.go:119-202`
**Severity:** Low

The starfield background is re-rendered on every View() call. On slow terminals or SSH connections, this can cause visible flicker because the star positions are deterministic (seeded) but the entire screen is redrawn each frame.

**Fix:** Cache the starfield output and only regenerate on resize.

---

## 3. REPL & Chat Interface

### UX-06: "Loading..." Text on Initial Render

**File:** `internal/tui/app_view.go:18-19`
**Severity:** Medium

```go
func (m *AppState) View() string {
    if m.width == 0 || m.height == 0 {
        return "Loading..."
    }
```

The very first thing a user sees when launching M31A is a bare "Loading..." text. This is the only frame before the terminal size is detected, but on slow terminals it may be visible for several hundred milliseconds. It looks unpolished compared to the rich welcome screen that follows.

**Fix:** Show the logo or a branded splash screen instead of plain text.

---

### UX-07: No Empty State Guidance After Clearing Conversation

**File:** `internal/tui/commands_core.go` (handleClear)
**Severity:** Medium

When the user runs `/clear`, all messages are removed and the viewport becomes empty. However, the welcome screen is NOT re-rendered — the user sees a blank viewport with the input area below. The welcome content is only shown when `len(m.messages) == 0` at initial render time.

**Fix:** After `/clear`, re-render the welcome screen content in the viewport to provide context and getting-started suggestions.

---

### UX-08: User Messages Created via `makeAssistantMsg` Have Wrong Visual Styling

**File:** `internal/tui/repl.go:272-273`
**Severity:** Medium

```go
userMsg := makeAssistantMsg(input)
userMsg.Role = "user"
```

The `makeAssistantMsg` function creates a message with assistant-role rendering properties, then the role is overridden to "user". If the message renderer checks properties beyond the Role field (like segments or formatting), user messages may render incorrectly.

**Fix:** Create a dedicated `makeUserMsg` function that properly initializes user-message rendering properties.

---

### UX-09: Shell Commands (`!`) Produce No Visual Feedback

**File:** `internal/tui/repl.go:285-300`
**Severity:** Medium

When the user types `!ls` to run a shell command, the input is added as a user message and emitted as a `SlashCommandMsg`. However, if the shell command handler doesn't produce output quickly, the user sees their input in the chat but no indication that the command is running.

**Fix:** Show a spinner or "Running..." indicator below the user's shell command while it executes.

---

### UX-10: No Scroll-to-Bottom After Receiving Assistant Response

**File:** `internal/tui/repl_stream.go`
**Severity:** Medium

When the user scrolls up to read earlier messages and a new response arrives, the viewport doesn't auto-scroll. A "new messages" indicator appears, but the user must manually press `ctrl+l`. This is correct behavior for not interrupting reading, but the indicator is easy to miss since it appears above the input area.

**Fix:** Add a more prominent visual indicator — perhaps a brief flash or a colored bar at the bottom of the viewport.

---

### UX-11: History Navigation Wraps Unexpectedly

**File:** `internal/tui/repl.go:354-386`
**Severity:** Low

When navigating command history with Up arrow, pressing Up at the oldest entry does nothing (good). But pressing Down at the newest entry clears the input entirely (`m.historyIndex = -1; m.textarea.SetValue("")`). If the user had typed something before pressing Up, their original input is lost.

**Fix:** Save the current input before navigating history and restore it when the user navigates past the newest entry.

---

### UX-12: Mention Autocomplete Has No Dismiss on Backspace

**File:** `internal/tui/repl.go:153-156`
**Severity:** Low

The `@mention` autocomplete popup only dismisses on `esc`. If the user backspaces past the `@` character, the popup remains visible with stale suggestions.

**Fix:** Dismiss the mention popup when the `@` character is deleted or when the cursor moves before the mention start position.

---

### UX-13: Quick Actions Panel Shows Below Messages Even When Scrolling

**File:** `internal/tui/repl_view.go:61-64`
**Severity:** Low

```go
if len(m.messages) > 0 && !m.streaming {
    quickActions = m.renderQuickActionsPanel(rw)
}
```

The quick actions panel renders below the messages when idle, but it's part of the vertical layout, not the viewport. This means it pushes the input area down, reducing the visible message area. During long conversations, this wastes valuable vertical space.

**Fix:** Either integrate quick actions into the viewport content or make them collapsible.

---

## 4. Error Handling & User Feedback

### UX-14: Silent Failures in Critical Operations

**Files:** Multiple — `app.go:146-147`, `app_update.go:1016-1018`, `app_state.go:248`
**Severity:** High

Many critical operations fail silently with only `slog.Warn`:

1. **Workflow engine init failure** (`app.go:185`): `slog.Error("workflow engine init failed", ...)` — user sees nothing
2. **New session failure** (`app_update.go:1017`): `slog.Error("new session failed", ...)` — user sees nothing
3. **Provider registration failure** (`app_state.go:248`): `slog.Warn("failed to register provider from wizard", ...)` — wizard proceeds as if successful
4. **Config save failure** (`app_state.go:289`): `slog.Warn("failed to save config after wizard", ...)` — user thinks config was saved

**Fix:** Surface all user-initiated operation failures as toasts or inline error messages. The `errors.UserMessage()` function already provides user-friendly text — it should be used consistently.

---

### UX-15: Generic Error Messages in REPL for Stream Failures

**File:** `internal/tui/app_update.go:407-409`
**Severity:** High

```go
case ErrorMsg:
    if m.replModel != nil {
        m.replModel.AddMessage(makeAssistantMsg("Error: " + msg.Err.Error()))
    }
```

Raw error strings are displayed to the user. The `errors.UserMessage()` function exists specifically to translate these into actionable messages, but it's not used here. A user might see:

> Error: context deadline exceeded

Instead of:

> Request cancelled

**Fix:** Use `errors.UserMessage(msg.Err)` to provide actionable error messages.

---

### UX-16: Provider Fallback Happens Silently

**File:** `internal/tui/app_update.go:336-347`
**Severity:** Medium

When the active provider fails and auto-fallback switches to another provider, the `FallbackEventMsg` is logged but the user sees no notification. They might wonder why their model suddenly changed.

The toast that should notify the user is not created in the `FallbackEventMsg` handler.

**Fix:** Add a toast notification when a fallback occurs: "Switched from OpenRouter to Zen (provider unavailable)".

---

### UX-17: No Feedback When Workflow Engine Is Not Initialized

**File:** `internal/tui/app.go:64-72`
**Severity:** Medium

```go
if m.workflowEngine == nil {
    return func() tea.Msg {
        return PhaseResultMsg{
            Phase:   phase,
            Success: false,
            Error:   "workflow engine not initialized",
        }
    }
}
```

When the user tries to run a workflow phase but the engine isn't initialized (e.g., no provider configured), they get a raw error string in a PhaseResultMsg. The error handling for this message may show the raw string or nothing at all.

**Fix:** Show a clear toast or message: "Cannot start workflow — no provider configured. Run /settings to configure a provider."

---

### UX-18: Config Validation Errors Are Developer-Facing

**File:** `internal/config/loader.go:297-503`
**Severity:** Medium

Config validation errors use field paths like `"permissions.rules[0].action"` and expected types like `"\"allow\", \"deny\", or \"ask\""`. While technically correct, these are not user-friendly for the typical developer using M31A.

**Fix:** Add human-readable descriptions to validation errors. E.g., "In your config file, the permission rule for tool '' has an invalid action. Valid actions are: 'allow', 'deny', or 'ask'."

---

## 5. Workflow Phase Screens

### UX-19: Plan Screen Has No Estimated Duration Display

**File:** `internal/tui/plan_view.go`, `internal/tui/plan_model.go`
**Severity:** Medium

The plan header shows task count and cost estimate, but not the estimated duration. The `PhaseResultMsg` carries `DurationMs`, and the `PlanReadyMsg` carries `TimeEstimate`, but the plan view doesn't prominently display expected completion time.

**Fix:** Display estimated time alongside cost in the plan header: "~5 min, ~$0.02".

---

### UX-20: Execute Screen Shows Minimal Tool Output

**File:** `internal/tui/app_update.go:233-250`
**Severity:** High

During task execution, tool calls produce minimal output:

```
→ Bash: Executing Bash
  ok Bash (234ms)
```

The user doesn't see:
- What command was executed
- What files were read/written
- What the output was (truncated or otherwise)
- Why a tool failed (only "failed" is shown)

This makes the execute screen nearly useless for understanding what the agent is actually doing.

**Fix:** Show a brief summary of each tool call's input and output. For Bash, show the command. For FileWrite, show the file path. For Grep, show the pattern.

---

### UX-21: No Progress Percentage on Execute Screen During Long Tasks

**File:** `internal/tui/execute_view.go:36-51`
**Severity:** Medium

The animated progress bar on the execute screen has a fixed width of 10 characters (`animatedProgressBarWidth` returns `10`). This is too narrow to convey meaningful progress, and the percentage is computed from `Animated.Progress()` which may not reflect actual task completion.

**Fix:** Make the progress bar wider (at least 20 chars) and show a numeric percentage alongside it.

---

### UX-22: Self-Heal Messages Use Raw Emoji Instead of Theme-Aware Icons

**File:** `internal/tui/app_update.go:254`
**Severity:** Low

```go
fmt.Sprintf("⚠ Self-heal attempt %d/%d for task %d", msg.Attempt, msg.Max, msg.TaskID)
```

The warning emoji is hardcoded rather than using a theme-aware icon. On terminals with dark backgrounds, this may not be visible. On light themes, it may clash.

**Fix:** Use theme-consistent styling for self-heal messages.

---

### UX-23: Verify Screen "Heal" Action Heals Only the First Failed Task

**File:** `internal/tui/verify.go:104-113`
**Severity:** Medium

Pressing `h` on the verify screen always heals the first failed task found by iterating the task list. If multiple tasks failed, the user has no way to choose which task to heal. The cursor is not used for selection.

**Fix:** Add a cursor or selection mechanism so the user can choose which failed task to heal.

---

### UX-24: Ship Screen Has No Way to View the Full Diff Before Committing

**File:** `internal/tui/ship_view.go`, `internal/tui/ship_model.go`
**Severity:** Medium

The ship summary shows stats (tasks done, files changed, commits) but doesn't offer a way to review the full diff before the final commit. The user must know to use `/diff` separately.

**Fix:** Add a "View diff" action (key: `d`) on the ship screen that opens the diff viewer.

---

### UX-25: Discuss Screen Timer Only Shows When < 30 Seconds Remain

**File:** `internal/tui/discuss.go:200-211`
**Severity:** Low

The timeout timer is invisible until the last 30 seconds. For a 5-minute timeout, the user has no idea a timer is running until the last 30 seconds, which can be surprising.

**Fix:** Always show the timer, but use muted styling until the last 30 seconds when it switches to warning color.

---

## 6. Navigation & Screen Transitions

### UX-26: No Breadcrumb or Navigation History

**Severity:** Medium

M31A has 17 screens but no breadcrumb trail or back-stack. Pressing `esc` on most screens returns to the REPL, but there's no way to go "back" to the previous non-REPL screen. For example, navigating Settings → Models → (esc) goes to REPL, not back to Settings.

**Fix:** Implement a simple screen stack where `esc` pops to the previous screen.

---

### UX-27: Screen Transitions Skip for Important Screens

**File:** `internal/tui/app_update.go:784-791`
**Severity:** Low

```go
skipTransition := screen == ScreenPermission || screen == ScreenDiff ||
    screen == ScreenFirstRun || screen == ScreenResume ||
    screen == ScreenConfig
```

The resume screen and config screen skip the transition animation. Since these are full-screen views the user navigates to intentionally, the lack of transition is jarring — the screen just snaps to the new view.

**Fix:** Enable transitions for resume and config screens.

---

### UX-28: "Loading..." Placeholder Text Differs Per Screen

**File:** `internal/tui/app_view.go:142-245`
**Severity:** Low

Each screen has a different loading message:
- "Loading model selector..."
- "Loading plan..."
- "Loading execution..."
- "Loading verification..."
- "Loading ship summary..."
- "Loading sessions..."
- "Loading goal input..."
- "Loading first-run wizard..."
- "Loading ledger..."
- "Loading rollback browser..."
- "Loading metrics..."
- "Loading discuss..."
- "Loading diff..."

This inconsistency is confusing. None of them include a spinner or progress indicator.

**Fix:** Use a consistent branded loading state with a spinner: "M31A · Loading..." or a shared `renderLoading()` helper.

---

## 7. Command Palette & Slash Commands

### UX-29: Command Palette Search Has No Fuzzy Matching

**File:** `internal/tui/cmdpalette.go:189-205`
**Severity:** Medium

The command palette uses simple substring matching:

```go
if strings.Contains(strings.ToLower(e.cmd.Name), q) ||
    strings.Contains(strings.ToLower(e.cmd.Description), q)
```

This means typing "mdl" won't match "model" and "sess" won't match "sessions". Users of tools like VS Code or Sublime Text expect fuzzy matching.

**Fix:** Implement fuzzy matching (e.g., score-based character subsequence matching) for the command palette filter.

---

### UX-30: Command Palette Has No Character-Level Search Highlighting

**File:** `internal/tui/cmdpalette.go:368-376`
**Severity:** Low

```go
func (cp *CommandPaletteModel) renderHighlightedQuery(t theme.Theme) string {
    // For simplicity, just renders the query in primary color
    return lipgloss.NewStyle().Foreground(t.Text).Render(cp.query)
}
```

The comment explicitly acknowledges that character-level highlighting was skipped. This makes it harder for users to see why a command matched their query.

**Fix:** Implement matched-character highlighting in the search results.

---

### UX-31: Unknown Command Suggestion Uses Levenshtein With Threshold 2

**File:** `internal/tui/commands.go:179-190`
**Severity:** Low

The `suggestCommand` function uses Levenshtein distance with a threshold of 3 (`bestDist := 3`). This means commands that are 3 edits away won't be suggested. For example, typing `/compress` as `/compress` (1 missing letter) works, but `/compres` (also 1 missing letter) might not if the distance calculation differs.

**Fix:** The threshold of 3 is actually reasonable, but the suggestion message could be improved by showing multiple suggestions when several are close matches.

---

### UX-32: `/help` Command Opens a Separate Screen Instead of Inline

**File:** `internal/tui/help.go`
**Severity:** Low

The help screen replaces the entire REPL view. Users might prefer an inline help display or a split-pane overlay so they can reference shortcuts while typing.

**Fix:** Consider rendering help as a scrollable overlay that can be toggled without leaving the REPL context.

---

## 8. Settings & Configuration UX

### UX-33: Settings Tab Navigation Conflicts With Field Interaction

**File:** `internal/tui/settings_model.go:210-222`
**Severity:** High

```go
case "tab", "right":
    if len(s.fields) > 0 && s.isCurrentFieldChoice() {
        return s.cycleChoice(1)
    }
    s.activeTab = SettingsTab((int(s.activeTab) + 1) % len(settingsTabNames))
```

Tab/Right arrow is overloaded: it cycles choice fields AND switches tabs. If the current field is a choice type, Tab cycles the choice instead of switching tabs. This is confusing because:
1. The user may want to switch tabs while on a choice field
2. There's no visual indicator of which action Tab will perform
3. Left/Right for tab switching conflicts with text cursor movement in text fields

**Fix:** Use dedicated keys for tab switching (e.g., `1-6` which already exist, or `[` / `]`) and reserve Tab for field interaction only.

---

### UX-34: API Key Edit in Settings Has No Validation

**File:** `internal/tui/settings_model.go:426-430`
**Severity:** Medium

When the user edits an API key in the Keys tab, there's no validation. The key is stored directly in the config. Worse, if the key was originally loaded from an environment variable, the config file value is silently overridden on the next load.

The Keys tab shows: "Keys are resolved: env var → OS keychain → config file." But doesn't indicate which source is currently active for each key.

**Fix:**
1. Show the active source for each key (env var, keychain, or config)
2. Warn when editing a key that's overridden by an env var
3. Validate key format before saving

---

### UX-35: Settings Save Doesn't Show Which File Was Written

**File:** `internal/tui/settings_model.go:475-489`
**Severity:** Low

After saving, the status shows `"✓ Config saved to " + s.configPath`. However, if `M31A_CONFIG` env var is set, the actual path may differ from what the user expects. The `Save()` method uses the env var override internally, but the settings UI shows the original path.

**Fix:** Show the actual path that was written to.

---

### UX-36: About Tab Has No Version Number

**File:** `internal/tui/settings_model.go:713-732`
**Severity:** Low

The About tab shows module path and dependencies but not the version number. The version is available (`m.version`) but isn't passed to the settings model.

**Fix:** Display the version prominently in the About tab.

---

## 9. Sidebar & Git Integration

### UX-37: Sidebar Shows "No active session" Before User Creates One

**File:** `internal/tui/sidebar.go:556-563`
**Severity:** Low

The sidebar displays "No active session" in the SESSION section before the user creates their first session. This is technically accurate but may confuse new users who haven't started working yet.

**Fix:** Show a more inviting message like "Start typing to begin a session" or hide the session section until a session exists.

---

### UX-38: Sidebar Auto-Hides Without Explanation

**File:** `internal/tui/app_view.go:50`
**Severity:** Medium

```go
hasSidebar := m.sidebarModel != nil && m.sidebarModel.IsVisible() && m.width >= WidthFull
```

When the terminal is narrower than 80 columns (`WidthFull`), the sidebar auto-hides. There's no indication to the user that the sidebar exists or how to show it. The `ctrl+b` toggle still works, but the user has no visual hint.

**Fix:** Show a brief indicator or toast when the sidebar auto-hides: "Sidebar hidden (terminal too narrow). ctrl+b to toggle."

---

### UX-39: Sidebar File Diff Opens in Full-Screen Diff Viewer

**File:** `internal/tui/sidebar.go:160-183`
**Severity:** Medium

Pressing Enter on a file in the sidebar opens a full-screen diff viewer (`DiffScreenMsg`). This is a significant context switch — the user goes from the sidebar to a completely different screen. There's no way to preview the diff inline or in a split view.

**Fix:** Consider a half-screen or split-pane diff preview that preserves the sidebar context.

---

### UX-40: Sidebar Git Status Doesn't Auto-Refresh

**File:** `internal/tui/sidebar.go:284-305`
**Severity:** Medium

The sidebar only refreshes git status on initial load and when explicitly triggered. After the user makes file changes or runs workflow tasks that modify files, the sidebar shows stale data until a `SidebarRefreshMsg` is received.

**Fix:** Trigger a sidebar refresh after:
- Workflow phase completion
- Tool execution that modifies files
- Returning from the diff screen

---

## 10. Permission & Question Modals

### UX-41: Permission Modal Auto-Deny on Timeout Without Clear Warning

**File:** `internal/tui/app_update.go:1087-1106`
**Severity:** Medium

When the permission countdown reaches zero, the request is auto-denied. The timeout is shown in the modal, but there's no audio or visual urgency cue when time is running low (e.g., last 5 seconds). The user might not realize the request will be denied.

**Fix:** Add visual urgency — flash the border red or add a pulsing effect when < 5 seconds remain.

---

### UX-42: Permission Modal Doesn't Show the Full Command

**File:** `internal/tui/app_view.go:408`
**Severity:** Medium

```go
"  Command:  "+TruncateWithEllipsis(req.Command, width-12),
```

The command displayed in the permission modal is truncated to fit the modal width. For complex Bash commands, the user may not see the full command they're being asked to approve. This is a security concern — the user can't make an informed decision.

**Fix:** Make the command scrollable or expandable. At minimum, show the full command in a multi-line format.

---

### UX-43: Question Modal Fallback Has No Timeout

**File:** `internal/tui/app_view.go:332-348`
**Severity:** Low

The fallback question renderer (when `m.questionModel` is nil) shows a simple text prompt with no timeout. If the AskUserQuestion tool has a timeout configured, it's not shown in the fallback view.

**Fix:** Include the timeout in the fallback renderer.

---

## 11. Streaming & Response Display

### UX-44: No Visual Distinction Between Thinking and Content Streaming

**File:** `internal/tui/statusbar.go:86-96`
**Severity:** Medium

The status bar shows "thinking..." and "responding..." in different styles, but the actual viewport content doesn't clearly distinguish between the two phases. When thinking blocks are auto-collapsed, the user may not realize the LLM is still in the reasoning phase.

**Fix:** Add a visual separator or background color change during the thinking phase.

---

### UX-45: Stream Cancellation Doesn't Preserve Partial Content

**File:** `internal/tui/streaming.go:86-96`
**Severity:** Medium

When the user cancels a stream with `ctrl+c`, the goroutine detects context cancellation and calls `iterator.Close()`. The partial content that was already received is discarded — it's in the `fullContent` builder inside the goroutine but never emitted.

**Fix:** Emit a `StreamDoneMsg` with the partial content on cancellation, so the user can see what was generated before they cancelled.

---

### UX-46: No Typing Indicator Before First Token Arrives

**File:** `internal/tui/repl_stream.go`
**Severity:** Low

After the user sends a message, there's a delay before the first streaming token arrives. During this time, the status bar shows "thinking..." but there's no visual indicator in the message viewport itself. The user sees their message and then nothing until the first token.

**Fix:** Show a typing indicator (e.g., animated dots) in the viewport where the response will appear.

---

## 12. Status Bar & Metadata

### UX-47: Cost Display Uses Raw Float Without Currency Context

**File:** `internal/tui/statusbar.go:117-121`
**Severity:** Low

```go
costStr := fmt.Sprintf("$%.4f", info.Cost)
```

The cost is shown as `$0.0012` which is hard to parse at a glance. Users would benefit from seeing the cost in a more readable format, like `$0.0012 (est.)` or switching to cents for small amounts.

**Fix:** Format costs more readably: `<$0.01` for very small amounts, or show cumulative session cost.

---

### UX-48: Token Count Shows "0 ctx" When No Usage Data Available

**File:** `internal/tui/statusbar.go:113-116`
**Severity:** Low

When `info.TotalTokens` is 0, the token count isn't shown (good). But if `ShowCost` is true and a request hasn't completed yet, the cost area may show `$0.0000` which is misleading.

**Fix:** Don't show cost until at least one request has completed with usage data.

---

## 13. Keyboard & Accessibility

### UX-49: `j` and `k` Scroll Keys Conflict With Text Input

**File:** `internal/tui/repl.go:205-216`
**Severity:** Medium

```go
case "j":
    if m.textarea.Value() == "" {
        m.viewport.LineDown(1)
        return nil
    }
case "k":
    if m.textarea.Value() == "" {
        m.viewport.LineUp(1)
        return nil
    }
```

The `j` and `k` vim-style scroll keys only work when the textarea is empty. This means:
1. Users who type "j" as the first character of their message will see the viewport scroll instead
2. After deleting all text, the cursor is still in the textarea, so `j`/`k` still type characters

This creates an inconsistent feel where the same key does different things depending on subtle state.

**Fix:** Either remove `j`/`k` scrolling entirely (since `ctrl+u`/`ctrl+d` exist) or require a more explicit "normal mode" toggle.

---

### UX-50: Leader Key Has No Visual Feedback on Activation

**File:** `internal/tui/keybindings.go:118-123`
**Severity:** Medium

When the user presses `ctrl+x` to activate the leader key, the only feedback is the which-key overlay that appears. But the overlay is positioned above the input area and may be missed. The status bar shows "ctrl+x" in the center zone, but this is subtle.

**Fix:** Add a more prominent visual indicator — flash the status bar brand color, or show a brief "Leader active" toast.

---

### UX-51: No Way to Copy Error Messages

**Severity:** Medium

When an error appears in the REPL (e.g., "Error: provider unreachable"), the user can't easily copy it. `ctrl+y` copies the last assistant message, but error messages are added as assistant messages with the "Error: " prefix. The user would need to copy the entire message and then extract the error.

**Fix:** Add a `/copy-error` command or make error messages selectable.

---

## 14. Session Management UX

### UX-52: Session Resume Screen Shows Truncated IDs Without Context

**File:** `internal/tui/resume_view.go:87-92`
**Severity:** Medium

```go
shortID := info.ID
if len(shortID) > 12 {
    shortID = shortID[:12]
}
```

Session IDs are truncated to 12 characters, but there's no label, goal, or other identifying information shown. The user sees:

```
▶ a1b2c3d4e5f6  or · idle · 5 msgs · 2h ago
```

Without knowing what the session was about, it's hard to choose which session to resume.

**Fix:** Show the session goal or label (if available) alongside the truncated ID. The `SessionInfo` struct already has a `Label` field that could be displayed.

---

## 15. Toast & Notification System

### UX-53: Toast Expiry Removes Wrong Toast Under Rapid Arrival

**File:** `internal/tui/app_update.go:305-309`
**Severity:** Medium

```go
case ToastExpiryMsg:
    if len(m.toasts) > 0 {
        m.toasts = m.toasts[1:]
    }
```

When toasts arrive rapidly, each toast creates a timer that emits `ToastExpiryMsg`. But the handler always removes the oldest toast (index 0), regardless of which toast's timer actually fired. If toast A's 3-second timer fires after toast B was added, toast B gets removed instead of toast A.

**Fix:** Associate each toast with a unique ID and remove by ID, or use a per-toast expiry check.

---

### UX-54: Toasts Hidden Below Compact Width Without Alternative

**File:** `internal/tui/app_view.go:42-45`
**Severity:** Low

```go
if len(m.toasts) > 0 && m.width >= WidthCompact {
    toastOverlay = renderToastStack(m.toasts, t, m.width)
}
```

On narrow terminals (< 60 cols), toasts are completely hidden. There's no alternative notification mechanism.

**Fix:** On narrow terminals, show the most recent toast inline above the input area.

---

## 16. Responsive Design & Terminal Sizing

### UX-55: Ultra-Compact Mode (< 40 cols) Shows Only Active Screen

**File:** `internal/tui/app_view.go:37-39`
**Severity:** Low

```go
if m.width < WidthUltraCompact {
    return m.renderActiveScreen()
}
```

Below 40 columns, all chrome (header, sidebar, status bar, toasts, transitions) is removed. The user sees only the active screen content. While this is a reasonable degradation, there's no indication to the user that they're in a degraded mode or that resizing the terminal will reveal more features.

**Fix:** Show a brief "Narrow terminal — resize for full UI" message when first entering ultra-compact mode.

---

## 17. Prioritized Action Plan

### P0 — Immediate Impact (fix this week)

| ID | Issue | Effort | Impact |
|---|---|---|---|
| UX-14 | Silent failures in critical operations | 2h | Very High |
| UX-15 | Generic error messages in REPL | 1h | High |
| UX-20 | Execute screen shows minimal tool output | 4h | High |
| UX-01 | No API key validation in first-run | 3h | High |
| UX-33 | Settings tab/field key conflict | 2h | High |

### P1 — High Value (fix this sprint)

| ID | Issue | Effort | Impact |
|---|---|---|---|
| UX-06 | "Loading..." on initial render | 1h | Medium |
| UX-07 | No empty state after /clear | 1h | Medium |
| UX-16 | Provider fallback happens silently | 1h | Medium |
| UX-17 | No feedback when engine not initialized | 1h | Medium |
| UX-23 | Verify heal always targets first task | 3h | Medium |
| UX-42 | Permission modal truncates command | 2h | Medium |
| UX-45 | Stream cancellation loses partial content | 2h | Medium |
| UX-52 | Session resume shows no context | 2h | Medium |
| UX-53 | Toast expiry removes wrong toast | 1h | Medium |

### P2 — Polish (fix this month)

| ID | Issue | Effort | Impact |
|---|---|---|---|
| UX-02 | Skip flow confusing model picker | 1h | Medium |
| UX-10 | Scroll-to-bottom indicator too subtle | 2h | Medium |
| UX-11 | History navigation loses typed input | 1h | Low |
| UX-26 | No breadcrumb/navigation history | 4h | Medium |
| UX-28 | Inconsistent loading placeholders | 2h | Low |
| UX-29 | No fuzzy matching in palette | 3h | Medium |
| UX-34 | API key edit has no validation | 2h | Medium |
| UX-38 | Sidebar auto-hides without explanation | 1h | Medium |
| UX-40 | Sidebar git status doesn't auto-refresh | 2h | Medium |
| UX-41 | Permission timeout lacks urgency cue | 1h | Medium |
| UX-44 | No visual distinction thinking vs content | 2h | Medium |
| UX-49 | j/k scroll conflicts with text input | 1h | Medium |
| UX-50 | Leader key has no visual feedback | 1h | Medium |

### P3 — Nice to Have (backlog)

| ID | Issue | Effort | Impact |
|---|---|---|---|
| UX-03 | No default provider indicator | 1h | Low |
| UX-04 | Keychain toggle lacks visual focus | 1h | Low |
| UX-05 | Starfield may flicker on slow terminals | 1h | Low |
| UX-08 | makeAssistantMsg used for user messages | 1h | Low |
| UX-09 | Shell commands produce no feedback | 2h | Low |
| UX-12 | Mention autocomplete no dismiss on backspace | 1h | Low |
| UX-13 | Quick actions push input area down | 2h | Low |
| UX-18 | Config validation errors developer-facing | 2h | Low |
| UX-19 | Plan screen lacks duration estimate | 1h | Low |
| UX-21 | Execute progress bar too narrow | 1h | Low |
| UX-22 | Self-heal uses raw emoji | 30m | Low |
| UX-24 | Ship screen no diff preview | 3h | Low |
| UX-25 | Discuss timer hidden until last 30s | 30m | Low |
| UX-27 | Screen transitions skip for some screens | 1h | Low |
| UX-30 | No character-level search highlighting | 2h | Low |
| UX-31 | Command suggestion could show multiple | 1h | Low |
| UX-32 | /help opens separate screen | 3h | Low |
| UX-35 | Settings save shows wrong path | 30m | Low |
| UX-36 | About tab has no version | 30m | Low |
| UX-37 | Sidebar "No active session" for new users | 30m | Low |
| UX-39 | Sidebar diff opens full-screen | 4h | Low |
| UX-43 | Question fallback has no timeout | 30m | Low |
| UX-46 | No typing indicator before first token | 2h | Low |
| UX-47 | Cost display hard to parse | 30m | Low |
| UX-48 | Shows $0.0000 before first request | 30m | Low |
| UX-51 | No way to copy error messages | 1h | Low |
| UX-54 | Toasts hidden on narrow terminals | 1h | Low |
| UX-55 | Ultra-compact mode no indication | 30m | Low |

---

## Summary Statistics

| Severity | Count |
|---|---|
| High | 6 |
| Medium | 23 |
| Low | 23 |
| **Total** | **52** |

| Category | Count |
|---|---|
| First-Run & Onboarding | 5 |
| REPL & Chat Interface | 8 |
| Error Handling & Feedback | 5 |
| Workflow Phase Screens | 7 |
| Navigation & Transitions | 3 |
| Command Palette & Commands | 4 |
| Settings & Configuration | 4 |
| Sidebar & Git | 4 |
| Permission & Question Modals | 3 |
| Streaming & Response | 3 |
| Status Bar & Metadata | 2 |
| Keyboard & Accessibility | 3 |
| Session Management | 1 |
| Toast & Notifications | 2 |
| Responsive Design | 1 |
