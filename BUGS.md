# BUGS.md — Logical Bug Audit Report

**Audit Date:** 2026-07-23
**Codebase:** M31A (github.com/eshanized/M31A), Go 1.25
**Baseline:** `go build ./...` clean, `go vet ./...` clean, `go test -race` passes on all testable packages (build failures are disk quota, not code issues)

---

## Summary Table

| ID | File:Line | Severity | Confidence | One-Line Description |
|----|-----------|----------|------------|----------------------|
| B01 | `internal/engine/workflow/execute.go:571` | Critical | Confirmed | Proactive compaction result discarded — compaction fires but has no effect |
| B02 | `internal/engine/workflow/engine.go:860` | High | Confirmed | `RunPhase` calls `SetPhase` bypassing transition validation |
| B03 | `internal/tools/permissions.go:280-341` | High | Confirmed | Last-match-wins allows broad allow to override specific deny |
| B04 | `internal/engine/session/manager.go:316` | High | Confirmed | `LoadWorkflowState` missing lock — races with `UpdateWorkflowState` |
| B05 | `internal/engine/session/manager.go:328-333` | High | Confirmed | `saveSessionAtomic` overwrites Messages in session.json |
| B06 | `internal/types/fileutil.go:29-44` | High | Confirmed | `flock` does not prevent cross-process dual-lock |
| B07 | `internal/engine/workflow/engine.go:537` | High | Confirmed | `Shutdown` sets `e.cache = nil` without synchronization |
| B08 | `internal/integrations/provider/handler_stream.go:57` | High | Confirmed | Auth/credit/model-not-found errors do not trigger fallback |
| B09 | `internal/integrations/provider/streaming.go:135` | High | Confirmed | Mid-stream SSE errors do not trigger provider fallback |
| B10 | `internal/tools/permissions.go:506-515` | Medium | Confirmed | Permission response silently dropped on timeout race |
| B11 | `internal/tools/permissions.go:384-396` | Medium | Confirmed | `matchAnyParamValue` hardcodes param keys — new tools skip matching |
| B12 | `internal/engine/workflow/engine.go:456` | Medium | Confirmed | `SaveCheckpointData` reads `planVersion` without lock |
| B13 | `internal/engine/workflow/engine.go:565` | Medium | Confirmed | `GetCheckpointData` reads without synchronization |
| B14 | `internal/engine/workflow/phase_coordinator.go:124` | Medium | Confirmed | Transition checkpoint lacks Goal and PlanVersion fields |
| B15 | `internal/engine/workflow/state_machine.go:101-105` | Medium | Confirmed | `SetPhase` doesn't reset `discussPlanCycles` on checkpoint restore |
| B16 | `internal/engine/tokens/estimator.go:194` | Medium | Confirmed | Token estimation truncates toward zero — systematic undercount |
| B17 | `internal/engine/workflow/engine.go:1026 vs 1078` | Medium | Confirmed | Preflight truncation uses inconsistent token accounting |
| B18 | `internal/core/config/merge.go:40-51` | Medium | Confirmed | Cannot override int/float config with zero value |
| B19 | `internal/core/config/config_validate.go:311` | Medium | Confirmed | Variable substitution covers only ~10 hardcoded fields |
| B20 | `internal/engine/session/checkpoint.go:30-53` | Medium | Confirmed | Checkpoint save is read-modify-write without locking |
| B21 | `internal/integrations/keychain/keychain.go:61-63` | Medium | Confirmed | Keychain permanently blacklists after first failure |
| B22 | `internal/integrations/provider/fallback.go:106` | Medium | Confirmed | "Degraded" health status treated as offline |
| B23 | `internal/ui/tui/components/repl.go:64` | Medium | Confirmed | `resizePending` mutated from goroutine — data race |
| B24 | `internal/tools/dispatcher.go:315` | Medium | Confirmed | `d.collector` read without lock in `Execute` |
| B25 | `internal/engine/workflow/engine.go:860+state_machine.go:96` | Low | Confirmed | Duplicate history entry on every TUI phase transition |
| B26 | `internal/engine/workflow/state_machine.go:96` | Low | Confirmed | History grows unboundedly — memory leak |
| B27 | `internal/tools/permissions.go:365,401` | Low | Confirmed | `doublestar.Match` errors silently swallowed |
| B28 | `internal/tools/search/dns_cache.go:134` | Low | Confirmed | Bare type assertion without comma-ok in eviction |
| B29 | `internal/integrations/provider/capabilities.go:355` | Low | Confirmed | Default model capabilities never cached |
| B30 | `internal/tools/dispatcher.go:432` | Low | Confirmed | `Stop()` does not drain `requestCh`/`questionReqCh` |

---

## CONCERNS.md Status

| ID | Location | Original Description | Status | Notes |
|----|----------|---------------------|--------|-------|
| BUG-01 | `git.go:451` | Commit channel ordering | **Already fixed** | Code now uses separate variables + WaitGroup. Comment documents completed fix. |
| BUG-06/07/19 | `concurrency.go`, `capabilities.go`, `cache.go` | sync.Map replacement races | **Already fixed** | All 5 sync.Map sites use comma-ok assertions correctly. No value-type replacement races remain. |
| BUG-17 | `cache.go:47` | Cache waiter stampede | **Already fixed** | `singleflight.Group` deduplicates concurrent fetches. No stampede possible. |
| BUG-18 | `loader.go:620` | Silent message drop | **Confirmed (B23-adjacent)** | `sendReload` drops message on ctx cancellation despite comment claiming otherwise. Goroutine leak possible. |
| BUG-29 | `estimator.go:400` | Window overflow | **Confirmed (B16, B17)** | Truncation rounding + inconsistent token accounting can allow requests exceeding context window. |
| Glob tool | `glob_test.go:164,185` | os.Stat relative paths | **Already fixed** | Both code paths now join relative paths with `workDir` before `os.Stat`. Test comments are stale. |
| Subagent injection | `subagent/loop.go:397` | Prompt injection gap | **Confirmed (low severity)** | Defenses exist as text instructions but cannot structurally prevent injection. Bounded by tool-call budget. |
| `panic("not implemented")` | `engine_verify.go:285` | Test-only panic | **False alarm** | String constant in detection pattern list, not an executable panic. |

---

## Detailed Bug Reports

### B01 — Proactive compaction result discarded [Critical]

**File:** `internal/engine/workflow/execute.go:571`

```go
e.proactiveCompactCheck(messages) //nolint:errcheck
```

**Intended:** Compact messages in-place during Execute phase when tool call threshold is met.

**Actual:** `proactiveCompactCheck` returns `[]m31types.Message` (the compacted message list), but the return value is silently discarded. The `messages` variable retains the original, un-compacted messages. The LLM summarization call is made (burning tokens and latency), but the result is thrown away.

**Trigger:** Execute phase with `cfg.Compaction.Proactive = true` and `toolCallsSinceLastCompact >= ToolCallsThreshold`.

**Impact:** Session grows unbounded during Execute. Compaction fires but has zero effect. Token budget exhaustion becomes more likely.

---

### B02 — RunPhase bypasses transition validation [High]

**File:** `internal/engine/workflow/engine.go:860`

```go
e.stateMachine.SetPhase(phase)
```

**Intended:** Phase transitions should be validated against the transition graph in `StateMachine.validTransitions`.

**Actual:** `RunPhase` calls `SetPhase()` which directly writes `currentPhase` without any validation. Any phase can be set from any current state, completely bypassing the state machine's transition graph. The validated `Transition()` method (engine.go:901) is only called from the TUI's `handlePhaseTransitionDecision` and `main.go`, not from the primary `RunPhase` path.

**Trigger:** Any call to `RunPhase` — the primary execution path used by the TUI and headless CLI.

---

### B03 — Permission last-match-wins allows broad allow to override specific deny [High]

**File:** `internal/tools/permissions.go:280-341`

```go
// Last-match-wins evaluation
for _, rule := range d.rules {
    // ...
    switch rule.Action {
    case "allow":
        lastMatch = &struct{...}{true, pctx, nil}
    case "deny":
        lastMatch = &struct{...}{false, pctx, m31errors.ErrPermissionDenied}
    }
}
```

**Intended:** "More specific rules override general ones by ordering them later."

**Actual:** If rules are ordered: `deny(rm -rf *)` then `allow(*)`, the broad allow wins. Users commonly write specific denies before broad allows (top-down thinking), creating a security gap where denied commands execute without a permission prompt.

**Trigger:** User puts specific deny rule before broader allow rule in config.

---

### B04 — LoadWorkflowState missing lock [High]

**File:** `internal/engine/session/manager.go:315-324`

```go
func (m *Manager) LoadWorkflowState(id string) (...) {
    session, loadErr := m.loadSessionMetadata()  // no m.lock.Lock()
    // ...
}
```

**Intended:** Read workflow state safely.

**Actual:** Every other method that calls `loadSessionMetadata` first acquires `m.lock` (e.g., `UpdateWorkflowState` at line 301). `LoadWorkflowState` does not. Concurrent `UpdateWorkflowState` could write `session.json` while `LoadWorkflowState` is mid-read, yielding a partial/corrupt JSON parse.

**Trigger:** `LoadWorkflowState` called concurrently with `UpdateWorkflowState`.

---

### B05 — saveSessionAtomic overwrites Messages in session.json [High]

**File:** `internal/engine/session/manager.go:328-333`

```go
func (m *Manager) saveSessionAtomic(session *Session) error {
    data, err := json.Marshal(session) // marshals Messages too
    // ...
    return m.atomicWrite(m.sessionJSONPath(), data)
}
```

**Intended:** Save session metadata only.

**Actual:** `Session` struct embeds `Messages []types.Message`. `loadSessionMetadata()` never reads `messages.json`, so `session.Messages` is whatever was in `session.json` (typically nil/empty). When `saveSessionAtomic` marshals the full struct and writes it to `session.json`, any messages previously serialized there are overwritten with empty data. After `UpdateWorkflowState` runs, `session.json` contains `"messages": null` while `messages.json` has the real messages.

**Trigger:** LoadSession -> add messages -> call UpdateWorkflowState -> session.json messages field becomes null/empty.

---

### B06 — flock does not prevent cross-process dual-lock [High]

**File:** `internal/types/fileutil.go:29-44`

```go
func (fl *FileLock) Lock() error {
    f, err := os.OpenFile(fl.path, os.O_CREATE|os.O_RDWR, 0600)
    // ...
    if err := syscall.Flock(int(f.Fd()), syscall.LOCK_EX); err != nil { ... }
}
```

**Intended:** Two processes cannot both hold the lock.

**Actual:** `flock(2)` is per file-descriptor, not per file. Two processes opening the same lock file get separate FDs and can both acquire `LOCK_EX` simultaneously. This is a well-known `flock(2)` limitation — `fcntl(F_SETLK)` or `O_EXCL` is needed for cross-process mutual exclusion.

**Trigger:** Run two M31A instances in the same project directory.

**Impact:** Two processes can corrupt session files simultaneously despite both "holding the lock."

---

### B07 — Shutdown sets e.cache = nil without synchronization [High]

**File:** `internal/engine/workflow/engine.go:537`

```go
e.cache = nil
```

**Intended:** Clean up resources on shutdown.

**Actual:** If any goroutine is concurrently using `e.cache` (e.g., `buildToolDefinitions` at line 1310, `loadProjectCached` at line 771), setting it to nil without synchronization causes a data race. Subsequent access to `e.cache` after it becomes nil will panic with a nil pointer dereference.

**Trigger:** `Shutdown` called while a workflow phase is still running or its goroutines are still active.

---

### B08 — Auth/credit/model-not-found errors do not trigger fallback [High]

**File:** `internal/integrations/provider/handler_stream.go:57`

```go
if stderrors.Is(msg.Err, m31errors.ErrRateLimited) || stderrors.Is(msg.Err, m31errors.ErrProviderUnreachable) {
    cmds = append(cmds, m.attemptAutoFallback(msg.Err))
}
```

**Intended:** Fallback should trigger on provider failures that are provider-specific.

**Actual:** Only `ErrRateLimited` and `ErrProviderUnreachable` trigger auto-fallback. `ErrInvalidKey` (auth failure), `ErrNoCredits` (billing), and `ErrModelNotFound` (deprecated model) silently fail the user with no fallback attempt.

**Trigger:** User's active provider returns 401, 402, or 404 for a model available on another provider.

---

### B09 — Mid-stream SSE errors do not trigger provider fallback [High]

**File:** `internal/integrations/provider/streaming.go:135`

```go
if err != nil {
    streamCh <- StreamErrorMsg{Err: err, ModelID: req.Model, ProviderName: providerName}
    return
}
```

**Intended:** Stream errors should allow fallback to another provider.

**Actual:** Mid-stream SSE errors (connection reset, malformed JSON) produce `StreamErrorMsg`, but `handleStreamErrorMsg` only checks for `ErrRateLimited` or `ErrProviderUnreachable`. Mid-stream errors are typically `io.ErrUnexpectedEOF`, `bufio.ErrTooLong`, or `json.SyntaxError` — none match. The user sees a partial response and an error, but no fallback.

**Trigger:** Provider connection drops mid-stream (e.g., NVIDIA NIM timeout on long generations).

---

### B10 — Permission response silently dropped on timeout race [Medium]

**File:** `internal/tools/permissions.go:69-76`

```go
// Fallback to shared channel for backwards compatibility
select {
case d.responseCh <- resp:
default:
    slog.Warn("permission response dropped: shared channel full", ...)
}
```

**Intended:** Backwards-compatible fallback if per-request channel is missing.

**Actual:** `sendAndWaitForPermission` only reads from the per-request `respCh`, never from `d.responseCh`. If the per-request channel is deleted (by deferred cleanup on timeout) before `ApprovePermission` sends, the response falls back to `d.responseCh` where nobody reads it. The user clicks "Allow" but permission times out anyway.

**Trigger:** Race between permission timeout and user click.

---

### B11 — matchAnyParamValue hardcodes param keys [Medium]

**File:** `internal/tools/permissions.go:384-396`

```go
func matchAnyParamValue(pattern string, params map[string]any) bool {
    paramKeys := []string{"path", "url", "command", "pattern"}
    for _, key := range paramKeys {
        v, ok := params[key]
        // ...
    }
    return false
}
```

**Intended:** Match permission patterns against tool parameter values.

**Actual:** Only checks `path`, `url`, `command`, `pattern`. Tools with other parameter names (e.g., `query` for WebSearch, `description` for Agent, `destination` for FileMove) never match any pattern-based permission rule. A `{Tool: "WebSearch", Pattern: "*", Action: "deny"}` rule silently fails because the param is `query`.

**Trigger:** Any tool with parameters not in the hardcoded list.

---

### B12 — SaveCheckpointData reads planVersion without lock [Medium]

**File:** `internal/engine/workflow/engine.go:456`

```go
PlanVersion: e.state.planVersion,
```

**Intended:** `planVersion` is protected by `planMu`.

**Actual:** `SaveCheckpointData` reads `e.state.planVersion` without acquiring `planMu`. Writes happen under `planMu.Lock()` at lines 1273 and 499. This is a data race.

**Trigger:** Concurrent `SaveCheckpointData` and `SetRefinementFeedback`.

---

### B13 — GetCheckpointData reads without synchronization [Medium]

**File:** `internal/engine/workflow/engine.go:565-567`

```go
func (e *Engine) GetCheckpointData() *CheckpointData {
    return e.state.checkpointData
}
```

**Intended:** Thread-safe read of shared state.

**Actual:** `SaveCheckpointData` (line 460) writes `e.state.checkpointData = cp` without any lock. `GetCheckpointData` reads it without any lock. Data race if called from different goroutines.

**Trigger:** Concurrent `GetCheckpointData` and `SaveCheckpointData`.

---

### B14 — Transition checkpoint lacks Goal and PlanVersion [Medium]

**File:** `internal/engine/workflow/phase_coordinator.go:124-127`

```go
cp := session.Checkpoint{
    Phase:     to,
    Timestamp: time.Now(),
}
```

**Intended:** Transition checkpoints should carry enough state for meaningful resume.

**Actual:** Only `Phase` and `Timestamp` are set. `Goal` and `PlanVersion` are zero values. If the process crashes between `Transition()` and the next `SaveCheckpointData`, the checkpoint on disk lacks goal/planVersion, making resume degraded.

**Trigger:** Process crash after `Transition()` but before `SaveCheckpointData()`.

---

### B15 — SetPhase doesn't reset discussPlanCycles [Medium]

**File:** `internal/engine/workflow/state_machine.go:101-105`

```go
func (sm *StateMachine) SetPhase(phase m31types.WorkflowPhase) {
    sm.mu.Lock()
    defer sm.mu.Unlock()
    sm.currentPhase = phase
    sm.history = append(sm.history, phase)
}
```

**Intended:** Checkpoint restore should give a clean state.

**Actual:** `discussPlanCycles` is never reset by `SetPhase`. If a checkpoint is saved mid-oscillation (e.g., `discussPlanCycles = 2`), restoring that checkpoint keeps the counter at 2, causing the next oscillation to hit the limit sooner.

**Trigger:** Checkpoint restore after 1-2 discuss/plan oscillation cycles.

---

### B16 — Token estimation truncates toward zero [Medium]

**File:** `internal/engine/tokens/estimator.go:194` (+ 8 identical sites)

```go
return int(float64(chars) / ratio)
```

**Intended:** Produce an upper-bound token estimate for context window safety.

**Actual:** `int()` truncates toward zero. `chars=100, ratio=3.0` -> `int(33.33)` = 33 tokens, but actual could be 34. Every non-exact-multiple underestimates by up to 1 token per message. In aggregate across hundreds of messages, the underestimate accumulates. This is the core BUG-29 window overflow vector.

**Trigger:** Any text whose char count is not an exact multiple of the provider ratio.

---

### B17 — Preflight truncation uses inconsistent token accounting [Medium]

**File:** `internal/engine/workflow/engine.go:1026 vs 1078-1089`

Line 1026 uses `EstimateMessages` (full estimate with per-message overhead + tool call JSON). Lines 1078-1089 use per-message `Estimate` (content only, no overhead, no tool calls). The second estimate is systematically lower, causing truncation to stop prematurely while the actual count is still above the threshold.

**Trigger:** Any time the 80% threshold is exceeded and truncation activates.

---

### B18 — Cannot override int/float config with zero value [Medium]

**File:** `internal/core/config/merge.go:40-51`

```go
func (m mergeHelper) intField(base, overlay *int, key string) {
    if *overlay != 0 {
        *base = *overlay
    }
}
```

**Intended:** Overlay TOML values override base when explicitly set.

**Actual:** Zero values in project TOML are indistinguishable from "not set." A project config cannot set `max_iterations = 0` to override a global non-zero default.

**Trigger:** Global config sets `max_iterations = 50`, project sets `max_iterations = 0`. Result: 50 persists.

---

### B19 — Variable substitution covers only ~10 hardcoded fields [Medium]

**File:** `internal/core/config/config_validate.go:311-337`

```go
func applyVarSubstitution(cfg *Config) []string {
    var unresolved []string
    unresolved = append(unresolved, substituteVarsReport(&cfg.Provider.Default, "provider.default")...)
    // ... only ~10 hardcoded fields
```

**Intended:** Walk all string fields in Config and substitute `${VAR}`.

**Actual:** Only ~10 fields are hardcoded. Fields like `Git.UserName`, `Git.UserEmail`, `Compaction.SummaryTemplate`, `Prompts.*`, `Narrative.*`, `Templates.*` preserve literal `${VAR}` patterns without resolving them or logging a warning.

**Trigger:** Set `git.user_name = "${USER}"` in m31a.toml — arrives as literal `${USER}`.

---

### B20 — Checkpoint save is read-modify-write without locking [Medium]

**File:** `internal/engine/session/checkpoint.go:30-53`

```go
func (m *Manager) SaveCheckpoint(sessionID string, cp Checkpoint) error {
    existing, err := m.loadCheckpointsRaw(sessionID) // no lock
    existing = append(existing, cp)
    // ...
    return m.atomicWrite(path, data)
}
```

**Intended:** Safely append a checkpoint.

**Actual:** Two concurrent saves will overwrite each other's checkpoint (last writer wins). Additionally, `LoadCheckpoints` rewrites the file when pruning (line 104) — a read-only operation mutating state.

**Trigger:** Two goroutines call `SaveCheckpoint` simultaneously.

---

### B21 — Keychain permanently blacklists after first failure [Medium]

**File:** `internal/integrations/keychain/keychain.go:61-63`

```go
if err != nil && (err == ErrKeychainUnavailable || err == ErrNotImplemented) {
    c.unavailable.Store(true)  // never reset
}
```

**Intended:** Avoid repeated failed connection attempts.

**Actual:** If the keychain is temporarily unavailable (D-Bus restart, pinentry timeout, GNOME Keyring crash), the application permanently switches to config-file fallback for the rest of the session. No TTL, no retry, no recovery without restart. Silent security downgrade.

**Trigger:** Any transient keychain failure on headless Linux or Docker.

---

### B22 — "Degraded" health status treated as offline [Medium]

**File:** `internal/integrations/provider/fallback.go:106-118`

```go
if status.Status == "live" { ... }
if status.Status == "slow" && slowFallback == "" { ... }
// "degraded" falls through — neither live nor slow
```

**Intended:** Three-tier health: live, slow, offline.

**Actual:** `HealthCheck` returns four statuses (live, slow, degraded, offline), but `FindFallbackProvider` only handles live and slow. A degraded provider (latency >= HealthSlowMs but still functional) is silently skipped — treated the same as offline.

**Trigger:** All providers have degraded status -> `ErrProviderUnreachable` even though they could serve requests.

---

### B23 — resizePending mutated from goroutine [Medium]

**File:** `internal/ui/tui/components/repl.go:64`

```go
m.resizeTimer = time.AfterFunc(100*time.Millisecond, func() {
    m.resizePending = true  // goroutine mutates Bubble Tea model
})
```

**Intended:** Debounce resize events.

**Actual:** `m.resizePending = true` mutates `ReplModel` from a `time.AfterFunc` goroutine, violating Bubble Tea's single-threaded model. The comment even acknowledges the rule it breaks. Data race on `resizePending`; can corrupt adjacent struct fields.

**Trigger:** Rapid terminal resize.

---

### B24 — d.collector read without lock in Execute [Medium]

**File:** `internal/tools/dispatcher.go:315-316`

```go
if d.collector != nil {
    d.collector.RecordToolCall(call.Name, err == nil, elapsed)
}
```

**Intended:** Read `d.collector` safely.

**Actual:** `d.collector` is read with no lock held. `SetCollector` writes it under `d.mu.Lock()`. Formal data race that `-race` will flag.

**Trigger:** `SetCollector` called concurrently with `Execute`.

---

### B25 — Duplicate history entry on every TUI phase transition [Low]

**File:** `internal/engine/workflow/engine.go:860` + `state_machine.go:96`

In the TUI flow, `Transition()` calls `stateMachine.Transition()` which appends to history (line 96), then `RunPhaseCmd()` calls `SetPhase()` which also appends to history (line 105). Every phase transition produces two history entries.

---

### B26 — History grows unboundedly [Low]

**File:** `internal/engine/workflow/state_machine.go:96,105`

Every `Transition()` and `SetPhase()` appends to `history` with no cap. Long-running sessions accumulate unbounded memory.

---

### B27 — doublestar.Match errors silently swallowed [Low]

**File:** `internal/tools/permissions.go:365,401,417`

```go
matched, _ := doublestar.Match(pattern, val)
```

Malformed glob patterns silently return no-match instead of erroring.

---

### B28 — Bare type assertion in DNS cache eviction [Low]

**File:** `internal/tools/search/dns_cache.go:134`

```go
key.(string)
```

No comma-ok check. Safe in practice (all keys are strings) but would panic on unexpected type.

---

### B29 — Default model capabilities never cached [Low]

**File:** `internal/integrations/provider/capabilities.go:355-366`

Default capabilities for unknown models are allocated on every call, never stored in `modelCapabilitiesCache`.

---

### B30 — Stop() does not drain requestCh/questionReqCh [Low]

**File:** `internal/tools/dispatcher.go:432-448`

`Stop()` drains `responseCh` but not `requestCh`, `questionReqCh`, or `questionRespCh`. Goroutines blocked sending on these channels will leak until process exit.

---

## Recommended Fix Order

Ordered by severity x blast radius:

| Priority | ID | Description | Rationale |
|----------|-----|-------------|-----------|
| 1 | B01 | Proactive compaction discarded | Critical — compaction fires but does nothing, session grows unbounded |
| 2 | B03 | Permission deny loses to allow | High — security: denied commands execute without prompt |
| 3 | B06 | flock cross-process dual-lock | High — two instances corrupt session files |
| 4 | B05 | saveSessionAtomic overwrites Messages | High — data corruption on session resume |
| 5 | B04 | LoadWorkflowState missing lock | High — data race on concurrent access |
| 6 | B02 | RunPhase bypasses transition validation | High — state machine invariant broken |
| 7 | B07 | Shutdown nil cache race | High — nil pointer panic on shutdown |
| 8 | B08 | Auth errors don't trigger fallback | High — users stuck on broken provider |
| 9 | B09 | Mid-stream errors don't trigger fallback | High — common failure mode, no recovery |
| 10 | B10 | Permission response dropped on timeout | Medium — user action silently ignored |
| 11 | B11 | Hardcoded param keys in permissions | Medium — permission rules silently fail for some tools |
| 12 | B12-B13 | Checkpoint data races | Medium — corrupted checkpoint on concurrent access |
| 13 | B16-B17 | Token estimation errors | Medium — context window overflow risk |
| 14 | B18-B19 | Config merge/substitution gaps | Medium — silent misconfiguration |
| 15 | B20 | Checkpoint R-M-W race | Medium — lost checkpoint data |
| 16 | B21 | Keychain permanent blacklist | Medium — silent security downgrade |
| 17 | B22 | Degraded health not handled | Medium — unnecessary fallback storms |
| 18 | B23 | TUI goroutine mutation | Medium — data race on resize |
| 19 | B24 | Collector read without lock | Medium — formal data race |
| 20 | B14-B15 | Checkpoint restore gaps | Medium — degraded resume |
| 21 | B25-B30 | Low-severity items | Low — correctness/cosmetic |

---

*Audit complete. Awaiting review before implementing fixes.*
