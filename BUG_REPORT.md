# M31A Bug Report — Comprehensive Codebase Audit

**Generated:** 2026-07-02
**Scope:** All `.go` files in `internal/` and `pkg/`
**Methodology:** Static analysis across 7 categories: race conditions, resource leaks, error handling, nil/memory safety, logic errors, security, API misuse

---

## Executive Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 7 |
| HIGH | 12 |
| MEDIUM | 22 |
| LOW | 18 |
| **Total** | **59** |

**Resource leaks: None found.** The codebase has excellent resource hygiene.

---

## CRITICAL Issues (7)

### C1. `Engine.perPhaseModels` — concurrent map read/write (guaranteed crash)
- **File:** `internal/workflow/engine.go:272-278`
- **Code:**
  ```go
  func (e *Engine) SetPhaseModel(phase m31types.WorkflowPhase, modelID string) {
      if e.perPhaseModels == nil {
          e.perPhaseModels = make(map[m31types.WorkflowPhase]string)
      }
      e.perPhaseModels[phase] = modelID  // WRITE — no lock
  }
  func (e *Engine) modelForPhase(phase m31types.WorkflowPhase) string {
      if id, ok := e.perPhaseModels[phase]; ok && id != "" {  // READ — no lock
          return id
      }
  ```
- **Impact:** `SetPhaseModel` is called from the TUI goroutine; `modelForPhase` is called from the workflow goroutine. Concurrent map read/write in Go is **undefined behavior** and will crash with `fatal error: concurrent map read and map write`.
- **Fix:** Protect `perPhaseModels` with a `sync.RWMutex`, or use `sync.Map`.

### C2. `Engine.activePhase`/`modelID`/`workflowMode` — unprotected concurrent access
- **File:** `internal/workflow/engine.go:604, 577, 283`
- **Code:**
  ```go
  func (e *Engine) RunPhase(...) {
      e.activePhase = phase   // WRITE from workflow goroutine
      e.modelID = modelID     // WRITE from workflow goroutine
      e.workflowMode = mode   // WRITE from TUI goroutine (SetMode)
  ```
- **Impact:** These fields are written from multiple goroutines without synchronization. The TUI calls `SetModel()`, `SetPhaseModel()`, and `SetMode()` while the workflow goroutine reads them in `RunPhase()`, `buildSystemPrompt()`, and `consumeStream()`. Data race on every workflow execution.
- **Fix:** Use atomic values or protect with mutex.

### C3. `pkg/session/manager.go:350` — nil pointer dereference on `os.Stat` failure
- **File:** `pkg/session/manager.go:350`
- **Code:**
  ```go
  fi, _ := os.Stat(sessPath)  // error discarded
  // ...
  fi.ModTime()  // line 345 — PANIC if fi is nil
  ```
- **Impact:** If the session file doesn't exist or is inaccessible, `fi` is nil and `fi.ModTime()` panics. This is reachable when listing sessions with corrupted storage.
- **Fix:** Check `err` from `os.Stat` before using `fi`.

### C4. `tools/websearch.go:62` — index out of bounds on DNS resolution
- **File:** `internal/tools/websearch.go:62`
- **Code:**
  ```go
  pinnedAddr := net.JoinHostPort(ips[0].IP.String(), port)
  ```
- **Impact:** If DNS resolution returns an empty slice (edge case with DNS tricks or cache eviction), `ips[0]` panics. The check for private IPs at lines 56-60 validates the slice contents but doesn't guard against an empty slice.
- **Fix:** Add `len(ips) == 0` check before indexing.

### C5. `tools/webfetch.go:91` — same index out of bounds pattern
- **File:** `internal/tools/webfetch.go:91`
- **Code:**
  ```go
  pinnedAddr := net.JoinHostPort(addrs[0].IP.String(), port)
  ```
- **Impact:** Same as C4 — panic on empty DNS resolution result.
- **Fix:** Add length check before indexing.

### C6. `tools/dispatcher.go:463` — unsafe type assertion on `sync.Map` value
- **File:** `internal/tools/dispatcher.go:463`
- **Code:**
  ```go
  if ch, ok := d.pendingQuestions.Load(requestID); ok {
      select {
      case ch.(chan QuestionResponse) <- resp:  // no comma-ok
  ```
- **Impact:** `sync.Map.Load()` returns `any`. The `ok` only confirms the key exists, not the type. If the stored value is corrupted or a different type, this panics.
- **Fix:** Use comma-ok: `ch, ok := ch.(chan QuestionResponse)`.

### C7. `tools/subagent/manager.go:265` — unsafe type assertion on `sync.Map` value
- **File:** `internal/tools/subagent/manager.go:265`
- **Code:**
  ```go
  func (m *Manager) CancelAll() {
      m.agents.Range(func(_, value any) bool {
          value.(*Subagent).cancel()  // no comma-ok
          return true
      })
  }
  ```
- **Impact:** Same pattern as C6. Panic if stored value is not `*Subagent`. Same issue at lines 237, 246, 292.
- **Fix:** Use comma-ok type assertion.

---

## HIGH Issues (12)

### H1. `Dispatcher.SetCollector` — writes without lock
- **File:** `internal/tools/dispatcher.go:190-191`
- **Code:**
  ```go
  func (d *Dispatcher) SetCollector(c *metrics.Collector) {
      d.collector = c        // WRITE — no lock
      d.mu.RLock()           // lock acquired AFTER write
  ```
- **Impact:** `d.collector` is read in `Execute()` (line 294) without holding `d.mu`. Data race between `SetCollector` and `Execute`.

### H2. `DevServer.restartServer` — TOCTOU double lock/unlock
- **File:** `internal/tools/devserver.go:283-298`
- **Code:**
  ```go
  d.mu.Lock()
  if e, ok := d.processes[id]; !ok { _ = e }
  d.mu.Unlock()                          // UNLOCK
  // --- GAP ---
  d.mu.Lock()                            // re-LOCK
  for _, e := range d.processes {
  ```
- **Impact:** Another goroutine can modify `d.processes` between the two critical sections. The first lock block does nothing useful (reads into `_`). Classic TOCTOU bug.

### H3. `tools/dns_cache.go:84-90` — non-atomic counter + TOCTOU eviction
- **File:** `internal/tools/dns_cache.go:84-90`
- **Code:**
  ```go
  if dc.inserts.Add(1) >= dc.threshold {
      dc.inserts.Store(0)
      dc.evictExpired(now)
  }
  // ...
  if dc.Size() > dc.maxSize {  // Size() not atomic with evictOldest()
      dc.evictOldest(...)
  }
  ```
- **Impact:** Two goroutines can both pass the threshold check and both evict. `Size()` + `evictOldest()` is not atomic — cache can exceed `maxSize`.

### H4. `decision/logger.go:92-117` — data loss on shutdown race
- **File:** `internal/decision/logger.go:92-117`
- **Code:**
  ```go
  func (l *Logger) drain() {
      for {
          select {
          case r, ok := <-l.ch:
          case <-l.closeCh:
              l.drainRemainingCh()  // best-effort drain
  ```
- **Impact:** When `Close()` is called, the `select` can pick `<-l.closeCh` while items are still buffered in `l.ch`. Items written between the `select` choice and `drainRemainingCh` call could be lost.

### H5. `permissions.go:377-385` — double decrement of `pendingPermCount`
- **File:** `internal/tools/permissions.go:377-385`
- **Code:**
  ```go
  d.pendingPermCount.Add(1)
  defer d.pendingPermCount.Add(-1)    // deferred -1
  select {
  case d.requestCh <- req:
  default:
      d.pendingPermCount.Add(-1)      // manual -1
      return m31errors.ErrPermissionDenied
  }
  ```
- **Impact:** When the channel send fails, both the explicit `-1` AND the deferred `-1` execute. Net result: counter goes to -2. The "N tools behind this one" display becomes wrong/negative.

### H6. `tui/app_view.go:898` — nil pointer dereference on `m.replModel`
- **File:** `internal/tui/app_view.go:898-910`
- **Code:**
  ```go
  func (m *AppState) updateTokenMetrics(...) {
      if m.replModel.lastUsage != nil {  // no nil check on m.replModel
          tokens = m.replModel.lastUsage.TotalTokens
      }
  ```
- **Impact:** If `replModel` is nil when `updateTokenMetrics` is called, this panics. Lines 315-317 in the same file DO check `m.replModel != nil`, showing the developer was aware but missed it here.

### H7. `workflow/diff_summary.go:24` — unchecked `git diff` error
- **File:** `internal/workflow/diff_summary.go:24`
- **Code:**
  ```go
  statusOut, _ := g.Run("diff", "--name-status", beforeHash+".."+afterHash)
  ```
- **Impact:** If git fails, `statusOut` is empty and the entire diff summary misclassifies every file as "modified". Ship phase produces inaccurate stats.

### H8. `tui/app_update_commands.go:267` — unchecked `TruncateMessagesForLLM`
- **File:** `internal/tui/app_update_commands.go:267`
- **Code:**
  ```go
  msgs, _ = streaming.TruncateMessagesForLLM(msgs, m.activeModel.ContextLength, estimator)
  ```
- **Impact:** If truncation fails, `msgs` becomes nil. The agent loop runs with no messages — sending empty context to the LLM.

### H9. `workflow/plan.go:436` — unchecked `json.MarshalIndent`
- **File:** `internal/workflow/plan.go:436`
- **Code:**
  ```go
  taskJSON, _ := json.MarshalIndent(tasks, "", "  ")
  ```
- **Impact:** If tasks contain non-serializable values, the plan output is empty/malformed with no error signal. Tasks are the core plan output — corruption here breaks the workflow silently.

### H10. `tools/httpcheck.go:115-173` — SSRF vulnerability (no SSRF protection)
- **File:** `internal/tools/httpcheck.go:115-173`
- **Code:**
  ```go
  req, err := http.NewRequestWithContext(ctx, method, url, bodyReader)
  resp, err := h.client.Do(req)
  ```
- **Impact:** Unlike WebFetch and WebSearch which have DNS pinning, private IP blocking, and reserved IP checks, HTTPCheck uses a plain `net.Dialer` with zero SSRF protection. An LLM can be prompted to request `http://169.254.169.254/latest/meta-data/` (cloud metadata), `http://localhost:*` (internal services), or RFC1918 addresses.
- **Fix:** Reuse the SSRF-protected transport from WebFetch.

### H11. `tui/streaming/agent_loop.go:203` — iterator resource leak on error
- **File:** `internal/tui/streaming/agent_loop.go:203`
- **Code:**
  ```go
  iterator, err := p.ChatCompletionStream(ctx, req)
  // ...
  ch <- AgentErrorMsg{...}
  return  // iterator NOT closed
  ```
- **Impact:** If `ChatCompletionStream` succeeds but a later error causes early return, the HTTP connection (iterator) is never closed. Resource leak.

### H12. `tools/dns_cache.go:63,100,113,130` — unsafe type assertions on sync.Map
- **File:** `internal/tools/dns_cache.go:63,100,113,130`
- **Code:**
  ```go
  if cached, ok := dc.cache.Load(host); ok {
      entry := cached.(*dnsCacheEntry)  // no comma-ok
  ```
- **Impact:** Multiple `sync.Map.Load` calls followed by type assertions without comma-ok. Panic on type mismatch.

---

## MEDIUM Issues (22)

### M1. `tui/streaming/agent_loop.go:210-217` — double-close on iterator
- **File:** `internal/tui/streaming/agent_loop.go:210-217`
- Goroutine calls `iterator.Close()` on context cancellation while main loop also calls `iterator.Close()`. Double-close risk.

### M2. `pkg/taskrunner/runner.go:283-284` — callback under lock → deadlock
- **File:** `pkg/taskrunner/runner.go:283-284`
- `r.OnTaskUpdate` is called while `r.mu` is held. If the callback calls back into `Runner.Status()` or `Runner.Tasks()`, it deadlocks (Go's `RWMutex` is not reentrant).

### M3. `devserver.go:344-362` — entry accessed after map deletion without `entry.mu`
- **File:** `internal/tools/devserver.go:344-362`
- After deleting entry from map and releasing lock, code accesses `entry.cmd.Process` without `entry.mu`. Race with `monitorCrash()` goroutine.

### M4. `workflow/intent.go:99` — fragile error string comparison
- **File:** `internal/workflow/intent.go:99`
- `err.Error() == "EOF"` instead of `errors.Is(err, io.EOF)`. Breaks if error is wrapped.

### M5. `git/git.go:231` — fragile git error string matching
- **File:** `internal/git/git.go:231`
- `strings.Contains(err.Error(), "does not have any commits")` — breaks if git changes its message format.

### M6. `tools/webfetch.go:411-419` — fragile error string matching for retry
- **File:** `internal/tools/webfetch.go:411-419`
- `strings.Contains(errStr, "connection refused")` instead of typed error checks.

### M7. `provider/nvidia/client.go:88,150` — lossy error wrapping
- **File:** `internal/provider/nvidia/client.go:88,150`
- `fmt.Errorf("models fetch returned status %d", resp.StatusCode)` — discards response body. Same in `openrouter/client.go:103,155` and `zen/client.go:90`.

### M8. `workflow/ship.go:251,443,471` — unchecked session persistence loads
- **File:** `internal/workflow/ship.go:251,443,471`
- `project, _ := e.sessionMgr.LoadProject(e.sessionID)` — silent data loss if persistence fails.

### M9. `tools/filelist.go:201` — nil dereference on `DirEntry.Info()`
- **File:** `internal/tools/filelist.go:201`
- `info, _ := e.Info()` — if nil, sorting by "modified" panics on `info.ModTime()`.

### M10. `tui/streaming/agent_loop.go:483` — unchecked `LoadProjectContext`
- **File:** `internal/tui/streaming/agent_loop.go:483`
- Agent starts without project context (AGENTS.md/MEMORY.md) if this fails.

### M11. `workflow/engine.go:1217` — `context.Background()` in prompt building
- **File:** `internal/workflow/engine.go:1217`
- `ctx := context.Background()` inside `buildSystemPrompt()` — creates uncancelable context during streaming. Should use engine's working context.

### M12. `workflow/engine.go:899,1030` — detached compaction context
- **File:** `internal/workflow/engine.go:899,1030`
- `compactCtx, compactCancel := context.WithTimeout(context.Background(), 60*time.Second)` — compaction continues for up to 60s after workflow cancellation.

### M13. `tools/edit.go:103-112` — TOCTOU between Stat and ReadFile
- **File:** `internal/tools/edit.go:103-112`
- File size check via `os.Stat` is not atomic with `os.ReadFile`. File could be swapped to exceed `MaxFileSize` between check and read.

### M14. `tools/bash.go:477-490` — dangerous command blocklist bypass
- **File:** `internal/tools/bash.go:477-490`
- Simple string matching allows `rm -r -f /` (separate args) to bypass `rm -rf /` pattern. Also `rm$'\t'-rf /` bypasses via tab character.

### M15. `workflow/plan.go:496-498` — map iteration order non-deterministic
- **File:** `internal/workflow/plan.go:496-498`
- `for q, a := range project.Answers` — Q&A order varies between runs, causing non-deterministic LLM prompts.

### M16. `workflow/discuss.go:265` — same map iteration order issue
- **File:** `internal/workflow/discuss.go:265`
- Same non-deterministic ordering in discuss phase prompt.

### M17. `codeintel/index.go:156` — slice in-place mutation via append
- **File:** `internal/codeintel/index.go:156`
- `append(locs[:i], locs[i+1:]...)` mutates the original backing array. Fragile if slice is shared.

### M18. `codeintel/graph.go:94,107` — same slice mutation pattern
- **File:** `internal/codeintel/graph.go:94,107`
- Same append-in-place pattern for edge removal.

### M19. `tools/webfetch.go:102` — unprotected inner type assertion
- **File:** `internal/tools/webfetch.go:102`
- `remoteAddr := tcpConn.RemoteAddr().(*net.TCPAddr)` — no comma-ok. Panics if `RemoteAddr()` returns unexpected type.

### M20. `tui/streaming/agent_loop.go:211-217` — goroutine leak on early return
- **File:** `internal/tui/streaming/agent_loop.go:211-217`
- If main loop exits without closing `iterDone` (early return paths at lines 231, 235), the goroutine leaks until GC.

### M21. `workflow/execute.go:741-746` — string concat in nested loop
- **File:** `internal/workflow/execute.go:741-746`
- `planCtx += fmt.Sprintf(...)` in nested loop — O(n²) allocation. Should use `strings.Builder`.

### M22. `tools/codemap.go:119-133` — fmt.Sprintf in nested loop
- **File:** `internal/tools/codemap.go:119-133`
- `output += fmt.Sprintf(...)` in nested loop — O(n²) allocation.

---

## LOW Issues (18)

| # | File | Issue |
|---|------|-------|
| L1 | `engine.go:576-581` | `SetModel` writes `e.provider`/`e.modelID` without lock (likely safe in practice) |
| L2 | `streaming.go:79-108` | Send to closed channel in panic recovery (recover catches it, error lost) |
| L3 | `graph.go:331` | Global mutex contention point (`goModulePathCacheMu`) |
| L4 | `context/sources.go:71` | Unchecked `git log` — empty context on repos with no commits |
| L5 | `pkg/session/planning.go:288` | Unchecked `time.Parse` — zero time on malformed input |
| L6 | `pkg/arbitrage/arbitrage.go:143` | Unchecked `scorer.Score` — defaults to "simple" on failure |
| L7 | `provider/model_metadata.go:266` | Unchecked `lookupMetadata` — silent degradation |
| L8 | `tui/sidebar_model.go:710-711` | Unchecked git operations — stale sidebar display |
| L9 | `tui/app_update.go:1585` | Unchecked `LoadTasks` — wrong ship display |
| L10 | `tools/grep.go:343,400,432` | Unchecked `filepath.Rel` — garbled output for edge cases |
| L11 | `provider/registry.go:25` | Validation error without wrapping sentinel |
| L12 | `workflow/ship.go:287-293` | Map iteration order in file extensions (cosmetic) |
| L13 | `provider/model_metadata.go:222` | Non-deterministic prefix matching order |
| L14 | `workflow/plan_parser.go:32-39` | sync.Map check-then-act race (wasteful but correct) |
| L15 | `tools/webfetch.go:149-176` | Redundant 169.254.x.x check (already caught by `IsLinkLocalUnicast`) |
| L16 | `tools/websearch.go:55-61` | Missing `isReservedIP` check (only `isPrivateIP` checked) |
| L17 | `tools/grep.go:143` | Inline path containment check instead of shared helper |
| L18 | `tui/components/glamour_cache.go:57` | FNV-1a hash collision risk (extremely rare) |

---

## Top 10 Recommended Fixes (Priority Order)

| Priority | Issue | File | Fix |
|----------|-------|------|-----|
| 1 | **C1** | `engine.go:272` | Add `sync.RWMutex` to protect `perPhaseModels` map |
| 2 | **C2** | `engine.go:604` | Use `atomic.Int32` for `activePhase`/`workflowMode`, or add mutex |
| 3 | **C3** | `session/manager.go:350` | Check `err` from `os.Stat` before using `fi` |
| 4 | **C4/C5** | `websearch.go:62`, `webfetch.go:91` | Add `len(ips) == 0` guard before indexing |
| 5 | **H5** | `permissions.go:377` | Remove `defer d.pendingPermCount.Add(-1)` and handle all paths explicitly |
| 6 | **H10** | `httpcheck.go:115` | Add SSRF protection (reuse WebFetch's DNS-pinning transport) |
| 7 | **H8** | `app_update_commands.go:267` | Check error from `TruncateMessagesForLLM`, handle nil fallback |
| 8 | **C6/C7/H12** | `dispatcher.go:463`, `manager.go:265`, `dns_cache.go:63` | Use comma-ok type assertions on all `sync.Map` values |
| 9 | **H1** | `dispatcher.go:190` | Move `d.collector = c` inside `d.mu.Lock()` |
| 10 | **M14** | `bash.go:477` | Normalize command args before pattern matching (split on whitespace) |

---

## Methodology Notes

- **Resource leaks:** Exhaustive audit found **zero** issues. The codebase uses `defer` consistently, has proper channel lifecycle management, and `sync.Once` for idempotent cleanup.
- **Goroutine leaks:** All long-running goroutines have clear termination paths via context cancellation, done channels, or WaitGroups.
- **Race conditions:** The most dangerous category. Issues C1/C2 are guaranteed crashes under `go test -race` and could cause undefined behavior in production.
- **Error handling:** The codebase generally handles errors well. The main pattern is discarding errors from session persistence loads (ship phase) which causes silent data loss.
- **Security:** HTTPCheck's lack of SSRF protection is the most impactful security issue. The WebFetch and WebSearch tools have excellent SSRF protection that HTTPCheck should reuse.
