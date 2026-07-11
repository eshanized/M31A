# M31A Requirements

## REQ-001: Emitter Stress Test Fix

**Description**: The current emitter stress test (`TestEmitterDropsUnderStreamingLoad`) calls `engine.RunPhase()` directly without running the Bubble Tea event loop, so the `drainEmitterCmd` (which drains the emitter channel at up to 4 messages per tick in production) never runs. This makes the test incapable of detecting drops.

**Acceptance Criteria**:
1. **Find production tick interval**: Locate where `drainEmitterCmd` is scheduled in production and determine the effective tick interval and throughput (msgs/sec).
2. **Add manual drain loop**: Modify the test to include a goroutine that drains the emitter channel at the production rate, reusing the actual drain logic (`drainEmitterCmd`/`drainMultipleCmd`) rather than reimplementing.
3. **Add positive control**: Create a test scenario that deliberately overflows the channel (e.g., produce a burst of messages far exceeding drain capacity with no drain running, or with artificially slow drain) and verify `DroppedMessages() > 0`.
4. **Run corrected streaming scenario**: With the real-rate drain running, execute the original streaming scenario (200 chunks/response, 1ms chunk delay, 10 concurrent streams, all 7 phases) and report actual `DroppedMessages()` count.
5. **Test tool-burst scenario**: Simulate up to 8 concurrent tool executions (matching `MaxConcurrentTools`) each emitting `TaskStartMsg`/`ToolStartMsg`/`ToolCompleteMsg` in a tight window, with the real-rate drain running, and report drops separately.
6. **Write audit report**: Create `docs/audits/emitter-stress-test-results-v2.md` superseding the previous report, documenting: real tick interval and throughput, positive control result, drop counts for both scenarios, and a clear verdict on whether 512 capacity / 4-per-tick drain is adequate.

**Priority**: High