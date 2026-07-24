# Phase 2: Fix CI Test Regressions - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-07-24
**Phase:** 02-fix-ci-regressions
**Areas discussed:** Fix strategy, Bash security, Lint fix, CI cleanup

---

## Fix Strategy (Root Cause C — 19 workflow engine tests)

| Option | Description | Selected |
|--------|-------------|----------|
| Fix the production code | Add RunPhaseForTest/bypass for direct phase jumps in tests | ✓ |
| Fix the tests | Update all 18 tests to drive through legal transitions | |
| You decide | Let planner choose based on codebase research | |

**User's choice:** Fix the production code (Recommended)
**Notes:** Preserves both test independence and runtime safety. The bypass should be clearly test-only.

---

## Bash Security (Root Cause E — 23 tests)

| Option | Description | Selected |
|--------|-------------|----------|
| Restore + upgrade to regex | Port patterns from f35077bd AND fix regex-as-literal bugs | ✓ |
| Restore patterns only | Port patterns but leave regex-as-literal bugs | |
| You decide | Let planner choose simplest validation path | |

**User's choice:** Restore + upgrade to regex (Recommended)
**Notes:** Fixes both missing patterns and obfuscation detection. The old strings.Contains with regex syntax was always broken.

---

## Lint Fix (execute.go:571)

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, fix it | Fix the ineffectual assignment in this phase | ✓ |
| No, defer | Separate cleanup chore | |

**User's choice:** Yes, fix it (Recommended)
**Notes:** Direct consequence of the same commit (c1e5dbda) that broke workflow tests.

---

## CI Cleanup (post-checkout git exit 128)

| Option | Description | Selected |
|--------|-------------|----------|
| No, defer | Separate CI config issue, not a code bug | ✓ |
| Yes, investigate and fix | Address as part of same CI run failure | |

**User's choice:** No, defer (Recommended)
**Notes:** Likely checkout@v7 + shallow clone interaction. Separate from the 59 test failures.

---

## Agent's Discretion

- Exact naming for RunPhase bypass method (D-02)
- Whether bash security regex upgrade needs additional tests
- Whether sessionMetadata Label needs extra regression tests
- Whether TestCheckDangerousCommand_LongCommand expectation is realistic

---

## Deferred Ideas

- Post-checkout `git exit 128` — CI config issue, defer to CI-chore phase
