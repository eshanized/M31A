# Phase 3: Fix Remaining CI Issues - Context

**Gathered:** 2026-07-27
**Status:** Ready for planning

<domain>
## Phase Boundary

Fix all remaining lint errors, test failures, and security issues blocking CI from passing. Restore CI to green with `golangci-lint run`, `go test -race ./...`, and CodeQL all clean.

</domain>

<decisions>
## Implementation Decisions

### Lint Fix Scope
- **D-01:** Full scan for deprecated API usage across entire codebase, not just the 3 known os.SEEK_SET warnings
- **D-02:** Direct replacement of os.SEEK_SET with io.SeekStart in fileutil.go
- **D-03:** Single commit for all lint fixes
- **D-04:** Verify with `make lint` after fixes

### Test Initialization Strategy
- **D-05:** Fix TestRegistry_Execute_PhaseAliases by properly initializing session.Manager with all required dependencies
- **D-06:** Use t.TempDir() for filesystem isolation in session manager initialization
- **D-07:** Use t.Cleanup() for automatic cleanup
- **D-08:** Test should pass without error - if initialization fails, test fails

### Test Timeout Handling
- **D-09:** Fix TestAskUserQuestion_ChannelFull by addressing the infinite loop in test logic
- **D-10:** Add proper channel close or context cancellation to prevent infinite loop
- **D-11:** Verify fix by running test in isolation

### Security Investigation
- **D-12:** Run gosec for additional security checks beyond CodeQL
- **D-13:** Scan entire codebase for security issues
- **D-14:** Fix all security issues found
- **D-15:** Single commit for all security fixes

### Agent's Discretion
- Exact gosec configuration and severity thresholds
- Whether to add new golangci-lint linters beyond existing config
- Whether to add regression tests for fixed issues

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Lint Configuration
- `Makefile` — Build/test/lint targets (make check, make test, make lint, make test-specific)
- `.golangci.yml` — golangci-lint configuration

### Test Files
- `internal/ui/tui/commands/commands_all_test.go` — TestRegistry_Execute_PhaseAliases (line 617)
- `internal/tools/extra_test.go` — TestAskUserQuestion_ChannelFull (line 3655)

### Security
- `internal/core/types/fileutil.go` — FileLock implementation with deprecated os.SEEK_SET

### Project Config
- `AGENTS.md` — Build commands, code style, conventional commits, gotchas

### Prior Phase Context
- `.planning/phases/02-fix-ci-regressions/02-CONTEXT.md` — Phase 2 decisions (D-08: RunPhaseDirect bypass, D-03: bash security patterns)
- `.planning/phases/01-audit-fixes/01-CONTEXT.md` — Phase 1 decisions (D-04: one commit per fix, D-08: strict literal)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `tests/testutil/mocks/` — MockProvider, MockTool with configurable responses
- `tests/testutil/builders/` — NewTestDispatcher, NewTestDispatcherWithConfig with auto-cleanup
- `tests/testutil/envtest.go` — RequireAPIKey, LoadTestDotEnv helpers

### Established Patterns
- Table-driven tests with `t.Run` subtests and `t.Parallel()`
- `t.TempDir()` for filesystem isolation, `t.Cleanup()` for teardown
- `t.Setenv()` for environment overrides (preferred over os.Setenv in tests)
- One commit per fix for clean bisect (Phase 1 D-04)

### Integration Points
- `internal/core/types/fileutil.go` — FileLock, AtomicWrite (deprecated os.SEEK_SET)
- `internal/ui/tui/commands/commands_all_test.go` — TestRegistry_Execute_PhaseAliases (nil pointer dereference)
- `internal/tools/extra_test.go` — TestAskUserQuestion_ChannelFull (timeout/infinite loop)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — fixes are driven by CI failure evidence. Each issue has a clear fix path based on test output and lint warnings.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 3-Fix Remaining CI Issues*
*Context gathered: 2026-07-27*
