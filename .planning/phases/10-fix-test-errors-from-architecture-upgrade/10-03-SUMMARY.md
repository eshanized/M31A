---
phase: 10-fix-test-errors-from-architecture-upgrade
plan: 03
subsystem: testing
tags: [testutil, fixtures, go-embed, integration-tests, e2e]

# Dependency graph
requires:
  - phase: 10-fix-test-errors-from-architecture-upgrade/10-01
    provides: "Fixed build, t.TempDir() migration"
  - phase: 10-fix-test-errors-from-architecture-upgrade/10-02
    provides: "Centralized mocks/ and builders/ packages"
provides:
  - "testutil/fixtures/ with unit/, integration/, e2e/ subdirectories using go:embed"
  - "testutil/integration/ with tool_integration_test.go (moved from tools/)"
  - "testutil/e2e/ canonical location for e2e tests"
affects: [testing, tools]

# Tech tracking
tech-stack:
  added: []
  patterns: [go-embed-fixtures, external-test-packages, centralized-integration-tests]

key-files:
  created:
    - internal/testutil/fixtures/unit/configs.go
    - internal/testutil/fixtures/unit/sessions.go
    - internal/testutil/fixtures/unit/minimal.toml
    - internal/testutil/fixtures/unit/full.toml
    - internal/testutil/fixtures/unit/session_template.json
    - internal/testutil/fixtures/integration/workflow.go
    - internal/testutil/fixtures/integration/plan_sample.md
    - internal/testutil/fixtures/integration/tasks.json
    - internal/testutil/fixtures/e2e/sample.go
    - internal/testutil/fixtures/e2e/prompt_simple.txt
    - internal/testutil/integration/tool_integration_test.go
    - internal/testutil/integration/README.md
    - internal/testutil/e2e/README.md
  modified:
    - internal/tools/edit_integration_test.go (deleted — moved)

key-decisions:
  - "E2E test stays at project root: go build ./cmd/m31a requires working directory to be project root"
  - "Provider integration tests stay colocated: need unexported constructors (New(apiKey, Options{}))"
  - "Config integration test stays colocated: needs unexported newMockKeychain()"
  - "Workflow integration test stays colocated: uses unexported multiTurnMockProvider type"
  - "Edit integration test successfully moved to testutil/integration/ with external test package"

patterns-established:
  - "go:embed fixtures: Go source files with //go:embed directives alongside data files"
  - "Fixture subdirectories per test type: unit/, integration/, e2e/"
  - "External test packages for cross-package integration testing (per D-08)"

requirements-completed: [NFR-4]

coverage:
  - id: D1
    description: "go:embed type-safe fixtures for unit, integration, and e2e tests"
    requirement: NFR-4
    verification:
      - kind: unit
        ref: "go build ./internal/testutil/fixtures/... (compilation check)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Edit integration test moved to testutil/integration/ with external test package"
    requirement: NFR-4
    verification:
      - kind: unit
        ref: "go test ./internal/testutil/integration/... -count=1"
        status: pass
    human_judgment: false
  - id: D3
    description: "Complete testutil directory structure with mocks/, builders/, fixtures/, e2e/, integration/"
    requirement: NFR-4
    verification:
      - kind: unit
        ref: "ls -la internal/testutil/ (verifies all subdirectories exist)"
        status: pass
    human_judgment: false

# Metrics
duration: 6min
completed: 2026-07-18
status: complete
---

# Phase 10 Plan 03: Test Infrastructure Centralization Summary

**Created go:embed fixtures in testutil/fixtures/ and centralized the edit integration test into testutil/integration/ with external test package**

## Performance

- **Duration:** 6 min
- **Started:** 2026-07-18T04:31:15Z
- **Completed:** 2026-07-18T04:37:19Z
- **Tasks:** 2
- **Files modified:** 13 (12 created, 1 deleted)

## Accomplishments
- Created testutil/fixtures/ with unit/, integration/, e2e/ subdirectories using go:embed
- Moved edit_integration_test.go from internal/tools/ to testutil/integration/ as external test package
- Created canonical testutil/e2e/ directory for future e2e test centralization
- Established go:embed fixture pattern for type-safe, compile-time embedded test data

## Task Commits

Each task was committed atomically:

1. **Task 1: Create testutil/fixtures/ with go:embed type-safe fixtures** - `d1f47ab6` (feat)
2. **Task 2: Move edit integration test to testutil/integration/** - `04e760f6` (feat)

## Files Created/Modified
- `internal/testutil/fixtures/unit/configs.go` - Embedded TOML config fixtures (MinimalConfig, FullConfig)
- `internal/testutil/fixtures/unit/sessions.go` - Embedded session JSON template
- `internal/testutil/fixtures/unit/minimal.toml` - Minimal TOML config data
- `internal/testutil/fixtures/unit/full.toml` - Full TOML config data
- `internal/testutil/fixtures/unit/session_template.json` - Session JSON template data
- `internal/testutil/fixtures/integration/workflow.go` - Embedded workflow plan and task fixtures
- `internal/testutil/fixtures/integration/plan_sample.md` - Sample plan markdown data
- `internal/testutil/fixtures/integration/tasks.json` - Sample task list data
- `internal/testutil/fixtures/e2e/sample.go` - Embedded e2e prompt fixture
- `internal/testutil/fixtures/e2e/prompt_simple.txt` - Simple prompt data
- `internal/testutil/integration/tool_integration_test.go` - Edit tool integration tests (moved from tools/)
- `internal/testutil/integration/README.md` - Documents integration test centralization
- `internal/testutil/e2e/README.md` - Documents e2e canonical location
- `internal/tools/edit_integration_test.go` - Deleted (moved)

## Decisions Made
- E2E test stays at project root: `go build ./cmd/m31a` requires working directory at project root; moving would break binary path resolution
- Provider integration tests stay colocated: need unexported constructors from their packages
- Config integration test stays colocated: needs unexported `newMockKeychain()`
- Workflow integration test stays colocated: uses unexported `multiTurnMockProvider` type
- Edit integration test successfully moved: only used exported APIs (`tools.NewEdit`, `types.ToolInput`)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] E2E test not moved to testutil/e2e/**
- **Found during:** Task 2 (Move tests)
- **Issue:** `e2e_test.go` uses `go build ./cmd/m31a` which requires working directory at project root. Moving to `internal/testutil/e2e/` would break binary path resolution.
- **Fix:** Kept e2e_test.go at project root per plan's fallback guidance. Created `internal/testutil/e2e/README.md` as canonical location.
- **Files modified:** internal/testutil/e2e/README.md (created)
- **Verification:** `go build ./...` passes, e2e_test.go unchanged
- **Committed in:** 04e760f6 (Task 2 commit)

**2. [Rule 3 - Blocking] Provider/config/workflow integration tests not moved**
- **Found during:** Task 2 (Move tests)
- **Issue:** Provider tests use unexported constructors. Config test uses unexported `newMockKeychain()`. Workflow test uses unexported `multiTurnMockProvider`. Cannot move to external test package without exporting these symbols.
- **Fix:** Left these tests in their source packages. Documented rationale in `testutil/integration/README.md`.
- **Files modified:** internal/testutil/integration/README.md (created)
- **Verification:** `go build ./...` passes
- **Committed in:** 04e760f6 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (2 blocking — path resolution and unexported symbols)
**Impact on plan:** Deviations follow plan's fallback guidance. Core deliverables achieved: fixtures created, edit integration test centralized, directory structure complete.

## Issues Encountered
- Pre-existing `go vet` errors in `edit_benchmark_test.go`, `commands_analysis_test.go`, `emitter_stress_test.go` — out of scope (not introduced by this plan)
- `make check` fails on pre-existing vet errors; these were documented in 10-02 SUMMARY as out of scope

## Known Stubs
None — all fixtures are functional, all moved tests pass.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Complete testutil directory structure: mocks/, builders/, fixtures/, e2e/, integration/
- E2E tests remain at project root (correct location for binary compilation)
- Integration tests consolidated where possible, colocated where needed
- Pre-existing vet errors need separate investigation

---
*Phase: 10-fix-test-errors-from-architecture-upgrade*
*Completed: 2026-07-18*

## Self-Check: PASSED
