# Phase 5: Fix Wiring Issues - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-29
**Phase:** 05-fix-wiring-issues
**Areas discussed:** Phase structure, Dead code strategy, Batching, Test strategy, Rollback integration, Instructions injection, Metrics wiring, Streaming, Config merge, Zen parity, Dead code cleanup, Narrative events, Exec constants, IP filter dedup

---

## Phase Structure

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, Phase 5 (Recommended) | Add 'Fix Wiring Issues' as Phase 5 to ROADMAP.md and proceed with context gathering | ✓ |
| Merge into Phase 1 | Extend the existing Phase 1 (Audit Bug Fixes) to also cover wiring issues | |

**User's choice:** Yes, Phase 5 (Recommended)
**Notes:** Wiring issues are a separate body of work from the BUGS.md logical bugs. All 4 existing roadmap phases are complete.

---

## Dead Code Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Remove by default (Recommended) | Delete dead code unless it clearly should be wired in | |
| Wire in everything | Connect all dead producers to their intended consumers — nothing gets deleted | ✓ |
| Case by case | Discuss each A-category issue individually during planning | |

**User's choice:** Wire in everything
**Notes:** 33 Category A issues should be wired to their consumers, not deleted. This changes the fix strategy from removal to connection.

---

## Batching

| Option | Description | Selected |
|--------|-------------|----------|
| By severity (Recommended) | Critical → High → Medium → Low per audit ratings | ✓ |
| By category | Group by fix type: Config, Dead code, Permissions, etc. | |
| By package | Fix all issues in a package at once | |

**User's choice:** By severity (Recommended)
**Notes:** Matches the audit's own severity ratings. 4 batches: Critical (2) → High (13) → Medium (18) → Low (17).

---

## Test Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Integration tests (Recommended) | Test that wiring works end-to-end | ✓ |
| Unit tests only | Test each wired component in isolation with mocks | |
| No new tests | Wiring fixes verified by existing tests passing | |

**User's choice:** Integration tests (Recommended)
**Notes:** Each wiring fix needs a test that fails on current code and passes after fix. Tests verify the connection works, not just that the code compiles.

---

## Rollback Integration (W02)

| Option | Description | Selected |
|--------|-------------|----------|
| SoftReset on bisect failure (Recommended) | Call rollback.SoftReset() to undo bad commits, less aggressive | ✓ |
| HardReset on bisect failure | Full reset to pre-execute state | |
| User prompt | Let user decide at runtime | |

**User's choice:** SoftReset on bisect failure (Recommended)
**Notes:** SoftReset is less aggressive and preserves non-offending work. Matches the "wire in everything" philosophy.

---

## Instructions Injection (W03)

| Option | Description | Selected |
|--------|-------------|----------|
| Auto-inject all (Recommended) | Register InstructionsSource alongside other sources in engine.go | ✓ |
| Config-gated | Gate behind FeaturesConfig.InjectInstructions flag | |
| Phase-selective | Register only for specific phases | |

**User's choice:** Auto-inject all (Recommended)
**Notes:** InstructionsSource is already implemented. Just needs registration at engine.go:665-669.

---

## Metrics Wiring (W04-W06)

| Option | Description | Selected |
|--------|-------------|----------|
| Wire all three (Recommended) | RecordLLMInteractionWithPrompt + RecordHealDuration + RecordHealLoop | ✓ |
| LLM recording only | Only RecordLLMInteractionWithPrompt | |
| All three, gated by config | Wire all three but only when MetricsEnabled is true | |

**User's choice:** Wire all three (Recommended)
**Notes:** All three methods are implemented. They need callers in the engine and heal paths.

---

## Streaming (W07)

| Option | Description | Selected |
|--------|-------------|----------|
| Wire into REPL (Recommended) | Handle StreamChunkMsg in app_update.go Update() | ✓ |
| Buffered rendering | Buffer chunks and render complete responses | |
| Investigate first | Check if streaming works via other means | |

**User's choice:** Wire into REPL (Recommended)
**Notes:** StreamChunkMsg is emitted but never handled. The TUI silently drops streaming responses.

---

## Config Merge (W01)

| Option | Description | Selected |
|--------|-------------|----------|
| Full merge in one pass (Recommended) | Add all ~30 missing fields across 6 sections | ✓ |
| Selective merge | Only merge fields with production consumers | |
| Reflection-based auto-merge | Use reflection to auto-merge all fields | |

**User's choice:** Full merge in one pass (Recommended)
**Notes:** 6 sections, ~30 fields. Match the existing merge pattern. Users setting these fields in project TOML currently see no effect.

---

## Zen Parity (W26-W27)

| Option | Description | Selected |
|--------|-------------|----------|
| Full parity (Recommended) | Add retry loop and credit detection matching OpenRouter/NVIDIA | ✓ |
| Keep Zen simple | No retry, no credit detection. Document the difference. | |
| Retry only | Add retry but not credit detection | |

**User's choice:** Full parity (Recommended)
**Notes:** Zen should behave consistently with other providers. Retry with exponential backoff and HandleChatHTTPErrorWithCredits.

---

## Dead Code Cleanup

| Option | Description | Selected |
|--------|-------------|----------|
| Delete all dead (Recommended) | Delete all confirmed dead code (~40 items) | ✓ |
| Delete high-confidence only | Only delete high-confidence items | |
| Packages only | Only delete packages/files, not individual functions | |

**User's choice:** Delete all dead (Recommended)
**Notes:** Despite "wire in everything" for producers with consumers, the remaining ~40 items with zero references are confirmed dead and should be removed.

---

## Narrative Events (W18-W19)

| Option | Description | Selected |
|--------|-------------|----------|
| Add WorkflowEvent methods (Recommended) | Add EventType()/EventData() to AgentSwitchMsg and DecisionsSnapshotMsg | ✓ |
| Leave as-is | TUI handles them directly, narrative doesn't need them | |
| Generic pass-through | Add minimal EventType returning 'other' | |

**User's choice:** Add WorkflowEvent methods (Recommended)
**Notes:** Narrative engine should classify these events for consistency. Full implementation, not pass-through.

---

## Exec Constants (W23)

| Option | Description | Selected |
|--------|-------------|----------|
| Remove all dead (Recommended) | Remove all ~23 dead constants, keep only BashKillGracePeriod and BashWaitTimeout | ✓ |
| Keep redeclared ones | Keep constants redeclared to avoid circular imports | |
| Case by case | Check each constant individually | |

**User's choice:** Remove all dead (Recommended)
**Notes:** Only 2 of ~25 constants are actually used. The rest are dead redeclarations.

---

## IP Filter Dedup (W24)

| Option | Description | Selected |
|--------|-------------|----------|
| Consolidate to ip_filter.go (Recommended) | Remove duplicate from websearch.go, use canonical IsReservedIP | ✓ |
| Keep both | websearch has different IP ranges | |

**User's choice:** Consolidate to ip_filter.go (Recommended)
**Notes:** The canonical IsReservedIP is exported and used by network/httpcheck.go. websearch's duplicate risks divergence.

---

## Agent's Discretion

- Test helper design (integration test structure, table-driven vs individual)
- Whether a fix needs additional related tests beyond the mandatory regression test
- Exact merge field order in config_merge.go
- Whether FallbackPriority validation is a warning or hard error
- Order of removal within each severity batch

## Deferred Ideas

None — discussion stayed within phase scope.
