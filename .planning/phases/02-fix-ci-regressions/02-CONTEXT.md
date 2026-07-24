# Phase 2: Fix CI Test Regressions - Context

**Gathered:** 2026-07-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Fix all 59 test failures across 6 root causes introduced during Phase 01 bug-fix batches. Restore CI to green with `go test -race ./...` passing clean. Each fix must have a test that fails on current code and passes after the fix.

</domain>

<decisions>
## Implementation Decisions

### Fix Strategy — Workflow Transitions (Root Cause C, 19 tests)
- **D-01:** Fix the production code, not the tests. Add a `RunPhaseForTest` or similar bypass method that allows direct phase jumps for single-phase test usage, while keeping `Transition()` enforcement for normal workflow paths. This preserves both test independence and runtime safety.
- **D-02:** The bypass should be clearly named to indicate test-only usage (e.g., `RunPhaseDirect`, `RunPhaseUnchecked`, or a test-only option on `RunPhase`). Avoid making it available in production code paths.

### Fix Strategy — Bash Security (Root Cause E, 23 tests)
- **D-03:** Restore expanded patterns from git history (commit `f35077bd`) AND upgrade to proper regex matching using `regexp.Compile`. The old `strings.Contains` with regex-syntax strings was always broken for obfuscation detection.
- **D-04:** Fix `containsVariableExpansion` to use `regexp.MatchString` with the `$[A-Za-z_]` pattern instead of `strings.Contains` with the literal string.
- **D-05:** Implement exact-prefix matching for custom blocklists (not substring matching via `strings.Contains`). The test name `partial_match_should_not_block` explicitly requires this.
- **D-06:** For `ChainingDetection` — only block `$()` when the inner command itself is dangerous, not indiscriminately. This requires parsing the inner command and running it through the same security check.
- **D-07:** Port the `dangerousCommandPatterns` and `dangerousObfuscationPatterns` lists from `f35077bd` into `internal/tools/exec/bash.go`. The full lists include: `shred`, `wipefs`, `nc -l`, `ncat -l`, `socat`, `/dev/tcp`, `curl|sh`, `wget|bash`, `curl|bash`, `rm -rf *`, `rm -rf ~`, and more.

### Lint Fix (execute.go:571)
- **D-08:** Fix the ineffectual assignment at `execute.go:571`. The `messages = e.proactiveCompactCheck(messages)` is assigned but `messages` is re-declared with `:=` at the top of each heal-loop iteration, so the compaction result is never consumed. Either remove the assignment or restructure the loop to use the compacted messages.

### Config Merge (Root Cause A, 6 tests)
- **D-09:** Restore the non-zero fallback in `intField()` and `float65Field()` in `merge.go`. The fix: `if m.hasKey(key) || *overlay != 0`. This matches how `boolField` already works and restores behavior that all tests rely on.

### Session Label (Root Cause B, 3 tests)
- **D-10:** Add `Label string \`json:"label,omitempty"\`` to the `sessionMetadata` struct in `manager.go` and add `Label: session.Label,` to the struct literal in `saveSessionAtomic`. This is a straightforward missing-field fix.

### TestIsCI (Root Cause D, 1 test)
- **D-11:** Remove `t.Parallel()` from the subtests in `ci_test.go`, or use `t.Setenv()` which auto-restores env vars. The parallel subtests mutate shared process-global environment variables without synchronization.

### AskUserQuestion Timeout (Root Cause F, 1 test)
- **D-12:** Fix the assertion in `extra_test.go:3616` to check `result.Error != ""` instead of `err != nil`. The `Execute` method returns errors in the `ToolResult.Error` field, not as a Go error return value.

### Commit Strategy
- **D-13:** One commit per root cause (6 atomic commits), matching Phase 1's D-04 convention. Order: A (config merge) -> B (session label) -> C (workflow transitions + lint) -> D (TestIsCI) -> E (bash security) -> F (timeout). Config merge and session label are runtime bugs that should be fixed first.

### CI Post-Checkout
- **D-14:** Defer the post-checkout `git exit 128` issue. It is a separate symptom (likely `checkout@v7` + shallow clone interaction) unrelated to the 59 test failures. Address in a CI-chore phase.

### Agent's Discretion
- Exact function/method naming for the RunPhase bypass (D-02)
- Whether the bash security regex upgrade needs additional test coverage beyond existing tests
- Whether to add a regression test for the `sessionMetadata` Label field beyond the 3 existing tests
- Whether `TestCheckDangerousCommand_LongCommand` test expectation is realistic (echo hello; x1000 is not actually dangerous)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Test Failures Report
- `TEST_FAILURES.md` — Full inventory of 59 failures, root causes, evidence, and recommended fixes

### Codebase Maps
- `.planning/codebase/TESTING.md` — Test framework, patterns, mock conventions, race test patterns
- `.planning/codebase/CONVENTIONS.md` — Go coding conventions, error handling, concurrency rules
- `.planning/codebase/STRUCTURE.md` — Package layout, key file locations, where to add new code

### Project Config
- `AGENTS.md` — Build commands, code style, conventional commits, gotchas
- `Makefile` — Build/test/lint targets (make check, make test, make lint, make test-specific)

### Prior Phase Context
- `.planning/phases/01-audit-fixes/01-CONTEXT.md` — Phase 1 decisions (D-02: class coverage tests, D-04: one commit per fix, D-08: strict literal)

### Key Source Files (by root cause)
- `internal/core/config/merge.go` — Config merge logic (Root Cause A)
- `internal/engine/session/manager.go` — Session metadata struct (Root Cause B)
- `internal/engine/workflow/engine.go` — RunPhase and Transition (Root Cause C)
- `internal/engine/workflow/state_machine.go` — Transition graph (Root Cause C)
- `internal/engine/workflow/execute.go` — Ineffectual assignment (Root Cause C lint)
- `internal/testutil/ci/ci_test.go` — IsCI test (Root Cause D)
- `internal/tools/exec/bash.go` — Bash security patterns (Root Cause E)
- `internal/tools/extra_test.go` — AskUserQuestion timeout (Root Cause F)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `tests/testutil/mocks/` — MockProvider, MockTool with configurable responses
- `tests/testutil/builders/` — NewTestDispatcher, NewTestDispatcherWithConfig with auto-cleanup
- `tests/testutil/envtest.go` — RequireAPIKey, LoadTestDotEnv helpers
- Existing `boolField` pattern in `merge.go` line 33: `m.hasKey(key) || *overlay` — template for intField/floatField fix

### Established Patterns
- Table-driven tests with `t.Run` subtests and `t.Parallel()`
- `t.TempDir()` for filesystem isolation, `t.Cleanup()` for teardown
- `t.Setenv()` for environment overrides (preferred over os.Setenv in tests)
- One commit per fix for clean bisect (Phase 1 D-04)

### Integration Points
- `internal/core/config/merge.go` — `intField()`, `float65Field()` methods (Root Cause A)
- `internal/engine/session/manager.go:336-349` — `sessionMetadata` struct (Root Cause B)
- `internal/engine/workflow/engine.go:880-883` — `RunPhase` transition enforcement (Root Cause C)
- `internal/engine/workflow/execute.go:571` — Ineffectual assignment (Root Cause C lint)
- `internal/tools/exec/bash.go:344-383` — `dangerousCommandPatterns`, `dangerousObfuscationPatterns` (Root Cause E)
- `internal/tools/exec/bash.go:431-444` — `containsVariableExpansion` (Root Cause E)
- `internal/testutil/ci/ci_test.go:68-76` — Parallel subtests with env mutation (Root Cause D)
- `internal/tools/extra_test.go:3616` — Timeout assertion (Root Cause F)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — fixes are driven by TEST_FAILURES.md evidence. Each root cause has a clear, documented fix path.

</specifics>

<deferred>
## Deferred Ideas

- Post-checkout `git exit 128` — CI config issue, not a code bug. Defer to CI-chore phase.

</deferred>

---

*Phase: 2-Fix CI Test Regressions*
*Context gathered: 2026-07-24*
