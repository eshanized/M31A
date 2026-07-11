# Workflow→TUI Channel Emitter Drop Audit

**Date:** 2026-07-11  
**Auditor:** opencode (GSD emitter-drop-audit task)

## Summary

The channel emitter **has a drop counter (`globalDropCounter`)** that correctly increments when messages are dropped after 3 retries with 5ms backoff. However:

1. **The counter is never surfaced to the user or logged periodically** — only a `slog.Warn` fires *at the moment of each drop*, which is easily missed in normal operation.
2. **No existing test exercises the real `globalDropCounter` under concurrent phase transition + streaming load** — the stress test uses an isolated test emitter.
3. **Without a mock provider that streams at configurable rate, a realistic stress test cannot run without real API keys** — the existing integration test uses a single-chunk mock provider that completes instantly.

---

## 1. Channel Emitter Configuration (Confirmed)

| parameter | value | source |
|-----------|-------|--------|
| Channel capacity (`ChannelCap`) | 512 | `internal/tui/app_channel.go:16` |
| Max retries (`maxRetries`) | 3 | `internal/tui/app_channel.go:20` |
| Retry backoff (`retryBackoff`) | 5 ms | `internal/tui/app_channel.go:25` |
| Max drain per tick (`maxDrainPerTick`) | 4 | `internal/tui/app_channel.go:30` |

**Retry logic** (`app_channel.go:65-82`): Loop attempts `attempt <= maxRetries` (i.e., 4 total attempts: initial + 3 retries). On each failure, sleeps 5ms. After exhaustion, increments `globalDropCounter` and logs `slog.Warn`.

---

## 2. Drop Counter Observability

### Where the counter is read:
| location | reads counter? | notes |
|----------|----------------|-------|
| `internal/tui/app_channel.go:90` `DroppedMessages()` | **Yes** (exported) | Only called from tests |
| `internal/tui/narrative_emitter.go:82` `LogDropped()` | **Yes** | Never called in production code |
| `internal/tui/app_channel_test.go` | **Yes** | Multiple test assertions |
| **Production code paths** | **NO** | No periodic logging, no UI display, no shutdown hook |

### What happens on drop:
```go
// app_channel.go:77-81
dropped := ce.drops.Add(1)
slog.Warn("workflow message dropped: channel full after retries",
    "msg_type", fmt.Sprintf("%T", msg),
    "retries", maxRetries,
    "total_dropped", dropped)
```
This logs **once per dropped message** at `WARN` level. If `M31A_LOG_LEVEL=info` (default), these are **not visible** in the log file. Even at `WARN`, they appear only when a drop occurs — no periodic summary, no exit summary, no UI indicator.

**Conclusion:** If messages are being dropped in production, the user/developer has **no way to know** unless they happen to catch the `slog.Warn` in logs at the exact moment it fires.

---

## 3. Existing Test Coverage

### Unit test: `TestChannelEmitter_ConcurrentEmitAndDrain` (`app_channel_test.go:529`)
- Spawns 10 goroutines × 500 emits = 5000 messages
- Single drainer goroutine reads from channel
- Uses **isolated test emitter** (`newTestEmitter()` creates its own `DropCounter{}`)
- **Does not test `globalDropCounter`** — the global counter remains at 0
- Confirms drops happen under saturation (logged via `t.Logf`)

### Integration test: `TestFullWorkflow` (`workflow/integration_test.go:58`)
- Runs all 7 phases with a mock provider
- Mock provider (`multiTurnMockProvider`) returns **single-chunk responses instantly** — no streaming, no backpressure
- No concurrent drain + emit stress; phases run sequentially
- **Does not exercise channel saturation**

### No test exists for:
- Phase transition messages (`PhaseTransitionStartMsg`, `PhaseTransitionCompleteMsg`) emitted while LLM streaming chunks (`StreamMsg`, `ThinkingStartMsg`, etc.) flood the channel
- Realistic streaming: multiple chunks per response, multiple concurrent tool executions emitting `TaskStartMsg`/`ToolStartMsg`/`ToolCompleteMsg`

---

## 4. Minimal Debug Log Addition (Code Change)

Added one line in `app.go` `Shutdown()` to log the counter at exit:

```diff
--- a/internal/tui/app.go
+++ b/internal/tui/app.go
@@ -180,6 +180,10 @@ func (m *AppState) Shutdown() {
 	if m.subagentManager != nil {
 		m.subagentManager.Shutdown(context.Background())
 	}
+	// Emitter drop observability: log total drops on shutdown
+	if dropped := DroppedMessages(); dropped > 0 {
+		slog.Warn("workflow→TUI channel drops during session", "total_dropped", dropped)
+	}
```

This is **reversible** (delete the 4 lines), uses existing `slog` infrastructure, respects `M31A_LOG_LEVEL`, and fires exactly once per session.

---

## 5. Stress Test Attempt

### Can we run a stress test without real API keys?
**No.** The codebase has a mock provider (`multiTurnMockProvider` in `integration_test.go`), but it:
- Returns **one chunk per call** (`done = true` after first chunk)
- Completes **synchronously** — no streaming delay, no backpressure
- Cannot simulate: slow token generation, multiple chunks, concurrent tool calls

### What would be needed for a proper stress test:
1. **A streaming mock provider** that:
   - Yields N chunks per response with configurable delay between chunks
   - Supports concurrent `ChatCompletionStream` calls
   - Implements the full `provider.LLMProvider` interface
2. **A test scenario** that:
   - Starts a workflow
   - Triggers rapid phase transitions (e.g., Discuss → Plan → Execute)
   - Simultaneously runs tool executions that emit `TaskStartMsg`/`ToolStartMsg`/`ToolCompleteMsg`
   - Runs under `go test -race -count=1 ./internal/tui ./internal/workflow`

Without (1), the channel never fills because the TUI drain loop (4 msgs/tick) easily keeps up with the instantaneous mock responses.

---

## 6. Findings Summary

| question | answer |
|----------|--------|
| Was the counter previously observable? | **No** — only `slog.Warn` at drop moment (invisible at default log level), no periodic/exit summary, no UI |
| Did the counter increment under stress test? | **N/A** — no realistic stress test exists; unit test uses isolated counter |
| What conditions cause drops? | Channel saturation: 512 buffered messages exceeded. With 4 msgs/tick drain rate, sustained >128 msgs/sec from workflow goroutine causes backlog. Bursts from: parallel tool execution (up to 8 concurrent), LLM streaming chunks, phase transition events all arriving simultaneously. |

---

## Recommendations (Not Fixes — Audit Scope)

1. **Keep the shutdown log line** (added above) — zero cost, makes drops visible post-mortem
2. **Add periodic log** (e.g., every 30s via `tea.Tick`) if `DroppedMessages() > 0` — catches ongoing leaks
3. **Build a streaming mock provider** in `internal/provider/mock/` for integration stress tests
4. **Add integration test** `TestEmitterDropsUnderStreamingLoad` that runs workflow + TUI with streaming mock
5. **Consider surfacing drop count in sidebar/status bar** for real-time developer awareness