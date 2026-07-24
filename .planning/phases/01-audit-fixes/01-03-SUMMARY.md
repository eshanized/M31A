# 01-03 Plan Summary — Permission, Token, Config, Keychain, and Provider Fixes

## Plan Reference
- **Plan:** [01-03-PLAN.md](./01-03-PLAN.md)
- **Phase:** 01 — Audit Bug Fixes
- **Wave:** 3

## Bugs Fixed

| Bug | Description | Severity | Approach |
|-----|-------------|----------|----------|
| B10 | Permission response channel dropped after timeout cleanup | Medium | 200ms grace period on per-request channel cleanup |
| B11 | `matchAnyParamValue` hardcoded to 4 param keys | Medium | Iterate all string-typed params dynamically |
| B14 | Transition checkpoint missing Goal and PlanVersion | Medium | Added fields to `CoordinateTransition` and engine state |
| B15 | `SetPhase` doesn't reset `discussPlanCycles` | Medium | Added `sm.discussPlanCycles = 0` in `SetPhase` |
| B16 | Token estimation uses `int()` truncation (under-counts) | Medium | Replaced with `math.Ceil` across 8 provider cases |
| B17 | Truncation uses per-message `Estimate` (no overhead) | Medium | Added perMessageOverhead + tool call estimates |
| B18 | Config merge zero-value int/float blocked by `!= 0` check | Medium | Changed to `hasKey()` for defined-key override |
| B19 | Variable substitution missing Git, Compaction, Prompt fields | Low | Expanded `applyVarSubstitution` to 13 additional fields |
| B21 | Keychain blacklist is permanent after transient failure | Medium | TTL-based recovery (5-minute window) |
| B22 | `FindFallbackProvider` ignores "degraded" health status | Low | Added degraded fallback handling similar to slow |

## Commits

1. `db3a973e` — B10/B11/B14/B15: permission/correctness bugs
2. `a28a011a` — B16/B17/B18/B19: token estimation and config bugs
3. `04b39fca` — B21/B22: keychain TTL and provider fallback

## Key Design Decisions

- **B10 grace period**: 200ms delayed cleanup via goroutine keeps the per-request channel alive long enough for late user responses. This is simpler than adding drain logic to `d.responseCh`.
- **B18 `hasKey()` pattern**: Aligns int/float with the existing bool field behavior. Zero-value override only works when the TOML key is explicitly present, matching `toml.MetaData.IsDefined()` semantics without importing BurntSushi/toml directly.
- **B21 TTL (5 min)**: Balances recovery against repeated failure overhead. Successful operations immediately clear the blacklist.
- **B22 degraded < slow**: Degraded providers are preferred over offline but deprioritized vs slow, maintaining the existing priority ordering.

## Test Results

- All new tests pass with `-race` flag
- Pre-existing test failures confirmed unchanged
- Build clean with `CGO_ENABLED=0`

## Verification

- [x] Build passes (`go build ./...`)
- [x] Race tests pass
- [x] Existing tests unaffected
- [x] All 10 bugs in plan fixed and committed
