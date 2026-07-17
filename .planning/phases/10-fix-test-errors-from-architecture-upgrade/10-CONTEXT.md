# Phase 10: Fix test errors from architecture upgrade - Context

**Gathered:** 2026-07-17
**Status:** Ready for planning

<domain>
## Phase Boundary

Fix test failures introduced by Phase 9 restructuring and centralize test helpers/fixtures in `internal/testutil/`. This phase reorganizes test infrastructure — it does not add new test coverage or change test behavior.

</domain>

<decisions>
## Implementation Decisions

### Testutil Organization
- **D-01:** Move ALL test infrastructure to `internal/testutil/` — shared mocks, setup helpers, test data builders, fixtures
- **D-02:** Organize testutil/ with subdirectories: `testutil/mocks/`, `testutil/builders/`, `testutil/fixtures/`
- **D-03:** Use `package testutil` as the umbrella package name
- **D-04:** Standardize naming with New* constructors: `NewMockProvider()`, `NewTestDispatcher()`, `NewSetupTestEngine()`

### Integration Test Location
- **D-05:** Centralize all `*_integration_test.go` files in `internal/testutil/integration/`
- **D-06:** Use descriptive file names: `provider_integration_test.go`, `workflow_integration_test.go`
- **D-07:** Move `e2e_test.go` from project root to `internal/testutil/e2e/`
- **D-08:** Use both internal (`package testutil`) and external (`package testutil_test`) test packages as needed

### Test Fixture Files
- **D-09:** Store fixtures in `internal/testutil/fixtures/` with subdirectories per test type: `unit/`, `integration/`, `e2e/`
- **D-10:** Use `go:embed` for fixture loading — compile-time embedding, no runtime file reads
- **D-11:** Use Go source files (not JSON) for fixtures — type-safe, compile-time checked

### Import Path Migration
- **D-12:** Use `goimports` automation to fix import paths after moves
- **D-13:** Batch verification — run `make check` after all moves complete
- **D-14:** Git commits after each major move for easy rollback
- **D-15:** Group commits by domain: testutil moves, integration moves, fixture moves

### the agent's Discretion
- Exact file naming within new sub-packages
- Import ordering in reorganized files
- Whether to do migration in one commit or incremental atomic commits
- Exact placement of borderline files (e.g., where does `mockProvider` belong after restructuring?)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Testing Patterns
- `.planning/codebase/TESTING.md` — test framework, mocking patterns, coverage requirements, test types
- `.planning/codebase/CONVENTIONS.md` — code style, naming conventions, import organization

### Project Structure
- `.planning/codebase/STRUCTURE.md` — full directory layout, where to add new code
- `.planning/codebase/ARCHITECTURE.md` — system design, layers, data flows

### Requirements
- `.planning/REQUIREMENTS.md` — NFR-4 (Maintainability): 75% coverage, gofmt-clean, golangci-lint
- `AGENTS.md` — build/test/lint commands, architecture rules

### Existing Test Infrastructure
- `internal/testutil/envtest.go` — existing shared test environment setup
- `internal/provider/mock/mock.go` — existing provider mock pattern
- `internal/tools/dispatcher_test.go` — existing mockTool pattern
- `internal/workflow/engine_test.go` — existing mockProvider and setupTestEngine patterns

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **Go test conventions**: `*_test.go` colocated with source, `t.TempDir()` for isolation, `t.Parallel()` for independent tests
- **Mock patterns**: Hand-written mocks implementing interfaces (no external mock library)
- **Test helpers**: `setup*` or `test*` prefix convention for helper functions
- **Coverage gates**: `internal/workflow/coverage_gates_test.go` enforces plan quality checks

### Established Patterns
- **Table-driven tests**: Preferred for parameterized cases with `t.Run()` subtests
- **Race tests**: `*_race_test.go` files for concurrent access patterns
- **E2E tests**: `e2e_test.go` at project root tests actual binary with real API calls
- **Integration tests**: `*_integration_test.go` files test multiple packages working together

### Integration Points
- **`cmd/m31a/main.go`**: Entry point — tests compile and run actual binary
- **`internal/workflow/engine.go`**: Core engine — extensive test coverage with mocks
- **`internal/tools/dispatcher.go`**: Tool dispatcher — permission flow tests
- **`internal/provider/`**: Provider layer — mock provider tests

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard Go test organization approaches. The user wants cleaner test infrastructure, not a rewrite.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 10-Fix test errors from architecture upgrade*
*Context gathered: 2026-07-17*
