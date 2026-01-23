# M31A — Deadlock & Concurrency Issues Report

> **Generated:** 2026-06-10
> **Scope:** Full codebase — 198 Go source files, 14 files with `sync` imports, 11 mutex instances, 6+ goroutine patterns
> **Methodology:** Static analysis of all mutex, channel, goroutine, context, and sync primitive usage; manual review of lock ordering, TOCTOU windows, goroutine lifecycles, and Bubble Tea threading model compliance.

---

## Executive Summary

**No hard deadlocks** (mutual exclusion cycles) were found. The codebase demonstrates solid concurrency hygiene — all mutexes have matching lock/unlock pairs, no nested lock acquisition exists, and channels are appropriately buffered.

However, **17 issues** were identified across 4 severity levels:

| Severity | Count | Description |
|----------|-------|-------------|
| **Critical** | 2 | Goroutine leak + silent message drop; context disconnect from shutdown |
| **High** | 5 | Mutex held during blocking I/O; shared channel routing; question response leak |
| **Medium** | 6 | TOCTOU races; rate limiter goroutine leak; SSE scanner blocking |
| **Low** | 4 | No file locking; minor race windows; design assumptions |

---

## Critical Issues (C-1 through C-2)

### C-1: Goroutine Leak in Question Response Handler

**File:** `internal/tui/app_update.go:1189-1191`

```go
go func() {
    respCh <- tools.QuestionResponse{Answer: msg.Answer}
}()
```

**Problem:** If `respCh` is the shared `questionRespCh` channel and it is full (previous response not consumed), this goroutine blocks **forever**. There is no timeout, no context cancellation, and no `select` fallback. The channel is buffered (`QuestionChannelBuffer`), but if multiple `AskUserQuestion` calls are in flight, the second response blocks indefinitely.

**Impact:** Goroutine leak. If the TUI is shut down while this goroutine is blocked, it never exits. Over time, repeated question prompts leak goroutines.

**Fix:** Use a `select` with timeout:

```go
go func() {
    select {
    case respCh <- tools.QuestionResponse{Answer: msg.Answer}:
    case <-time.After(30 * time.Second):
        slog.Warn("question response dropped: channel full")
    }
}()
```

---

### C-2: Task Runner Context Disconnected from Shutdown

**File:** `pkg/taskrunner/runner.go:189-195`

```go
taskCtx, cancel = context.WithTimeout(context.Background(), r.TaskTimeout)
// or
taskCtx, cancel = context.Background(), func() {}
```

**Problem:** The task runner creates a context detached from any parent using `context.Background()`. When the TUI shuts down (via `shutdownCtx`), individual tasks continue running until their 30-minute timeout expires. The backoff select (lines 208-215) uses `taskCtx`, not the parent context, so cancellation of the parent doesn't interrupt retries.

**Impact:** Tasks continue executing after the user has exited the application. Bash commands with 30-minute timeouts keep running. This can leave orphaned processes and corrupted session state.

**Fix:** Propagate parent context:

```go
if r.TaskTimeout > 0 {
    taskCtx, cancel = context.WithTimeout(parentCtx, r.TaskTimeout)
} else {
    taskCtx, cancel = context.WithCancel(parentCtx)
}
```

The `ExecuteGroup` signature should accept `ctx context.Context` as a parameter, and `ExecuteFunc` should use the provided context.

---

## High Severity Issues (H-1 through H-5)

### H-1: Mutex Held During Potentially-Blocking Pipe Write

**File:** `internal/tools/bash.go:263-276`

```go
func (lw *limitWriter) Write(p []byte) (int, error) {
    lw.mu.Lock()                          // Lock acquired
    remaining := lw.limit - lw.written
    if remaining <= 0 {
        lw.mu.Unlock()
        return len(p), nil
    }
    if int64(len(p)) > remaining {
        p = p[:remaining]
    }
    n, err := lw.w.Write(p)               // ← BLOCKING: io.Pipe Write
    lw.written += int64(n)
    lw.mu.Unlock()                        // Unlock only after write completes
    return n, err
}
```

**Problem:** `lw.w` is an `io.Pipe` writer. `io.Pipe.Write` blocks until the corresponding `io.Pipe.Read` consumes data. If the reader goroutine is stuck (e.g., TUI is paused, output processing is slow, or context is cancelled before readers drain), `lw.w.Write(p)` blocks while holding `lw.mu`.

This means the Bash process's stdout/stderr writes can stall indefinitely if the pipe reader is blocked. The process itself may hang, waiting for its stdout buffer to drain.

**Impact:** Bash tool hangs if the pipe reader is interrupted. The 30-minute Bash timeout may fire, but the mutex is held across the entire duration.

**Fix:** Check limit before acquiring the lock, or use a non-blocking write with a fallback:

```go
func (lw *limitWriter) Write(p []byte) (int, error) {
    lw.mu.Lock()
    remaining := lw.limit - lw.written
    if remaining <= 0 {
        lw.mu.Unlock()
        return len(p), nil
    }
    if int64(len(p)) > remaining {
        p = p[:remaining]
    }
    lw.mu.Unlock()    // Release lock before potentially-blocking write

    n, err := lw.w.Write(p)

    lw.mu.Lock()
    lw.written += int64(n)
    lw.mu.Unlock()
    return n, err
}
```

---

### H-2: Shared Question Channel — Cross-Caller Response Routing

**File:** `internal/tools/question.go:120-139` + `internal/tui/app.go:159-172`

```go
// question.go — all AskUserQuestion instances share the same responseCh
func (t *AskUserQuestion) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
    // ...
    select {
    case t.requestCh <- req:
    // ...
    case resp := <-t.responseCh:   // ← shared channel
    }
}

// app.go — questionListenerCmd always returns the same shared channel
func questionListenerCmd(ctx context.Context, d *tools.Dispatcher) tea.Cmd {
    return func() tea.Msg {
        select {
        case req := <-d.QuestionRequestCh():
            return QuestionRequestMsg{
                // ...
                ResponseCh: d.QuestionResponseCh(),  // ← shared among all callers
            }
        }
    }
}
```

**Problem:** All `AskUserQuestion` instances share a single `questionRespCh`. If two questions are asked concurrently (or in rapid succession), the first user response is consumed by the first question, but the second response is consumed by the second question — even if it was intended for the first.

This is a **correctness bug**, not a deadlock (timeouts prevent blocking). But user answers can be routed to the wrong question.

**Fix:** Use per-request response channels (same pattern as `pendingResponses sync.Map` in permissions):

```go
respCh := make(chan QuestionResponse, 1)
d.pendingQuestions.Store(req.ID, respCh)
defer d.pendingQuestions.Delete(req.ID)
```

---

### H-3: ApprovePermission Silent Drop on Full Channel

**File:** `internal/tools/permissions.go:16-30`

```go
func (d *Dispatcher) ApprovePermission(requestID int64, allowed bool, remember bool) {
    resp := PermissionResponse{RequestID: requestID, Allowed: allowed, Remember: remember}
    if ch, ok := d.pendingResponses.Load(requestID); ok {
        select {
        case ch.(chan PermissionResponse) <- resp:
        default:                              // ← SILENT DROP
        }
        return
    }
    // Fallback to shared channel
    select {
    case d.responseCh <- resp:
    default:                                  // ← SILENT DROP
    }
}
```

**Problem:** If the per-request channel is full (capacity 1, already has a response), the permission response is **silently dropped**. The tool goroutine waiting in `sendAndWaitForPermission` will then time out after `permissionTimeout` seconds and return `ErrPermissionDenied`.

**Impact:** User approves a permission, but the tool is denied due to timeout. The user sees "Permission denied" even though they approved. This is a UX correctness issue.

**Fix:** Log the drop and consider using a larger channel buffer or draining stale responses before sending.

---

### H-4: Dispatcher Rate Limiter Goroutine Leak

**File:** `internal/tools/dispatcher.go:57-65`

```go
d.rateTicker = time.NewTicker(time.Second / ToolRateLimitPerSec)
go func() {
    for range d.rateTicker.C {
        select {
        case d.rateTokens <- struct{}{}:
        default:
        }
    }
}()
```

**Problem:** The goroutine runs forever with no shutdown mechanism. The `rateTicker` is never stopped. If the `Dispatcher` is recreated (e.g., on session switch), the old goroutine and ticker leak.

**Impact:** Goroutine leak on session switch. Each session switch leaks one goroutine and one ticker. Over time, this accumulates.

**Fix:** Add a `Stop()` method to `Dispatcher`:

```go
func (d *Dispatcher) Stop() {
    d.rateTicker.Stop()
}
```

Call `Stop()` during session cleanup.

---

### H-5: SSE Parser Scanner Blocks Between Context Checks

**File:** `internal/provider/sse.go:44-67`

```go
for p.scanner.Scan() {
    // Check context cancellation between lines
    if p.ctx != nil {
        select {
        case <-p.ctx.Done():
            return "", "", p.ctx.Err()
        default:
        }
    }
    line := p.scanner.Text()
    // ...
}
```

**Problem:** `scanner.Scan()` on an `io.Reader` (HTTP response body) blocks until data is available. The context cancellation check is only evaluated **between lines**, not during the blocking `Scan()` call. If the provider stops sending data (network stall), `scanner.Scan()` blocks indefinitely.

The streaming goroutine (`streaming.go:86-92`) does call `iterator.Close()` on context cancellation, which closes the HTTP body and should unblock the scanner. However, this depends on the goroutine being scheduled and the body close propagating to the blocked `Read()`.

**Impact:** Low in practice (the streaming goroutine handles cleanup), but under extreme load or with certain HTTP client implementations, the scanner may not unblock promptly.

**Fix:** Already mitigated by `streaming.go` goroutine. No immediate fix needed, but consider adding a `ReadTimeout` on the HTTP response body for defense-in-depth.

---

## Medium Severity Issues (M-1 through M-6)

### M-1: TOCTOU Race in Dispatcher.Execute()

**File:** `internal/tools/dispatcher.go:124-145`

```go
// First RLock: lookup tool
d.mu.RLock()
tool, ok := d.tools[call.Name]
d.mu.RUnlock()

// ... gap: UpdatePermissions() could change rules here ...

// Second RLock: check permission
d.mu.RLock()
allowedByRule, pctx, ruleErr := d.checkPermission(call.Name, input)
d.mu.RUnlock()
```

**Problem:** Between the two `RLock` sections, `UpdatePermissions()` (which takes a write lock) could change the permission rules. The tool lookup and permission check use **different snapshots** of the state.

**Impact:** A tool could be registered but then blocked by a rule that was added between the lookup and permission check. Or a tool could be blocked by a rule that was removed. This is a correctness issue, not a deadlock.

**Fix:** Combine both operations into a single `RLock` section:

```go
d.mu.RLock()
tool, ok := d.tools[call.Name]
allowedByRule, pctx, ruleErr := d.checkPermission(call.Name, input)
d.mu.RUnlock()
```

---

### M-2: TOCTOU Race in Session Manager ListSessions

**File:** `pkg/session/manager.go:310-317`

```go
m.cacheMu.RLock()
if time.Since(m.sessionCacheTime) < m.sessionCacheTTL && m.sessionCache != nil {
    result := make([]SessionInfo, len(m.sessionCache))
    copy(result, m.sessionCache)
    m.cacheMu.RUnlock()
    return result, nil
}
m.cacheMu.RUnlock()
// ... expensive filesystem walk ...
m.cacheMu.Lock()
m.sessionCache = make([]SessionInfo, len(sessions))
// ...
m.cacheMu.Unlock()
```

**Problem:** Multiple goroutines can simultaneously miss the cache and all perform the filesystem walk. This is a performance concern (redundant I/O), not a deadlock.

**Impact:** Redundant filesystem I/O on cache miss. Low impact since sessions are typically single-user.

**Fix:** Use a `singleflight.Group` to deduplicate concurrent filesystem walks.

---

### M-3: Config Watcher Uses Unbuffered Channel

**File:** `internal/config/loader.go` (WatchConfig function)

```go
select {
case ch <- ConfigReloadMsg{Config: cfg, Error: err}:
case <-ctx.Done():
    return
}
```

**Problem:** If the receiver is slow to consume config reload messages, the watcher blocks. This is mitigated by the context cancellation fallback, but if the receiver is temporarily busy, config reloads can be delayed.

**Impact:** Low — config reloads are infrequent and non-critical.

**Fix:** Use a buffered channel or drop stale reloads.

---

### M-4: WebFetch DNS Cache Uses sync.Map Without TTL

**File:** `internal/tools/webfetch.go:51`

```go
dnsCache sync.Map
```

**Problem:** The DNS cache has no TTL eviction. DNS records can change, and stale entries are never purged. This is a correctness issue, not a concurrency issue.

**Impact:** Stale DNS resolution for long-running sessions. Low impact for a CLI tool.

**Fix:** Add TTL-based eviction or a periodic cleanup goroutine.

---

### M-5: Token Estimator Race on First Use

**File:** `internal/tokens/estimator.go:77-79`

```go
e.mu.Lock()
factor := e.emaFactor
e.mu.Unlock()
```

**Problem:** The EMA factor is read and written with separate locks, but the estimation logic that uses the factor is not atomic. If `Calibrate()` is called concurrently with `Estimate()`, the factor can change mid-estimation.

**Impact:** Minor token estimation inaccuracy. The EMA correction is gradual and self-correcting.

**Fix:** Acceptable for the current use case. No fix needed.

---

### M-6: Provider Registry TrySetActive and HealthCheck Gap

**File:** `internal/provider/fallback.go:30-40`

```go
p, err := registry.TrySetActive(name)
// ...
ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
status := func() types.HealthStatus {
    defer cancel()
    return p.HealthCheck(ctx)
}()
```

**Problem:** `TrySetActive` atomically sets the provider as active, but the health check runs after the switch. If the health check fails, the provider remains active but is unhealthy. The caller must handle this case separately.

**Impact:** The provider could be set active then fail the health check, leaving the system in an inconsistent state.

**Fix:** Check health before calling `TrySetActive`, or add a rollback mechanism.

---

## Low Severity Issues (L-1 through L-4)

### L-1: No File Locking for Session Files

**File:** `pkg/session/manager.go` (all methods)

**Problem:** The session manager uses atomic writes (temp file + rename) but no file locking (`flock`). If two M31A instances access the same session simultaneously, they could corrupt session files.

**Impact:** Low — single-user tool, concurrent access not expected. But if two terminals run M31A on the same project, data loss is possible.

**Fix:** Add `flock`-based file locking for critical session files, or document the single-instance assumption.

---

### L-2: Emitter Channel Has No Backpressure

**File:** `internal/tui/app_channel.go:50-56`

```go
func (ce *channelEmitter) Emit(msg tea.Msg) {
    select {
    case ce.ch <- msg:
    case <-time.After(types.ChannelSendTimeout * 2):
        slog.Warn("workflow message dropped: channel full")
    }
}
```

**Problem:** Workflow messages are dropped if the TUI is slow to consume them. This is by design (non-blocking), but can cause missed phase transition events or tool completion notifications.

**Impact:** Low — the TUI typically consumes messages quickly. Dropped messages cause missing status updates but don't affect correctness.

**Fix:** Acceptable for the current design. Consider increasing channel buffer if drops are frequent.

---

### L-3: Bash Tool killOnce Prevents Double-Signal but Not Timeout

**File:** `internal/tools/bash.go:115-141`

```go
var killOnce sync.Once
// ...
go func() {
    select {
    case <-ctx.Done():
        if cmd.Process != nil {
            killOnce.Do(func() {
                // Send SIGINT
            })
            time.AfterFunc(BashKillGracePeriod, func() {
                killOnce.Do(func() {
                    // Send SIGKILL
                })
            })
        }
    case <-cmdDone:
    }
}()
```

**Problem:** The `time.AfterFunc` for SIGKILL is not cancellable. If the process exits normally during the grace period, the `killOnce` prevents the second signal from being sent, but the timer still fires (wasted but harmless).

**Impact:** Negligible — the `killOnce` ensures correctness.

**Fix:** Store the `time.AfterFunc` return value and cancel it in the `cmdDone` case.

---

### L-4: Session Manager Cache Has No Eviction

**File:** `pkg/session/manager.go:310-317`

**Problem:** The session cache is never evicted. Once populated, it grows unboundedly as sessions are created. The cache is only rebuilt when the TTL expires.

**Impact:** Low — sessions are created infrequently, and the cache is small.

**Fix:** Add maximum cache size or periodic eviction.

---

## Concurrency Architecture Assessment

### Bubble Tea Threading Model Compliance

The codebase correctly follows the Bubble Tea single-threaded update model:

- **All state mutations** go through `Update()` — no `AppState` mutation from goroutines detected.
- **Workflow engine** runs in a `tea.Cmd` goroutine and emits events via `channelEmitter` → `tea.Msg` → `Update()`.
- **Streaming pipeline** uses goroutine-owned channels (`streamCh`) with proper lifecycle management.
- **Permission system** uses per-request channels with timeout fallback.
- **Health check** runs in a `tea.Cmd` with timeout context.

**Verdict:** COMPLIANT — no threading model violations.

### Mutex Analysis

| Struct | Type | Lock Count | Nested Locks | Missing Unlocks | Verdict |
|--------|------|-----------|--------------|-----------------|---------|
| `Dispatcher.mu` | RWMutex | 7 sites | 0 | 0 | PASS |
| `ModelCache.mu` | RWMutex | 7 sites | 0 | 0 | PASS |
| `Registry.mu` | RWMutex | 6 sites | 0 | 0 | PASS |
| `Ledger.mu` | RWMutex | 6 sites | 0 | 0 | PASS |
| `Consolidator.mu` | RWMutex | 8 sites | 0 | 0 | PASS |
| `Manager.cacheMu` | RWMutex | 5 sites | 0 | 0 | PASS |
| `Bash termMu` | Mutex | 3 sites | 0 | 0 | PASS |
| `Bash outMu` | Mutex | 4 sites | 0 | 0 | PASS |
| `limitWriter.mu` | Mutex | 4 sites | 0 | 0 | PASS (but H-1: held during blocking write) |
| `Estimator.mu` | Mutex | 2 sites | 0 | 0 | PASS |
| `SSEParser.closeOnce` | sync.Once | 1 site | 0 | 0 | PASS |

**No nested lock acquisition found anywhere in the codebase.**

### Channel Analysis

| Channel | Type | Buffer | Blocking Sends | Silent Drops | Verdict |
|---------|------|--------|---------------|--------------|---------|
| `Dispatcher.requestCh` | PermissionRequest | `PermissionChannelBuffer` | Non-blocking | No | PASS |
| `Dispatcher.responseCh` | PermissionResponse | `PermissionChannelBuffer` | Non-blocking | Yes (H-3) | FIX NEEDED |
| `Dispatcher.pendingResponses` | per-request chan | 1 | Non-blocking | Yes (H-3) | FIX NEEDED |
| `Dispatcher.questionReqCh` | QuestionRequest | `QuestionChannelBuffer` | Non-blocking | No | PASS |
| `Dispatcher.questionRespCh` | QuestionResponse | `QuestionChannelBuffer` | Non-blocking | Yes (H-3) | FIX NEEDED |
| `streamCh` (streaming.go) | tea.Msg | 64 | Non-blocking (select) | No | PASS |
| `emitterCh` | tea.Msg | 128 | Non-blocking (timeout) | Yes (L-2) | ACCEPTABLE |
| `rateTokens` | struct{} | `ToolRateLimitBurst` | Non-blocking | Yes (by design) | PASS |
| `cmdDone` (bash.go) | struct{} | 0 | Close-based | No | PASS |
| `waitCh` (bash.go) | error | 1 | Non-blocking | No | PASS |
| `done` (streaming.go) | struct{} | 0 | Close-based | No | PASS |

### Goroutine Lifecycle Analysis

| Goroutine | Launch Site | Termination | Leak Risk | Verdict |
|-----------|-------------|-------------|-----------|---------|
| Rate limiter | `dispatcher.go:58` | Never | HIGH (H-4) | FIX NEEDED |
| Bash signal forwarder | `bash.go:119` | `cmdDone` | NONE | PASS |
| Bash wait | `bash.go:145` | `cmd.Wait()` | NONE | PASS |
| Bash stdout reader | `bash.go:159` | pipe EOF | NONE | PASS |
| Bash stderr reader | `bash.go:168` | pipe EOF | NONE | PASS |
| Streaming reader | `streaming.go:73` | `close(streamCh)` | NONE | PASS |
| Streaming cancel | `streaming.go:86` | `done` | NONE | PASS |
| Question response | `app_update.go:1189` | channel send | HIGH (C-1) | FIX NEEDED |
| Signal handler | `main.go:216` | `sigCh` | NONE | PASS |
| Config watcher | `loader.go:638` | `ctx.Done()` | NONE | PASS |
| Permission listener | `app.go:147` | `ctx.Done()` | NONE | PASS |
| Question listener | `app.go:159` | `ctx.Done()` | NONE | PASS |
| Emitter drain | `app.go:243` | `ctx.Done()` | NONE | PASS |
| Health tick | `health.go:14` | tea.Tick | NONE | PASS |

---

## Recommendations Priority

| Priority | Issue | Effort | Impact |
|----------|-------|--------|--------|
| 1 | C-2: Task runner context disconnect | Small | Critical — tasks run after exit |
| 2 | C-1: Question response goroutine leak | Small | Critical — goroutine leak |
| 3 | H-1: Mutex held during pipe write | Small | High — Bash tool hang risk |
| 4 | H-2: Shared question channel routing | Medium | High — wrong answer delivery |
| 5 | H-4: Rate limiter goroutine leak | Small | High — session switch leak |
| 6 | M-1: TOCTOU in dispatcher | Small | Medium — correctness |
| 7 | H-3: Silent permission drop | Small | Medium — UX |
| 8 | M-6: Provider health check gap | Medium | Medium — consistency |
| 9 | M-2: TOCTOU in session cache | Small | Medium — performance |
| 10 | L-1: No file locking | Medium | Low — design assumption |
| 11 | Others | Varies | Low |

---

## Files Reviewed

| Package | Files | Sync Primitives |
|---------|-------|----------------|
| `internal/tools/` | dispatcher.go, permissions.go, bash.go, question.go | RWMutex, Mutex, WaitGroup, sync.Map, sync.Once |
| `internal/provider/` | registry.go, cache.go, sse.go, fallback.go, openrouter/client.go, zen/client.go | RWMutex, sync.Once |
| `internal/tui/` | app.go, app_channel.go, app_update.go, streaming.go, health.go | sync.Once, channels |
| `internal/tokens/` | estimator.go | Mutex |
| `pkg/taskrunner/` | runner.go | context.Background() |
| `pkg/autodream/` | autodream.go | RWMutex, atomic.Bool |
| `pkg/ledger/` | ledger.go | RWMutex |
| `pkg/session/` | manager.go | RWMutex |
| `internal/log/` | log.go | sync.Once |
| `internal/config/` | loader.go | channels |

---

*End of report.*
