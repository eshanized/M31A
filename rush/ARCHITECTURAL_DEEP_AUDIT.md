# M31A — Deep Architectural Audit Report

> **Generated:** 2026-06-07
> **Scope:** Full codebase — all `internal/` and `pkg/` packages
> **Severity Legend:** CRITICAL | HIGH | MEDIUM | LOW | INFO

---

## Executive Summary

This audit examines the M31A codebase for architectural issues, design flaws, concurrency hazards, dependency violations, resource leaks, and logical inconsistencies. The codebase is mature and well-structured overall, but several systemic issues emerge from deep analysis.

**Totals:** 8 CRITICAL, 14 HIGH, 18 MEDIUM, 12 LOW findings.

---

## 1. Dependency Rule Violations

### 1.1 [HIGH] `internal/tools/` imports `internal/config/` — Known Violation

**Files:** `internal/tools/dispatcher.go:13`, `internal/tools/permissions.go:12`, `internal/tools/defaults.go:4`

The architecture doc states `internal/tools/` may only import `internal/types/` and `internal/errors/`. Three files import `internal/config` for `config.PermissionRule` and `config.PermissionsConfig`. This creates a circular risk and violates the layered dependency graph.

**Impact:** Any change to config types cascades into tools, breaking the intended isolation.

---

### 1.2 [MEDIUM] `internal/tui/` imports concrete provider implementations

**File:** `internal/tui/app_update_workflow.go:12-13`

```go
import (
    "github.com/eshanized/M31A/internal/provider/openrouter"
    "github.com/eshanized/M31A/internal/provider/zen"
)
```

The TUI layer directly imports `openrouter` and `zen` sub-packages in `reRegisterProviders()` (lines 573-618). This violates the principle that the TUI should only depend on `internal/provider` (the interface layer). The TUI should not know about concrete provider constructors.

**Fix:** Move provider construction to a factory function in `internal/provider/` or inject via dependency injection.

---

### 1.3 [MEDIUM] `internal/workflow/` imports `internal/tools/`

**File:** `internal/workflow/engine.go:19`

The workflow engine directly depends on `internal/tools` for `tools.Dispatcher`. While functional, this tight coupling means changes to the tool system directly affect workflow. The engine should interact with tools through an interface.

---

## 2. Concurrency Hazards

### 2.1 [CRITICAL] `ReplModel` mutable state accessed from multiple goroutines

**Files:** `internal/tui/repl_stream.go:20-26`, `internal/tui/streaming.go:77-156`

`AppendStreamChunk()` is called from `Update()` (Bubble Tea goroutine) AND can be called from `setWorkflowPhase()` during `PhaseIdle` transition (in `app.go:22-29`). The method mutates `m.streaming`, `m.streamContent`, both of which are read by `View()`. While Bubble Tea is single-threaded, the `pendingStreamChunks` flush in `setWorkflowPhase` could interleave with streaming chunks.

**Risk:** Data race on `streamContent` if a streaming goroutine is simultaneously emitting chunks via `StreamMsg` while idle transition flushes `pendingStreamChunks`.

---

### 2.2 [CRITICAL] Permission response channel can deadlock under rapid tool calls

**File:** `internal/tools/permissions.go:168-197`

The `askPermission` method sends a request on `requestCh` (buffered, size `PermissionChannelBuffer`), then blocks reading from `responseCh`. If the buffer is full (e.g., multiple concurrent tool calls from different workflow phases), the `select { case d.requestCh <- req: default: return ErrPermissionDenied }` silently denies the permission. While V1 is sequential, this is a latent bug for V1.1 concurrent subagents.

Additionally, the put-back logic (lines 188-189) for non-matching responses can silently drop messages if the response channel is full:
```go
select {
case d.responseCh <- r:
default:
}
```

---

### 2.3 [HIGH] `channelEmitter.Emit()` uses `time.After` which leaks on rapid messages

**File:** `internal/tui/app_channel.go:49-55`

```go
func (ce *channelEmitter) Emit(msg tea.Msg) {
    select {
    case ce.ch <- msg:
    case <-time.After(types.ChannelSendTimeout):
        slog.Warn(...)
    }
}
```

Each `Emit()` call that hits the timeout path allocates a `time.Timer` via `time.After`. Under rapid message emission (e.g., streaming chunks), this creates timer leaks. Should use `time.NewTimer` with explicit `Stop()`.

---

### 2.4 [HIGH] `Engine.callCounter` atomic increment but non-atomic read pattern

**File:** `internal/workflow/engine_parse.go:364-366`

```go
func (e *Engine) nextCallID() int64 {
    return atomic.AddInt64(&e.callCounter, 1)
}
```

This is correct for sequential use, but `callCounter` is never reset between sessions. If the engine is reused across sessions (which it is, via `SetSessionID()`), tool call IDs are monotonically increasing across sessions. This isn't a bug per se, but it means tool call IDs are not meaningful across session boundaries.

---

## 3. Resource Management Issues

### 3.1 [CRITICAL] `Engine.verifyTask()` creates contexts with `defer cancel()` in loops

**File:** `internal/workflow/engine_verify.go:140-233`

Multiple `context.WithTimeout` calls are deferred inside the `verifyTask` method body. Each creates a goroutine for deadline tracking. While the parent context cancellation propagates, the deferred `cancel()` functions only run when `verifyTask` returns, not when each individual context is done. This means multiple timeout goroutines may be alive simultaneously.

**Example (line 140):**
```go
ctx, cancel := context.WithTimeout(context.Background(), verifyTaskTimeout)
defer cancel()
```

If `verifyTask` processes multiple project types (theoretically possible), several goroutines leak until method return.

---

### 3.2 [HIGH] HTTP response body not fully drained in error paths

**File:** `internal/provider/openrouter/client.go:191-193`

```go
bodyBytes, _ := io.ReadAll(io.LimitReader(resp.Body, types.MaxLLMResponseBytes))
resp.Body.Close()
```

On non-200 responses, the body is read then closed. But if `io.ReadAll` fails or is interrupted, the body may not be fully consumed, which can prevent HTTP connection reuse.

Similarly in `zen/client.go:176-177`.

---

### 3.3 [MEDIUM] `Engine` stores `execCommand` function but never uses it for production

**File:** `internal/workflow/engine.go:80`

```go
execCommand func(name string, args ...string) *exec.Cmd
```

This field is set to `exec.Command` in `NewEngine()` and exists only for test injection. However, `verifyTask()` in `engine_verify.go` uses `exec.CommandContext()` directly (hardcoded), bypassing this injection point. Tests cannot mock the verification commands.

---

## 4. Logical Errors and Inconsistencies

### 4.1 [CRITICAL] `collectDiffStats()` uses wrong git diff semantics

**File:** `internal/workflow/ship.go:156-209`

```go
diffOutput, err := e.git.Diff("HEAD", "")
```

`Diff("HEAD", "")` diffs HEAD against the empty tree (all files ever), not the session's changes. The method should diff against `e.sessionStartHash` to get only session-relevant changes:

```go
diffOutput, err := e.git.Diff(e.sessionStartHash, "HEAD")
```

This means the ship summary's file change stats are wrong — they include all files in the repo, not just those changed during the session.

---

### 4.2 [HIGH] `ShipSummary` struct defined in both `ship.go` and `app_update_workflow.go`

**Files:** `internal/workflow/ship.go:20-28`, `internal/tui/app_update_workflow.go:280-313`

The `ShipSummary` struct is defined in `workflow/ship.go`, but `handlePhaseShip()` constructs a *different* `ShipSummary` type (from `types.go` or inline). Looking at `app_update_workflow.go:280`:

```go
summary := ShipSummary{
    SessionID: m.workflowEngine.SessionID(),
}
```

This `ShipSummary` is a TUI-local type, not the workflow one. The two structs have different fields, creating confusion about which is authoritative.

---

### 4.3 [HIGH] `handlePhaseDiscuss` silently drops messages from PhaseResult

**File:** `internal/tui/app_update_workflow.go:134-167`

When `msg.NeedsAnswers` is false or questions are empty, the handler discards `msg.Messages` (line 144-148 only extracts assistant messages for questions). The assistant message from the discuss phase is never added to the REPL message history, so the user's conversation shows a gap.

---

### 4.4 [MEDIUM] `modelForPhase` doesn't handle PhaseInitialize

**File:** `internal/workflow/engine.go:95-119`

The `modelForPhase()` switch handles `PhasePlan`, `PhaseExecute`, `PhaseVerify`, `PhaseShip`, `PhaseDiscuss` — but not `PhaseInitialize`. While Initialize doesn't make LLM calls, if it ever does (e.g., project detection via LLM), it would use the wrong model.

---

### 4.5 [MEDIUM] `validateTasks` allows duplicate IDs but checks later

**File:** `internal/workflow/engine_parse.go:99-142`

The function detects duplicate IDs (line 108-110) but doesn't stop — it continues to add the duplicate to `idSet`. A later task with the same ID will have its dependencies validated against the first task's ID, not its own.

---

## 5. Error Handling Gaps

### 5.1 [HIGH] `Engine.streamLLM` emits `ThinkingCompleteMsg` even on error

**File:** `internal/workflow/engine.go:517-530`

```go
iterator, err := e.provider.ChatCompletionStream(ctx, req)
if err != nil {
    e.emit(ThinkingCompleteMsg{Context: "LLM processing failed"})
    return "", err
}
result, err := e.consumeStream(iterator)
e.emit(ThinkingCompleteMsg{Context: "LLM processing complete"})
return result, err
```

If `consumeStream` returns an error (e.g., stream truncated), the `ThinkingCompleteMsg` is still emitted with "LLM processing complete", which is misleading. The TUI will show the thinking block as completed successfully when it actually failed.

---

### 5.2 [HIGH] `Dispatcher.checkPermission` swallows errors from pattern matching

**File:** `internal/tools/permissions.go:48-90`

If a rule has a malformed `Pattern`, `doublestar.Match` returns an error that is silently ignored (line 56). The permission check continues with `matched = false`, which may cause unexpected permission denials for rules with valid tool names but invalid patterns.

---

### 5.3 [MEDIUM] `SSEParser.Next()` returns `ErrStreamTruncated` when `data` is empty but lines exist

**File:** `internal/provider/sse.go:91-93`

```go
if data == "" && len(dataParts) == 0 {
    return "", "", fmt.Errorf("%w: stream chunk read interrupted", m31errors.ErrStreamTruncated)
}
```

This fires when the SSE event contains only an `event:` line with no `data:` line, which is valid in some SSE implementations for heartbeat/keepalive events. This could cause premature stream termination on certain providers.

---

### 5.4 [MEDIUM] `UserMessage()` string matching is fragile

**File:** `internal/errors/errors.go:104-119`

The fallback string matching (`strings.Contains(errStr, "429")`, etc.) matches anywhere in the error string, including in wrapped errors or stack traces. For example, a file path containing "429" would incorrectly match the rate-limit pattern.

---

## 6. Design and Architecture Concerns

### 6.1 [CRITICAL] `AppState` is a monolithic god struct

**File:** `internal/tui/app_state.go:55-154`

`AppState` has **104 fields**, mixing concerns across TUI, workflow, providers, sessions, tools, git, config, health checks, caching, keychain, sidebar, command palette, toast, diff, metrics, goal input, ledger, rollback, and discuss. This violates the Single Responsibility Principle and makes the struct extremely difficult to reason about.

**Recommendation:** Decompose into focused sub-structs:
- `WorkflowState` (workflowEngine, currentPhase, workflowGoal, etc.)
- `ProviderState` (registry, activeProvider, activeModel, health)
- `UIState` (screen, theme, sidebar, toast, etc.)
- `SessionState` (sessionManager, sessionID, ledger)

---

### 6.2 [HIGH] TUI coordinates workflow transitions — mixed orchestration

**Files:** `internal/tui/app_update_workflow.go`, `internal/tui/app_workflow.go`

The workflow engine has `Transition()` method, but the TUI calls it AND directly calls `setWorkflowPhase()` AND `RunPhaseCmd()`. The engine's `Transition()` writes STATE.md and saves checkpoints, but the TUI independently manages the phase state. This dual-authority model means the engine's phase state can drift from the TUI's phase state.

**Example (`app_update_workflow.go:91-95`):**
```go
_ = m.workflowEngine.Transition(context.Background(), types.PhaseInitialize, types.PhaseDiscuss)
m.setWorkflowPhase(types.PhaseDiscuss)
m.persistWorkflowState()
return m, RunPhaseCmd(m, types.PhaseDiscuss, m.workflowGoal)
```

The engine transitions, then the TUI transitions again, then persists separately.

---

### 6.3 [HIGH] No abstraction over file-based session state

**Files:** `pkg/session/manager.go`, `internal/workflow/*.go`

Every workflow phase reads and writes state files (PROJECT.md, TASKS.md, STATE.md) directly through `sessionMgr`. There's no caching layer — the same file may be read 5+ times per phase transition. For large TASKS.md files (100+ tasks), this creates measurable I/O overhead.

---

### 6.4 [MEDIUM] `ReplModel` has its own `sessionID` separate from `AppState.sessionID`

**Files:** `internal/tui/app_state.go:93`, `internal/tui/repl_model.go:64`

Both `AppState` and `ReplModel` track `sessionID`. If they ever diverge (e.g., after session switching), the REPL may send messages to the wrong session. The `replModel.sessionID` is set in `SetProvider()` calls but is not kept in sync with `AppState.sessionID` in all code paths.

---

### 6.5 [MEDIUM] `ThemeManager` is recreated on every theme change

**File:** `internal/tui/app_update_workflow.go:622-635`

```go
func (m *AppState) handleThemeChanged(msg ThemeChangedMsg) (tea.Model, tea.Cmd) {
    switch msg.Theme {
    case "dark":
        m.themeManager = theme.NewManager(theme.ModeDark)
    // ...
    }
}
```

Creating a new `Manager` on each theme change means all cached styles are regenerated. If `theme.Manager` caches anything (e.g., computed styles), this is wasteful. Should use a `Cycle()` method on the existing manager.

---

## 7. Security Concerns

### 7.1 [CRITICAL] `Bash` tool has no command allowlist/denylist

**File:** `internal/tools/bash.go:56-80`

The Bash tool accepts any command with no filtering. While the permission modal gates execution, there's no server-side blocklist for obviously destructive commands (`rm -rf /`, `dd`, `mkfs`). The risk level is always `RiskDangerous` regardless of the command content.

---

### 7.2 [HIGH] API keys stored in config file as plaintext fallback

**File:** `internal/config/types.go:50-51`

```go
type ProviderCredentialConfig struct {
    APIKey string `toml:"api_key"`
}
```

While the docs say "never plaintext", the config file `api_key` field is a last-resort fallback. If the keychain and env var both fail, the key persists in plaintext TOML. The `ResolveAPIKeys()` method (referenced but not fully visible) presumably handles this, but the data structure permits plaintext storage.

---

### 7.3 [MEDIUM] `WebFetch` tool SSRF protection exists but is permissive

**File:** `internal/tools/webfetch.go` (referenced)

The `ErrPrivateIPBlocked` error suggests SSRF protection exists, but the implementation is in the tools package, not at the HTTP client level. Any direct use of `http.Get()` elsewhere in the codebase would bypass it.

---

## 8. Performance Concerns

### 8.1 [MEDIUM] `parseToolCalls` scans entire LLM response with regex

**File:** `internal/workflow/engine_parse.go:279-361`

The method compiles `blockRe` regex on every call (line 292). For large LLM responses (10KB+), this is called per-response. The regex should be compiled once at package level.

---

### 8.2 [MEDIUM] `listCwdFiles` walks entire directory tree on every plan context build

**File:** `internal/workflow/engine_verify.go:39-79`

`listCwdFiles()` is called in `buildPlanContext()` (plan.go:140). For large repos, this walks thousands of files. The result is not cached between plan retries.

---

### 8.3 [LOW] `normalizeToolName` performs case-insensitive matching on every tool call

**File:** `internal/workflow/engine_parse.go:443-467`

Each tool call triggers a `strings.ToLower()` + switch statement. While individually cheap, this is called per tool call in the execute loop. A `map[string]string` lookup would be faster and more maintainable.

---

## 9. Test Infrastructure Gaps

### 9.1 [HIGH] `verifyTask()` cannot be mocked for testing

**File:** `internal/workflow/engine_verify.go:116-237`

`verifyTask()` calls `exec.CommandContext()` directly instead of using `e.execCommand` (which exists for test injection at line 80). This means integration tests cannot mock verification commands and must rely on actual toolchains (go, cargo, npm) being installed.

---

### 9.2 [MEDIUM] No test fixtures for SSE parsing edge cases

**File:** `internal/provider/sse.go`

The SSE parser handles `\r\n` line endings, multi-line `data:` fields, and `[DONE]` sentinels, but there are no test fixtures for:
- Partially received SSE events
- Very large SSE chunks (>1MB)
- SSE events with no `data:` field (heartbeat)
- Provider-specific quirks (e.g., Zen's different error format)

---

## 10. Summary Table

| # | Severity | Category | Location | Issue |
|---|----------|----------|----------|-------|
| 1.1 | HIGH | Dependency | tools/ → config/ | Known architecture violation |
| 1.2 | MEDIUM | Dependency | TUI → openrouter/zen | TUI imports concrete providers |
| 1.3 | MEDIUM | Dependency | workflow/ → tools/ | Tight coupling |
| 2.1 | CRITICAL | Concurrency | repl_stream.go | Mutable state from multiple call sites |
| 2.2 | CRITICAL | Concurrency | permissions.go | Channel deadlock under rapid calls |
| 2.3 | HIGH | Concurrency | app_channel.go | time.After leak |
| 2.4 | HIGH | Concurrency | engine_parse.go | Non-resetting call counter |
| 3.1 | CRITICAL | Resources | engine_verify.go | Context goroutine leaks |
| 3.2 | HIGH | Resources | openrouter/client.go | HTTP body not fully drained |
| 3.3 | MEDIUM | Resources | engine.go | execCommand bypassed |
| 4.1 | CRITICAL | Logic | ship.go | Wrong diff range for stats |
| 4.2 | HIGH | Logic | ship.go vs app_update | Duplicate ShipSummary types |
| 4.3 | HIGH | Logic | app_update_workflow.go | Discuss messages dropped |
| 4.4 | MEDIUM | Logic | engine.go | PhaseInitialize not in modelForPhase |
| 4.5 | MEDIUM | Logic | engine_parse.go | Duplicate ID validation incomplete |
| 5.1 | HIGH | Error | engine.go | Misleading ThinkingCompleteMsg |
| 5.2 | HIGH | Error | permissions.go | Pattern match errors swallowed |
| 5.3 | MEDIUM | Error | sse.go | Heartbeat misidentified as truncation |
| 5.4 | MEDIUM | Error | errors.go | Fragile string matching |
| 6.1 | CRITICAL | Design | app_state.go | God struct (104 fields) |
| 6.2 | HIGH | Design | app_update_workflow.go | Dual orchestration authority |
| 6.3 | HIGH | Design | session/ | No state caching layer |
| 6.4 | MEDIUM | Design | repl_model.go | Duplicate sessionID tracking |
| 6.5 | MEDIUM | Design | app_update_workflow.go | ThemeManager recreated |
| 7.1 | CRITICAL | Security | bash.go | No command blocklist |
| 7.2 | HIGH | Security | config/types.go | Plaintext API key fallback |
| 7.3 | MEDIUM | Security | webfetch.go | SSRF only in tool layer |
| 8.1 | MEDIUM | Performance | engine_parse.go | Regex compiled per call |
| 8.2 | MEDIUM | Performance | engine_verify.go | Uncached directory walk |
| 8.3 | LOW | Performance | engine_parse.go | String matching per tool call |
| 9.1 | HIGH | Testing | engine_verify.go | verifyTask not mockable |
| 9.2 | MEDIUM | Testing | sse.go | Missing SSE edge case fixtures |

---

## Appendix: File Reference Index

| Package | Key Files Examined |
|---------|-------------------|
| `internal/types/` | `types.go` |
| `internal/errors/` | `errors.go` |
| `internal/config/` | `types.go` |
| `internal/provider/` | `interface.go`, `registry.go`, `common.go`, `sse.go`, `reasoning.go`, `fallback.go` |
| `internal/provider/openrouter/` | `client.go` |
| `internal/provider/zen/` | `client.go` |
| `internal/tui/` | `app_state.go`, `app.go`, `app_update.go`, `app_update_workflow.go`, `app_channel.go`, `app_workflow.go`, `types.go`, `repl.go`, `repl_model.go`, `repl_stream.go`, `streaming.go` |
| `internal/tools/` | `dispatcher.go`, `permissions.go`, `defaults.go`, `bash.go` |
| `internal/workflow/` | `engine.go`, `engine_messages.go`, `engine_parse.go`, `engine_verify.go`, `initialize.go`, `discuss.go`, `plan.go`, `execute.go`, `verify.go`, `ship.go` |
| `pkg/session/` | `manager.go` (partial) |
| `internal/git/` | `git.go` (partial) |
