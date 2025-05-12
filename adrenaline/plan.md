# M31A — Comprehensive Bug & Loophole Fix Plan

> Generated from full codebase audit of 78 Go source files.
> 63 issues identified across critical, high, medium, low severity.
> Plan organized in execution waves — each wave is independently verifiable.

---

## Wave 0: AGENTS.md Corrections

**Goal**: Align project rules with actual codebase state. The user has confirmed V1 tools are built and some prohibited tools exist in the code.

### Changes to `AGENTS.md`

**1. V1 tools list — acknowledge existing tools**

Current rule says V1 tools are "Bash, FileRead, FileWrite, Glob, Grep ONLY" but the codebase also includes:
- `internal/tools/edit.go` — FileEdit
- `internal/tools/webfetch.go` — WebFetch
- `internal/tools/question.go` — AskUserQuestion
- `internal/tools/todo.go` — TodoWrite
- `internal/tools/dispatcher.go` — Tool dispatcher with permission system

**Update**: Change the V1 tools rule to reflect reality. These tools exist and are functional. Update to:

```
- V1 tools: Bash, FileRead, FileWrite, Glob, Grep, FileEdit, WebFetch,
  TodoWrite, and a permission-gated dispatcher. AskUserQuestion exists
  but must NOT be used in V1 task execution flow.
```

**2. Absolute Prohibitions — remove stale entries**

Remove:
- "DO NOT use the AskUserQuestion tool in V1" (the tool exists, just don't use it in automated flows)
- Replace "DO NOT implement FileEdit, WebFetch..." with "DO NOT add new tools beyond the current set without explicit request"

**3. Package layout — add missing packages**

The package layout comment is missing:
- `internal/tokens/` — token estimation
- `internal/tools/` should note the dispatcher

---

## Wave 1: Critical Fixes (Data Loss / Crash / Security)

### Fix #1: Cache data race — `internal/provider/cache.go`

**Problem**: `IsExpired()` and `IsStale()` read `c.fetched` without holding the mutex. Called from `FetchModels()` in both clients with no lock.

**Fix**:
- Add `c.mu.RLock()` / `c.mu.RUnlock()` to `IsExpired()` and `IsStale()`
- Alternatively, make them read fields that are already protected by the caller's lock, but this is fragile. Better to make them self-safe.
- Also protect `c.ttl` and `c.staleTTL` reads under the same lock.

```go
func (c *ModelCache) IsExpired() bool {
    c.mu.RLock()
    defer c.mu.RUnlock()
    return time.Since(c.fetched) > c.ttl
}

func (c *ModelCache) IsStale() bool {
    c.mu.RLock()
    defer c.mu.RUnlock()
    return time.Since(c.fetched) > c.staleTTL
}
```

**Files**: `internal/provider/cache.go` (lines 55-61)

---

### Fix #2: API key plaintext leak — `internal/config/loader.go`

**Problem**: `Save()` marshals the entire config to TOML, including API keys that came from env vars or keychain.

**Fix**:
- Before marshaling in `Save()`, zero out any API key fields:
  ```go
  // Don't persist keys sourced from env vars or keychain
  cfg.Provider.OpenRouter.APIKey = ""
  cfg.Provider.Zen.APIKey = ""
  ```
- Add a struct tag or comment field to track which keys are "persistable" (only user-entered ones).
- Simpler approach: never write API keys to the config file. Users must use env vars or keychain.

**Files**: `internal/config/loader.go` (lines 60-82), `internal/config/types.go` (line 22)

---

### Fix #3: msgChan close panic — `internal/tui/app.go`

**Problem**: `close(app.msgChan)` in `RunPhaseCmd` while a previous phase's goroutine may still be writing to it.

**Fix**:
- Use a new channel per phase instead of reusing `app.msgChan`.
- Or use a sync mechanism: set `app.msgChan = nil` after close, and have the old goroutine check before writing.
- Best approach: make `RunPhaseCmd` create a fresh channel and assign it to `app.msgChan`. The old drainer reads from the old channel which is independently managed. Don't close the old channel — let it drain naturally. When the engine's context is cancelled, the goroutine exits and the channel is GC'd.

**Files**: `internal/tui/app.go` (lines 289-310)

---

### Fix #4: Nil git panic — `internal/workflow/ship.go`, `internal/workflow/engine.go`

**Problem**: `e.git` is nil unless `SetGit()` was called. `NewEngine()` doesn't initialize it. `BuildSummary()`, `runShip()`, and `executeTaskWithTools()` all call `e.git` methods.

**Fix**:
- Add nil guards before every `e.git` call:
  ```go
  if e.git == nil {
      return nil, fmt.Errorf("git not initialized")
  }
  ```
- Or initialize `e.git` in `NewEngine()` with a no-op/stub implementation.
- Or make `SetGit()` mandatory and document it.

**Files**:
- `internal/workflow/ship.go` (lines 44, 128)
- `internal/workflow/engine.go` (wherever `e.git` is used)
- `internal/workflow/execute.go` (lines 177-184)

---

### Fix #5: Bash `waitErr` data race — `internal/tools/bash.go`

**Problem**: `waitErr` written in goroutine (line 101) and read in main goroutine (line 156) with no synchronization.

**Fix**:
- Use an `error` channel instead of a shared variable:
  ```go
  waitCh := make(chan error, 1)
  go func() { waitCh <- cmd.Wait() }()
  ```
- Or use `sync.WaitGroup` + atomic error storage.
- Or simply call `cmd.Wait()` directly in the main goroutine after signalling process exit.

**Files**: `internal/tools/bash.go` (lines 99-105, 156)

---

### Fix #6: Bash goroutine leak on Start failure — `internal/tools/bash.go`

**Problem**: If `cmd.Start()` fails, pipe read/write ends are never closed, and goroutines at lines 114/123 block forever.

**Fix**:
- On `cmd.Start()` failure, close all pipe ends and return immediately:
  ```go
  if err := cmd.Start(); err != nil {
      stdoutW.Close()
      stderrW.Close()
      stdoutR.Close()
      stderrR.Close()
      return nil, err
  }
  ```

**Files**: `internal/tools/bash.go` (line 73)

---

### Fix #7: Heal silent success — `internal/workflow/execute.go`

**Problem**: `healTask()` returns `Success: true` when LLM produces no tool calls. No fix was applied.

**Fix**:
- After parsing tool calls, check if the slice is empty:
  ```go
  toolCalls := parseToolCalls(content)
  if len(toolCalls) == 0 {
      return taskrunner.TaskResult{Success: false, Error: "heal: no tool calls generated"}
  }
  ```

**Files**: `internal/workflow/execute.go` (lines 250-261)

---

### Fix #8: Streaming goroutine leak — `internal/tui/streaming.go`

**Problem**: Stream goroutine blocks on `iterator.Next()` if HTTP body never closes. Context cancellation doesn't unblock it.

**Fix**:
- Ensure the HTTP response body is closed when context is cancelled. This requires storing a reference to `resp.Body` in the streaming state and closing it on cancel.
- Or use a context-aware wrapper around `iterator.Next()` that checks context before each call.
- The cleanest fix: in `StartStreamCmd`, store the `http.Response` reference and close its body when the cancel function is called.

**Files**: `internal/tui/streaming.go` (lines 37-122)

---

## Wave 2: High Severity Fixes (Incorrect Behavior)

### Fix #9: JSON extraction bug — `internal/workflow/engine.go`

**Problem**: `extractJSONArray` finds first `[` with `strings.Index`, but `[` in surrounding text causes wrong extraction.

**Fix**:
- Instead of finding the first `[`, find the first `[` that is followed by a valid JSON structure.
- Better: use a two-pass approach — find all `[` positions, try to parse from each, return first successful parse.
- Or: strip markdown code fences first, then find `[`.

**Files**: `internal/workflow/engine.go` (lines 370-384)

---

### Fix #10: Duplicate tool call parsing — `internal/workflow/engine.go`

**Problem**: `parseToolCalls` scans at every `{` position. Nested `{` inside valid JSON can be parsed as separate calls.

**Fix**:
- After successfully parsing a tool call at position `i`, skip past the end of that JSON object before continuing the scan.
- Track the furthest position scanned and don't re-parse overlapping regions.

**Files**: `internal/workflow/engine.go` (lines 586-602)

---

### Fix #11: Nil context — `internal/tui/commands.go`

**Problem**: `FetchModels(nil)` called with nil context.

**Fix**:
- Replace `nil` with `context.Background()` at both call sites.

**Files**: `internal/tui/commands.go` (lines 234, 733)

---

### Fix #12: Content loss on partial stream error — `internal/workflow/engine.go`

**Problem**: `consumeStream` returns error before processing chunk when both chunk and error are non-nil.

**Fix**:
- Process the chunk first, then check for error:
  ```go
  chunk, err := iterator.Next()
  if chunk != nil && chunk.Delta != "" {
      sb.WriteString(chunk.Delta)
  }
  if err != nil {
      return sb.String(), err
  }
  ```

**Files**: `internal/workflow/engine.go` (lines 311-313)

---

### Fix #13: Context ignored in task runner — `pkg/taskrunner/runner.go`

**Problem**: Per-task context created with `context.Background()`, discarding parent context cancellation.

**Fix**:
- Use the parent context as the base:
  ```go
  taskCtx, cancel := context.WithTimeout(ctx, r.TaskTimeout)
  ```

**Files**: `pkg/taskrunner/runner.go` (line 184)

---

### Fix #14: Wrong truncation count — `internal/tools/glob.go`

**Problem**: After truncating `matches` to `maxResults`, `len(matches)-maxResults` is always 0.

**Fix**:
- Save the original length before truncation:
  ```go
  truncated := len(matches) - maxResults
  if truncated > 0 {
      matches = matches[:maxResults]
  }
  ```

**Files**: `internal/tools/glob.go` (lines 90-99)

---

### Fix #15: TOCTOU path traversal — `internal/tools/fileread.go`

**Problem**: Symlink resolved then checked, but between check and `os.Open`, file could be swapped.

**Fix**:
- Use `os.Open` on the resolved path directly without re-checking.
- Or use `unix.O_NOFOLLOW` flag to prevent following symlinks.
- Or: after opening, `f.Stat()` and verify the inode matches the pre-check.

**Files**: `internal/tools/fileread.go` (lines 66-81)

---

### Fix #16: Symlink bypass in filewrite — `internal/tools/filewrite.go`

**Problem**: If target doesn't exist, `os.Stat` fails and symlinks in parent dirs aren't resolved.

**Fix**:
- Resolve the full path through symlinks using `filepath.EvalSymlinks` on the parent directory, even if the target file doesn't exist yet.
- Or use `filepath.Dir(targetPath)` to resolve the parent directory.

**Files**: `internal/tools/filewrite.go` (line 86)

---

### Fix #17: Empty data SSE parse — `internal/provider/openrouter/client.go`

**Problem**: `ParseSSEChunk("", modelID)` called when `eventType == "message"` but `data == ""`.

**Fix**:
- Add guard before the unconditional `ParseSSEChunk` call:
  ```go
  if data != "" {
      chunk, err := provider.ParseSSEChunk(data, modelID)
  }
  ```

**Files**: `internal/provider/openrouter/client.go` (lines 204-215), `internal/provider/zen/client.go` (same pattern)

---

### Fix #18: Healed task status not updated — `internal/workflow/verify.go`

**Problem**: Healed task that passes re-verification never gets status set to `StatusDone`.

**Fix**:
- After successful re-verification, set the status:
  ```go
  if newResult.FilesExist && newResult.SyntaxOK && newResult.TestsOK {
      tasks[i].Status = m31types.StatusDone
      e.logger.Info("task healed and verified", "id", task.ID)
  }
  ```

**Files**: `internal/workflow/verify.go` (lines 98-105)

---

## Wave 3: Medium Severity Fixes (Operational)

### Fix #19: Silent Zen failures — `internal/provider/zen/client.go`

**Problem**: Zen `FetchModels` swallows all errors with no logging.

**Fix**:
- Add `log.Printf("[zen] failed to fetch models: %v; falling back to cache", err)` to every error path, matching the OpenRouter pattern.

**Files**: `internal/provider/zen/client.go` (lines 72-91)

---

### Fix #20: Register accepts empty name — `internal/provider/registry.go`

**Problem**: `Registry.Register("", provider)` succeeds.

**Fix**:
- Add validation at the top of `Register()`:
  ```go
  if name == "" {
      return fmt.Errorf("provider name cannot be empty")
  }
  ```

**Files**: `internal/provider/registry.go`

---

### Fix #21: Unused stream timeout — `internal/provider/sse.go`

**Problem**: `DefaultStreamTimeout = 5min` defined but never enforced.

**Fix**:
- Option A: Remove the unused constant (simplest).
- Option B: Enforce it in the streaming iterator by passing a context with timeout.
- Given AGENTS.md says "NO body read timeout (streaming)", Option A is correct — remove the constant.

**Files**: `internal/provider/sse.go`

---

### Fix #22: filepath.Walk errors swallowed — `internal/workflow/engine.go`

**Problem**: `filepath.Walk` errors silently discarded.

**Fix**:
- Log the error: `e.logger.Warn("walk error", "path", path, "error", err)`
- Still return `nil` to continue walking (best effort), but log it.

**Files**: `internal/workflow/engine.go` (line 750)

---

### Fix #23: Duplicate tool call IDs — `internal/workflow/engine.go`

**Problem**: Tool call IDs use `time.Now().UnixNano()` — identical if parsed within same nanosecond.

**Fix**:
- Add a counter or use `crypto/rand`:
  ```go
  var callCounter int64
  ID: fmt.Sprintf("call_%s_%d", name, atomic.AddInt64(&callCounter, 1))
  ```
- Or attach the counter to the engine struct.

**Files**: `internal/workflow/engine.go` (line 638)

---

### Fix #24: Git ops uncancelable — `internal/workflow/execute.go`

**Problem**: `git.AddAll()` and `git.Commit()` ignore context. Cancelled workflow still commits.

**Fix**:
- Check context before git operations:
  ```go
  if ctx.Err() != nil {
      return ctx.Err()
  }
  ```
- Or pass context to git methods (requires changes to `internal/git/git.go`).

**Files**: `internal/workflow/execute.go` (lines 177-184)

---

### Fix #25: Task ID 0 allowed — `internal/workflow/engine.go`

**Problem**: Error says "missing ID" but task with ID=0 still gets added to `idSet`.

**Fix**:
- After flagging the error, skip adding to `idSet`:
  ```go
  if t.ID == 0 {
      errs = append(errs, fmt.Sprintf("task: missing ID"))
      continue  // Don't add to idSet
  }
  ```

**Files**: `internal/workflow/engine.go` (lines 428-430)

---

### Fix #26: Invalid input in settings — `internal/tui/settings.go`

**Problem**: Int fields accept `--5`, float fields accept `1.2.3`.

**Fix**:
- Track whether `-` has already been used, and only allow it at position 0.
- For floats, track `.` usage similarly.
- Validate on application (`applyFieldValues`) with `strconv.ParseInt`/`strconv.ParseFloat` and reject invalid values.

**Files**: `internal/tui/settings.go` (lines 284-297)

---

### Fix #27: UTF-8 corruption in cmdpalette — `internal/tui/cmdpalette.go`

**Problem**: String truncation by byte index splits multi-byte characters.

**Fix**:
- Use rune slicing:
  ```go
  runes := []rune(entry)
  if len(runes) > maxLen {
      entry = string(runes[:maxLen]) + "..."
  }
  ```

**Files**: `internal/tui/cmdpalette.go` (lines 183-188)

---

### Fix #28-29: Type assertion panics — `internal/tui/resume.go`, `internal/tui/modelselector.go`

**Problem**: `item.(sessionItem)` and `item.(ModelItem)` panic if wrong type.

**Fix**:
- Use comma ok idiom:
  ```go
  si, ok := item.(sessionItem)
  if !ok {
      return m, nil
  }
  ```

**Files**:
- `internal/tui/resume.go` (line 283)
- `internal/tui/modelselector.go` (lines 223, 236)

---

### Fix #30: nil, nil return in git status — `internal/git/git.go`

**Problem**: `StatusPorcelain()` returns `nil, nil` on error.

**Fix**:
- Return the error:
  ```go
  if err != nil {
      return nil, fmt.Errorf("git status: %w", err)
  }
  ```

**Files**: `internal/git/git.go` (line 189)

---

### Fix #31: Timestamp parse ignored in git log — `internal/git/git.go`

**Problem**: Malformed commit timestamps silently produce zero `time.Time`.

**Fix**:
- Log the error: `log.Printf("git: failed to parse timestamp %q: %v", parts[4], err)`
- Or return the error to the caller.

**Files**: `internal/git/git.go` (line 141)

---

### Fix #32: Corrupt ledger entries silent — `pkg/ledger/ledger.go`

**Problem**: `parseInt`/`parseFloat` return 0 on failure.

**Fix**:
- Return an error or log a warning: `l.logger.Warn("failed to parse int", "value", v, "error", err)`

**Files**: `pkg/ledger/ledger.go` (lines 500-510)

---

### Fix #33: Timestamp parse ignored in planning — `pkg/session/planning.go`

**Problem**: Malformed STATE.md timestamp silently produces zero `time.Time`.

**Fix**:
- Log the error during state loading.

**Files**: `pkg/session/planning.go` (line 282)

---

### Fix #34: Non-deterministic planning output — `pkg/session/planning.go`

**Problem**: Comment says "Sort keys" but sorting is missing.

**Fix**:
- Add `sort.Strings(keys)` before the iteration.

**Files**: `pkg/session/planning.go` (lines 31-36)

---

### Fix #35: Checkpoint failure ignored — `internal/workflow/engine.go`

**Problem**: `Transition` proceeds even when `SaveCheckpoint` fails.

**Fix**:
- Return the error instead of continuing:
  ```go
  if err := e.sessionMgr.SaveCheckpoint(...); err != nil {
      return fmt.Errorf("save checkpoint: %w", err)
  }
  ```
- Or at minimum, return a warning flag that the caller can handle.

**Files**: `internal/workflow/engine.go` (lines 177-179)

---

### Fix #36: Unbounded FetchModels — `internal/tui/repl.go`

**Problem**: `FetchModels(context.Background())` during auto-arbitrage has no timeout.

**Fix**:
- Add a timeout:
  ```go
  ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
  defer cancel()
  allModels, err := p.FetchModels(ctx)
  ```

**Files**: `internal/tui/repl.go` (line 269)

---

### Fix #37: Negative timeout in bash — `internal/tools/bash.go`

**Problem**: `context.WithTimeout` with negative duration creates already-expired context.

**Fix**:
- Clamp to a minimum:
  ```go
  if timeoutSec < 1 {
      timeoutSec = 30 // default
  }
  ```

**Files**: `internal/tools/bash.go` (line 51)

---

### Fix #38: ReDoS risk in grep — `internal/tools/grep.go`

**Problem**: User-supplied regex pattern passed directly to `regexp.Compile`.

**Fix**:
- Use `regexp.MustCompile` with a timeout wrapper, or document that complex patterns may be slow.
- Go's `regexp` package is ReDoS-safe (uses RE2), so this is actually low risk. But still worth validating pattern length (e.g., max 1000 chars).

**Files**: `internal/tools/grep.go` (line 174)

---

### Fix #39: Empty params on JSON error — `internal/tools/dispatcher.go`

**Problem**: Malformed tool call input creates empty `ToolInput` instead of returning error.

**Fix**:
- Return the error:
  ```go
  if err := json.Unmarshal(call.Input, &input); err != nil {
      return nil, fmt.Errorf("tool %s: invalid input JSON: %w", call.Name, err)
  }
  ```

**Files**: `internal/tools/dispatcher.go` (lines 60-62)

---

### Fix #40: Redirect not validated — `internal/tools/webfetch.go`

**Problem**: HTTP client follows redirects to any URL.

**Fix**:
- Set `CheckRedirect` to limit redirects or validate target hostname:
  ```go
  client.CheckRedirect = func(req *http.Request, via []*http.Request) error {
      if len(via) >= 5 {
          return errors.New("stopped after 5 redirects")
      }
      return nil
  }
  ```

**Files**: `internal/tools/webfetch.go` (line 91)

---

## Wave 4: Low Severity & Code Quality

### Fix #41-58: Remaining low-severity issues

| # | File | Fix |
|---|------|-----|
| 41 | `internal/provider/reasoning.go:46-56` | Sort keys before iteration for deterministic prefix matching |
| 42 | `internal/provider/fallback.go:30` | Increase timeout to 10s to detect "degraded" providers |
| 43 | `internal/workflow/engine.go:482-501` | Convert recursive `hasCycle` DFS to iterative with explicit stack |
| 44 | `internal/workflow/prompts/execute-task.md:32` | Update prompt commit format to match code: `feat: <description>` |
| 45 | `internal/workflow/prompts/self-heal.md:24` + `execute.go:109` | Either change loop to `< MaxHealAttempts` or update prompt to say "3 attempts" |
| 46 | `internal/tui/header.go:96` | Use lipgloss.Truncate or rune-aware truncation for styled strings |
| 47 | `internal/tui/keybindings.go:105-107` | Remove dead `time.AfterFunc` call |
| 48 | `internal/tui/sidebar.go:96-99` | Display actual error message: `m.err` instead of hardcoded string |
| 49 | `internal/tui/components/toolcard.go:88-91` | Remove dead `isBinaryContent` check, or move it before sanitization |
| 50 | `internal/tui/firstrun.go:115-116` | Use `len(m.options)` instead of hardcoded `3` |
| 51 | `internal/tui/app.go:94` | Log `os.Getwd()` error, use fallback "." |
| 52 | `internal/tui/app.go:98` | Log `config.Load()` error to user |
| 53 | `internal/tui/app.go:104` | Log `keychain.New()` error |
| 54 | `internal/workflow/execute.go:109` | Change `<=` to `<` for 2 attempts, or update prompt |
| 55 | `pkg/rollback/rollback.go:267` | Return error or swap hashes for reverse order |
| 56 | `pkg/autodream/autodream.go:174` | Append summary instead of prepending to prevent accumulation |
| 57 | `pkg/session/manager.go:51` | Use `os.OpenFile` with 0644 permissions |
| 58 | `internal/tools/bash_windows.go` | Add runtime check for bash availability |

---

## Wave 5: AGENTS.md Violations — Tool Inventory Update

### Current state vs documented rules

The AGENTS.md says V1 tools should be "Bash, FileRead, FileWrite, Glob, Grep ONLY" but the actual codebase has additional tools. Since the user confirmed "I have built the things which I needed for V1", we update AGENTS.md to reflect reality:

**Tools that exist but weren't documented**:
1. `FileEdit` (`internal/tools/edit.go`) — atomic file editing with backup
2. `WebFetch` (`internal/tools/webfetch.go`) — URL content fetching
3. `TodoWrite` (`internal/tools/todo.go`) — session TODO management
4. `AskUserQuestion` (`internal/tools/question.go`) — interactive questions (not used in automated V1 flows)
5. Tool dispatcher with permission system (`internal/tools/dispatcher.go`)

**Updated AGENTS.md tool section**:
```
- V1 tools: Bash, FileRead, FileWrite, Glob, Grep (core).
  Additional tools available: FileEdit, WebFetch, TodoWrite, AskUserQuestion.
  AskUserQuestion is available for interactive sessions but MUST NOT be
  used in automated task execution flows.
- V1 task execution is SEQUENTIAL. The dispatcher handles permission
  prompts synchronously.
```

**Updated Absolute Prohibitions**:
```
- DO NOT add new tools beyond the current set without explicit request
- DO NOT use AskUserQuestion in automated task execution
- DO NOT add direct Anthropic or OpenAI provider support
- DO NOT use CSS-style animations
- DO NOT implement V1.1 features (ghost mode, PiP, subagents, deferred tools)
- DO NOT store API keys in plaintext
- DO NOT add telemetry or analytics
- DO NOT hardcode model lists
```

---

## Execution Order & Dependencies

```
Wave 0 (AGENTS.md) — can be done immediately
Wave 1 (Critical) — independent, any order
Wave 2 (High) — depends on Wave 1 (some fixes touch same files)
Wave 3 (Medium) — independent
Wave 4 (Low) — independent
Wave 5 (AGENTS.md update) — done alongside Wave 0
```

## Files Modified Summary

| Wave | Files | Changes |
|------|-------|---------|
| 0 | `AGENTS.md` | Update tool rules, prohibitions |
| 1 | `cache.go`, `loader.go`, `app.go`, `ship.go`, `bash.go`, `streaming.go`, `execute.go` | 7 files, race conditions, leaks, panics |
| 2 | `engine.go`, `commands.go`, `glob.go`, `fileread.go`, `filewrite.go`, `verify.go`, `openrouter/client.go`, `zen/client.go` | 8 files, logic bugs |
| 3 | `zen/client.go`, `registry.go`, `sse.go`, `git.go`, `ledger.go`, `planning.go`, `settings.go`, `cmdpalette.go`, `resume.go`, `modelselector.go`, `repl.go`, `grep.go`, `dispatcher.go`, `webfetch.go` | 14 files, operational fixes |
| 4 | `reasoning.go`, `fallback.go`, `header.go`, `keybindings.go`, `sidebar.go`, `toolcard.go`, `firstrun.go`, `rollback.go`, `autodream.go`, `manager.go`, prompt files | 12 files, code quality |

**Total: ~42 files modified across 5 waves.**
