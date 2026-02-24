# M31A — Full Codebase Bug Audit

> All findings are **real bugs** — nil panics, state divergence, resource leaks, or logic errors
> that cause incorrect behavior. Style/quality issues are excluded.

---

## 🔴 CRITICAL (will panic or corrupt data)

---

**#1 [Nil Pointer Dereference]** `internal/tui/app_update.go:1615`
> `handlePermissionResponse` reads `m.permRequest.ID` before checking if `m.permRequest` is nil. The auto-deny timeout path (`handlePermissionTick` → `handlePermissionResponse`) can race with an `esc` key that already cleared `m.permRequest`, causing a nil dereference panic.
```go
// Line 1615 — permRequest may be nil if user pressed 'n' before the tick fires
reqID := m.permRequest.ID  // PANIC if m.permRequest == nil
```
> **Fix**: Add `if m.permRequest == nil { return nil }` at the top of `handlePermissionResponse`.

---

**#2 [Nil Pointer Dereference]** `internal/tui/app_update.go:1635`
> Inside `handlePermissionTick`, when `permCountdown` reaches zero, it calls `handlePermissionResponse` which itself reads `m.permRequest.ID`. But `permRequest` could have been cleared by a concurrent `PermissionResponseMsg` that arrived in the same Update() batch — both would fire in the same `cmds` slice.
```go
// Line 1635 — same root cause as #1 but in timeout path
RequestID: m.permRequest.ID,  // PANIC if permRequest was already nil'd
```
> **Fix**: Read and nil-check `m.permRequest` before calling `handlePermissionResponse`.

---

**#3 [State Divergence / Stale Model]** `internal/tui/repl_state.go:82-98`
> `handleProviderModelsFetched` only updates `m.activeModel` when the model is found in `msg.Models` AND `p.GetModel()` succeeds. If the provider's local cache isn't warm yet (GetModel returns nil), `m.activeModel` is left as the stub from `Init()` — but `m.modelValid` is NOT set to false, so no warning is shown. The user chats with a stub model that has zero pricing/context info.
```go
// Line 90 — if info == nil (cache miss), falls through with no update
if info, _ := p.GetModel(msg.Model.ID); info != nil {
    m.activeModel = info  // only set when cache warm
    return
}
```
> **Fix**: After the loop, if no match was found, keep the stub but log a warning; don't set `m.modelValid = false` unless the model truly doesn't exist.

---

**#4 [Nil Pointer Dereference]** `internal/tui/app_update.go:108`
> `TickMsg` handler calls `m.executeModel.Update(msg)` and stores the result as `m.executeModel = execM` (untyped `tea.Model`). But `executeModel` is typed `*ExecuteModel` throughout; if `Update` ever returns a different concrete type (e.g. on screen transition), the subsequent `m.executeModel.paused = msg.Paused` (line 224) will panic on nil.
```go
// Line 108 — no type assertion performed
m.executeModel = execM  // execM is tea.Model, not *ExecuteModel
// ...later:
m.executeModel.paused = msg.Paused  // field access — nil/wrong type panic
```
> **Fix**: Add type assertion: `if ne, ok := execM.(*ExecuteModel); ok { m.executeModel = ne }`.

---

**#5 [Nil Pointer Dereference]** `internal/tui/config_model.go:709`
> In `activateField()` for a `cfgChoice` field, `f.choices[(idx+1)%len(f.choices)]` panics with **division by zero** when `len(f.choices) == 0`. A field with an empty choices slice (or a new field added without choices) causes an immediate crash when the user presses Enter/Space on it.
```go
// Line 709
next := f.choices[(idx+1)%len(f.choices)]  // PANIC: integer divide by zero
```
> **Fix**: Guard with `if len(f.choices) == 0 { return m, nil }` before the modulo.

---

## 🟠 HIGH (silent data loss or incorrect behavior)

---

**#6 [Resource Leak — Goroutine]** `internal/tui/streaming.go:86-92`
> The cancellation watcher goroutine is launched with `done` channel, but `close(done)` happens in a `defer` that runs **after** `close(streamCh)`. If `close(streamCh)` panics (double-close), the watcher goroutine leaks indefinitely — it's stuck in `select` with no way to exit.
```go
defer func() {
    iterator.Close()
    close(done)  // if this is not reached due to panic, goroutine leaks
}()
```
> **Fix**: Add `defer close(done)` as the outermost defer so it runs even on panic.

---

**#7 [State Sync Gap]** `internal/tui/repl_state.go:55-77` + `app_update.go:397-403`
> When `ModelSelectedMsg` arrives (user picks a new model), `AppState.activeModel` is set correctly but `replModel.SetProvider()` is called with the new model. Inside `SetProvider`, the async FetchModels fires and emits `ProviderModelsFetchedMsg`. The handler at line 463 syncs back `m.activeModel = m.replModel.activeModel`. But if the user sends a chat message **between** ModelSelectedMsg and ProviderModelsFetchedMsg arriving, `sendChatMessage` uses the newly-set `m.activeModel` (the stub) while `replModel.activeModel` is still the old model — the REPL renders the old model name in the spinner, creating a split-brain for one request cycle.
> **Fix**: When `ModelSelectedMsg` fires, immediately set `replModel.activeModel = &msg.Model` (don't wait for the async fetch). The async fetch only enriches it further.

---

**#8 [Silent Config Loss]** `internal/tui/config_model.go:392-606`
> `setFieldValue` for `cfgPassword` fields (API keys) does nothing — the keys `"provider.openrouter.api_key"` and `"provider.zen.api_key"` have no case in the switch. So when a user edits an API key in `/config`, the value is displayed as changed but is never written to `m.cfg`, and `saveConfig()` saves the old (empty) value.
```go
// Missing cases in setFieldValue switch:
// "provider.openrouter.api_key" → no case
// "provider.zen.api_key"        → no case
```
> **Fix**: Add cases that write to `c.Provider.OpenRouter.APIKey` / `c.Provider.Zen.APIKey`, matching how the settings model handles them.

---

**#9 [Arithmetic Bug — Negative Width]** `internal/tui/repl_view.go:79`
> The shelf fill string is built with `strings.Repeat("▁", rw-1)`. If `rw` is 0 (terminal narrower than minimum or race before first `WindowSizeMsg`), `rw-1` wraps to a very large negative number via unsigned arithmetic, causing `strings.Repeat` to panic or allocate a huge string.
```go
shelfFill := ... Render(strings.Repeat("▁", rw-1))  // rw-1 can be -1 or huge
```
> **Fix**: `fill := rw - 1; if fill < 0 { fill = 0 }` before the Repeat call.

---

**#10 [Logic Error — Backup pruned before new backup written]** `internal/tools/filewrite.go:155-160`
> `pruneBackups` is called immediately after the backup is written. But the backup filename uses `time.Now().Format(...)` with millisecond precision. On fast disks where two writes happen in the same millisecond, the lexicographic sort in `pruneBackups` may delete the backup that was just written (it's the lexicographically last one among those with the same prefix).
```go
if err := os.WriteFile(backupPath, existingContent, FilePermission); err != nil { ... }
t.pruneBackups(sanitized)  // could delete backupPath if it happened to be oldest
```
> **Fix**: Run `pruneBackups` before writing the new backup (prune old ones first, then write new one).

---

**#11 [Incorrect `fuzzyAnchorReplace` with empty middle]** `internal/tools/edit.go:473-478`
> When `oldLines` has exactly 2 lines (first + last, no middle), `middleOld` is empty (`oldLines[1:1]`). The similarity loop body never executes, leaving `avgSimilarity = 0.0 / 0.0 = NaN`. The `NaN >= LevenshteinThreshold` comparison is always `false` in Go, so the match is never applied even when first+last lines match exactly.
```go
totalSimilarity := 0.0
// loop doesn't execute when len(middleOld) == 0
avgSimilarity := totalSimilarity / float64(len(middleOld))  // 0/0 = NaN
if avgSimilarity >= LevenshteinThreshold {  // NaN >= x is always false
```
> **Fix**: Skip the similarity check when `len(middleOld) == 0` and accept the match directly (first+last match is sufficient).

---

**#12 [Context not cancelled on StreamDoneMsg]** `internal/tui/app_update.go:82-86`
> When `StreamDoneMsg` arrives successfully, `m.streamCancelFn` is never cleared. The cancel function holds a reference to the (now completed) stream context. If the user then presses `ctrl+c`, `streamCancelFn()` is called on an already-closed context, which is harmless but the guard `if m.streamCancelFn != nil` (line 37) fires — showing the "Response cancelled" toast even though nothing was streaming.
```go
case StreamDoneMsg:
    m.replModel.handleStreamDoneMsg(msg)
    // streamCancelFn never set to nil here
```
> **Fix**: Add `m.streamCancelFn = nil` after `handleStreamDoneMsg`.

---

**#13 [Context not cancelled on StreamErrorMsg]** `internal/tui/app_update.go:87-96`
> Same as #12 — on stream error, `m.streamCancelFn` is not cleared. A subsequent `ctrl+c` shows a spurious "Response cancelled" toast.
> **Fix**: Add `m.streamCancelFn = nil` in the `StreamErrorMsg` case.

---

## 🟡 MEDIUM (incorrect behavior in edge cases)

---

**#14 [Off-by-one in Edit tool line range]** `internal/tools/edit.go:310`
> `replaceByLineRange` uses `append(lines[:startIdx], append(...)...)` which mutates the underlying array of `lines` via the first `append`. This is the classic Go slice-append aliasing bug — the `lines[endIdx+1:]` in the second append reads from the already-mutated slice if `len(lines[:startIdx]) + len(newContent lines) > cap(lines[:startIdx])`.
```go
newLines := append(lines[:startIdx], append(strings.Split(newContent, "\n"), lines[endIdx+1:]...)...)
```
> **Fix**: Copy slices explicitly: `newLines = make([]string, 0, len(lines)); newLines = append(newLines, lines[:startIdx]...); ...`

---

**#15 [Rate limiter goroutine not stopped on Dispatcher reuse]** `internal/tools/dispatcher.go:61-73`
> `NewDispatcher` starts a goroutine that only exits when `d.rateDone` is closed. If the `Dispatcher` is re-created (e.g., session restart without calling `Stop()`), the old goroutine leaks — it holds a reference to the old `rateTokens` and `rateTicker` forever.
> **Fix**: Ensure callers always call `d.Stop()` before creating a new dispatcher, or document that `Stop()` must be called. Consider adding a finalizer.

---

**#16 [SSE parser: empty `data:` line treated as error]** `internal/provider/sse.go:95-97`
> When a provider sends a keep-alive event (`data:` with no payload, empty string), `data == "" && len(dataParts) == 0` is false (dataParts has one empty string element). But `data == ""` is true and `len(dataParts) == 1` — the code returns the empty string as valid data. The upstream JSON parser then gets `""` and fails with a JSON unmarshal error, which is wrapped as `ErrStreamTruncated` and terminates the stream.
```go
data = strings.Join(dataParts, "\n")
if data == "" && len(dataParts) == 0 {  // WRONG: this doesn't catch [""]
    return "", "", fmt.Errorf(...ErrStreamTruncated)
}
```
> **Fix**: After joining, if `strings.TrimSpace(data) == ""`, continue to next SSE event instead of returning error.

---

**#17 [Viewport mutation in View()]** `internal/tui/repl_view.go:196-199`
> `ViewContent()` mutates `m.viewport.Width` and `m.viewport.Height` when they differ from the expected values. Bubble Tea's `View()` is supposed to be a pure read — mutating state in `View()` violates the contract and can cause subtle rendering drift (viewport scrolls reset, cursor jumps).
```go
func (m *ReplModel) ViewContent(...) string {
    ...
    if m.viewport.Width != rw || m.viewport.Height != vpH {
        m.viewport.Width = rw   // STATE MUTATION IN VIEW — forbidden
        m.viewport.Height = vpH
        m.renderMessages()
    }
```
> **Fix**: Move resize logic to `Update(tea.WindowSizeMsg)` only; let `View()` render whatever the viewport currently contains.

---

**#18 [Integer overflow in viewport height]** `internal/tui/config_model.go:623`
> `m.viewport.Height = msg.Height - 8` — if the terminal is fewer than 9 rows tall (e.g., running in a tiny split pane), this goes negative. Bubble Tea's viewport with a negative height can enter an infinite loop in `GotoBottom()`.
```go
m.viewport.Height = msg.Height - 8  // negative if msg.Height < 9
```
> **Fix**: `h := msg.Height - 8; if h < 1 { h = 1 }; m.viewport.Height = h`.

---

**#19 [configModel not rebuilt on re-entry]** `internal/tui/app_update.go:1262-1272`
> When `ScreenConfig` is navigated to a second time, `ensureSubModel` syncs `m.configModel.cfg = m.config` but does NOT call `m.configModel.buildSections()`. This means any config changes made elsewhere (e.g., via `/settings`) are not reflected in the field values shown — the old cached values remain until the user manually presses `r` (reload).
> **Fix**: Call `m.configModel.buildSections()` when syncing cfg pointer in the `else` branch.

---

**#20 [Discuss transition ignores Transition() error]** `internal/tui/app_update.go:1730`
> `handleDiscussComplete` calls `m.workflowEngine.Transition(...)` and logs the error but continues regardless. If the checkpoint or STATE.md write fails (e.g., disk full), the workflow proceeds to the plan phase without a valid checkpoint — the session can never be resumed correctly.
```go
if err := m.workflowEngine.Transition(...); err != nil {
    slog.Error(...)  // error logged but not returned
}
m.persistWorkflowState()
return m.RunPhaseCmd(types.PhasePlan)  // proceeds anyway
```
> **Fix**: Show a toast and return nil (abort the phase start) if `Transition()` returns an error.

---

**#21 [Auto-fallback updates provider but not replModel]** `internal/tui/app_update.go:1756-1764`
> `attemptAutoFallback` sets `m.activeProvider` and updates `m.workflowEngine` but never calls `m.replModel.SetProvider(...)`. So the REPL continues to use the old provider's token for the next manual chat message, bypassing the fallback that was just set.
> **Fix**: After switching provider, call `m.replModel.SetProvider(ctx, m.registry, result.Event.To, m.activeModel, m.sessionID, m.config)`.

---

**#22 [fileExplorerModel screen never registers `ScreenConfig` key events]** `internal/tui/app_update.go` (routeKeyMsg)
> `routeKeyMsg` has no `case ScreenConfig:` branch. When the user is on the Config screen and presses a key, it falls through to `default` (returns nil). All keyboard input in `ScreenConfig` is silently dropped. The Config screen only works because `Update()` forwards keys via the `default:` branch in the type switch — but `routeKeyMsg` is the primary key router and takes priority.

Wait — rechecking: `routeKeyMsg` returns nil for ScreenConfig, and then in `Update()` the `tea.KeyMsg` case only calls `routeKeyMsg`. The `default:` in the main switch only handles non-KeyMsg messages. So yes — **all keyboard input on ScreenConfig is dropped.**
> **Fix**: Add `case ScreenConfig:` to `routeKeyMsg` that forwards to `m.configModel.Update(msg)`.

---

## Summary Table

| # | Severity | Package | Type |
|---|----------|---------|------|
| 1 | 🔴 Critical | tui | Nil dereference on permRequest |
| 2 | 🔴 Critical | tui | Nil dereference in permission timeout |
| 3 | 🔴 Critical | tui | Model stub never enriched (silent) |
| 4 | 🔴 Critical | tui | No type assert on executeModel.Update() |
| 5 | 🔴 Critical | tui/config | Divide by zero on empty choices |
| 6 | 🟠 High | tui/streaming | Goroutine leak on panic path |
| 7 | 🟠 High | tui | Split-brain model between AppState/replModel |
| 8 | 🟠 High | tui/config | API key writes silently discarded |
| 9 | 🟠 High | tui | Negative width in shelf repeat |
| 10 | 🟠 High | tools | Backup pruned before new backup confirmed |
| 11 | 🟠 High | tools | NaN similarity causes fuzzy match to always fail |
| 12 | 🟡 Medium | tui | streamCancelFn not cleared on StreamDoneMsg |
| 13 | 🟡 Medium | tui | streamCancelFn not cleared on StreamErrorMsg |
| 14 | 🟡 Medium | tools | Slice aliasing in replaceByLineRange |
| 15 | 🟡 Medium | tools | Rate limiter goroutine leaks on Dispatcher recreate |
| 16 | 🟡 Medium | provider | Empty SSE keep-alive terminates stream |
| 17 | 🟡 Medium | tui | State mutation in View() violates BubbleTea contract |
| 18 | 🟡 Medium | tui/config | Negative viewport height on tiny terminal |
| 19 | 🟡 Medium | tui/config | ConfigModel sections not rebuilt on re-entry |
| 20 | 🟡 Medium | tui/workflow | Phase transition error ignored → broken checkpoint |
| 21 | 🟡 Medium | tui | Auto-fallback doesn't update replModel provider |
| 22 | 🟡 Medium | tui | ScreenConfig key events silently dropped |
