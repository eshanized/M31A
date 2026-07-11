# Phase 01: Fix Emitter Stress Test Methodology - Research

**Researched:** 2026-07-11
**Domain:** Go/Bubble Tea TUI emitter channel stress testing
**Confidence:** HIGH

## Summary

The current emitter stress test (`TestEmitterDropsUnderStreamingLoad` in `internal/tui/emitter_stress_test.go`) calls `engine.RunPhase()` directly without running the Bubble Tea event loop. The production drain mechanism (`drainEmitterCmd`/`drainMultipleCmd`/`drainAdaptiveCmd`) only executes when the TUI's `Update()` loop processes messages and returns drain commands. Without the event loop, the emitter channel is never drained, making the test incapable of detecting message drops even under extreme load.

**Primary recommendation:** Add a manual drain goroutine in the test that invokes the actual production drain logic (`drainAdaptiveCmd`) at the real Bubble Tea event loop rate, reusing existing drain functions rather than reimplementing. Add a positive control that deliberately saturates the channel to verify drop detection works.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| Emitter channel emission | API / Backend (workflow engine) | — | `Engine.emit()` called from workflow goroutines |
| Emitter channel drain | Frontend Server (Bubble Tea TUI) | — | `drainAdaptiveCmd` runs in `Update()` loop, single-threaded |
| Drop counting | Shared (atomic counter) | — | `DropCounter` in `app_channel.go` used by both emitter and TUI |
| Stress test execution | Test Infrastructure | — | Unit test runs workflow engine without TUI event loop |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `github.com/charmbracelet/bubbletea` | v0.25+ | TUI framework, event loop, commands | Project standard; Elm architecture |
| `github.com/eshanized/M31A/internal/tui` | (local) | App state, emitter, drain commands | Project-internal, production code |
| `github.com/eshanized/M31A/internal/provider/mock` | (local) | `StreamingMockProvider` for load generation | Purpose-built for stress testing |
| `github.com/eshanized/M31A/internal/workflow` | (local) | `Engine`, `RunPhase`, phases | Core workflow execution |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `sync/atomic` | stdlib | `DropCounter` thread-safe counting | Drop tracking |
| `context` | stdlib | Cancellation, timeouts | Drain goroutine lifecycle |
| `testing` | stdlib | Test framework | Standard Go testing |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Manual drain goroutine | Full `tea.Program` integration test | Heavier, slower, flakier; unit test approach is sufficient for channel saturation testing |
| Custom drain reimplemention | Reuse `drainAdaptiveCmd` | **Don't reimplement** — must use production logic exactly (per CONTEXT.md) |

**Installation:** No new packages required. All dependencies are internal or stdlib.

## Package Legitimacy Audit

> Required since this phase uses internal packages only (no external installs).

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| (internal) | N/A | N/A | N/A | N/A | N/A | N/A |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none
*No external packages installed in this phase — all dependencies are internal or stdlib.*

## Architecture Patterns

### System Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        PRODUCTION RUNTIME (TUI)                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────┐     Emit()      ┌──────────────────┐                      │
│  │ Workflow     │ ──────────────► │ channelEmitter   │                      │
│  │ Engine       │   tea.Msg       │ (buffered chan   │                      │
│  │ (goroutine)  │                 │  cap=512)        │                      │
│  └──────────────┘                 └────────┬─────────┘                      │
│                                            │                                 │
│                                            │ Retry (3×, 5ms backoff)        │
│                                            ▼                                 │
│                                   ┌──────────────────┐                      │
│                                   │ DropCounter      │◄── DroppedMessages() │
│                                   │ (atomic.Int64)   │                      │
│                                   └────────┬─────────┘                      │
│                                            │                                 │
│  ┌─────────────────────────────────────────┘                                 │
│  │ Bubble Tea Event Loop (single-threaded)                                   │
│  │                                                                              │
│  │  Update(msg) ───► Returns tea.Cmd ───► drainAdaptiveCmd()                  │
│  │                      │                         │                            │
│  │                      │                         ▼                            │
│  │                      │                ┌──────────────────┐                  │
│  │                      │                │ Reads 1-4 msgs   │                  │
│  │                      │                │ from channel     │                  │
│  │                      │                │ (batch if >25%   │                  │
│  │                      │                │  capacity)       │                  │
│  │                      │                └──────────────────┘                  │
│  │                      ▼                                                     │
│  │                 Render UI                                                    │
│  └──────────────────────────────────────────────────────────────────────────┘
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
                                      ▲
                                      │ Test must simulate this drain rate
                                      │
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CURRENT TEST (BROKEN)                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Test calls engine.RunPhase() directly                                       │
│  │                                                                           │
│  ▼                                                                           │
│  Workflow emits messages ──────► Channel fills but NEVER drained            │
│  │                                                                           │
│  ▼                                                                           │
│  Test checks DroppedMessages() = 0 (always, because channel never full)     │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Recommended Project Structure (Test File)
```
internal/tui/
├── emitter_stress_test.go        # FIX: add drain goroutine, positive control, scenarios
├── app_channel_test.go           # Existing unit tests for drain logic (keep)
├── app.go                        # Production: drainEmitterCmd, drainMultipleCmd, drainAdaptiveCmd
├── app_channel.go                # Production: channelEmitter, DropCounter, ChannelCap=512
└── narrative_emitter.go          # Production: narrativeEmitter wraps channelEmitter
```

### Pattern 1: Production Drain Logic (Reuse, Don't Reimplement)
**What:** The adaptive drain logic in `app.go` that chooses single vs batch drain based on channel load.
**When to use:** In the test's manual drain goroutine to mimic production exactly.
**Example:**
```go
// Source: internal/tui/app.go (lines 609-620)
func (m *AppState) drainAdaptiveCmd() tea.Cmd {
	if m.emitterCh == nil {
		return nil
	}
	if m.emitterLoad() > ChannelCap/4 {  // >25% capacity = high load
		return m.drainMultipleCmd()      // batch: up to 4 msgs
	}
	return m.drainEmitterCmd()           // single: 1 msg
}
```

### Pattern 2: StreamingMockProvider for Load Generation
**What:** Configurable mock provider that emits N chunks with M delay, supports concurrency limit.
**When to use:** Both streaming scenario (200 chunks, 1ms delay, 10 concurrent) and tool-burst scenario.
**Example:**
```go
// Source: internal/provider/mock/streaming.go
provider := &mock.StreamingMockProvider{
    ChunksPerResponse: 200,
    ChunkDelay:        1 * time.Millisecond,
    ConcurrencyLimit:  10,
}
```

### Anti-Patterns to Avoid
- **Don't reimplement drain logic:** CONTEXT.md explicitly requires reusing `drainEmitterCmd`/`drainMultipleCmd`. Custom drain logic won't match production behavior (adaptive batching, 25% threshold).
- **Don't use fixed timer for drain:** Production "tick" is the Bubble Tea event loop cycle (event-driven), not a fixed interval. The drain rate = message processing rate.
- **Don't reduce ChannelCap for testing:** CONTEXT.md says "Keep channel capacity at production value (512)". Test at real capacity.
- **Don't run full `tea.Program` in unit test:** Too heavy, flaky. Manual drain goroutine is the prescribed approach.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Drain loop timing | Custom ticker with fixed interval | Call `drainAdaptiveCmd()` in a loop with `time.Sleep` calibrated to event loop rate | Production uses adaptive batching (1 vs 4 msgs) based on channel load; fixed timer misses this |
| Drop detection | Custom counter | `DroppedMessages()` / `ResetDropCounter()` from `app_channel.go` | Atomic, tested, shared with production |
| Load generation | Custom streaming logic | `StreamingMockProvider` | Already supports chunks, delay, concurrency; tested |
| Positive control | Hope drops happen | Deliberately disable/slow drain, verify `DroppedMessages() > 0` | Guarantees test can catch regressions |

**Key insight:** The drain behavior is adaptive (1 msg at low load, up to 4 at high load). A fixed-rate drain misses this critical production behavior. Reusing `drainAdaptiveCmd` ensures the test exercises the exact same logic path.

## Runtime State Inventory

> This is a **greenfield test fix phase** — no rename/refactor/migration. No runtime state inventory needed. **SKIPPED.**

## Common Pitfalls

### Pitfall 1: Test Passes But Doesn't Actually Test Drops
**What goes wrong:** Test runs, reports 0 drops, looks "green" but the drain never ran so channel never saturated.
**Why it happens:** Calling `engine.RunPhase()` without any drain goroutine. Messages emit but accumulate; test ends before channel fills.
**How to avoid:** Add drain goroutine *before* running phases. Verify drain is running by logging channel length periodically.
**Warning signs:** `DroppedMessages() == 0` under extreme load (200 chunks × 10 concurrent × 7 phases = 14,000+ msgs into 512 cap).

### Pitfall 2: Drain Rate Doesn't Match Production
**What goes wrong:** Test uses fixed `time.Tick(16ms)` (60fps) but production event loop runs faster/slower depending on message volume.
**Why it happens:** Assuming Bubble Tea runs at fixed frame rate. Actually, `Update()` is called per message — more messages = more drains.
**How to avoid:** Calibrate drain loop to match production throughput. Start with ~60Hz (16ms) but verify against actual message rates.
**Warning signs:** Test shows drops at much lower/higher load than expected from capacity math.

### Pitfall 3: Positive Control Fails Silently
**What goes wrong:** Positive control (no drain) doesn't show drops because channel capacity (512) isn't exceeded by test volume.
**Why it happens:** Test only runs Initialize phase (current code) or sequential phases without enough concurrent emitters.
**How to avoid:** Positive control must run the full streaming scenario (200 chunks × 10 concurrent) with drain *disabled* or severely throttled (e.g., 1 msg/100ms). Verify `DroppedMessages() > 0`.
**Warning signs:** Positive control reports 0 drops — test is invalid.

### Pitfall 4: NarrativeEmitter Double-Wraps Channel
**What goes wrong:** Production uses `narrativeEmitter` which wraps `channelEmitter`. Test creates raw `channelEmitter` or uses engine without narrative wrapper.
**Why it happens:** `initWorkflowEngine()` in `app.go` creates `narrativeEmitter` but test setup in `emitter_stress_test.go` uses `simplePlanProvider` without narrative emitter.
**How to avoid:** Ensure test uses same emitter setup as production: `narrativeEmitter` wrapping `channelEmitter` with `ChannelCap` capacity.
**Warning signs:** Drop counter behavior differs between test and production.

## Code Examples

### Production Drain Commands (Source: `internal/tui/app.go`)
```go
// Single message drain (low load)
func (m *AppState) drainEmitterCmd() tea.Cmd {
	if m.emitterCh == nil {
		return nil
	}
	return func() tea.Msg {
		select {
		case msg := <-m.emitterCh:
			return msg
		case <-m.shutdownCtx.Done():
			return nil
		}
	}
}

// Batch drain up to maxDrainPerTick (4) messages (high load)
func (m *AppState) drainMultipleCmd() tea.Cmd {
	if m.emitterCh == nil {
		return nil
	}
	return func() tea.Msg {
		select {
		case first := <-m.emitterCh:
			var batch []tea.Msg
			batch = append(batch, first)
		drainLoop:
			for i := 1; i < maxDrainPerTick; i++ {
				select {
				case msg := <-m.emitterCh:
					batch = append(batch, msg)
				default:
					break drainLoop
				}
			}
			if len(batch) == 1 {
				return first
			}
			return DrainBatchMsg{Messages: batch}
		case <-m.shutdownCtx.Done():
			return nil
		}
	}
}

// Adaptive: chooses single vs batch based on channel load (>25% = batch)
func (m *AppState) drainAdaptiveCmd() tea.Cmd {
	if m.emitterCh == nil {
		return nil
	}
	if m.emitterLoad() > ChannelCap/4 {  // 512/4 = 128
		return m.drainMultipleCmd()
	}
	return m.drainEmitterCmd()
}
```

### StreamingMockProvider Usage (Source: `internal/provider/mock/streaming.go`)
```go
provider := &mock.StreamingMockProvider{
    ChunksPerResponse: 200,           // chunks per stream response
    ChunkDelay:        1 * time.Millisecond,
    ConcurrencyLimit:  10,            // max concurrent streams
    ResponseContent:   "chunk-",      // base content
}
```

### Existing Channel/Drain Unit Tests (Source: `internal/tui/app_channel_test.go`)
```go
// Tests adaptive drain chooses batch under high load
func TestDrainAdaptiveCmd_HighLoad(t *testing.T) {
    ch := make(chan tea.Msg, ChannelCap)
    for i := 0; i < ChannelCap/4+1; i++ {  // >128 messages = high load
        ch <- i
    }
    m := &AppState{emitterCh: ch, shutdownCtx: ctx}
    cmd := m.drainAdaptiveCmd()
    msg := cmd()
    _, ok := msg.(DrainBatchMsg)
    if !ok {
        t.Fatalf("expected DrainBatchMsg under high load, got %T", msg)
    }
}
```

### Current Flawed Stress Test (Source: `internal/tui/emitter_stress_test.go`)
```go
func TestEmitterDropsUnderStreamingLoad(t *testing.T) {
    // ... setup ...
    result, err := engine.RunPhase(ctx, m31types.PhaseInitialize, "Build a Go CLI tool")
    // NO DRAIN LOOP HERE
    dropped := DroppedMessages()  // Always 0
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No drain in test | Manual drain goroutine using `drainAdaptiveCmd` | This phase | Test actually measures drops |
| Fixed-rate drain | Adaptive drain matching production | This phase | Correct batch/single behavior |
| No positive control | Positive control with disabled drain | This phase | Verifies drop detection works |
| Single phase (Initialize) | All 7 phases + tool-burst scenario | This phase | Realistic load profile |

**Deprecated/outdated:**
- Running only `PhaseInitialize` in stress test — insufficient message volume
- Assuming 0 drops means "working" — without drain, 0 drops is expected (channel never fills)

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Production drain rate ≈ Bubble Tea event loop frequency (message-driven, not timer-driven) | Architecture Patterns | Drain loop calibration wrong → test doesn't match production saturation point |
| A2 | `drainAdaptiveCmd` called once per `Update()` cycle in production | Architecture Patterns | If called more/less often, test drain rate is wrong |
| A3 | `StreamingMockProvider` with 200 chunks × 1ms × 10 concurrent generates enough load to saturate 512-cap channel with 4/4-per-tick drain | Standard Stack | If load too low, even corrected test shows 0 drops (false negative) |
| A4 | `narrativeEmitter` doesn't significantly alter drop behavior vs raw `channelEmitter` | Common Pitfalls | If narrative processing adds overhead, drop threshold shifts |
| A5 | Channel capacity 512 and maxDrainPerTick 4 are production constants not to be changed | Don't Hand-Roll | If these change, test expectations must update |

## Open Questions

1. **What is the actual Bubble Tea event loop frequency under load?**
   - What we know: `Update()` called per message; more messages = more drains
   - What's unclear: Exact msg/sec rate in typical streaming scenario
   - Recommendation: Start drain loop at ~60Hz (16ms), add logging to measure actual drain throughput, calibrate if needed

2. **Does `narrativeEmitter` emit additional messages (NarrativeBubbleMsg) that increase channel load?**
   - What we know: `narrativeEmitter.Emit()` calls `inner.Emit()` for original + each narrative
   - What's unclear: How many narratives per workflow message on average
   - Recommendation: Test with narrative emitter (production config) to capture full load

3. **Tool-burst scenario message types and volume?**
   - What we know: Up to 8 concurrent tools (`MaxConcurrentTools`), each emits TaskStartMsg/ToolStartMsg/ToolCompleteMsg
   - What's unclear: Exact message count per tool execution, timing
   - Recommendation: Instrument a real tool run or estimate from code; aim for burst that exceeds drain capacity

## Environment Availability

> This phase is code/config-only (test changes). No external dependencies.

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Go toolchain | Build/test | ✓ | 1.25+ (per go.mod) | — |
| Bubble Tea | TUI framework | ✓ | v0.25+ (go.mod) | — |
| Internal packages | All test code | ✓ | Local | — |

**Missing dependencies with no fallback:** none
**Missing dependencies with fallback:** none

## Validation Architecture

> Nyquist validation is enabled (workflow.nyquist_validation not explicitly false in config).

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `testing` (stdlib) + `github.com/stretchr/testify` (if used elsewhere) |
| Config file | none — standard `go test` |
| Quick run command | `go test -run TestEmitterDropsUnderStreamingLoad ./internal/tui/ -v -count=1` |
| Full suite command | `make test` (race-enabled, coverage) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| REQ-001.1 | Find production tick interval & throughput | Analysis | N/A (research) | ✅ RESEARCH.md |
| REQ-001.2 | Add manual drain loop at production rate | Unit | `go test -run TestEmitterDropsUnderStreamingLoad ./internal/tui/` | ❌ Wave 0 |
| REQ-001.3 | Add positive control (verify drops detectable) | Unit | `go test -run TestEmitterPositiveControl ./internal/tui/` | ❌ Wave 0 |
| REQ-001.4 | Run corrected streaming scenario (200 chunks, 1ms, 10 concurrent, 7 phases) | Unit | `go test -run TestEmitterStreamingScenario ./internal/tui/` | ❌ Wave 0 |
| REQ-001.5 | Run tool-burst scenario (8 concurrent tools) | Unit | `go test -run TestEmitterToolBurstScenario ./internal/tui/` | ❌ Wave 0 |
| REQ-001.6 | Write audit report v2 | Doc | Manual verification | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `go test -run TestEmitterDropsUnderStreamingLoad ./internal/tui/ -count=1`
- **Per wave merge:** `go test ./internal/tui/ -count=1`
- **Phase gate:** `make test` (full suite, race detector, coverage) green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `TestEmitterDropsUnderStreamingLoad` — rewritten with drain goroutine (covers REQ-001.2, .4)
- [ ] `TestEmitterPositiveControl` — new test, disabled drain (covers REQ-001.3)
- [ ] `TestEmitterStreamingScenario` — new test, full 7-phase streaming (covers REQ-001.4)
- [ ] `TestEmitterToolBurstScenario` — new test, 8 concurrent tools (covers REQ-001.5)
- [ ] `docs/audits/emitter-stress-test-results-v2.md` — audit report (covers REQ-001.6)

## Security Domain

> Security enforcement enabled (not explicitly disabled in config).

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | — |
| V3 Session Management | no | — |
| V4 Access Control | no | — |
| V5 Input Validation | yes | Go type system, bounded channels |
| V6 Cryptography | no | — |
| V7 Error Handling | yes | Bounded retry (3×), graceful drop with logging |
| V8 Logging | yes | `slog.Warn` on drop, periodic drop log tick |

### Known Threat Patterns for Go Channel Emitter

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Channel saturation → message loss | Denial of Service | Bounded channel (512), retry with backoff (3×5ms), drop counter observability |
| Unbounded goroutine leak in drain | Resource Exhaustion | Drain runs in Bubble Tea event loop (single-threaded), no extra goroutines |
| Drop counter race | Tampering | `atomic.Int64` for `DropCounter` |
| Test false negative (no drain) | Integrity | **This phase fixes it** — positive control verifies drop detection |

## Sources

### Primary (HIGH confidence)
- `internal/tui/app.go` — `drainEmitterCmd`, `drainMultipleCmd`, `drainAdaptiveCmd`, `ChannelCap=512`, `maxDrainPerTick=4` — **production drain logic** [VERIFIED: source code]
- `internal/tui/app_channel.go` — `channelEmitter`, `DropCounter`, `maxRetries=3`, `retryBackoff=5ms` — **emission & drop counting** [VERIFIED: source code]
- `internal/tui/app_channel_test.go` — Unit tests for all drain variants, drop counter, concurrent emit/drain — **verified test patterns** [VERIFIED: source code]
- `internal/tui/emitter_stress_test.go` — Current flawed test (no drain loop) — **baseline to fix** [VERIFIED: source code]
- `internal/provider/mock/streaming.go` — `StreamingMockProvider` with configurable chunks/delay/concurrency — **load generator** [VERIFIED: source code]
- `internal/tui/narrative_emitter.go` — `narrativeEmitter` wrapping `channelEmitter` — **production emitter setup** [VERIFIED: source code]
- `.planning/phases/01-fix-emitter-stress-test-methodology/01-CONTEXT.md` — Phase decisions, specifics, deferred items — **authoritative constraints** [VERIFIED: source code]

### Secondary (MEDIUM confidence)
- `internal/workflow/engine.go` — `Engine.RunPhase`, `emit()`, phase execution — **workflow integration** [VERIFIED: source code]
- `docs/audits/emitter-stress-test-results.md` — Previous audit identifying the flaw — **historical context** [VERIFIED: source code]

### Tertiary (LOW confidence)
- Bubble Tea documentation (event loop behavior) — **framework behavior** [ASSUMED]

## Metadata

**Confidence breakdown:**
- Standard Stack: HIGH — All internal packages, verified in codebase
- Architecture: HIGH — Production drain logic and emitter fully read and understood
- Pitfalls: HIGH — Derived from code analysis and previous audit
- Code Examples: HIGH — Direct from source files

**Research date:** 2026-07-11
**Valid until:** 2026-08-11 (30 days — stable internal APIs)

---

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| REQ-001 | Fix emitter stress test methodology to properly measure message drops under streaming load | All sections above — drain logic, test patterns, load generator, positive control design |

---

## User Constraints (from CONTEXT.md)

### Locked Decisions
- Reuse actual drain logic (`drainEmitterCmd`/`drainMultipleCmd`) from `internal/tui/app.go` rather than reimplementing
- Keep channel capacity at production value (512)
- Run drain in a goroutine during the test
- Test both streaming scenario (200 chunks, 1ms delay, 10 concurrent, 7 phases) and tool-burst scenario (8 concurrent tools)
- Positive control: deliberately overflow channel (no drain or slow drain) → verify `DroppedMessages() > 0`
- New audit report: `docs/audits/emitter-stress-test-results-v2.md` superseding previous report

### the agent's Discretion
- Exact implementation of drain goroutine (timing, batching)
- Whether to modify existing test or add new test alongside
- Specific positive control implementation details

### Deferred Ideas (OUT OF SCOPE)
- Full TUI integration test with `tea.Program` (too heavy for unit test)
- Testing with reduced channel capacity (not needed — test at production capacity)
- Cross-phase message ordering guarantees (out of scope)