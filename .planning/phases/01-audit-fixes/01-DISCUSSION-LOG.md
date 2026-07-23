# Phase 1: Audit Bug Fixes - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-23
**Phase:** 01-audit-fixes
**Areas discussed:** Fix ordering, Test strategy, Investigation approach, Commit granularity, Race test infrastructure, B02/B25 dependency, B18 approach, Refactor strictness

---

## Fix Ordering

| Option | Description | Selected |
|--------|-------------|----------|
| By listed order | Fix B01, then B02, etc. — matches audit priority, easiest to track | ✓ |
| Group by file/package | Fix all bugs in engine.go together — reduces context switching | |
| Dependency-first | Fix foundational bugs first (B06 before B04, B02 before B25) | |

**User's choice:** By listed order
**Notes:** Matches audit priority and simplifies tracking.

---

## Test Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Minimal regression | One test per fix that fails before, passes after — fast, focused | |
| Class coverage | Tests cover the bug class — catches similar bugs | ✓ |
| Agent discretion | Let agent decide per-fix based on complexity and risk | |

**User's choice:** Class coverage
**Notes:** Tests cover the bug class, not just the individual bug.

---

## Investigation Approach

| Option | Description | Selected |
|--------|-------------|----------|
| Investigate first, then batch | Resolve B02/B06 investigation before starting any Batch 1 fixes | ✓ |
| Parallel investigate+fix | Start other Batch 1 fixes while investigating B02/B06 in background | |
| Investigate inline | Investigate B02/B06 as first step within Batch 1 | |

**User's choice:** Investigate first, then batch
**Notes:** Block Batch 1 on B02/B06 investigation results.

---

## Commit Granularity

| Option | Description | Selected |
|--------|-------------|----------|
| One per fix | 30 atomic commits — cleanest bisect, most granular review | ✓ |
| One per batch | 4 commits — less noise, each batch is a reviewable unit | |
| Agent discretion | Let agent decide based on fix complexity and dependencies | |

**User's choice:** One per fix
**Notes:** 30 atomic commits for clean bisect.

---

## Race Condition Test Infrastructure

| Option | Description | Selected |
|--------|-------------|----------|
| Existing testutil | Use mocks/dispatchers from tests/testutil/ — consistent with codebase | ✓ |
| Minimal new helpers | Create small test helpers only where existing ones don't fit | |
| Agent discretion | Let agent choose per-fix based on what's needed | |

**User's choice:** Existing testutil
**Notes:** Consistent with codebase patterns.

---

## B02/B25 Dependency

| Option | Description | Selected |
|--------|-------------|----------|
| Verify after B02 | Fix B02 first, then check if B25 is resolved | ✓ |
| Fix both independently | Write separate fixes and tests for B02 and B25 regardless | |
| Agent discretion | Let agent decide based on investigation findings | |

**User's choice:** Verify after B02
**Notes:** B02 routing RunPhase through Transition() may eliminate the duplicate entry.

---

## B18 Approach

| Option | Description | Selected |
|--------|-------------|----------|
| Follow the spec | Use toml.MetaData.IsDefined() as specified in BUGS.md | ✓ |
| Agent chooses | Let agent pick the best approach after investigating the code | |

**User's choice:** Follow the spec
**Notes:** Use toml.MetaData.IsDefined() from BurntSushi/toml.

---

## Refactor Strictness

| Option | Description | Selected |
|--------|-------------|----------|
| Strict literal | Only change exactly what's described in each bug | ✓ |
| Adjacent cleanup OK | Minor cleanup of directly related code is fine | |
| Agent discretion | Let agent decide based on what's needed for a correct fix | |

**User's choice:** Strict literal
**Notes:** No opportunistic refactors. Fix exactly what's listed.

---

## Agent's Discretion

- Test helper design (exact function signatures, table-driven vs individual)
- Whether a fix needs additional related tests beyond the mandatory regression test
- Exact lock type selection (sync.Mutex vs sync.RWMutex) per fix context

## Deferred Ideas

None — discussion stayed within phase scope.
