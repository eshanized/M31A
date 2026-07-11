# Phase 01: Fix Emitter Stress Test Methodology - Context

**Gathered:** 2026-07-11
**Status:** Ready for planning
**Source:** Direct task specification

<domain>
## Phase Boundary

Fix the emitter stress test methodology in `internal/tui/emitter_stress_test.go` to properly measure message drops under streaming load. The current test calls `engine.RunPhase()` directly without running the Bubble Tea event loop, so the `drainEmitterCmd` (which drains the emitter channel at up to 4 messages per tick in production) never runs. This makes the test incapable of detecting drops.

The fix requires:
1. Finding the production tick interval and throughput for the drain
2. Adding a manual drain loop in the test that mimics production behavior at the real rate
3. Adding a positive control to verify drops can be detected
4. Running corrected streaming and tool-burst scenarios
5. Writing a new audit report (v2)

</domain>

<decisions>
## Implementation Decisions

### Technical Approach
- Reuse the actual drain logic (`drainEmitterCmd`/`drainMultipleCmd`) from `internal/tui/app.go` rather than reimplementing
- Keep channel capacity at production value (512)
- Run drain in a goroutine during the test
- Test both streaming scenario (200 chunks, 1ms delay, 10 concurrent, 7 phases) and tool-burst scenario (8 concurrent tools)

### Test Design
- Positive control: deliberately overflow channel (no drain or slow drain) → verify DroppedMessages() > 0
- Streaming scenario: mock provider with 200 chunks/response, 1ms delay, 10 concurrent, all 7 phases, with real-rate drain
- Tool-burst scenario: 8 concurrent tools emitting TaskStartMsg/ToolStartMsg/ToolCompleteMsg in tight window, with real-rate drain

### Output
- New audit report: `docs/audits/emitter-stress-test-results-v2.md` superseding previous report

### Discretion
- Exact implementation of drain goroutine (timing, batching)
- Whether to modify existing test or add new test alongside
- Specific positive control implementation details

</decisions>

<canonical_refs>
## Canonical References

Downstream agents MUST read these before planning or implementing.

### Architecture
- `internal/tui/app.go` — `drainEmitterCmd`, `drainMultipleCmd`, `drainAdaptiveCmd`, `maxDrainPerTick=4`, `ChannelCap=512`
- `internal/tui/app_channel.go` — `channelEmitter.Emit` with retry/backoff, `DroppedMessages()`, `ResetDropCounter()`
- `internal/tui/app_channel_test.go` — existing channel/drain tests
- `internal/tui/emitter_stress_test.go` — current flawed test to fix
- `internal/provider/mock/streaming.go` — `StreamingMockProvider` for test load generation
- `internal/workflow/engine.go` — `RunPhase` execution, `emit()` via `MsgEmitter`
- `docs/audits/emitter-stress-test-results.md` — previous audit identifying the flaw
- `docs/ARCHITECTURE.md` — message-passing architecture docs

</canonical_refs>

<specifics>
## Specific Ideas

- The production "tick" is not a fixed timer — it's the Bubble Tea event loop cycle. Drain happens each time `Update()` processes a message and returns a drain command. Effective rate = message processing rate.
- `drainAdaptiveCmd()` chooses between single (1 msg) and batch (up to 4 msgs) based on channel load (>25% capacity = batch)
- Positive control can disable drain entirely and blast messages, or use a very slow drain (e.g., 1 msg/100ms)
- StreamingMockProvider already supports configurable chunks, delay, concurrency — reuse it
- Keep existing periodic drop-log addition from previous audit

</specifics>

<deferred>
## Deferred Ideas

- Full TUI integration test with `tea.Program` (too heavy for unit test)
- Testing with reduced channel capacity (not needed — test at production capacity)
- Cross-phase message ordering guarantees (out of scope)

</deferred>

---

*Phase: 01-fix-emitter-stress-test-methodology*
*Context gathered: 2026-07-11 via direct task specification*