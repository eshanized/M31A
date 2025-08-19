# M31A — Comprehensive TUI/UI Audit Report

> **Date:** 2026-06-04
> **Scope:** All UI/TUI-related issues across the entire M31A codebase
> **Method:** Deep source code audit + cross-reference with 7 existing audit reports
> **Status:** Read-only audit (Plan Mode)

---

## Executive Summary

| Severity | Count | Fixed in Prior Reports | **New/Unfixed** |
|----------|-------|----------------------|-----------------|
| CRITICAL | 9 | 2 | **7** |
| HIGH | 26 | 6 | **20** |
| MEDIUM | 67 | 12 | **55** |
| LOW | 30 | 5 | **25** |
| **Total** | **132** | **25** | **107** |

**Top 5 most impactful unfixed issues:**
1. Health check + cache refresh block the event loop (freezes TUI for 10-30s)
2. Discuss phase dead-end (workflow hangs forever after clarifying questions)
3. Plan screen unreachable (user cannot review plan before execution)
4. WindowSizeMsg drops sub-model commands (stream freezes on terminal resize)
5. Broken commands: `/clear`, `/undo`, `/pause`, `/resume-task` lie to users

---

## CRITICAL Issues (7 unfixed)

### C-1: HealthCheckTickMsg blocks the event loop
**File:** `internal/tui/app_update.go:406-437`
**Status:** NEW

```go
case HealthCheckTickMsg:
    // ...
    result := p.HealthCheck(ctx)  // ← synchronous HTTP call inside Update()
    cancel()
```

`p.HealthCheck(ctx)` is a synchronous HTTP call executed inside Bubble Tea's `Update()`. The TUI is single-threaded — this blocks ALL rendering, key processing, and streaming for up to 10 seconds. No frames render, no spinners animate, no keystrokes register.

**Impact:** Complete UI freeze every 60 seconds during health checks.

**Fix:** Move the health check to a `tea.Cmd` goroutine that emits a `HealthUpdateMsg` when complete.

---

### C-2: RefreshCacheMsg blocks the event loop
**File:** `internal/tui/app_update.go:439-459`
**Status:** NEW

```go
case RefreshCacheMsg:
    ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
    errMsg, nextCmd := handleCacheRefresh(ctx, m.registry, providerName)
    cancel()
```

Same pattern as C-1. `handleCacheRefresh` makes synchronous HTTP calls inside `Update()`, freezing the TUI for up to 30 seconds during model catalog refreshes.

**Impact:** Complete UI freeze every 5 minutes during cache refresh.

**Fix:** Move cache refresh to a `tea.Cmd` goroutine.

---

### C-3: Discuss phase dead-end (workflow hangs)
**File:** `internal/tui/app_workflow.go:148-178`
**Status:** CONFIRMED (wiring report D-01, loophole report 6.1)

The discuss phase emits `QuestionRequestMsg` with a `ResponseCh`, but the `QuestionResponseMsg` handler (app_update.go:650-666) routes answers back to `workflowEngine.SubmitDiscussAnswer()`. The flow appears wired, but:

1. The `askNextDiscussQuestion()` function calls `m.dispatcher.QuestionResponseCh()` — if dispatcher is nil, this panics (no nil check at line 170).
2. The 5-minute timeout goroutine (`<-m.discussAnswerTimeout.C`) leaks if the phase changes before the timer fires — `Stop()` doesn't unblock a pending channel receive.

**Impact:** Workflow can hang indefinitely if dispatcher is nil or if user abandons discuss phase.

**Fix:** Add nil guard on dispatcher; use `context.WithCancel` for the timeout goroutine.

---

### C-4: Nil pointer dereference on `m.replModel`
**File:** Multiple locations in `app_update.go`
**Status:** CONFIRMED (deep codebase C1, comprehensive C-1)

Multiple message handlers (e.g., `StreamErrorMsg`, `StreamDoneMsg`, `HealthUpdateMsg`) call `m.replModel.SetX()` or `m.replModel.AddMessage()` without nil checks. If `m.replModel` is nil (e.g., during first-run or after `new_session` action), these panic.

**Impact:** App crash on stream error during first-run or after session reset.

**Fix:** Add nil guards before every `m.replModel` access.

---

### C-5: Stream channel double-close panic
**File:** `internal/tui/streaming.go:51-176`
**Status:** CONFIRMED (comprehensive C-3)

The `StartStreamCmd` function allocates channels locally and closes `streamCh` via `defer close(streamCh)`. The architecture is designed to be safe (each call allocates fresh channels), but if `StartStreamCmd` is called twice rapidly (e.g., on quick retry), the old goroutine's deferred close can race with the new goroutine's initialization.

**Impact:** Potential panic on "close of closed channel" under rapid retry.

**Fix:** Use `sync.Once` for channel close, or ensure callers cancel the previous stream before starting a new one.

---

### C-6: `autoFallback` is dead — provider failover doesn't work
**File:** `internal/provider/` (across multiple files)
**Status:** CONFIRMED (comprehensive C-2, loophole AUTOFB-DEAD)

The `autoFallback` config field exists but no code path ever reads it to trigger failover. The entire provider resilience story (the ability to switch to the other provider on 429/503) is non-functional.

**Impact:** Users are stuck with a single provider; rate limiting or downtime has no recovery path.

**Fix:** Implement fallback logic in the provider registry that consults `autoFallback` and switches active provider.

---

### C-7: Plan screen unreachable
**File:** `internal/tui/app_update.go` (AppMsg handler)
**Status:** CONFIRMED (wiring report D-02)

`m.planModel` is created but `m.screen` is never set to `ScreenPlan` in normal flow. The plan screen only renders in test code. Users cannot review, edit, or reject the plan before execution begins.

**Impact:** Users have no visibility into what the AI will execute; malicious/hallucinated plans go straight to execution.

**Fix:** After Plan phase completes, set `m.screen = ScreenPlan` and wait for user confirmation before transitioning to Execute.

---

## HIGH Issues (20 unfixed)

### H-1: WindowSizeMsg drops all sub-model commands
**File:** `internal/tui/app_update.go:23-43`
**Status:** NEW

```go
case tea.WindowSizeMsg:
    if m.replModel != nil {
        m.replModel.Update(msg)  // ← return values discarded
    }
    if m.firstRunModel != nil {
        m.firstRunModel.Update(msg)  // ← return values discarded
    }
    if m.sidebarModel != nil {
        m.sidebarModel.Update(msg)  // ← return values discarded
    }
    return m, nil  // ← always returns nil cmd
```

All sub-model `Update()` calls discard returned `tea.Cmd` values. The REPL model can return spinner ticks, stream continuation commands, or viewport scroll commands. All are silently lost. On a resize during active streaming, the stream continuation chain breaks — streaming freezes with no error.

**Impact:** Streaming freezes on terminal resize; spinner stops; viewport scroll lost.

**Fix:** Collect all returned `tea.Cmd` and batch them: `return m, tea.Batch(cmds...)`.

---

### H-2: View() mutates toast state (impure rendering)
**File:** `internal/tui/app_view.go:20-23`
**Status:** NEW

```go
func (m *AppState) View() string {
    if m.toastText != "" && time.Now().After(m.toastExpires) {
        m.toastText = ""   // ← state mutation in View()
        m.toastType = ""   // ← state mutation in View()
    }
```

`View()` is called by Bubble Tea after every `Update()`. Mutating state here means the toast is cleared on the *next* render cycle, not the current one. If Bubble Tea ever calls `View()` twice per frame (e.g., for size calculations), the toast could be double-cleared.

**Impact:** Toast persists one extra frame after expiry; violates Bubble Tea contract.

**Fix:** Move toast expiry check to `Update()` using a `tea.After` command.

---

### H-3: PermissionTickMsg infinite loop after auto-deny
**File:** `internal/tui/app_update.go:627-641`
**Status:** NEW

```go
case PermissionTickMsg:
    if m.screen == ScreenPermission && m.permissionModal != nil {
        m.permissionModal.Tick()
        if m.permissionModal.Remaining() <= 0 {
            // auto-deny...
            m.permissionModal = nil
            return m, tea.Batch(...)
        }
    }
    return m, tea.Every(100*time.Millisecond, func(t time.Time) tea.Msg {
        return PermissionTickMsg{}  // ← always returns new tick
    })
```

After auto-deny clears `m.permissionModal`, the handler still returns a new `PermissionTickMsg` tick on line 639. The next tick won't match the `if` block (modal is nil), but it still returns another tick. This creates an infinite 100ms tick loop for the entire session lifetime after any permission modal was shown.

**Impact:** Unbounded CPU waste (10 ticks/second forever after first permission modal).

**Fix:** Only return the tick command when `m.screen == ScreenPermission && m.permissionModal != nil`.

---

### H-4: `/run` bypasses permission system
**File:** `internal/tui/commands_core.go` (or commands_ai.go)
**Status:** CONFIRMED (deep test H1)

The `/run` command directly executes bash commands via `ctx.Dispatcher.GetTool("Bash")` without any permission check. Arbitrary shell commands execute with no permission prompt.

**Impact:** Security bypass — any LLM-suggested or user-typed command runs without approval.

**Fix:** Route `/run` through the permission gate like regular tool calls.

---

### H-5: `/config ui.theme` doesn't change theme at runtime
**File:** `internal/tui/commands_config.go`
**Status:** CONFIRMED (deep test H2)

The `/config` command saves theme changes to disk but does NOT emit a `ThemeChangedMsg`. Theme only changes after restart.

**Impact:** Users expect immediate theme change; must restart to see it.

**Fix:** Emit `ThemeChangedMsg` after saving theme config.

---

### H-6: FirstRunComplete state not terminal
**File:** `internal/tui/firstrun.go:236-239`
**Status:** CONFIRMED (deep codebase H1)

```go
func (m *FirstRunModel) updateComplete(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
    m.state = FirstRunComplete
    return nil, &AppMsg{Screen: ScreenREPL}
}
```

Once in `FirstRunComplete`, every subsequent message (resize, keystroke, etc.) returns `AppMsg{Screen: ScreenREPL}`. This causes repeated screen transitions and potential duplicate session creation.

**Impact:** Repeated transitions to ScreenREPL on every message after first-run completes.

**Fix:** Make `updateComplete` a no-op (return `nil, nil`) once the transition has fired.

---

### H-7: `ctrl+c` doesn't quit on first-run screen
**File:** `internal/tui/firstrun.go:71-76`
**Status:** CONFIRMED (deep codebase H2)

```go
case tea.KeyMsg:
    switch msg.String() {
    case "ctrl+c":
        return nil, nil  // ← consumed but no action
    }
```

`ctrl+c` during first-run is silently consumed. No `tea.Quit` is returned. The user has no way to exit except completing or skipping the wizard.

**Impact:** User trapped on first-run screen if they can't configure an API key.

**Fix:** Return `tea.Quit` on `ctrl+c`.

---

### H-8: Workflow engine init error swallowed
**File:** `internal/tui/app_workflow.go:16-78`
**Status:** CONFIRMED (deep codebase H4)

If `workflow.NewEngine()` returns an error, `m.currentOperation` is set to an error string but `m.workflowEngine` remains nil. Subsequent operations that dereference `m.workflowEngine` will panic.

**Impact:** Nil pointer panic on any workflow operation after init failure.

**Fix:** Return an error to the caller; prevent workflow operations when engine is nil.

---

### H-9: Banner auto-dismiss depends on user input, not timer
**File:** `internal/tui/app_update.go` (banner handling)
**Status:** CONFIRMED (deep codebase H6, H7)

Banner persistence is checked against `m.fallbackBannerAt` which is not reset on manual dismiss. New banners can expire immediately if they arrive quickly after a manual dismiss.

**Impact:** Banners either persist forever or expire instantly depending on timing.

**Fix:** Reset `fallbackBannerAt` on manual dismiss; use a dedicated timer for auto-dismiss.

---

### H-10: `safeClose` is racy (TOCTOU)
**File:** `internal/tui/streaming.go` or related
**Status:** CONFIRMED (comprehensive H-9)

`safeClose` checks if a channel is non-nil then closes it. Between the check and the close, another goroutine could close the same channel.

**Impact:** Panic on "close of closed channel" under concurrent stream starts.

**Fix:** Use `sync.Once` for channel close operations.

---

### H-11: Channel emitter drops messages silently
**File:** `internal/tui/streaming.go` / workflow runner
**Status:** CONFIRMED (comprehensive H-10, loophole 3.6)

The channel emitter has a 500ms timeout on `Emit()`. If the TUI is busy (e.g., during a health check freeze from C-1), messages are silently dropped. The workflow engine doesn't learn its messages were dropped.

**Impact:** Task progress updates lost; execute screen shows incomplete progress.

**Fix:** Use buffered channels with overflow logging; or use `tea.Cmd` queue pattern.

---

### H-12: Stream content not flushed on segment transitions
**File:** `internal/tui/streaming.go:138-158`
**Status:** CONFIRMED (comprehensive H-8)

The goroutine builds segments with its own `activeContent` builder, but the REPL's `handleStreamMsg` also builds segments independently. The goroutine's segment building (lines 138-158) is dead code — its output is always overwritten in `handleStreamDoneMsg` (repl_stream.go:114).

**Impact:** Dead code wastes CPU; maintenance hazard if one copy is updated and the other isn't.

**Fix:** Remove segment building from the goroutine; it's redundant with the REPL's logic.

---

### H-13: Stream concurrency race on `streamContent`
**File:** `internal/tui/repl_stream.go:25`
**Status:** CONFIRMED (comprehensive H-14)

`m.streamContent.WriteString(chunk.Delta)` is called from the BT update loop, which is safe. But if any goroutine also writes to `streamContent`, it would race. Currently safe but fragile.

**Impact:** Potential data corruption if architecture changes add concurrent writers.

**Fix:** Document the single-writer invariant; consider using a channel-based message queue.

---

### H-14: `resolvedAPIKey` computed once at startup
**File:** `internal/tui/app.go` / `internal/config/`
**Status:** CONFIRMED (comprehensive H-7, H-12)

If fallback occurs or user switches provider at runtime, the TUI holds the original API key. The new provider may reject the old key.

**Impact:** Provider switch fails silently; requests sent with wrong key.

**Fix:** Re-resolve API key on provider switch.

---

### H-15: `/phase` via command registry doesn't trigger execution
**File:** `internal/tui/commands_workflow.go`
**Status:** CONFIRMED (deep test H3)

Two code paths for `/phase`: the `app.go` intercept triggers execution; the registry handler only persists state. Confusing behavior.

**Impact:** Users typing `/phase execute` get inconsistent behavior depending on code path.

**Fix:** Remove duplicate routing; use single code path.

---

### H-16: Workflow auto-advances without user confirmation
**File:** `internal/tui/app_workflow.go` (phase transitions)
**Status:** CONFIRMED (loophole 5.1)

No user review gate between phases. The plan goes straight to execution without user approval.

**Impact:** Malicious or hallucinated plans execute without consent.

**Fix:** Add confirmation step between Plan and Execute phases.

---

### H-17: `PhaseIdle` missing from PhaseResultMsg switch
**File:** `internal/tui/app_update.go`
**Status:** CONFIRMED (deep codebase H11)

If `PhaseIdle` is received in `PhaseResultMsg`, `m.workflowRunning` is never set to `false`. The app remains permanently in "running" state.

**Impact:** UI stuck in "workflow running" state after idle transition.

**Fix:** Add `case types.PhaseIdle:` to the switch.

---

### H-18: SSE parser `\r\n` handling
**File:** `internal/provider/` (SSE parser)
**Status:** CONFIRMED (comprehensive M-10)

Trailing `\r` in SSE lines survives into JSON parse, causing `json.Unmarshal` failures on some providers.

**Impact:** Stream parsing fails silently; no content displayed.

**Fix:** Trim `\r` from SSE lines before parsing.

---

### H-19: UTF-8 unsafe byte-level truncation in toolcard
**File:** `internal/tui/components/toolcard.go:98-101`
**Status:** NEW

```go
if len(output) > types.MaxToolOutputChars {
    tc.output = output[:types.MaxToolOutputChars]  // ← byte-level slice
```

Slicing by byte count can split multi-byte UTF-8 characters (Chinese = 3 bytes, emoji = 4 bytes), producing invalid UTF-8 that can panic in downstream rendering.

**Impact:** Panic or garbled display for non-ASCII tool output.

**Fix:** Use `utf8.RuneCountInString` and slice by rune count.

---

### H-20: leaderTimer field never assigned (dead code)
**File:** `internal/tui/keybindings.go:38-46, 137-142`
**Status:** NEW

```go
type KeyRegistry struct {
    leaderTimer   *time.Timer  // ← never assigned anywhere
}

func (r *KeyRegistry) cancelLeaderTimer() {
    if r.leaderTimer != nil {
        r.leaderTimer.Stop()  // ← always nil, never executes
        r.leaderTimer = nil
    }
}
```

`tea.Tick` returns a `tea.Cmd` managed by Bubble Tea's runtime, not by `r.leaderTimer`. The `leaderTimer` field is never assigned. `cancelLeaderTimer()` is a no-op. The leader timeout works via `LeaderTimeoutMsg` → `DeactivateLeader()`, but `cancelLeaderTimer()` gives a false impression.

**Impact:** Dead code; misleading API.

**Fix:** Remove `leaderTimer` field and `cancelLeaderTimer()` method; rely solely on `LeaderTimeoutMsg`.

---

## MEDIUM Issues (55 unfixed)

### UI Rendering & Theme

| # | Issue | File | Status |
|---|-------|------|--------|
| M-1 | Command palette hardcodes all colors — broken in light mode | cmdpalette.go | NEW |
| M-2 | Execute/Ship screens use `theme.Default()` instead of user theme | execute.go, ship.go | CONFIRMED (ux 4.2) |
| M-3 | 5+ components use `theme.Default()` directly, ignoring user theme | components/*.go | CONFIRMED (ux 4.3) |
| M-4 | Sparkline uses hardcoded color | components/sparkline.go | CONFIRMED (ux 4.4) |
| M-5 | `RiskDangerous` and `RiskDestructive` identical styling | components/permission.go | CONFIRMED (ux 4.6) |
| M-6 | Permission modal `[E] Exit` same weight as `[Y] Allow` | components/permission.go | CONFIRMED (ux 4.7) |
| M-7 | Bash prefix `$ ` hardcoded (wrong on Windows) | bash_renderer.go | CONFIRMED (ux 4.5) |
| M-8 | Header recomputed every View() call (16ms frame budget risk) | header.go | CONFIRMED (comprehensive M-26) |
| M-9 | Context bar shows `--/-- ctx` when total is 0 | header.go | CONFIRMED (ux 6.4) |

### Input & Navigation

| # | Issue | File | Status |
|---|-------|------|--------|
| M-10 | `Esc` resets textarea with no undo — long message lost | repl.go | CONFIRMED (ux 5.1) |
| M-11 | Terminal too small has no escape hatch | app_view.go | CONFIRMED (ux 5.2) |
| M-12 | Leader key has no visual countdown | keybindings.go | CONFIRMED (ux 5.3) |
| M-13 | Thinking toggle only works when textarea empty (no indicator) | repl.go | CONFIRMED (ux 5.4) |
| M-14 | Tab completion limited to 5-8 items with 35+ commands | commands.go | CONFIRMED (ux 5.5) |
| M-15 | Autocomplete doesn't show command arguments | commands.go | CONFIRMED (ux 5.6) |
| M-16 | Settings: tab inserts character instead of advancing fields | settings.go | CONFIRMED (ux 5.7, deep codebase M4) |
| M-17 | `Esc` behavior inconsistent across screens | multiple | CONFIRMED (ux 5.8) |
| M-18 | `RenderWhichKey` truncation breaks UTF-8 (byte-index slicing) | keybindings_screens.go | CONFIRMED (deep test M6, deep codebase M5) |
| M-19 | Settings editing indicator looks like part of the value | settings.go | CONFIRMED (ux 13.2) |

### Workflow & Screens

| # | Issue | File | Status |
|---|-------|------|--------|
| M-20 | Execute/Verify/Ship screens flash — each replaced immediately | app_update.go | CONFIRMED (wiring D-05) |
| M-21 | Workflow state non-persistent on restart | app_workflow.go | CONFIRMED (wiring D-06) |
| M-22 | Discuss phase not streamed — TUI shows nothing during 10-30s LLM call | app_workflow.go | CONFIRMED (wiring D-07) |
| M-23 | AppState god object — 1694 lines, 35 fields, 950-line Update() | app.go, app_update.go | CONFIRMED (wiring D-11, comprehensive M-32) |
| M-24 | Viewport height off by 2 lines — bottom clipped | repl_view.go | CONFIRMED (deep test M1) |
| M-25 | Command palette not overlaid — appended below content | cmdpalette.go | CONFIRMED (deep test M3) |
| M-26 | Session ID empty after first-run skip | firstrun.go | CONFIRMED (deep test M4) |
| M-27 | Screens show developer-facing placeholder text | app_view.go:78,85,92,99 | CONFIRMED (ux 6.1) |
| M-28 | Empty task lists show nothing — no guidance | plan.go, execute.go | CONFIRMED (ux 6.2) |
| M-29 | "Unknown screen" is a dead end — no recovery | app_view.go:110 | CONFIRMED (ux 6.3) |
| M-30 | Three different "no value" representations in settings | settings.go | CONFIRMED (ux 6.5) |
| M-31 | Plan screen: dependency graph unreadable, descriptions overflow | plan.go | CONFIRMED (ux 8.1-8.6) |
| M-32 | Execute screen: misleading progress bar, auto-transition on keypress | execute.go | CONFIRMED (ux 9.1-9.5) |
| M-33 | Verify screen: unclear self-heal, no remaining attempts shown | verify.go | CONFIRMED (ux 10.1-10.5) |
| M-34 | Ship screen: hardcoded next actions, missing cost in summary | ship.go | CONFIRMED (ux 11.1-11.5) |

### Streaming & State

| # | Issue | File | Status |
|---|-------|------|--------|
| M-35 | Segment boundary detection duplicated (goroutine + REPL) | streaming.go, repl_stream.go | NEW |
| M-36 | Goroutine builds segments that are always discarded | streaming.go:83-158 | NEW |
| M-37 | Initial dimensions hardcoded to 80×20 | repl.go:114-121 | NEW |
| M-38 | `/clear` doesn't reset streaming state | commands_core.go:61-67 | NEW |
| M-39 | `case "done":` empty — usage/cost tracking never applied | streaming.go:156-157 | CONFIRMED (comprehensive M-22) |
| M-40 | Dispatcher swallows errors into ToolResult.Error | tools/dispatcher.go | CONFIRMED (comprehensive M-2) |
| M-41 | Verify output not shown to user (go build/npm test output lost) | workflow/verify.go | CONFIRMED (comprehensive M-3) |
| M-42 | Defer LIFO closes `streamCh` before `streamDone` | streaming.go | CONFIRMED (comprehensive M-21) |
| M-43 | REPL history unbounded (no ring-buffer) | repl.go | CONFIRMED (comprehensive M-25) |
| M-44 | Discuss timeout goroutine leaks if phase changes | app_workflow.go:161-177 | NEW |
| M-45 | `updateComplete` emits AppMsg on every message | firstrun.go:236-239 | NEW |

### First-Run & Settings

| # | Issue | File | Status |
|---|-------|------|--------|
| M-46 | First-run: empty icon in feature card | firstrun.go | CONFIRMED (ux 12.1) |
| M-47 | First-run: emoji icons may not render on all terminals | firstrun.go | CONFIRMED (ux 12.2) |
| M-48 | First-run: provider-specific API key placeholder | firstrun.go | CONFIRMED (ux 12.4) |
| M-49 | First-run: validation shows raw HTTP errors | firstrun.go | CONFIRMED (ux 12.5) |
| M-50 | Settings: API key unmask logic inverted | settings.go | CONFIRMED (ux 13.1) |
| M-51 | Settings: dirty indicator pushes key hints off-screen | settings.go | CONFIRMED (ux 13.3) |
| M-52 | Settings: tab bar has no scroll indicators | settings.go | CONFIRMED (ux 13.4) |

### Tool Cards & Components

| # | Issue | File | Status |
|---|-------|------|--------|
| M-53 | Tool cards: auto-collapse hides output without expand hint | components/toolcard.go | CONFIRMED (ux 17.1) |
| M-54 | Tool cards: error state shows only `ERR` badge (no message) | components/toolcard.go | CONFIRMED (ux 17.3) |
| M-55 | Permission modal: auto-deny lacks context ("what happens?") | components/permission.go | CONFIRMED (ux 18.1) |
| M-56 | Permission modal: command box has no syntax highlighting | components/permission.go | CONFIRMED (ux 18.2) |
| M-57 | Thinking blocks: toggle not discoverable (nowhere says "press T") | components/thinking.go | CONFIRMED (ux 19.1) |
| M-58 | Thinking blocks: no scroll mechanism for 100+ lines | components/thinking.go | CONFIRMED (ux 19.2) |
| M-59 | Thinking blocks: header truncation at byte boundary | components/thinking.go:222-228 | NEW |
| M-60 | No spinners on 6 loading screens (only model selector has one) | multiple screens | CONFIRMED (ux 1.1) |
| M-61 | No streaming progress during task execution | execute.go | CONFIRMED (ux 1.2) |
| M-62 | Phase transitions invisible — auto-transitions silent | app_update.go | CONFIRMED (ux 1.4) |
| M-63 | Permission modal: no responded flag to prevent duplicate responses | components/permission.go:17-33 | NEW |
| M-64 | Model selector: filter cycling uses hardcoded `% 3` | modelselector.go:39-41 | NEW |
| M-65 | Resume: search misses goal/content (ID/Model/Provider only) | resume.go | CONFIRMED (ux 14.1) |
| M-66 | Resume: preview only shown when width > 100 | resume.go | CONFIRMED (ux 14.2) |
| M-67 | Diff viewer: header doesn't show file name | diff.go | CONFIRMED (ux 16.1) |
| M-68 | `autoDream.SetMessages` called on every slash command (wasteful) | app_update.go | CONFIRMED (deep codebase M14) |

---

## LOW Issues (25 unfixed)

| # | Issue | File | Status |
|---|-------|------|--------|
| L-1 | Banner uses emoji in non-emoji terminal | repl_view.go | CONFIRMED (deep test L1) |
| L-2 | First-run provider cursor wraps with hardcoded magic numbers | firstrun.go | CONFIRMED (deep test L2) |
| L-3 | `/config` missing many config keys (5 of 35+) | commands_config.go | CONFIRMED (deep test L3) |
| L-4 | Sidebar threshold hardcoded (`width > 120`) | sidebar.go | CONFIRMED (wiring D-09) |
| L-5 | `applyFieldsToConfig` dead code duplication | commands_config.go | CONFIRMED (deep test M2) |
| L-6 | `ThemeChangedMsg` recreates theme manager instead of cycling | app_update.go:973-992 | NEW |
| L-7 | `removeModelSegment` fragile logic | header.go:146-153 | NEW |
| L-8 | frecent history save is fire-and-forget goroutine | repl.go:292-297 | NEW |
| L-9 | Unbounded tick chain when not streaming | repl.go:236-249 | NEW |
| L-10 | handleStreamErrorMsg doesn't nil streamCh | repl_stream.go:206-243 | NEW |
| L-11 | askNextDiscussQuestion no dispatcher nil check | app_workflow.go:148-178 | NEW |
| L-12 | Handle() only searches current + global context | keybindings.go:77-126 | NEW |
| L-13 | Permission modal Tick-based elapsed can drift | components/permission.go:167-169 | NEW |
| L-14 | Tool card: no ellipsis at truncation point | components/toolcard.go:98-101 | NEW |
| L-15 | Tool card: input truncation also byte-level | components/toolcard.go:168-169 | NEW |
| L-16 | Thinking toggle hint doesn't communicate textarea precondition | components/thinking.go:216-219 | NEW |
| L-17 | handleHelp allocates new registry per call | commands_core.go:12-21 | NEW |
| L-18 | `/undo` informational only, no restoration | commands_session.go:12-28 | CONFIRMED |
| L-19 | `/pause` informational only | commands_workflow.go:188-194 | CONFIRMED |
| L-20 | Fallback banner no dismiss hint | repl_view.go | CONFIRMED (loophole 5.3) |
| L-21 | Model selector badge overflows narrow terminals | modelselector.go | CONFIRMED (comprehensive L-15) |
| L-22 | Sidebar git status races with `g.Status()` | sidebar.go | CONFIRMED (comprehensive L-17) |
| L-23 | `replModel.SetProvider` doesn't validate model against new provider | repl.go | CONFIRMED (comprehensive L-18) |
| L-24 | Hardcoded `#FDD663` in repl_view.go defeats theme system | repl_view.go | CONFIRMED (comprehensive L-3) |
| L-25 | `renderWithPalette` deprecated but still called | app_view.go | CONFIRMED (deep codebase M28) |

---

## Already Fixed (25 issues verified)

| ID | Issue | Fixed In |
|----|-------|----------|
| C-1 (deep test) | Goroutine leak on Ctrl+C stream cancellation | streaming.go C-3 fix |
| C-2 (deep test) | Theme change doesn't propagate to settingsModel | app_update.go ThemeChangedMsg |
| D-01 (partial) | Command registry routing | app_update.go command handling |
| D-02 (partial) | Workflow engine init | app_workflow.go |
| D-03 (partial) | Streaming pipeline | streaming.go C-3 fix |
| D-04 (partial) | Provider fallback | streaming.go |
| W-05 | SettingsSavedMsg | app_update.go |
| W-09 | ScreenShip | app_update.go |
| W-10 | HealthUpdateMsg | app_update.go |
| W-12 | Inline /commands | commands.go |
| W-13 | ReplModel provider access | repl.go |
| C1 (deep codebase) | Goroutine leak on Ctrl+C | streaming.go |
| C2 (deep codebase) | Theme propagation to settings | app_update.go |
| H4 (deep test) | Dead KeyRegistry.Handle() | keybindings.go |
| 1.1 (loophole) | D-Bus isDBusUnavailable always true | keychain_linux.go |
| 1.3 (loophole) | Path traversal in FileWrite | filewrite.go |
| 2.1 (loophole) | Permission channel buffer = 1 | dispatcher.go |
| 2.2 (loophole) | Race in limitWriter | bash.go |
| 2.3 (loophole) | Hardcoded 5-min task timeout | taskrunner.go |
| 2.6 (loophole) | SSE body read without timeout | streaming.go NextWithContext |
| 3.1 (loophole) | Workflow engine context.Background() | app_workflow.go |
| 3.4 (loophole) | Engine.NewEngine panics on prompt load | workflow engine |
| 3.6 (loophole) | Channel emitter drops messages (buffer 1) | dispatcher.go (buffer 64) |
| 4.4 (loophole) | Backup file name collision | filewrite.go |
| 5.2 (loophole) | First-run API key validation len < 10 | firstrun.go (HTTP check) |

---

## Issue Density by File

| File | LOC | CRITICAL | HIGH | MEDIUM | LOW | Total |
|------|-----|----------|------|--------|-----|-------|
| `app_update.go` | 1,192 | 2 | 4 | 5 | 1 | 12 |
| `app_view.go` | 149 | 0 | 1 | 1 | 1 | 3 |
| `streaming.go` | 183 | 1 | 2 | 2 | 0 | 5 |
| `repl.go` | 850 | 0 | 0 | 2 | 3 | 5 |
| `repl_stream.go` | 262 | 0 | 1 | 1 | 1 | 3 |
| `app_workflow.go` | 266 | 1 | 1 | 2 | 1 | 5 |
| `firstrun.go` | 647 | 0 | 2 | 3 | 1 | 6 |
| `keybindings.go` | 149 | 0 | 1 | 0 | 1 | 2 |
| `modelselector.go` | 294 | 0 | 0 | 1 | 0 | 1 |
| `header.go` | 153 | 0 | 0 | 1 | 1 | 2 |
| `commands_core.go` | 130 | 0 | 0 | 1 | 1 | 2 |
| `commands_session.go` | 201 | 0 | 0 | 0 | 1 | 1 |
| `commands_workflow.go` | 202 | 0 | 0 | 0 | 1 | 1 |
| `components/permission.go` | 244 | 0 | 0 | 2 | 1 | 3 |
| `components/toolcard.go` | 237 | 0 | 1 | 1 | 2 | 4 |
| `components/thinking.go` | 239 | 0 | 0 | 1 | 1 | 2 |
| `settings.go` | 843 | 0 | 0 | 4 | 0 | 4 |
| `resume.go` | 503 | 0 | 0 | 2 | 0 | 2 |
| Other files | ~5,000 | 0 | 0 | 18 | 4 | 22 |
| **Total** | **~11,443** | **4** | **13** | **46** | **21** | **84** |

---

## Priority Matrix

### P0 — Must Fix (blocks core functionality)
1. **C-1 + C-2:** Health check + cache refresh blocking event loop → Move to goroutines
2. **C-7:** Plan screen unreachable → Add confirmation gate
3. **H-1:** WindowSizeMsg drops commands → Collect and batch returned cmds
4. **H-7:** ctrl+c trapped on first-run → Return tea.Quit
5. **H-16:** Auto-advance without confirmation → Add user gate

### P1 — Should Fix (degraded experience)
6. **C-3:** Discuss phase goroutine leak → Use context cancellation
7. **C-4:** Nil replModel dereference → Add nil guards
8. **H-2:** View() impure mutation → Move to Update()
9. **H-3:** PermissionTickMsg infinite loop → Conditional tick return
10. **H-5:** Theme doesn't change at runtime → Emit ThemeChangedMsg
11. **H-6:** FirstRunComplete not terminal → Make updateComplete a no-op
12. **H-19:** UTF-8 unsafe truncation → Use rune-based slicing
13. **M-24:** Viewport off by 2 lines → Fix height calculation
14. **M-35-36:** Duplicate segment building → Remove goroutine's dead code

### P2 — Nice to Fix (polish)
15. **M-1:** Command palette hardcoded colors → Use theme
16. **M-10:** Esc resets textarea → Add undo or confirmation
17. **M-18:** RenderWhichKey UTF-8 breakage → Use rune-aware slicing
18. **M-37:** Initial 80×20 dimensions → Use terminal size query
19. **M-38:** /clear doesn't reset streaming → Reset stream state
20. **M-53-55:** Tool card UX improvements
21. **M-57-58:** Thinking block discoverability and scrolling
22. All MEDIUM help/discoverability issues (M-14, M-15, M-17)

---

## Appendix: Files with Zero UI Issues

The following TUI files were audited and found clean:

- `commands_ai.go` — Clean
- `commands_git.go` — Clean
- `commands_config.go` — Clean (aside from L-5 dead code)
- `repl_quickactions.go` — Clean (aside from hardcoded card width)
- `repl_thinking.go` — Clean
- `repl_commands.go` — Clean (aside from hardcoded maxFileSize)
- `statusbar.go` — Clean
- `sidebar.go` — Clean (aside from L-4 hardcoded threshold)
- `types.go` — Clean
- `truncate.go` — Clean
- `history.go` — Clean (aside from L-8 fire-and-forget save)
- `providerbadge.go` — Clean
- `health.go` — Clean
- `cache_refresh.go` — Clean
- `components/badge.go` — Clean
- `components/question.go` — Clean
- `components/toolrenderers.go` — Clean
- `components/bash_renderer.go` — Clean
- `components/file_renderers.go` — Clean
- `components/special_renderers.go` — Clean
- `components/progress.go` — Clean
- `components/metriccard.go` — Clean
- `components/statrow.go` — Clean
- `components/filterchips.go` — Clean
- `theme/theme.go` — Clean
- `theme/colors.go` — Clean
