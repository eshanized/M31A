---
phase: 05-fix-wiring-issues
plan: 03
subsystem: workflow, provider, config, tools
tags: [workflow-event, zen-provider, config-validation, dead-code-removal, ip-filter]

# Dependency graph
requires:
  - phase: 05-fix-wiring-issues/01
    provides: prior wiring fixes
provides:
  - WorkflowEvent interface on AgentSwitchMsg and DecisionsSnapshotMsg
  - Zen provider retry loop and credit detection parity with OpenRouter/NVIDIA
  - FallbackPriority validation against known provider names
  - Consolidated IP filter (single IsReservedIP)
  - Removed dead constants, errors, placeholders, and layout primitives
affects: [narrative-engine, provider-layer, config-validation]

# Tech tracking
tech-stack:
  added: []
  patterns: [retry-with-exponential-backoff, provider-parity, config-field-validation]

key-files:
  created:
    - internal/engine/workflow/narrative_events_test.go
    - internal/integrations/provider/zen/retry_test.go
    - internal/core/config/fallback_priority_test.go
  modified:
    - internal/engine/workflow/engine_event_methods.go
    - internal/engine/workflow/phase_coordinator.go
    - internal/engine/workflow/agent_switch.go
    - internal/core/types/constants.go
    - internal/core/errors/errors.go
    - internal/tools/exec/bash.go
    - internal/tools/exec/output_store.go
    - internal/tools/search/websearch.go
    - internal/integrations/provider/zen/client.go
    - internal/core/config/config_validate.go
  deleted:
    - internal/integrations/provider/mock/streaming.go

key-decisions:
  - "Kept layout primitives (W30-W32) — box.go, page.go, and external callers depend on them; plan incorrectly assessed as dead"
  - "Removed error sentinel tests that referenced deleted errors; kept tests for errors still in use"
  - "FallbackPriority validation is advisory (appends to error list, does not block startup)"

patterns-established:
  - "Zen provider retry: maxRetries=2 with exponential backoff on IsRetryable errors"
  - "Config validation: FallbackPriority entries checked against types.Provider* constants"

requirements-completed: [W16, W17, W18, W19, W20, W21, W22, W23, W24, W25, W26, W27, W29]

# Coverage metadata
coverage:
  - id: D1
    description: "AgentSwitchMsg and DecisionsSnapshotMsg implement WorkflowEvent interface"
    requirement: "W18, W19"
    verification:
      - kind: unit
        ref: "internal/engine/workflow/narrative_events_test.go#TestWorkflowEvent_AgentSwitchMsg"
        status: pass
      - kind: unit
        ref: "internal/engine/workflow/narrative_events_test.go#TestWorkflowEvent_DecisionsSnapshotMsg"
        status: pass
    human_judgment: false
  - id: D2
    description: "Dead workflow placeholders removed (ModelForPhase, ProviderForPhase, BuildAgentSwitchMessage, IsPlanComplete)"
    requirement: "W20, W21, W22"
    verification:
      - kind: unit
        ref: "go build ./internal/engine/workflow/"
        status: pass
    human_judgment: false
  - id: D3
    description: "Dead constants and sentinel errors removed from types/constants.go and errors/errors.go"
    requirement: "W16, W17"
    verification:
      - kind: unit
        ref: "go build ./internal/core/types/ ./internal/core/errors/"
        status: pass
    human_judgment: false
  - id: D4
    description: "Zen provider has retry loop and credit detection matching OpenRouter/NVIDIA"
    requirement: "W26, W27"
    verification:
      - kind: unit
        ref: "internal/integrations/provider/zen/retry_test.go#TestChatCompletionStream_Retry"
        status: pass
    human_judgment: false
  - id: D5
    description: "FallbackPriority validated against known provider names"
    requirement: "W29"
    verification:
      - kind: unit
        ref: "internal/core/config/fallback_priority_test.go#TestValidateConfig_FallbackPriority"
        status: pass
    human_judgment: false
  - id: D6
    description: "Duplicate isReservedIP removed, canonical IsReservedIP used"
    requirement: "W24"
    verification:
      - kind: unit
        ref: "go build ./internal/tools/search/"
        status: pass
    human_judgment: false
  - id: D7
    description: "Dead exec constants removed, StreamingMockProvider deleted"
    requirement: "W23, W25"
    verification:
      - kind: unit
        ref: "go build ./internal/tools/exec/ ./internal/integrations/provider/"
        status: pass
    human_judgment: false

# Metrics
duration: 9min
completed: 2026-07-28
status: complete
---

# Phase 5 Plan 03: Fix Wiring Issues (W16-W32) Summary

**WorkflowEvent methods on AgentSwitchMsg/DecisionsSnapshotMsg, Zen retry/credit parity, FallbackPriority validation, dead code removal across constants/errors/exec/layout**

## Performance

- **Duration:** 9 min
- **Started:** 2026-07-28T22:33:41Z
- **Completed:** 2026-07-28T22:43:34Z
- **Tasks:** 2
- **Files modified:** 15

## Accomplishments
- Added WorkflowEvent interface (EventType/EventData) to AgentSwitchMsg and DecisionsSnapshotMsg for narrative engine classification
- Added retry loop with exponential backoff to Zen provider, matching OpenRouter/NVIDIA pattern
- Added FallbackPriority validation rejecting invalid provider names in config
- Consolidated duplicate isReservedIP into single canonical IsReservedIP
- Removed 10 dead constants, 5 dead sentinel errors, 27 dead exec constants, dead workflow placeholders, and StreamingMockProvider

## Task Commits

Each task was committed atomically:

1. **Task 1: Add WorkflowEvent methods, remove dead constants/errors/placeholders (W16-W22)** - `9cd85b8f` (fix)
2. **Task 2: Remove dead exec constants, consolidate IP filter, Zen parity, FallbackPriority validation (W23-W32)** - `6eca9bc4` (fix)

## Files Created/Modified
- `internal/engine/workflow/engine_event_methods.go` - Added AgentSwitchMsg/DecisionsSnapshotMsg event methods
- `internal/engine/workflow/phase_coordinator.go` - Removed ModelForPhase, ProviderForPhase, unused provider import
- `internal/engine/workflow/agent_switch.go` - Removed BuildAgentSwitchMessage, truncatePlan, IsPlanComplete, unused strings import
- `internal/core/types/constants.go` - Removed 10 dead constants (MaxPlanRefinements, CompressCooldown, ChannelSendTimeout, ToastDuration, HealthCheckRetryDelay, DefaultVerifyTimeout, DefaultFetchModelsTimeout, DefaultUserAgent, DefaultXTitle, DateFormat)
- `internal/core/errors/errors.go` - Removed 5 dead sentinel errors (ErrInvalidInput, ErrNotFound, ErrAlreadyExists, ErrCancelled, ErrInternal) and their UserMessage cases
- `internal/tools/exec/bash.go` - Removed 27 dead constants, kept BashKillGracePeriod/BashWaitTimeout
- `internal/tools/exec/output_store.go` - Fixed to use types.DefaultOutputMaxLines/Bytes
- `internal/tools/search/websearch.go` - Deleted duplicate isReservedIP, using canonical IsReservedIP
- `internal/integrations/provider/zen/client.go` - Added retry loop, changed to HandleChatHTTPErrorWithCredits
- `internal/core/config/config_validate.go` - Added FallbackPriority validation
- `internal/integrations/provider/mock/streaming.go` - Deleted (StreamingMockProvider)
- `internal/engine/workflow/narrative_events_test.go` - Created (WorkflowEvent tests)
- `internal/integrations/provider/zen/retry_test.go` - Created (retry loop test)
- `internal/core/config/fallback_priority_test.go` - Created (validation test)

## Decisions Made
- **Kept layout primitives (W30-W32):** constraints.go, solver.go, and stack.go are NOT dead. box.go uses Box/Solve/RenderFunc from constraints/solver. stack.go defines RenderModalOverlay and truncateToWidth used by page.go, minscreen.go, and externally by app_view.go/app_screens.go. Plan incorrectly assessed these as dead.
- **FallbackPriority validation is advisory:** Appends to validation error list rather than blocking startup, matching the low-severity disposition in the threat model.
- **Kept error tests for remaining errors:** Removed only test cases for deleted errors; kept tests for errors still in use.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed output_store.go referencing removed constants**
- **Found during:** Task 2 (dead exec constants removal)
- **Issue:** output_store.go referenced DefaultOutputMaxLines/DefaultOutputMaxBytes directly from the exec package aliases
- **Fix:** Changed to types.DefaultOutputMaxLines/Bytes (canonical source)
- **Files modified:** internal/tools/exec/output_store.go
- **Verification:** go build passes
- **Committed in:** 6eca9bc4 (Task 2 commit)

**2. [Rule 1 - Bug] Fixed test files referencing deleted functions**
- **Found during:** Task 1 (placeholder removal)
- **Issue:** agent_switch_test.go, phase_coordinator_test.go, execute_quality_test.go, coverage_boost_test.go referenced BuildAgentSwitchMessage, truncatePlan, IsPlanComplete, ModelForPhase, ProviderForPhase
- **Fix:** Removed or updated test functions to match removed implementations
- **Files modified:** 4 test files
- **Verification:** go test passes
- **Committed in:** 9cd85b8f (Task 1 commit)

### Plan Assessment Corrections

**3. [Rule 4 - Assessment] W30-W32 layout primitives are NOT dead**
- **Found during:** Task 2 (dead layout primitives removal)
- **Issue:** Plan identified constraints.go, solver.go, stack.go as dead with zero external usage
- **Reality:** These files define core types (Box, Solve, RenderFunc, RenderModalOverlay, truncateToWidth) used internally by box.go, page.go, minscreen.go and externally by app_view.go, app_screens.go. Deleting them would break the layout system.
- **Action:** Skipped W30-W32 entirely
- **Impact:** No code change; plan's assessment was incorrect

---

**Total deviations:** 3 (2 auto-fixed, 1 plan assessment correction)
**Impact on plan:** All auto-fixes necessary for correctness. Layout primitives deviation is a plan assessment error — code is actively used. No scope creep.

## Issues Encountered
- Disk quota exceeded during build — resolved by setting GOTMPDIR and using CGO_ENABLED=0

## Known Stubs
None — all changes are removals or additions of working code with passing tests.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- W16-W27, W29 complete. W28 (test quality) deferred to separate phase per plan note.
- W30-W32 (layout primitives) skipped — code is actively used, not dead.
- Ready for remaining phase 05 plans.

---
*Phase: 05-fix-wiring-issues*
*Completed: 2026-07-28*

## Self-Check: PASSED
