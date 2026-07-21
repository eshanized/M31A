# Phase 2: Fix Test Hanging and CI Issues - Context

**Gathered:** 2026-07-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Investigate and resolve test hanging issues and CI pipeline problems to ensure reliable test execution. This includes deep investigation of test hanging root causes, CI pipeline fixes, and test reliability improvements.

</domain>

<decisions>
## Implementation Decisions

### Test Hanging Investigation Approach
- **D-01:** Use verbose output + timeouts to identify hanging tests (run tests with -v flag, add timeouts to individual tests, use -race flag)
- **D-02:** Add 30 second timeout to test commands to fail tests that take too long
- **D-03:** Always use race detector during investigation to catch race conditions that cause hangs
- **D-04:** Focus on tests that use goroutines, channels, sync primitives, or concurrent operations

### CI Pipeline Issues Investigation
- **D-05:** Investigate environment differences (tests pass locally but fail in CI due to environment differences)
- **D-06:** Conduct environment audit (check Go version, dependencies, environment variables, file permissions)
- **D-07:** Focus on tests that use file system, network, or external services
- **D-08:** Add CI detection to tests that use file system or network resources, skip tests that require local resources

### Fix Strategy for Hanging Tests
- **D-09:** Fix individual tests rather than improving test infrastructure
- **D-10:** Add 30 second timeout to tests that use goroutines or channels
- **D-11:** Add cleanup with defer statements to ensure goroutines are stopped and resources released

### CI Pipeline Fixes Approach
- **D-12:** Update both Makefile and GitHub Actions workflow for comprehensive fixes
- **D-13:** Add -timeout 30s to all test commands in Makefile
- **D-14:** Add timeout-minutes: 10 to test jobs in GitHub Actions
- **D-15:** Add CI detection to tests that depend on external resources

### the agent's Discretion
- Agent has flexibility in identifying which specific tests are hanging
- Agent can decide on specific cleanup strategies based on test implementation
- Agent can adjust timeout values if needed during implementation

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Testing Infrastructure
- `Makefile` — Build, test, lint targets with timeout and race flags
- `.golangci.yml` — Linter configuration
- `AGENTS.md` — Build/test/lint commands, architecture rules

### Codebase Analysis
- `.planning/codebase/TESTING.md` — Test framework, structure, patterns, and commands
- `.planning/codebase/CONVENTIONS.md` — Code conventions including test patterns
- `.planning/codebase/STRUCTURE.md` — Directory layout and key file locations

### CI Configuration
- `.github/workflows/` — GitHub Actions CI pipeline configuration

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `internal/tools/dispatcher_test.go` — Example of table-driven tests with parallel execution and test helpers
- `internal/rollback/rollback_test.go` — Example of test setup with temporary directories and git operations
- `internal/bisect/bisect_test.go` — Example of test setup with interface injection for mocking

### Established Patterns
- Table-driven tests are the dominant pattern in the codebase
- `t.Parallel()` used extensively for independent test cases
- `t.TempDir()` for isolated filesystem operations
- `context.Background()` or `context.WithTimeout` for timeouts
- Custom `cleanEnv()` removes API keys for isolation

### Integration Points
- `internal/testutil/` — Test utilities and mocks
- `internal/testutil/integration/` — Integration tests
- `e2e_test.go` — Binary E2E tests (will be moved to `tests/e2e/e2e_test.go` in Phase 1)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 2-Fix Test Hanging and CI Issues*
*Context gathered: 2026-07-21*
