# Phase 1: Audit Bug Fixes - Context

**Gathered:** 2026-07-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Resolve all 30 confirmed bugs (B01-B30) from the logical bug audit (BUGS.md), organized into 4 severity-based batches. Each fix must have a test that fails on current code and passes after the fix. All CI checks must be clean after each batch.

</domain>

<decisions>
## Implementation Decisions

### Fix Ordering
- **D-01:** Fix within each batch in the listed order (B01, B02, B03...). Matches audit priority and simplifies tracking.

### Test Strategy
- **D-02:** Class coverage tests — tests cover the bug class, not just the individual bug. E.g., all race conditions in a file get a shared stress test. Catches similar bugs.

### Investigation Approach
- **D-03:** Investigate B02 and B06 before starting any Batch 1 fixes. Block Batch 1 on investigation results.

### Commit Granularity
- **D-04:** One commit per fix (30 atomic commits). Cleanest bisect, most granular review.

### Race Condition Test Infrastructure
- **D-05:** Use existing testutil from `tests/testutil/` (mocks, builders, env helpers). Consistent with codebase patterns.

### B02/B25 Dependency
- **D-06:** Fix B02 first, then verify if B25 (duplicate history) is resolved before writing a separate fix. B02 routing RunPhase through Transition() may eliminate the duplicate entry.

### B18 Approach
- **D-07:** Follow the spec exactly — use `toml.MetaData.IsDefined()` from BurntSushi/toml for B18, not more zero-value special-casing.

### Refactor Strictness
- **D-08:** Strict literal — only change exactly what's described in each bug. No adjacent cleanups, no opportunistic refactors.

### Agent's Discretion
- Test helper design (exact function signatures, table-driven vs individual)
- Whether a fix needs additional related tests beyond the mandatory regression test
- Exact lock type selection (sync.Mutex vs sync.RWMutex) per fix context

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Bug Audit
- `BUGS.md` — Full audit report with all 30 bug descriptions, file locations, triggers, and impact analysis

### Codebase Maps
- `.planning/codebase/TESTING.md` — Test framework, patterns, mock conventions, race test patterns
- `.planning/codebase/CONVENTIONS.md` — Go coding conventions, error handling, concurrency rules
- `.planning/codebase/STRUCTURE.md` — Package layout, key file locations, where to add new code

### Project Config
- `AGENTS.md` — Build commands, code style, conventional commits, gotchas
- `Makefile` — Build/test/lint targets (make check, make test, make lint, make test-specific)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `tests/testutil/mocks/` — MockProvider, MockTool with configurable responses, call counting, health injection
- `tests/testutil/builders/` — NewTestDispatcher, NewTestDispatcherWithConfig with auto-cleanup
- `tests/testutil/envtest.go` — RequireAPIKey, LoadTestDotEnv helpers
- `internal/tools/dispatcher_test.go` — 991 lines of dispatcher test patterns including race stress tests
- `internal/engine/workflow/engine_test.go` — 1028 lines of workflow engine test patterns

### Established Patterns
- Table-driven tests with `t.Run` subtests and `t.Parallel()`
- `t.TempDir()` for filesystem isolation, `t.Cleanup()` for teardown
- `t.Setenv()` for environment overrides
- Mock interfaces: Provider (`LLMProvider`), Tool (`types.Tool`), Keychain
- Race stress tests: 50-100 goroutines x 1000 iterations for concurrent data structures

### Integration Points
- `internal/engine/workflow/engine.go` — RunPhase, Transition, Shutdown, checkpointing (B01, B02, B07, B12, B13, B25)
- `internal/tools/permissions.go` — Rule evaluation, matchAnyParamValue, doublestar matching (B03, B10, B11, B27)
- `internal/engine/session/manager.go` — LoadWorkflowState, saveSessionAtomic (B04, B05)
- `internal/engine/session/checkpoint.go` — Checkpoint save/restore (B20)
- `internal/types/fileutil.go` — FileLock/flock (B06)
- `internal/integrations/provider/` — Fallback, streaming, handler_stream (B08, B09, B22)
- `internal/core/config/` — merge.go, config_validate.go (B18, B19)
- `internal/engine/tokens/estimator.go` — Token estimation (B16, B17)
- `internal/integrations/keychain/keychain.go` — Keychain blacklist (B21)
- `internal/ui/tui/components/repl.go` — resizePending race (B23)
- `internal/tools/search/dns_cache.go` — Type assertion (B28)
- `internal/integrations/provider/capabilities.go` — Default model caching (B29)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches per audit report recommendations.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 1-Audit Bug Fixes*
*Context gathered: 2026-07-23*
