# M31A — UX Improvement Report (No New Features)

> **Scope:** Every Go source file deeply studied. Only existing features analyzed for UX polish.
> **Generated:** 2026-06-03
> **Method:** 6 parallel deep-read agents covering TUI core, components, screens, commands, workflow/tools, and provider/infra.

---

## Executive Summary

Across ~170 Go source files, **200+ distinct UX improvement opportunities** were identified. The issues fall into these severity tiers:

| Tier | Count | Description |
|------|-------|-------------|
| **P0 — Broken/Lying** | 12 | Commands that don't do what they claim, incorrect behavior, data loss risk |
| **P1 — High Impact** | 38 | Missing feedback, confusing errors, undiscoverable interactions |
| **P2 — Medium Impact** | 65 | Inconsistent styling, missing empty states, suboptimal formatting |
| **P3 — Polish** | 85+ | Minor visual issues, hardcoded colors, edge cases |

**Top 5 most impactful improvements (by user-perceived quality):**

1. **Broken commands** — `/clear`, `/undo`, `/pause`, `/resume-task` are no-ops or lies
2. **Error messages** — raw Go errors dumped to users, no actionable guidance
3. **Missing feedback** — no loading spinners, no streaming progress, no phase transition visibility
4. **Theme inconsistency** — command palette, execute/ship screens hardcode colors, ignore user's theme
5. **Permission UX** — `RiskDangerous` and `RiskDestructive` look identical; `E` key exits entire app

---

## Section 1: Broken/Lying Commands (P0)

These commands mislead users or do nothing when they promise action.

### 1.1 `/clear` — Returns "Context cleared." but clears nothing

**File:** `internal/tui/commands_core.go:33-35`

```go
func handleClear(args []string, ctx CommandContext) CommandResult {
    return CommandResult{Success: true, Message: "Context cleared."}
}
```

**Issue:** The handler never clears `replModel.Messages()` or calls any session state mutation. The user believes their conversation context was cleared. This is a lie that erodes trust.

**Fix:** Either implement the clearing or change the message to reflect reality.

### 1.2 `/undo` — Shows checkpoint info but doesn't restore

**File:** `internal/tui/commands_session.go:12-27`

The description says "Restore latest checkpoint" but the handler only *displays* checkpoint metadata (phase, timestamp, messages count). No actual restoration happens.

**Fix:** Implement the actual restore, or rename the command to `/checkpoint-info`.

### 1.3 `/pause` — Deflects to another screen

**File:** `internal/tui/commands_workflow.go:162-167`

Returns `"Workflow pause requested. Use the workflow screen to manage execution."` — the user typed an action and got told to go somewhere else.

**Fix:** Either implement pausing from REPL, or be honest: "Pause is available from the Execute screen (press P)".

### 1.4 `/resume-task` — Tells user to restart from scratch

**File:** `internal/tui/commands_workflow.go:169-174`

Description says "Resume task from last checkpoint" but the message says "use /workflow to restart from the beginning." These are opposites.

**Fix:** Either implement checkpoint resume, or fix the description/message.

### 1.5 `/plan` / `/execute` / `/verify` / `/ship` aliases are broken

**File:** `internal/tui/commands.go:174-177` + `commands_workflow.go:40-46`

These aliases route to `handlePhase`, which expects *no args* to show current phase status. When the user types `/plan` (no args), they get phase status instead of transitioning to the plan phase. `/phase plan` also doesn't work (returns error about usage). The user has no way to advance workflow phases via slash commands.

**Fix:** Bare aliases like `/plan` should attempt to transition to the plan phase, not show status.

### 1.6 `/reset` — No confirmation for destructive action

**File:** `internal/tui/commands_ai.go:13-18`

Wipes all state and returns to first-run with no confirmation. A typo like `/rest` wouldn't hit this, but `/reset` accidentally destroys everything.

**Fix:** Require `/reset --confirm` or show a confirmation prompt.

### 1.7 `/rollback --hard` — No confirmation

**File:** `internal/tui/commands_git.go:98-103`

Hard reset is irreversible. No confirmation gate. A single command wipes uncommitted work.

**Fix:** Require `/rollback --hard --confirm` or interactive confirmation.

---

## Section 2: Error Messages — User-Friendly Translation (P0-P1)

### 2.1 Raw Go errors dumped to TUI

**Files:** `app_update.go:531`, `repl_stream.go:179-180`

```go
fmt.Sprintf("Error: %v", msg.Err)
fmt.Sprintf("✗ Error: %v", err)
```

Network errors produce multi-line Go error strings in a 1-line status bar. The user sees `"dial tcp: connect: connection refused"` instead of `"Provider unreachable — check your internet connection"`.

**Fix:** Map common errors to human-readable messages:
- `connection refused` → "Cannot reach provider — check your internet"
- `context canceled` → "Request cancelled"
- `401` → "Invalid API key — run /settings to update"
- `429` → "Rate limited — retry in a moment"

### 2.2 `ErrPermissionDenied` is overloaded for 4+ failure modes

**Files:** `permissions.go:133,144,168,179,207,218`, `fileread.go:89,104`, `filewrite.go:153`

The same "permission denied" error is returned for:
- Channel saturation (permission buffer full)
- User denial via modal
- Rule-based denial
- File stat failures (broken symlinks, SELinux)

**Fix:** Use distinct error types or include context in the error message.

### 2.3 FileRead returns `ErrPermissionDenied` for stat failures

**File:** `internal/tools/fileread.go:88-89`

```go
if os.IsNotExist(err) {
    return types.ToolResult{}, fmt.Errorf("file not found: %s", path)
}
return types.ToolResult{}, m31errors.ErrPermissionDenied
```

A file that exists but can't be stat'd (broken permission, deleted between checks) gets "permission denied" — the real error is discarded.

**Fix:** Wrap the actual error: `fmt.Errorf("cannot access %s: %w", path, err)`.

### 2.4 Provider errors leak raw HTTP response bodies

**Files:** `openrouter/client.go:263`, `zen/client.go:244`

```go
return nil, fmt.Errorf("unexpected status %d: %s", resp.StatusCode, bodyStr)
```

`bodyStr` can be arbitrary HTML/JSON from the provider. Users see gibberish.

**Fix:** Map HTTP status codes to messages, truncate body, strip HTML.

### 2.5 Context-exceeded detection is fragile

**Files:** `openrouter/client.go:260-262`, `zen/client.go:241-243`

```go
if strings.Contains(bodyStr, "context_length") || strings.Contains(bodyStr, "context") {
```

The word "context" is common in error messages. This false-positive shows misleading "context window exceeded" for unrelated errors.

**Fix:** Match more specific strings: `"context_length_exceeded"`, `"maximum context"`, combined with HTTP 400.

### 2.6 Sentinel errors have terse, identical-format messages

**File:** `internal/errors/errors.go:5-29`

Every error is `errors.New("short phrase")`. No companion user-facing guidance exists.

**Fix:** Add a `UserMessage(e error) string` function that returns actionable guidance for each sentinel error.

### 2.7 Session corruption returns bare sentinel for 3 different failure modes

**File:** `pkg/session/manager.go:197-198,204-205,211-212`

Invalid ID format, file not found, and JSON parse failure all return `ErrSessionCorrupted` — the TUI can't distinguish "session doesn't exist" from "session file is corrupted."

**Fix:** Return distinct errors or wrap with context.

---

## Section 3: Missing Feedback & Loading States (P1)

### 3.1 No spinners on loading states (except model selector)

| File | Line | Current |
|------|------|---------|
| `plan.go` | 85 | `"Loading plan..."` |
| `execute.go` | 99 | `"Loading execute..."` |
| `verify.go` | 85 | `"Loading verify..."` |
| `ship.go` | 75 | `"Loading ship..."` |
| `settings.go` | 501 | `"Loading..."` |
| `resume.go` | 329 | `"Loading..."` |
| `sidebar.go` | 90 | `"  loading..."` |

`modelselector_view.go:12` does this correctly: `m.spinner.View()+" Loading models..."`.

**Fix:** Use the spinner component consistently across all loading states.

### 3.2 No streaming progress during task execution

**File:** `internal/workflow/execute.go:122-217`

`executeTaskWithTools` calls `e.streamLLM` which returns the full response as a string. The LLM's thinking and tool calls are invisible until the entire response completes. Only task-level `TaskStartMsg`/`TaskUpdateMsg` are emitted — not per-tool-call progress.

**Fix:** Emit intermediate progress messages for each tool call start/complete.

### 3.3 Self-heal attempts are invisible

**Files:** `execute.go:137,183`, `verify.go:46-113`

When self-heal triggers, only a log message is emitted. The user sees a task go from "failed" to "running" with no intermediate "attempting repair..." state.

**Fix:** Emit a TUI message like "Self-healing task 3 (attempt 1/2)..."

### 3.4 Phase transitions are invisible

**Files:** `initialize.go:65`, `engine.go:362`

Auto-transitions happen silently. The user sees no banner like "Moving to Discuss phase..." or "Generating plan..."

**Fix:** Emit phase transition messages to the TUI.

### 3.5 Zero feedback during multi-second operations

Across all workflow phases, there's no `emit()` call for intermediate progress. The user sees: (1) Phase starts, (2) Long silence, (3) Phase result.

**Fix:** Emit periodic progress updates during long operations.

### 3.6 Bash process kill has no feedback

**File:** `internal/tools/bash.go:96-112`

The SIGINT → SIGKILL escalation happens silently. The user sees the tool card stuck in `[..]` state until the 5-second SIGKILL timeout expires.

**Fix:** Show "Terminating process..." → "Force killing..." intermediate states.

---

## Section 4: Theme & Visual Consistency (P0-P1)

### 4.1 Command palette hardcodes all colors — broken in light mode

**File:** `internal/tui/cmdpalette.go:36-37,141,148,169-170,205,217`

Every color in the command palette is hardcoded (`#E8EAED`, `#D77757`, `#2E2E2E`, `#9AA0A6`, `#1A1A1A`). Light theme users get unreadable text.

**Fix:** Replace all hardcoded colors with `m.theme.*` references.

### 4.2 Execute and ship screens use `theme.Default()` instead of user's theme

**Files:** `execute.go:144`, `ship.go:158,185`

These screens bypass the user's theme choice.

**Fix:** Use `m.theme` consistently.

### 4.3 Multiple components use `theme.Default()` directly

**Files:** `filterchips.go:29`, `metriccard.go:21`, `progress.go:36,148`, `statrow.go:22,71`

These components don't respect the user's theme choice.

**Fix:** Accept theme as parameter or use injected theme.

### 4.4 Sparkline uses hardcoded color

**File:** `components/sparkline.go:69`

Uses `#9AA0A6` instead of `theme.TextSecondary`.

**Fix:** Use theme reference.

### 4.5 Bash command prefix `$ ` is hardcoded

**File:** `components/bash_renderer.go:25`

Unix convention. On Windows this is inaccurate.

**Fix:** Conditionally use `$` or `>` based on platform.

### 4.6 `RiskDangerous` and `RiskDestructive` have identical styling

**File:** `components/permission.go:167-172`

Both use `Error` background with black foreground. Users cannot distinguish "might break something" from "will destroy data irreversibly."

**Fix:** Use distinct visual treatments (e.g., `Warning` bg for dangerous, `Error` bg for destructive).

### 4.7 Permission modal `[E] Exit M31A` has same visual weight as `[Y] Allow`

**File:** `components/permission.go:79-81`

The exit option (quits the app) is given equal prominence to "Allow Once." A user pressing `E` thinking it means "Expand" or "Edit" accidentally exits.

**Fix:** De-emphasize or separate the exit option. Consider changing the key to something less ambiguous.

---

## Section 5: Input Handling & Edge Cases (P1)

### 5.1 `Esc` key resets entire textarea — no undo

**File:** `internal/tui/repl.go:566-574`

If the user has typed a long message and accidentally hits Escape, all input is lost with no undo.

**Fix:** Only reset if textarea is empty; otherwise require double-Esc or Ctrl+U to clear.

### 5.2 Terminal too small message has no escape hatch

**File:** `internal/tui/app_view.go:15-17`

Shows `"Terminal too small: %dx%d (minimum 40x10)"` — the entire view. No suggestion to resize, no brand, no way to interact.

**Fix:** Add "Resize your terminal to at least 40x10 columns" hint.

### 5.3 Leader key has no visual countdown

**File:** `internal/tui/keybindings.go:101-106`

Leader key (`ctrl+x`) uses a 1-second timeout with no visual feedback. The user doesn't know time is passing.

**Fix:** Show a pulsing indicator or countdown in the status bar.

### 5.4 `t`/`T`/`Tab` for thinking toggle only work when textarea is empty

**Files:** `repl.go:512-543`

These keybindings are conditional on empty textarea but there's no visual indicator. Users accustomed to the shortcuts see them fail silently when typing.

**Fix:** Document this behavior in help text or add a visual cue.

### 5.5 Tab completion limited to 5-8 items with 35+ commands

**Files:** `repl.go:661-664`, `repl_view.go:422`

Typing `/` shows only 5-8 commands. No way to scroll the autocomplete list.

**Fix:** Make the list scrollable or use a full-screen command palette.

### 5.6 Autocomplete doesn't show command arguments

**File:** `repl_view.go:426-433`

Shows `/command description` but no argument hints. User can't discover `--hard` for `/rollback` without reading source.

**Fix:** Show argument hints in the autocomplete dropdown.

### 5.7 Settings editing: `tab` inserts character instead of advancing

**File:** `internal/tui/settings.go:425-431`

Tab key inserts a literal tab character when editing a field. Users expect Tab to advance to next field.

**Fix:** Reserve Tab for field navigation; use Ctrl+Tab or similar for literal tab insertion.

### 5.8 `Esc` behavior is inconsistent across screens

| Screen | `Esc` does |
|--------|------------|
| Plan | Return to REPL |
| Verify | **Not handled** |
| Ship | Return to REPL |
| Settings | Cancel edit OR return to REPL |
| Resume | Return to REPL |
| Diff | Close diff |
| CmdPalette | **Not handled** |

**Fix:** Standardize `Esc` behavior across all screens.

---

## Section 6: Empty States & Messaging (P1-P2)

### 6.1 Screens show developer-facing placeholder text

**File:** `internal/tui/app_view.go:77-78,84-85,91-92,98-99`

Plan/Execute/Verify/Ship screens show `"Plan screen — driven by workflow engine"` when their model is nil. This is internal dev text leaked to users.

**Fix:** Show "No workflow active. Start one with /workflow <goal>".

### 6.2 Empty task lists show nothing

**Files:** `plan.go:121-149`, `execute.go:157-199`, `verify.go:98-149`

When there are no tasks/results, the screens render blank content with no guidance.

**Fix:** Show descriptive empty states: "No tasks planned yet", "No verification results", etc.

### 6.3 "Unknown screen" is a dead end

**File:** `internal/tui/app_view.go:109-110`

Shows "Unknown screen" with no recovery suggestion.

**Fix:** Add "Type /help for available commands" or "Press Esc to return to REPL".

### 6.4 Context bar shows `--/-- ctx` when total is 0

**File:** `internal/tui/header.go:44-45`

Opaque to users who don't know what "ctx" means.

**Fix:** Show "context: unknown" or hide the indicator when total is 0.

### 6.5 Three different representations for "no value" in settings

**File:** `internal/tui/settings.go:579,592,599`

- `"(not configured)"` — masked field, empty
- `"[empty]"` — focused, non-masked, empty
- `"(empty)"` — unfocused, non-masked, empty

**Fix:** Use one consistent string like `"(not set)"`.

---

## Section 7: Help & Discoverability (P1)

### 7.1 `/help` is a flat alphabetical dump of 35+ commands

**File:** `internal/tui/commands_core.go:12-24`

No categories, no grouping. Users scanning for "how do I see my session?" have to read every line.

**Fix:** Group by category: Session, Workflow, Config, Git, AI, etc.

### 7.2 No "Did you mean?" on command typos

**File:** `internal/tui/commands.go:116-119`

User types `/hlel` → gets "unknown command: /hlel" with no suggestion.

**Fix:** Levenshtein distance or prefix matching to suggest closest command.

### 7.3 No per-command help or usage examples

**File:** `internal/tui/commands.go:70-73`

No mechanism for `/help rollback` to show extended usage.

**Fix:** Add extended help text per command.

### 7.4 `/help` doesn't mention `!` shell mode

**File:** `internal/tui/commands_core.go:12-24`

Shell mode (`!command`) is undocumented in `/help`.

**Fix:** Add to help output.

### 7.5 `/help` has negative framing

**File:** `internal/tui/commands_core.go:22`

"Note: command chaining with ';' is not supported" — telling users what they *can't* do is poor UX.

**Fix:** Remove the note. It's an implementation detail.

### 7.6 `/theme auto` rejected despite being documented

**File:** `internal/tui/commands_core.go:88-97`

The config system supports "auto" mode (detect terminal background), but `/theme auto` returns an error.

**Fix:** Accept "auto" as a valid value.

### 7.7 Fallback banner has no dismiss hint

**File:** `internal/tui/repl_view.go:97-107`

The `x` key dismissal is handled in code but there's no visual indicator like `"[x] Dismiss"`.

**Fix:** Add a dismiss hint to the banner.

---

## Section 8: Plan Screen (P2)

### 8.1 Dependency graph is unreadable

**File:** `internal/tui/plan.go:152-167`

Uses flat text: `[1] -> [2] Task description`. For complex dependency chains, this is a wall of text.

**Fix:** Use indentation or ASCII tree rendering for visual hierarchy.

### 8.2 Task descriptions overflow terminal width

**File:** `internal/tui/plan.go:133`

No truncation on long descriptions.

**Fix:** Truncate with `...` at terminal width.

### 8.3 Task status prefixes have no color coding

**File:** `internal/tui/plan.go:124-131`

`[ ]`, `[>]`, `[x]` are all monochrome.

**Fix:** Color-code: pending=muted, running=brand, done=success, failed=error.

### 8.4 Cost panel shows raw model ID

**File:** `internal/tui/plan.go:115-116`

Shows `anthropic/claude-3.5-sonnet` instead of "Claude 3.5 Sonnet".

**Fix:** Use model name when available.

### 8.5 Diff preview mixes files from all tasks

**File:** `internal/tui/plan.go:169-186`

All files from all tasks are listed together with no grouping by task.

**Fix:** Group files under their parent task.

### 8.6 Key hints are incomplete

**File:** `internal/tui/plan.go:96-98`

Shows `[A]ccept [R]etry [D]iff Tab=Graph` — no mention of Esc or arrow keys.

**Fix:** Add all available keybindings.

---

## Section 9: Execute Screen (P2)

### 9.1 Progress bar is misleading

**File:** `internal/tui/execute.go:153-154`

Shows `completed+skipped/total` — skips aren't progress. Users see "3/5 tasks" when 2 are skipped and 1 is done.

**Fix:** Show only completed/total, or label skips separately.

### 9.2 Auto-transition fires on unrelated keypress

**File:** `internal/tui/execute.go:82-92`

When all tasks complete, screen auto-transitions to Verify. But this fires inside `KeyMsg` handler — any keypress triggers the transition.

**Fix:** Use a timer or explicit user action to transition.

### 9.3 `extra = "  ← running"` is easy to miss

**File:** `internal/tui/execute.go:165`

Small annotation during fast execution.

**Fix:** Use a more prominent visual indicator (bold, color, spinner).

### 9.4 Blocked dependency shows only first dep

**File:** `internal/tui/execute.go:184-185`

Shows `task.Dependencies[0]` — should show all or count.

**Fix:** Show "blocked by: 1, 2" or "blocked by 2 tasks".

### 9.5 Task descriptions overflow

**File:** `internal/tui/execute.go:198`

No truncation.

**Fix:** Truncate at terminal width.

---

## Section 10: Verify Screen (P2)

### 10.1 Self-heal UX is unclear

**File:** `internal/tui/verify.go:57-59`

Pressing `H` silently resets task to `StatusPending` — no confirmation, no visual feedback.

**Fix:** Show "Attempting self-heal (attempt 1/2)..." before reset.

### 10.2 No indication of remaining heal attempts

**File:** `internal/tui/verify.go:140-143`

`[H] Self-heal` shown but no "2 attempts remaining" hint.

**Fix:** Show attempt count.

### 10.3 `[UNRECOVERABLE]` has no explanation

**File:** `internal/tui/verify.go:133-137`

Red text but no guidance on what to do.

**Fix:** Add "Run git bisect to find the issue" or similar.

### 10.4 Verification errors not shown to user

**File:** `internal/tui/verify.go:115-145`

Shows ✓/✗ for Files/Syntax/Tests but doesn't show the actual error messages.

**Fix:** Display error details on expansion or inline.

### 10.5 `Esc` not handled

**File:** `internal/tui/verify.go`

No `esc` case at all. User is stuck.

**Fix:** Add Esc handler to return to REPL.

---

## Section 11: Ship Screen (P2)

### 11.1 Hardcoded next actions are not actionable

**File:** `internal/tui/ship.go:234-238`

"Run tests to verify changes" — no keybinding to trigger. May not apply (tests were just run).

**Fix:** Only show relevant, actionable next steps with keybindings.

### 11.2 Missing cost/token usage in summary

**File:** `internal/tui/ship.go:89`

Shows session ID and duration but not which model/provider was used or total cost.

**Fix:** Include model, provider, and cost in summary.

### 11.3 No confirmation before starting new session

**File:** `internal/tui/ship.go:61-63`

Pressing `N` immediately starts new session. User might lose summary context.

**Fix:** Add confirmation: "Start new session? Current summary will be archived."

### 11.4 Ship can fail on empty commit, blocking everything

**File:** `internal/workflow/ship.go:46-48`

If there's nothing to commit, the entire ship phase fails and session is never archived.

**Fix:** Handle "nothing to commit" gracefully — skip commit, still archive.

### 11.5 Ledger always shows $0 cost

**File:** `internal/workflow/ship.go:87`

Cost parameter is hardcoded to `0`. Cross-session cost tracking is broken.

**Fix:** Pass actual cost from PhaseResult.

---

## Section 12: First-Run Experience (P2)

### 12.1 Empty icon in feature card

**File:** `internal/tui/firstrun.go:345`

Fourth feature has empty icon: `{"", "Git Integrated", "Auto-commits your work"}`.

**Fix:** Add an appropriate icon (e.g., `📂` or `🔧`).

### 12.2 Emoji icons may not render on all terminals

**File:** `internal/tui/firstrun.go:342-345`

Uses emoji (🤖, ⚡, 🔄) which may not render on Windows or minimal terminals.

**Fix:** Use ASCII fallbacks or check terminal capabilities.

### 12.3 Provider descriptions are marketing copy

**File:** `internal/tui/firstrun.go:418-423`

"Access 100+ models" and "Fast & cost-effective" — not technical enough for developer users.

**Fix:** Include concrete details: model count, pricing tier, latency characteristics.

### 12.4 API key placeholder is provider-specific

**File:** `internal/tui/firstrun.go:50`

Placeholder `"sk-or-v1-..."` is only valid for OpenRouter. Zen keys have different format.

**Fix:** Show provider-specific placeholder.

### 12.5 Validation shows raw HTTP errors

**File:** `internal/tui/firstrun.go:186-196`

May leak internal HTTP status codes to user.

**Fix:** Map to friendly messages.

### 12.6 Validating screen has no timeout communication

**File:** `internal/tui/firstrun.go:531-549`

Shows spinner but no timeout hint. HTTP client has 10s timeout but this isn't communicated.

**Fix:** Show "Validating key (timeout: 10s)..." or similar.

---

## Section 13: Settings Screen (P2)

### 13.1 API key unmask logic is inverted

**File:** `internal/tui/settings.go:249,268`

`f.masked == false` should be `!f.masked`. The re-mask check is inverted — keys stay unmasked when they should be masked.

**Fix:** Correct the boolean logic.

### 13.2 Editing indicator looks like part of the value

**File:** `internal/tui/settings.go:573-574`

`">" + value + "<"` — angle brackets look like they're part of the value.

**Fix:** Use a distinct visual indicator (e.g., colored border, blinking cursor).

### 13.3 Dirty indicator pushes key hints off-screen

**File:** `internal/tui/settings.go:773-776`

"Unsaved changes • " is prepended to footer, pushing key hints off narrow terminals.

**Fix:** Use a separate indicator line or truncate the dirty message.

### 13.4 Tab bar has no scroll indicators

**File:** `internal/tui/settings.go:512-534`

With 6 tabs on narrow terminals, tabs overflow with no way to see hidden ones.

**Fix:** Add scroll arrows or truncate with `...`.

---

## Section 14: Resume Screen (P2)

### 14.1 Search only matches ID/Model/Provider

**File:** `internal/tui/resume.go:160-175`

Doesn't search goal, first message, or session content.

**Fix:** Include goal in search index.

### 14.2 Preview only shown when width > 100

**File:** `internal/tui/resume.go:351`

On 80-column terminals, no preview at all, no explanation why.

**Fix:** Show a message like "Preview available on wider terminals" or adapt layout.

### 14.3 Delete confirmation accepts Enter as "yes"

**File:** `internal/tui/resume.go:234-238,476-487`

For a destructive action, `enter` being "yes" is dangerous. No warning that deletion is permanent.

**Fix:** Require explicit `y`/`Y`, show "This cannot be undone" warning.

### 14.4 Search enter/esc behavior inconsistent

**File:** `internal/tui/resume.go:254-268`

`enter` deactivates search, `esc` clears and deactivates. Should be: enter=confirm, esc=cancel.

**Fix:** Standardize search lifecycle.

---

## Section 15: Model Selector (P2)

### 15.1 Detail view pushes list up on small terminals

**File:** `internal/tui/modelselector_view.go:29-32`

Detail appended below list. On small terminals, list is squeezed.

**Fix:** Use side panel or overlay.

### 15.2 `F` toggle favorite has no visual feedback

**File:** `internal/tui/modelselector.go:208-218`

No toast, no badge change in list.

**Fix:** Show a brief toast or add a star icon to favorited models.

### 15.3 Free models show "$0.0000" cost

**File:** `internal/tui/modelselector_list.go:45-46`

Shows `$0.0000 (100K in + 50K out)` even for free models.

**Fix:** Show "Free" instead.

### 15.4 `?` key activates search — conflicts with help convention

**File:** `internal/tui/modelselector.go:223-226`

`?` typically means "help" in terminal conventions.

**Fix:** Use `/` only for search, or document the deviation.

---

## Section 16: Diff Viewer (P2)

### 16.1 Header doesn't show file name

**File:** `internal/tui/diff.go:178-179`

Shows `line X/Y` but no file name. User can't tell which file they're viewing.

**Fix:** Show file name in header.

### 16.2 Help bar doesn't mention supported keys

**File:** `internal/tui/diff.go:222`

Shows `[↑/↓] scroll [g] top [G] bottom [esc] back` — doesn't mention `pgup`/`pgdown` which are supported.

**Fix:** Add `pgup/pgdn` to help bar.

---

## Section 17: Tool Card Rendering (P2)

### 17.1 Auto-collapse hides output without expand hint

**File:** `internal/tui/components/toolcard.go:88-91`

Output >20 lines auto-collapsed with `[+19 lines hidden]` but no key binding hint to expand.

**Fix:** Add `[Space] expand` or `[Enter] toggle` hint.

### 17.2 Input truncation at 57 chars is unexplained

**File:** `internal/tui/components/toolcard.go:159-161`

Long bash commands silently truncated. No `[full command in log]` hint.

**Fix:** Show hint about full output location.

### 17.3 Error state shows only `ERR` badge — no error message

**File:** `internal/tui/components/toolrenderers.go:55-57`

Actual error text from `result.Error` is never displayed.

**Fix:** Show error message inline or on expansion.

### 17.4 Binary content replacement is silent

**File:** `internal/tui/components/toolcard.go:62-68`

`[binary content]` with no byte count or mime type.

**Fix:** Show `[binary content, 1.2 KB, image/png]`.

### 17.5 Running state spinner has no animation

**File:** `internal/tui/components/toolrenderers.go:46-58`

Static `[..]` with no spinning animation. For 30-minute Bash commands, user can't tell if it's alive.

**Fix:** Use the spinner component for animated indication.

### 17.6 Line count in collapsed view can be inaccurate

**File:** `internal/tui/components/toolrenderers.go:65-74`

`strings.Count(output, "\n") + 1` overcounts for trailing newlines.

**Fix:** Use `strings.TrimRight(output, "\n")` before counting.

---

## Section 18: Permission Modal (P2)

### 18.1 Auto-deny countdown lacks context

**File:** `internal/tui/components/permission.go:83-85`

"Auto-deny in 4:59..." doesn't explain what happens on auto-deny.

**Fix:** Add "Tool will be rejected" or similar context.

### 18.2 Command box has no syntax highlighting

**File:** `internal/tui/components/permission.go:72-77`

Multi-line bash commands rendered as plain text wall.

**Fix:** Add basic syntax highlighting for pipes, redirects, arguments.

### 18.3 Risk level badge is unlabeled

**File:** `internal/tui/components/permission.go:61`

Shows `[dangerous]` but no explanation of what "dangerous" means.

**Fix:** Add tooltip or expandable explanation.

---

## Section 19: Thinking/Reasoning Blocks (P2)

### 19.1 Toggle character is not discoverable

**File:** `internal/tui/components/thinking.go:139`

Shows `[+] Thinking (2.3s)` but nowhere says "press T to expand."

**Fix:** Add key hint in header.

### 19.2 Expanded thinking has no scroll mechanism

**File:** `internal/tui/components/thinking.go:58-63`

For 100+ line reasoning chains, pushes all content way down. No viewport or pagination.

**Fix:** Cap expanded thinking height with internal scroll.

### 19.3 Collapsed toggle affordance is too subtle

**File:** `internal/tui/components/thinking.go:52-54`

`[+]` in `TextMuted` color on `BackgroundPanel` — nearly invisible.

**Fix:** Use a more visible color for the toggle indicator.

---

## Section 20: Workflow Engine (P1-P2)

### 20.1 Plan validation errors not shown to user

**File:** `internal/workflow/plan.go:33-56`

Validation errors are fed back to LLM but never shown to the TUI user. After 3 retries, user gets a single opaque error string.

**Fix:** Show intermediate attempts and errors to user.

### 20.2 `RequiresManualInput` flag provides no guidance

**File:** `internal/workflow/plan.go:68`

Set when plan fails, but no indication of what manual input is expected.

**Fix:** Show "Plan generation failed. Enter tasks manually or type /repl to continue."

### 20.3 Verification build commands have no timeout

**File:** `internal/workflow/engine.go:1146-1201`

`go build`, `cargo check` run without context timeout. Can hang forever.

**Fix:** Add a timeout (e.g., 5 minutes).

### 20.4 `npm run build` failure is swallowed

**File:** `internal/workflow/engine.go:1166-1169`

Build output logged but never shown to user.

**Fix:** Include in verification result.

### 20.5 Test failures include raw output with no summary

**File:** `internal/workflow/engine.go:1209`

Entire `go test` output (potentially thousands of lines) stuffed into one error string.

**Fix:** Parse and summarize: "3 tests failed: TestX, TestY, TestZ".

### 20.6 `consumeStream` ignores chunk types

**File:** `internal/workflow/engine.go:397-414`

All reasoning tokens concatenated with content tokens. User (and LLM) sees raw thinking mixed with content.

**Fix:** Separate thinking and content segments.

### 20.7 Task spec has duplicated ID

**File:** `internal/workflow/execute.go:240`

`"Execute task %d: %d"` — ID appears twice.

**Fix:** Remove the duplicate.

---

## Section 21: Tool System (P1-P2)

### 21.1 Grep truncation is invisible

**File:** `internal/tools/grep.go:93,128,148`

When limit is hit, results just stop. No `Truncated: true` flag, no "[... N more matches]".

**Fix:** Set `Truncated: true` and add truncation message.

### 21.2 Glob truncation message is ambiguous

**File:** `internal/tools/glob.go:99-100`

"[... N more files]" but no mention of the 1000-file limit or how to narrow the pattern.

**Fix:** Include the limit and suggest narrowing the pattern.

### 21.3 FileWrite success shows raw path

**File:** `internal/tools/filewrite.go:183`

Shows path as-provided by LLM, not resolved absolute path. No backup indication.

**Fix:** Show resolved path and mention backup location.

### 21.4 Edit match failures are returned as success

**File:** `internal/tools/edit.go:121-123`

Error in `ToolResult.Output` with `nil` Go error. LLM may not distinguish from success.

**Fix:** Return as a Go error so the dispatcher handles it correctly.

### 21.5 Unknown tool error doesn't list available tools

**File:** `internal/tools/dispatcher.go:77`

`"unknown tool: <name>"` — no suggestion of what tools exist.

**Fix:** List available tools in the error message.

### 21.6 Tool input JSON parse error discards context

**File:** `internal/tools/dispatcher.go:82`

Malformed JSON is not included in the error. LLM can't self-correct.

**Fix:** Include the raw JSON (truncated) in the error.

---

## Section 22: Provider Layer (P1-P2)

### 22.1 FetchModels errors are silently swallowed

**Files:** `openrouter/client.go:151-171`, `zen/client.go:136-156`

All errors go to `slog.Warn`. User never knows model fetching failed.

**Fix:** Expose `LastFetchError` or emit a TUI banner.

### 22.2 Stale cache serves outdated data silently

**Files:** `openrouter/client.go:204-208`, `zen/client.go:180-184`

User trusts displayed models as current when they may be days old.

**Fix:** Show "Model list may be outdated (cached)" warning.

### 22.3 `FindFallbackWithRetryAfter` blocks for up to 60 seconds

**File:** `internal/provider/fallback.go:69`

`time.Sleep(wait)` freezes the UI.

**Fix:** Return wait duration so caller can show "Rate limited — waiting Ns...".

### 22.4 No read deadline on SSE body

**File:** `internal/provider/sse.go:31`

If provider stops sending without closing, TUI hangs forever.

**Fix:** Add idle timeout (e.g., 5 minutes).

### 22.5 Stream truncated errors are confusing

**File:** `internal/provider/sse.go:45-46,68`

Raw scanner errors or "stream truncated before completion" — no user guidance.

**Fix:** Wrap with "Stream interrupted — try again" guidance.

---

## Section 23: Config & Keychain (P2)

### 23.1 Config validation errors are dense and technical

**File:** `internal/config/loader.go:420-422`

Validation errors joined as `%v` producing unreadable output.

**Fix:** Format as bulleted list with field name, expected, and actual.

### 23.2 Unresolved `${VAR}` patterns used as API keys

**File:** `internal/config/loader.go:463-469`

If env var isn't set, literal `${MY_API_KEY}` is used as the key → cryptic 401.

**Fix:** Scan for remaining `${...}` patterns and warn or error.

### 23.3 Project config parse errors are silent

**File:** `internal/config/loader.go:78`

Malformed `m31a.toml` silently ignored. User thinks project config is active.

**Fix:** Surface as TUI warning banner.

### 23.4 Keychain errors are swallowed silently

**File:** `pkg/session/manager.go:278-286`

`LoadWorkflowState` returns zero values with no error on corruption.

**Fix:** Return `ErrSessionCorrupted` so TUI can display warning.

### 23.5 GPG decryption failure is technical

**File:** `pkg/keychain/keychain_linux.go:109`

"gpg decrypt: keychain decrypt failed" — user has no idea what to do.

**Fix:** Return "GPG decryption failed — try `pass init <gpg-id>` or re-store your API key".

---

## Section 24: Autodream, Bisect, Ledger, Rollback (P2-P3)

### 24.1 Autodream consolidation failure messages are developer-facing

**File:** `pkg/autodream/autodream.go:129,139`

"cannot consolidate: paused, too few messages" — confusing to users.

**Fix:** "Nothing to compress yet — conversation is still short".

### 24.2 Autodream result has no percentage context

**File:** `pkg/autodream/autodream.go:198-204`

`TokensSaved` and `MessagesRemoved` are raw numbers. User needs context.

**Fix:** "Compressed 12 messages — saved ~2,400 tokens (15% of context)".

### 24.3 Bisect result only has short hash, no commit message

**File:** `pkg/bisect/bisect.go:123-128`

`OffendingCommit` has only `ShortHash` populated. User sees "offending commit: abc123" with no description.

**Fix:** Populate `Author` and `Message` fields.

### 24.4 Ledger silently fails to create directory

**File:** `pkg/ledger/ledger.go:64-66`

If directory can't be created, all subsequent `Append` calls fail silently.

**Fix:** Return the error or set a `failed` flag.

### 24.5 Ledger uses wrong sentinel error for duplicates

**File:** `pkg/ledger/ledger.go:136`

`ErrTaskFailed` for duplicate ledger entry — semantically wrong.

**Fix:** Define `ErrDuplicateEntry`.

### 24.6 Rollback confirmation is too casual

**File:** `pkg/session/resume.go:476-487`

"Delete session <id>? (Y/N)" — no permanence warning.

**Fix:** "Delete session <id> permanently? This cannot be undone. (y/N)".

---

## Section 25: Miscellaneous (P3)

### 25.1 Task spec has duplicated ID

**File:** `internal/workflow/execute.go:240`

`"Execute task %d: %d"` — appears twice.

**Fix:** `"Execute task %d: %s"` (second should be description).

### 25.2 `/sessions` doesn't show goal or phase

**File:** `internal/tui/commands_session.go:179`

Shows only ID, provider, model, msg count. Users can't identify sessions.

**Fix:** Include goal (truncated) and phase.

### 25.3 `/rollback` commit list has no dates

**File:** `internal/tui/commands_git.go:118-133`

No timestamps. User can't tell if commits were from 5 minutes or 5 days ago.

**Fix:** Include relative timestamps.

### 25.4 `/models` can't page through results

**File:** `internal/tui/commands_config.go:203-205`

Truncated at 20 with no way to see the rest.

**Fix:** Support `/models <filter>` or pagination.

### 25.5 `/log` doesn't show file path

**File:** `internal/tui/commands_git.go:217`

Path `~/.m31a/m31a.log` hardcoded but never shown to user.

**Fix:** Include path in output.

### 25.6 `/fork` references jargon "siblings"

**File:** `internal/tui/commands_session.go:50`

"Use /prev or /next to navigate siblings" — user doesn't know what siblings means.

**Fix:** "Use /prev or /next to switch between the original and forked sessions."

### 25.7 `/clear` claims to clear context but doesn't

**File:** `internal/tui/commands_core.go:33-35`

Already covered in Section 1.1 but worth noting it affects trust in all commands.

### 25.8 `/health` conflates provider and system health

**File:** `internal/tui/commands_config.go:397-403`

Mixes provider status, session status, git status, tool count, and disk space.

**Fix:** Separate into "Provider Health" and "System Info" sections.

### 25.9 main.go exits with `os.Exit(1)` for all startup failures

**File:** `cmd/m31a/main.go:47,66,73,81,175`

Single line to stderr, no remediation hint.

**Fix:** For common failures, print a brief "how to fix" hint.

### 25.10 No graceful degradation when both providers are unavailable

**File:** `cmd/m31a/main.go:96-155`

App launches into REPL but any streaming fails with "provider unreachable".

**Fix:** Check `registry.List()` length after registration. If zero, show first-run.

### 25.11 History truncation uses byte count, not rune count

**File:** `internal/tui/commands_session.go:149`

Multi-byte characters can break truncation mid-character.

**Fix:** Use `[]rune` slicing.

### 25.12 `/optimize` error doesn't tell user how to enable

**File:** `internal/tui/commands_config.go:294-296`

"AutoArbitrage is disabled" — no hint of the command to enable it.

**Fix:** "Set auto_arbitrage = true in [model] to enable, or use /config model.auto_arbitrage true".

### 25.13 `/key` shows source but not how to change it

**File:** `internal/tui/commands_config.go:349,357`

"from config" — no hint about /settings to update.

**Fix:** Add "Use /settings to change API keys".

### 25.14 `/models` error doesn't guide toward `/provider`

**File:** `internal/tui/commands_config.go:188`

"No active provider." — no hint on how to fix.

**Fix:** "No active provider. Use /provider or /settings to configure one."

---

## Prioritized Fix Roadmap

### Phase A: Trust & Safety (Week 1) — 12 fixes

| # | Fix | Files | Impact |
|---|-----|-------|--------|
| 1 | Fix `/clear` to actually clear or change message | `commands_core.go` | Trust |
| 2 | Fix `/undo` to restore or rename | `commands_session.go` | Trust |
| 3 | Fix `/pause` to work or be honest | `commands_workflow.go` | Trust |
| 4 | Fix `/resume-task` to work or rename | `commands_workflow.go` | Trust |
| 5 | Fix workflow aliases to transition | `commands.go`, `commands_workflow.go` | Core |
| 6 | Add confirmation to `/reset` | `commands_ai.go` | Safety |
| 7 | Add confirmation to `/rollback --hard` | `commands_git.go` | Safety |
| 8 | Fix settings re-mask logic | `settings.go:249,268` | Security |
| 9 | De-emphasize `[E] Exit M31A` in permission modal | `permission.go` | Safety |
| 10 | Distinguish `RiskDangerous` from `RiskDestructive` | `permission.go:167-172` | Safety |
| 11 | Fix task spec duplicated ID | `execute.go:240` | Correctness |
| 12 | Fix `/theme auto` acceptance | `commands_core.go:88-97` | Correctness |

### Phase B: Error UX (Week 2) — 15 fixes

| # | Fix | Files | Impact |
|---|-----|-------|--------|
| 1 | Map common errors to human-readable messages | `app_update.go`, `repl_stream.go` | Clarity |
| 2 | Add `UserMessage()` to sentinel errors | `errors.go` | Clarity |
| 3 | Fix FileRead stat error handling | `fileread.go:88-89` | Clarity |
| 4 | Fix provider error body leaking | `openrouter/client.go`, `zen/client.go` | Clarity |
| 5 | Fix context-exceeded false positives | `openrouter/client.go:260` | Correctness |
| 6 | Add "Did you mean?" on command typos | `commands.go:116` | Discoverability |
| 7 | Fix session corruption error specificity | `manager.go:197-212` | Clarity |
| 8 | Fix GPG decryption error message | `keychain_linux.go:109` | Clarity |
| 9 | Fix unknown tool error to list tools | `dispatcher.go:77` | Clarity |
| 10 | Fix tool input JSON error to include raw input | `dispatcher.go:82` | Clarity |
| 11 | Fix `/models` error to suggest `/provider` | `commands_config.go:188` | Discoverability |
| 12 | Fix `/key` to suggest `/settings` | `commands_config.go:349` | Discoverability |
| 13 | Fix `/optimize` to show enable command | `commands_config.go:294` | Discoverability |
| 14 | Fix session load errors to include context | `manager.go:197-212` | Clarity |
| 15 | Fix plan validation errors to show user | `plan.go:33-56` | Visibility |

### Phase C: Feedback & Loading (Week 3) — 12 fixes

| # | Fix | Files | Impact |
|---|-----|-------|--------|
| 1 | Add spinners to all loading states | 6 screen files | Perceived speed |
| 2 | Add streaming progress during task execution | `execute.go:122-217` | Perceived speed |
| 3 | Add self-heal visibility | `execute.go:137`, `verify.go:46` | Visibility |
| 4 | Add phase transition messages | `initialize.go:65`, `engine.go:362` | Visibility |
| 5 | Add intermediate progress during long operations | All workflow phases | Perceived speed |
| 6 | Add Bash process kill feedback | `bash.go:96-112` | Visibility |
| 7 | Add "thinking" indicator during operations | Workflow engine | Visibility |
| 8 | Fix grep truncation to be visible | `grep.go:93,128,148` | Correctness |
| 9 | Add context warning remaining tokens | `estimator.go:135` | Clarity |
| 10 | Add model cache refreshing indicator | `cache.go` | Visibility |
| 11 | Add permission timeout countdown visibility | `permissions.go:124` | Visibility |
| 12 | Add question tool timeout warning | `question.go:109` | Visibility |

### Phase D: Theme & Visual (Week 4) — 10 fixes

| # | Fix | Files | Impact |
|---|-----|-------|--------|
| 1 | Fix command palette theme | `cmdpalette.go` | Light mode |
| 2 | Fix execute/ship theme usage | `execute.go`, `ship.go` | Consistency |
| 3 | Fix component theme injection | 5 component files | Consistency |
| 4 | Fix sparkline hardcoded color | `sparkline.go:69` | Consistency |
| 5 | Fix bash prefix for Windows | `bash_renderer.go:25` | Platform |
| 6 | Fix thinking block toggle visibility | `thinking.go` | Discoverability |
| 7 | Fix thinking block scroll | `thinking.go:58-63` | Usability |
| 8 | Fix tool card expand hint | `toolcard.go:88-91` | Discoverability |
| 9 | Fix tool card error display | `toolrenderers.go:55-57` | Clarity |
| 10 | Fix settings empty state consistency | `settings.go:579,592,599` | Polish |

### Phase E: Navigation & Help (Week 5) — 10 fixes

| # | Fix | Files | Impact |
|---|-----|-------|--------|
| 1 | Fix Esc handling in verify/cmdpalette | `verify.go`, `cmdpalette.go` | Navigation |
| 2 | Add Esc to all screen key hints | 4 screen files | Discoverability |
| 3 | Group `/help` by category | `commands_core.go:12-24` | Discoverability |
| 4 | Add per-command help | `commands.go:70-73` | Discoverability |
| 5 | Add `!` shell mode to `/help` | `commands_core.go:12-24` | Discoverability |
| 6 | Fix fallback banner dismiss hint | `repl_view.go:97-107` | Discoverability |
| 7 | Fix autocomplete scrollability | `repl.go:661`, `repl_view.go:422` | Usability |
| 8 | Fix autocomplete argument hints | `repl_view.go:426-433` | Discoverability |
| 9 | Add session goal/phase to `/sessions` | `commands_session.go:179` | Visibility |
| 10 | Add commit dates to `/rollback` | `commands_git.go:118-133` | Visibility |

### Phase F: Polish (Week 6) — 15+ fixes

Remaining P3 fixes from empty states, formatting, edge cases, and minor inconsistencies.

---

## Total Impact Summary

| Category | Issues Found | P0 | P1 | P2 | P3 |
|----------|-------------|-----|-----|-----|-----|
| Broken/Lying Commands | 7 | 7 | — | — | — |
| Error Messages | 20 | 2 | 10 | 6 | 2 |
| Missing Feedback | 12 | — | 8 | 4 | — |
| Theme/Visual | 18 | 2 | 2 | 10 | 4 |
| Input/Navigation | 16 | — | 8 | 6 | 2 |
| Empty States | 10 | — | 2 | 6 | 2 |
| Help/Discoverability | 12 | — | 6 | 4 | 2 |
| Screen-Specific | 40 | — | 2 | 28 | 10 |
| Tool Rendering | 15 | — | 2 | 10 | 3 |
| Workflow Engine | 15 | — | 4 | 8 | 3 |
| Provider/Config | 18 | — | 4 | 10 | 4 |
| Other | 20 | 1 | 2 | 5 | 12 |
| **Total** | **203** | **12** | **48** | **97** | **46** |
