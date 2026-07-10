# Phase 2: Wiring Remediation - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-10
**Phase:** 2-Wiring Remediation
**Areas discussed:** Critical fix strategy, Dead code & duplication, Test coverage strategy, Lifecycle & cleanup

---

## Critical fix strategy

### Session save error handling (PS-01)

| Option | Description | Selected |
|--------|-------------|----------|
| Fail-fast (Recommended) | Session save fails → return error → app.Shutdown logs and exits | ✓ |
| Graceful degradation | Session save fails → log warning, continue | |
| Retry then fail | Session save fails → retry 3x with backoff, then fail-fast | |

**User's choice:** Fail-fast (Recommended)
**Notes:** Prevents silent data loss. Simplest to implement and reason about.

### Rollback session handling (PS-04)

| Option | Description | Selected |
|--------|-------------|----------|
| Save-then-restore (Recommended) | Before git restore: save session. After: reload session. | ✓ |
| Files only | Rollback only restores files. Session state lost. | |
| Atomic rollback | Save session, restore files, restore session. Revert on failure. | |

**User's choice:** Save-then-restore (Recommended)
**Notes:** Uses existing SaveSession/LoadSession methods. Session state preserved through rollback.

---

## Dead code & duplication

### Keychain consolidation (PK-02)

| Option | Description | Selected |
|--------|-------------|----------|
| Consolidate to pkg (Recommended) | Keep pkg/keychain, remove internal/keychain | ✓ |
| Keep both, document | Keep both, add clear docs on which to use | |
| Consolidate to internal | Keep internal/keychain, make pkg a wrapper | |

**User's choice:** Consolidate to pkg (Recommended)
**Notes:** Single source of truth. All imports updated to use pkg/keychain.

### Unused packages (DC-07/DC-08)

| Option | Description | Selected |
|--------|-------------|----------|
| Remove (Recommended) | Delete pkg/skills and pkg/arbitrage entirely | |
| Keep as stubs | Keep but add stub implementations | |
| Move to internal | Move to internal/ to preserve code but correct layering | ✓ |

**User's choice:** Move to internal
**Notes:** Preserve code but correct the layering. These are internal implementation details.

### CodeComplexity tool (DC-04)

| Option | Description | Selected |
|--------|-------------|----------|
| Remove tool (Recommended) | Remove from defaults.go registration | ✓ |
| Implement and wire | Wire up Execute() with a code complexity command | |
| Keep as experimental | Keep registered, mark as experimental | |

**User's choice:** Remove tool (Recommended)
**Notes:** Tool is dead — Execute() never called, no consumer.

---

## Test coverage strategy

### Test framework

| Option | Description | Selected |
|--------|-------------|----------|
| testing + testify (Recommended) | Go testing + testify for assertions | ✓ |
| testing only | Go testing only, no external deps | |
| testing + gomock + testify | Full mocking + assertion stack | |

**User's choice:** testing + testify (Recommended)
**Notes:** Standard in Go ecosystem, already in go.mod.

### Mocking approach

| Option | Description | Selected |
|--------|-------------|----------|
| Hand-written fakes (Recommended) | Simple fakes for internal interfaces | |
| Generated mocks (mockery) | Use mockery to generate from interfaces | |
| Hybrid (Recommended for providers) | httptest for providers, hand-written fakes for internal | ✓ |

**User's choice:** Hybrid (Recommended for providers)
**Notes:** Providers get realistic HTTP mocking via httptest, internal interfaces get simple fakes.

### Coverage enforcement

| Option | Description | Selected |
|--------|-------------|----------|
| Enforce in CI (Recommended) | Fail CI if below 75% overall or 90% for target packages | ✓ |
| Report only | Generate report but don't fail | |
| Enforce with buffer | Fail at 73% not 75% for margin | |

**User's choice:** Enforce in CI (Recommended)
**Notes:** Catches regressions. Simple and clear.

---

## Lifecycle & cleanup

### Background worker tracking (SW-04)

| Option | Description | Selected |
|--------|-------------|----------|
| WaitGroup + timeout (Recommended) | Add WaitGroup, wait with timeout | |
| Context cancellation | Use context.Context with cancel | |
| Both (context + WaitGroup) | Context for signal, WaitGroup for completion | ✓ |

**User's choice:** Both (context + WaitGroup)
**Notes:** Most robust. Context cancels, WaitGroup tracks completion.

### Config watcher shutdown (SW-05)

| Option | Description | Selected |
|--------|-------------|----------|
| Store cancel func (Recommended) | Store cancel func in AppState, call in Shutdown | ✓ |
| Watcher struct with Stop() | Wrap in struct with Stop() method | |
| Use app context | Inherit from app context | |

**User's choice:** Store cancel func (Recommended)
**Notes:** Clean and explicit lifecycle management.

### Agent tool behavior (TL-06)

| Option | Description | Selected |
|--------|-------------|----------|
| Make child (Recommended) | Change false → true, prevent recursive spawning | ✓ |
| Keep background + depth limit | Keep background but add max-recursion | |
| Configurable | Add config field for child vs background | |

**User's choice:** Make child (Recommended)
**Notes:** Simplest fix. Prevents recursive background spawning.

---

## the agent's Discretion

- Medium severity issues (34 items): Agent decides fix approach per issue
- Low severity issues (85 items): Agent decides fix approach per issue
- Specific file-level implementation details for each fix

## Deferred Ideas

None — discussion stayed within phase scope
