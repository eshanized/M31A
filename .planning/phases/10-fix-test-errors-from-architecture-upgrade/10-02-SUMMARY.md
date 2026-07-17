---
phase: 10-fix-test-errors-from-architecture-upgrade
plan: 02
subsystem: testing
tags: [mocks, testutil, builders, test-infrastructure, go]

# Dependency graph
requires:
  - phase: 10-fix-test-errors-from-architecture-upgrade/10-01
    provides: "Fixed test compilation errors, DefaultDispatcher API"
provides:
  - "Centralized testutil/mocks/ package with MockProvider, MockKeychain, MockTool, WorkflowDispatcher, SubagentDispatcher"
  - "Centralized testutil/builders/ package with NewTestDispatcher, NewTestDispatcherWithConfig"
  - "All consumer test files migrated to centralized mocks where import cycles allow"
affects: [testing, workflow, tools, provider, keychain, config]

# Tech tracking
tech-stack:
  added: []
  patterns: [centralized-mocks, compile-time-interface-checks, import-cycle-aware-mocking]

key-files:
  created:
    - internal/testutil/mocks/provider.go
    - internal/testutil/mocks/keychain.go
    - internal/testutil/mocks/tool.go
    - internal/testutil/mocks/dispatcher.go
    - internal/testutil/builders/dispatcher.go
  modified:
    - internal/workflow/engine_test.go
    - internal/workflow/engine_extra_test.go
    - internal/workflow/execute_test.go
    - internal/workflow/plan_test.go
    - internal/workflow/coverage_boost_test.go
    - internal/workflow/verify_test.go
    - internal/workflow/workflow_test.go
    - internal/workflow/integration_test.go
    - internal/workflow/plan_race_test.go
    - internal/workflow/phase_coordinator_test.go
    - internal/tools/dispatcher_test.go
    - internal/tools/extra_test.go
    - internal/tools/testutil_test.go

key-decisions:
  - "Removed builders/engine.go due to import cycle: builders→workflow→tools→builders"
  - "Provider/registry_test.go, keychain_test.go, config/loader_test.go keep local mocks due to import cycles"
  - "tools/subagent/extra_test.go keeps local mockDispatcher due to import cycle: mocks→tools/subagent"
  - "cmd/m31a/main_test.go keeps local mockKeychain due to different behavioral pattern (returns empty vs ErrKeyNotFound)"
  - "workflow.Dispatcher is concrete type, not interface — created minimal WorkflowDispatcher mock for phase coordinator"

patterns-established:
  - "Compile-time interface checks: var _ Interface = (*MockType)(nil)"
  - "Exported fields with trailing underscore for mock configuration (Name_, Response_, Err_)"
  - "Import cycle avoidance: packages with cycles keep justified local mocks documented in SUMMARY"

requirements-completed: [NFR-4]

coverage:
  - id: D1
    description: "Centralized mock types (MockProvider, MockKeychain, MockTool, WorkflowDispatcher, SubagentDispatcher)"
    requirement: NFR-4
    verification:
      - kind: unit
        ref: "internal/testutil/mocks/provider.go (compile-time check), keychain.go, tool.go, dispatcher.go"
        status: pass
    human_judgment: false
  - id: D2
    description: "Builder functions (NewTestDispatcher, NewTestDispatcherWithConfig)"
    requirement: NFR-4
    verification:
      - kind: unit
        ref: "internal/testutil/builders/dispatcher.go (compile-time check)"
        status: pass
    human_judgment: false
  - id: D3
    description: "Consumer test files migrated to centralized mocks"
    requirement: NFR-4
    verification:
      - kind: unit
        ref: "internal/workflow/*_test.go, internal/tools/dispatcher_test.go, extra_test.go, testutil_test.go"
        status: pass
    human_judgment: false

duration: 1h 41m
completed: 2026-07-18
status: complete
---

# Phase 10 Plan 02: Test Infrastructure Consolidation Summary

**Centralized testutil/mocks/ and testutil/builders/ packages eliminating duplicate mockProvider, mockKeychain, mockTool, and mockDispatcher across workflow, tools, and phase_coordinator test files**

## Performance

- **Duration:** 1h 41m
- **Started:** 2026-07-18T02:15:15Z
- **Completed:** 2026-07-18T03:56:00Z
- **Tasks:** 3
- **Files modified:** 19

## Accomplishments
- Created testutil/mocks/ with 5 consolidated mock types (MockProvider, MockKeychain, MockTool, WorkflowDispatcher, SubagentDispatcher)
- Created testutil/builders/ with 2 shared setup functions (NewTestDispatcher, NewTestDispatcherWithConfig)
- Migrated 13 consumer test files to use centralized mocks, removing local definitions where import cycles allow
- Zero duplicate mockTool definitions remain across the codebase

## Task Commits

Each task was committed atomically:

1. **Task 1: Create testutil/mocks/ with consolidated mock types** - `12f39d5a` (feat)
2. **Task 2: Create testutil/builders/ with shared setup functions** - `194176c5` (feat)
3. **Task 3: Update consumer test files to use centralized mocks/builders** - `ff2fda8b` (feat)

## Files Created/Modified
- `internal/testutil/mocks/provider.go` - MockProvider with NewMockProvider/NewMockProviderWithResponse constructors
- `internal/testutil/mocks/keychain.go` - MockKeychain with NewMockKeychain constructor
- `internal/testutil/mocks/tool.go` - MockTool with NewMockTool constructor
- `internal/testutil/mocks/dispatcher.go` - WorkflowDispatcher and SubagentDispatcher mocks
- `internal/testutil/builders/dispatcher.go` - NewTestDispatcher, NewTestDispatcherWithConfig helpers
- `internal/workflow/engine_test.go` - setupTestEngine uses mocks.NewMockProvider
- `internal/workflow/engine_extra_test.go` - mockProviderWithModel embeds mocks.MockProvider
- `internal/workflow/execute_test.go` - mockProviderWithCapture embeds mocks.MockProvider
- `internal/workflow/plan_test.go` - updated mockProvider type assertions and field accesses
- `internal/workflow/coverage_boost_test.go` - uses mocks.MockProvider
- `internal/workflow/verify_test.go` - uses mocks.NewMockProvider and tools.DefaultDispatcher
- `internal/workflow/workflow_test.go` - uses mocks.MockProvider
- `internal/workflow/integration_test.go` - tools.NewDispatcher(nil) replaced with tools.DefaultDispatcher
- `internal/workflow/plan_race_test.go` - tools.NewDispatcher(nil) replaced with tools.DefaultDispatcher
- `internal/workflow/phase_coordinator_test.go` - removed local mockDispatcher, uses mocks.WorkflowDispatcher
- `internal/tools/dispatcher_test.go` - replaced local mockTool with mocks.MockTool
- `internal/tools/extra_test.go` - replaced local mockTool with mocks.MockTool
- `internal/tools/testutil_test.go` - updated to use tools.DefaultDispatcher

## Decisions Made
- Removed builders/engine.go (planned but caused import cycle: builders→workflow→tools→builders)
- Kept local mocks in provider/registry_test.go, keychain/keychain_test.go, config/loader_test.go, tools/subagent/extra_test.go (all import cycles)
- Kept local mockKeychain in cmd/m31a/main_test.go (different behavioral pattern: returns empty vs ErrKeyNotFound)
- workflow.Dispatcher is concrete type not interface — created minimal WorkflowDispatcher mock for phase coordinator

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Removed builders/engine.go to fix import cycle**
- **Found during:** Task 2 (Create testutil/builders/)
- **Issue:** builders/engine.go imported workflow package, creating cycle: builders→workflow→tools→builders
- **Fix:** Removed engine.go from builders package; NewTestEngine not shipped
- **Files modified:** internal/testutil/builders/engine.go (deleted)
- **Verification:** go build ./... passes
- **Committed in:** 194176c5 (Task 2 commit)

**2. [Rule 3 - Blocking] Reverted testutil_test.go to use tools.DefaultDispatcher directly**
- **Found during:** Task 3 (Update consumer test files)
- **Issue:** testutil_test.go is in package tools — importing builders would create cycle: tools_test→builders→tools
- **Fix:** Reverted to using tools.DefaultDispatcher directly in testutil_test.go
- **Files modified:** internal/tools/testutil_test.go
- **Verification:** go build ./internal/tools/... passes
- **Committed in:** ff2fda8b (Task 3 commit)

---

**Total deviations:** 2 auto-fixed (2 blocking import cycles)
**Impact on plan:** Both deviations required for Go module constraint correctness. No scope creep.

## Issues Encountered
- Disk quota issues on /tmp caused cgo vet failures — worked around with rm -rf /tmp/go-build* and GOTMPDIR
- Pre-existing test errors in grep_test.go, edit_benchmark_test.go, filedelete_test.go, grep_skip_comments_test.go are out of scope

## Known Stubs
None — all mock types have complete implementations with compile-time interface checks.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Test infrastructure consolidated, ready for subsequent phases
- Remaining local mocks justified by import cycles — no further consolidation possible
- Pre-existing test errors (grep_test.go, etc.) need separate investigation

---
*Phase: 10-fix-test-errors-from-architecture-upgrade*
*Completed: 2026-07-18*
