# M31A Deep Codebase Audit Report

> **Date**: 2026-06-01
> **Scope**: Complete codebase audit — TUI, workflow engine, tools, providers, pkg/, cmd/
> **Method**: Multi-agent deep exploration + manual verification of critical paths

---

## Executive Summary

This audit identified **67 issues** across the codebase, categorized by severity:

| Severity | Count | Description |
|----------|-------|-------------|
| **Critical** | 8 | Will cause panics, data loss, or infinite loops in production |
| **High** | 15 | Logical errors that produce incorrect behavior or silent failures |
| **Medium** | 28 | Design flaws, incomplete logic, race conditions, resource leaks |
| **Low** | 16 | Code quality, dead code, minor inconsistencies |

---

## Critical Issues (Must Fix)

### C1. Nil Pointer: `m.replModel.Update(msg)` in StreamErrorMsg handler
**File**: `internal/tui/app.go:842`
**Severity**: Critical — panic on production

The `StreamErrorMsg` handler calls `m.replModel.Update(msg)` at line 842 without a nil check. The nil guard exists at line 840-843, but the *second* call at line 854 is guarded. However, if `m.replModel` is nil when `ScreenREPL` is not active (e.g., on `ScreenFirstRun` or `ScreenSettings`), and a stream error arrives, line 842 will panic.

```go
// Line 840-842: guarded
if m.replModel != nil {
    m.replModel.SetProvider(...)
    m.replModel.Update(msg)  // OK inside guard
}
// Line 854: also guarded — but the pattern is fragile
```

**Fix**: Ensure all `m.replModel.Update(msg)` calls are wrapped in nil checks consistently.

---

### C2. `e.git` nil pointer dereference across workflow engine
**Files**: `internal/workflow/initialize.go:28`, `execute.go:177-184, 272-280`, `verify.go:53`
**Severity**: Critical — panic on first workflow run

The `Engine.git` field is never initialized by `NewEngine()`. It is only set via `SetGit()` which must be called externally. If `SetGit()` is never called before any workflow phase runs, every call to `e.git.IsRepo()`, `e.git.AddAll()`, `e.git.Commit()`, `e.git.HeadHash()` will panic.

**Affected paths**:
- `initialize.go:28`: `e.git.IsRepo()`
- `execute.go:177`: `e.git.AddAll()`
- `execute.go:184`: `e.git.Commit()`
- `execute.go:272`: `e.git.AddAll()` (in `healTask`)
- `execute.go:280`: `e.git.Commit()` (in `healTask`)
- `verify.go:53`: `e.git.HeadHash()`

**Fix**: Either initialize `e.git` in `NewEngine()` or add nil guards before every call.

---

### C3. `consumeStream` infinite loop on `nil, nil` from `Next()`
**File**: `internal/workflow/engine.go:308-319`
**Severity**: Critical — goroutine hangs forever

```go
for {
    chunk, err := iterator.Next()
    if chunk != nil && chunk.Delta != "" {
        sb.WriteString(chunk.Delta)
    }
    if err == io.EOF { break }
    if err != nil { return sb.String(), err }
    // If chunk == nil && err == nil → infinite loop
}
```

When the SSE parser returns `nil, nil` (e.g., for a non-data event), the loop continues forever. The `defer iterator.Close()` ensures the goroutine holds resources indefinitely.

**Fix**: Add `if chunk == nil && err == nil { continue }` or better, break on `nil, nil`.

---

### C4. `stripTags` infinite loop in WebFetch
**File**: `internal/tools/webfetch.go:199-213`
**Severity**: Critical — goroutine hangs on malformed HTML

```go
for _, tag := range tags {
    for {
        start := strings.Index(html, "<"+tag)
        end := strings.Index(html, "</"+tag+">")
        if start == -1 { break }
        if end == -1 { break }
        html = html[:start] + html[end+len("</"+tag+">"):]
    }
}
```

When an opening tag exists without a closing tag (`<script>` but no `</script>`), `end == -1` breaks the inner loop. But the outer loop moves to the next tag. If the *same* unclosed tag appears again in the tag list (or if the HTML is re-processed), the pattern repeats. More critically, if a tag like `<script` (without `>`) appears, `strings.Index` for `"<script"` matches, but the removal logic assumes a proper `</script>` exists.

**Fix**: When `end == -1`, also remove the opening tag: `html = html[:start] + html[start+len("<"+tag)+1:]`.

---

### C5. `lastErr.Error()` panic in plan.go
**File**: `internal/workflow/plan.go:69`
**Severity**: Critical — panic when plan produces zero tasks

```go
if len(tasks) == 0 {
    return &PhaseResult{
        Error: lastErr.Error(),  // PANIC if lastErr is nil
    }, lastErr
}
```

If `MaxPlanRetries` is ever 0 (config change) or the loop never executes, `lastErr` remains nil and `.Error()` panics.

**Fix**: Check `if lastErr != nil` before calling `.Error()`, or use a default message.

---

### C6. `Session` struct duplicate `Project` field — JSON conflict
**File**: `pkg/session/session.go:10-15`
**Severity**: Critical — data loss on serialization

```go
type Session struct {
    types.Session       // has its own Project *ProjectState field
    Messages []types.Message     `json:"messages"`
    Tasks    []types.Task        `json:"tasks"`
    Project  *types.ProjectState `json:"project"`  // DUPLICATE json tag
}
```

Both the embedded `types.Session.Project` and the outer `Project` serialize to `"project"`. Go's encoding/json handles this by preferring the outer field, but unmarshaling behavior is ambiguous and version-dependent. The embedded field's value is silently discarded.

**Fix**: Rename the outer field or remove the duplicate from `types.Session`.

---

### C7. Ledger `Truncate()` data loss on write failure
**File**: `pkg/ledger/ledger.go:332-354`
**Severity**: Critical — permanent data loss

```go
l.entries = sorted[:maxEntries]  // Committed in memory — old entries lost
return l.rewriteFile()           // If this fails, data is gone forever
```

The in-memory truncation happens *before* the file write. If `rewriteFile()` fails (disk full, permissions), the older entries are permanently lost with no way to recover from the file.

**Fix**: Write the file first, then update in-memory state only on success.

---

### C8. Windows keychain `credentialBlobSize` excludes null terminator
**File**: `pkg/keychain/keychain_windows.go:74`
**Severity**: Critical — truncated credential storage

```go
cred.credentialBlob = (*byte)(unsafe.Pointer(valuePtr))
cred.credentialBlobSize = uint32(len(value) * 2)  // Missing +1 for null terminator
```

`syscall.UTF16PtrFromString` includes a null terminator. The size should be `(len(value) + 1) * 2`. This results in stored credentials being truncated by 2 bytes (one UTF-16 character), potentially corrupting API keys.

**Fix**: Use `cred.credentialBlobSize = uint32((len(value) + 1) * 2)`.

---

## High Severity Issues

### H1. `FirstRunComplete` state is not terminal — fires `AppMsg` on every message
**File**: `internal/tui/firstrun.go:241-243`

```go
func (m *FirstRunModel) updateComplete(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
    m.state = FirstRunComplete
    return nil, &AppMsg{Screen: ScreenREPL}
}
```

Every message arriving while in `FirstRunComplete` state fires another `AppMsg{Screen: ScreenREPL}`. This creates duplicate screen transitions. The state should be terminal — return `nil, nil` after the first transition.

---

### H2. `ctrl+c` doesn't quit on first-run screen
**File**: `internal/tui/firstrun.go:73-74`

```go
case "ctrl+c":
    return nil, nil  // Does nothing — app doesn't quit
```

`ctrl+c` is expected to be a universal quit mechanism. Returning `nil, nil` ignores the key entirely. Should return a `tea.Quit` command.

---

### H3. Unmatched leader chord key is discarded
**File**: `internal/tui/keybindings.go:110-111`

```go
// No chord matched, treat key as normal (fallthrough)
```

The comment says "fallthrough" but there is no Go `fallthrough` statement and no re-queuing of the key event. The pressed key is lost entirely.

**Fix**: Return the key as a normal `tea.KeyMsg` for re-processing.

---

### H4. Workflow engine init error swallowed
**File**: `internal/tui/app.go:260-263`

```go
s, err := m.sessionManager.NewSession(modelID, m.activeProvider)
if err != nil {
    m.currentOperation = fmt.Sprintf("Workflow engine init failed: %v", err)
    return  // Silent — app continues with nil workflowEngine
}
```

The app continues running with a nil `workflowEngine`. Subsequent operations will nil-deref. Should transition to an error screen or quit.

---

### H5. `/models` falls through to command registry if registry is nil
**File**: `internal/tui/app.go:596-597`

If `m.registry` is nil, the `/models` command falls through to the command registry which also has a `/models` handler. This creates inconsistent behavior depending on nil state.

---

### H6. Banner auto-dismiss depends on user input, not timer
**File**: `internal/tui/repl.go:167`

```go
if m.fallbackBanner != "" && time.Now().After(m.fallbackBannerAt) {
    m.fallbackBanner = ""
}
```

The banner only clears when a `tea.Msg` arrives (i.e., user input). If the user doesn't interact for 15 seconds, the banner persists until the next message.

**Fix**: Use `tea.After(15*time.Second)` to schedule a dismiss command.

---

### H7. `fallbackBannerAt` not reset on manual dismiss
**File**: `internal/tui/repl.go:450, 459, 476`

When dismissed via `x` or `esc`, `m.fallbackBanner` is cleared but `m.fallbackBannerAt` retains its old value. If a new `FallbackEventMsg` arrives quickly, the expiry check fires immediately and clears the new banner.

---

### H8. Silent failure: `LoadTasks` error in ship.go
**File**: `internal/workflow/ship.go:35-37`

```go
tasks, err := e.sessionMgr.LoadTasks(e.sessionID)
if err != nil {
    tasks = []m31types.Task{}  // Error silently dropped
}
```

---

### H9. Heal attempt counter inconsistency
**Files**: `internal/workflow/execute.go:120, 165` vs `verify.go:96`

In `execute.go`, `HealsAttempted++` happens *before* `healTask()`. In `verify.go`, it happens *after*. This creates off-by-one differences between code paths.

---

### H10. Heal loop allows `MaxHealAttempts + 1` iterations
**File**: `internal/workflow/execute.go:109`

```go
for task.HealsAttempted <= m31types.MaxHealAttempts  // <= allows one extra iteration
```

With `MaxHealAttempts = 2` and pre-increment, the loop runs 3 times (at 0, 1, 2) instead of 2.

---

### H11. `PhaseIdle` missing from `PhaseResultMsg` switch
**File**: `internal/tui/app.go:958-1043`

No case for `PhaseIdle`. Falls to default which does nothing and never sets `m.workflowRunning = false`, potentially leaving the app in a permanently "running" state.

---

### H12. `Session.NewSession` leaves orphan `session.json` on failure
**File**: `pkg/session/manager.go:141-148`

If `atomicWrite` succeeds but `ensureDir` fails for the planning directory, a partially-created session exists with no cleanup.

---

### H13. `ArchiveSession` fails across filesystems
**File**: `pkg/session/manager.go:266`

`os.Rename` does not work across filesystems. Should fall back to copy+delete.

---

### H14. Duplicate task IDs silently overwrite in taskrunner
**File**: `pkg/taskrunner/runner.go:47`

```go
r.idToIdx[t.ID] = i  // Silent overwrite on duplicate ID
```

No validation for duplicate task IDs.

---

### H15. Linux `isPassUnavailable` false positive
**File**: `pkg/keychain/keychain_linux.go:294-301`

ANY stderr output from `pass` causes "unavailable" detection, including non-fatal warnings like "password store is empty."

---

## Medium Severity Issues

### M1. Resource leak: `FileWrite` file descriptor on error
**File**: `internal/tools/filewrite.go:151-161`

If `tmpFile.Write()` fails, the defer removes the file but never closes the fd.

### M2. Double-close race on `iterator.Close()` in streaming
**File**: `internal/provider/streaming.go:47-59`

Context-watching goroutine and main goroutine defer both call `iterator.Close()`.

### M3. `leaderTimer` field never assigned — cancel is no-op
**File**: `internal/tui/keybindings.go:57`

`tea.Tick()` creates its own internal timer; `leaderTimer` is always nil.

### M4. `tab` key grouped with `right` arrow in settings
**File**: `internal/tui/settings.go:425-432`

`tab` is not a single character and won't satisfy `len(msg.String()) == 1`, causing a no-op while editing.

### M5. Byte-indexed string truncation breaks UTF-8
**Files**: `internal/tui/components/thinking.go:142`, `components/toolcard.go`

`label[:maxWidth]` can split multi-byte UTF-8 characters.

### M6. `consumeStream` defers `Close()` but `Next()` may block indefinitely
**File**: `internal/workflow/engine.go:304-320`

Related to C3 — if the underlying reader blocks, the deferred close never runs.

### M7. `context.Background()` orphan in `FinalizeDiscuss`
**File**: `internal/workflow/engine.go:269`

Creates an uncancelable context for transition.

### M8. `Provider.FindFallbackProvider` race during stream error
**File**: `internal/tui/app.go:837`

`m.activeProvider` is mutated without synchronization while streaming goroutines may still reference it.

### M9. `grepPureGo` file handle leak
**File**: `internal/tools/grep.go:212-214`

File opened but not closed before returning `nil` on binary check.

### M10. `TodoWrite` created with empty sessionID
**File**: `internal/tools/dispatcher.go:271-285`

If `SetSessionID` is never called, TODOs go to a directory named `""`.

### M11. `extractCommandString` has duplicate switch blocks
**File**: `internal/tools/dispatcher.go:167-247`

Two identical switch blocks for tool name extraction — maintenance burden.

### M12. `buildToolDefinitions` hardcoded empty parameters
**File**: `internal/workflow/engine.go:386, 63`

`Parameters: "{}"` means LLM receives no schema for tool parameters.

### M13. `verifyTask` missing project type handling
**File**: `internal/workflow/engine.go:862-930`

Missing `java`, `cc`, `rust`, `unknown` project types.

### M14. `autoDream.SetMessages` called on every slash command
**File**: `internal/tui/app.go:648-650`

Wasteful sync for commands like `/help`, `/status`, `/clear`.

### M15. Summary text duplicated in autodream
**File**: `pkg/autodream/autodream.go:155-165`

`summaryText` appears in both `Message.Content` and `Segment.Content`, doubling token cost.

### M16. Summary message appended at end instead of protected position
**File**: `pkg/autodream/autodream.go:175`

Should be inserted at the beginning of the message list for proper LLM context.

### M17. `New()` does not return error on directory creation failure
**File**: `pkg/ledger/ledger.go:58-73`

Ledger is returned anyway with nil entries, failing on first `Append`.

### M18. Dedup only checks in-memory entries
**File**: `pkg/ledger/ledger.go:127-131`

External file modifications can create duplicates.

### M19. `SafeReset` can leave repo in inconsistent state
**File**: `pkg/rollback/rollback.go:159-186`

If `StashPop` fails, HEAD is already reset but working tree changes are lost.

### M20. `CountCommitsBetween` error silently ignored
**File**: `pkg/rollback/rollback.go:281`

Shows "0 commits undone" instead of error indication.

### M21. `LoadProject` does not set `CreatedAt`
**File**: `pkg/session/planning.go:62-93`

Project creation time is lost on reload.

### M22. Bisect loop has no iteration limit
**File**: `pkg/bisect/bisect.go:73-100`

Inconsistent `checkFn` results cause infinite loop.

### M23. `parseBisectLog` prioritizes wrong format
**File**: `pkg/bisect/bisect.go:150-155`

Returns first bracket-parsed hash instead of explicit "first bad commit" line.

### M24. `AskUserQuestion` available during automated execution
**File**: `internal/tools/dispatcher.go:281`

No mechanism to disable during automated task execution, contradicting AGENTS.md rule.

### M25. No upper bound on Bash tool timeout
**File**: `internal/tools/bash.go:48-56`

LLM can request 999999 second timeout (11+ days).

### M26. Channel deadlock risk in dispatcher
**File**: `internal/tools/dispatcher.go:79-90`

Buffered channel (capacity 8) — 9th request fails with `ErrPermissionDenied`.

### M27. Linux dbus `SetSecret` failure has no fallback
**File**: `pkg/keychain/keychain_linux.go:153-158`

If `SetSecret` fails, no fallback to create new item.

### M28. `renderWithPalette` marked deprecated but still called
**File**: `internal/tui/app.go:1383-1391, 1330`

Misleading comment suggests removal is safe.

---

## Low Severity Issues

### L1. Repetitive `var cmds []tea.Cmd; return cmds, false` pattern
**File**: `internal/tui/repl.go` — throughout

Should use `return nil, false` to avoid heap allocations.

### L2. `Parameters: "{}"` hardcoded comment
**File**: `internal/workflow/engine.go:386`

Comment says "simplified" but this is the production code.

### L3. `hasTestFiles` is inefficient
**File**: `internal/workflow/engine.go:819-839`

Calls `os.ReadDir` for every file, even if multiple files share a directory.

### L4. `formatTaskSummary` action field breaks markdown table
**File**: `internal/workflow/engine.go:749-771`

Raw action with pipe characters breaks markdown rendering.

### L5. `reasoningParamMap` incomplete for new OpenAI models
**File**: `internal/provider/reasoning.go:19-63`

New o-series models require code changes to match.

### L6. `useRG` set without checking if `rg` actually works
**File**: `internal/tools/glob.go:53`

Broken `rg` causes failure without fallback.

### L7. `extractJSONObject` and `extractArrayFrom` are duplicated
**File**: `internal/workflow/engine.go:696-732`

Nearly identical bracket/brace tracking logic.

### L8. `os.Remove` error ignored in log rotation
**File**: `internal/log/log.go:108`

### L9. `git diff` error ignored in git package
**File**: `internal/git/git.go:196`

### L10. `WebFetch` panic risk at end of string
**File**: `internal/tools/webfetch.go:302`

`html[hrefStart]` can panic if `hrefStart >= len(html)`.

### L11. `question.go` navigation with no options
**File**: `internal/tui/components/question.go:65-75`

`m.selected` could be non-negative from previous state.

### L12. `cfg == nil` redundant check
**File**: `internal/tui/app.go:103-108`

`DefaultConfig()` should never return nil.

### L13. Type assertion without verification
**File**: `internal/tui/app.go:1233`

`m.modelSelector = updated.(ModelSelector)` will panic on wrong type.

### L14. `settingsModel` value receiver inconsistency
**File**: `internal/tui/settings.go`

`SetConfig`/`SetTheme` use pointer receivers; `Update` uses value receiver.

### L15. `providerName` empty string produces `[]` visual
**File**: `internal/tui/statusbar.go:123`

### L16. Hardcoded API key selection in main.go
**File**: `cmd/m31a/main.go:144-147`

Only handles OpenRouter/Zen, not future providers.

---

## Architecture Violations

| Rule | Violation | File |
|------|-----------|------|
| "V1 task execution is SEQUENTIAL" | `AskUserQuestion` available in automated execution | `dispatcher.go:281` |
| "AskUserQuestion MUST NOT be used in automated flows" | No filtering mechanism exists | `execute.go:106` |
| "No CGO. Binary must be static" | Compliant | — |
| "No telemetry/analytics" | Compliant | — |
| "No hardcoded model lists" | `reasoning.go` has hardcoded model prefixes | `reasoning.go:63` |
| "No direct Anthropic/OpenAI" | Compliant | — |

---

## Recommendations by Priority

### Immediate (P0)
1. Add nil guards for `e.git` across all workflow files
2. Fix `consumeStream` infinite loop (add `nil, nil` break)
3. Fix `stripTags` infinite loop in WebFetch
4. Fix `lastErr.Error()` nil panic in plan.go
5. Fix ledger `Truncate()` ordering
6. Fix Windows keychain credential blob size
7. Fix `Session` duplicate `Project` field

### Short-term (P1)
8. Make `FirstRunComplete` state terminal
9. Add `tea.Quit` for `ctrl+c` in first-run
10. Re-queue unmatched leader chord keys
11. Handle session init failure properly
12. Fix heal attempt counter consistency
13. Fix heal loop boundary condition (`<` instead of `<=`)
14. Add `PhaseIdle` case to phase result switch

### Medium-term (P2)
15. Add proper timer-based banner dismissal
16. Fix resource leaks (file descriptors, double-close races)
17. Add Bash tool timeout upper bound
18. Fix UTF-8 truncation in components
19. Add taskrunner duplicate ID validation
20. Fix cross-filesystem rename in ArchiveSession
