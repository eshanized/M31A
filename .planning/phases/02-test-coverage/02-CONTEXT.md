# Phase 02: Test Coverage Remediation - Context

**Gathered:** 2026-08-02
**Status:** Ready for planning

<domain>
## Phase Boundary

Achieve 75% test coverage across all packages, with 90% target for critical path packages (`pkg/taskrunner`, `pkg/bisect`, `pkg/rollback`).

This phase addresses:
1. Zero-coverage packages (0% → 75%): `internal/tools/ai`, `internal/tools/exec`, `internal/tools/fileops`, `internal/tools/network`, `internal/integrations/provider/nvidia`
2. Critical coverage gaps (<25%): `internal/tools/search`, `internal/core/types`, `cmd/m31a`, `internal/integrations/keychain`
3. Moderate gaps (25-60%): TUI packages, `internal/tools/todo`, `internal/engine/rollback`
4. Near-target packages (60-75%): `internal/engine/decision`, `internal/tools`, `internal/engine/workflow`

</domain>

<decisions>
## Implementation Decisions

### Priority Order
- **D-01:** Start with zero-coverage packages first — they have no existing tests and need the most work. Order: `exec` → `fileops` → `network` → `ai` → `nvidia` (by dependency and complexity) — **Reversibility:** reversible — test files only
- **D-02:** Then address critical coverage gaps (<25%) — `search` (2%), `types` (15.6%), `cmd/m31a` (21.6%), `keychain` (25.1%) — **Reversibility:** reversible — test files only
- **D-03:** Finally fill moderate gaps to reach 75% threshold — TUI packages, todo, rollback, and near-target packages — **Reversibility:** reversible — test files only

### Testing Strategy
- **D-04:** Use table-driven tests as the primary pattern — consistent with existing test style in the codebase — **Reversibility:** reversible — test files only
- **D-05:** Mock external dependencies (API calls, filesystem, OS) using interfaces and test doubles — **Reversibility:** reversible — test files only
- **D-06:** Focus on unit tests over integration tests — faster feedback, easier to isolate failures — **Reversibility:** reversible — test files only

### Coverage Targets
- **D-07:** All packages must reach 75% line coverage minimum — **Reversibility:** reversible — test files only
- **D-08:** Critical path packages (`internal/engine/rollback`, `internal/engine/workflow`, `internal/engine/compaction`) must reach 90% — **Reversibility:** reversible — test files only

### Test File Organization
- **D-09:** Create `*_test.go` files alongside source files — standard Go convention — **Reversibility:** reversible — test files only
- **D-10:** Use `testdata/` directories for test fixtures when needed — **Reversibility:** reversible — test files only

### the agent's Discretion
- Exact test case selection and coverage prioritization within each package
- Mock implementations and test doubles
- Test helper functions and utilities
- Coverage reporting and verification approach

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Coverage Report
- `AUDIT_REPORT.md` §Coverage Gaps — Source of truth for coverage targets

### Package Source Code (Zero Coverage)
- `internal/tools/ai/` — AI tool implementations (0% coverage)
- `internal/tools/exec/` — Execution tools (0% coverage)
- `internal/tools/fileops/` — File operation tools (0% coverage)
- `internal/tools/network/` — Network tools (0% coverage)
- `internal/integrations/provider/nvidia/` — Nvidia provider (0% coverage)

### Package Source Code (Critical Gaps)
- `internal/tools/search/` — Search tools (2% coverage)
- `internal/core/types/` — Core types (15.6% coverage)
- `cmd/m31a/` — Main entry point (21.6% coverage)
- `internal/integrations/keychain/` — Keychain integration (25.1% coverage)

### Existing Test Examples
- `internal/tools/search/*_test.go` — Existing minimal tests to extend
- `internal/engine/workflow/*_test.go` — Good test patterns to follow
- `internal/engine/rollback/*_test.go` — Critical path test examples

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- Table-driven test patterns in `internal/engine/workflow/engine_extra_test.go`
- Mock implementations in `internal/tools/search/*_test.go`
- Test helper functions in `internal/engine/rollback/rollback_test.go`

### Established Patterns
- Tests use `testing.T` with subtests (`t.Run`)
- Table-driven tests with `struct{ name string; ... }` pattern
- Mock interfaces for external dependencies
- `testdata/` directories for fixtures

### Integration Points
- Go test framework (`testing` package)
- Coverage tool: `go test -coverprofile`
- Coverage verification: `make test` (includes coverage)

</code_context>

<specifics>
## Specific Ideas

### Zero-Coverage Package Testing Priorities
- `exec/`: Test command execution, timeout handling, sandbox behavior
- `fileops/`: Test file read/write/modify operations, atomic writes
- `network/`: Test HTTP client, SSRF protection, DNS resolution
- `ai/`: Test AI tool invocations, response parsing
- `nvidia/`: Test provider API integration, model discovery

### Critical Path Coverage
- `rollback/`: Test rollback state machine, commit creation, diff filtering
- `workflow/`: Test phase transitions, state machine, error handling

</specifics>

<deferred>
## Deferred Ideas

- Integration test suite (could be a separate phase)
- Performance/load testing for tools
- Property-based testing (advanced pattern)

</deferred>

---

*Phase: 02-Test Coverage Remediation*
*Context gathered: 2026-08-02*
