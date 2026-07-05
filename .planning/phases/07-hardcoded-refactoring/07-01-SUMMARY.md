---
phase: 07-hardcoded-refactoring
plan: 01
subsystem: config
tags: [toml, config, rate-limit, retry, toast, coordinator, workflow]

# Dependency graph
requires: []
provides:
  - "Extended Config struct with tools, features, UI fields for hardcoded value configurability"
  - "DefaultConfig() with matching defaults for all new fields"
  - "Threat model validations for rate limits, concurrency, and retry parameters"
  - "Consumer code reads from config with constant fallbacks"
affects: [07-02, 07-03, 07-04, 07-05]

# Tech tracking
tech-stack:
  added: []
  patterns: [config-with-fallback, threat-model-validation]

key-files:
  created: []
  modified:
    - internal/config/types.go
    - internal/config/loader.go
    - internal/tools/bash.go
    - internal/tools/webfetch.go
    - internal/tools/defaults.go
    - internal/workflow/engine.go
    - internal/workflow/execute.go
    - pkg/retry/policy.go
    - pkg/coordinator/coordinator.go
    - pkg/session/manager.go
    - internal/tui/toast.go
    - internal/tui/app_state.go
    - cmd/m31a/main.go

key-decisions:
  - "Constants remain as compiled fallback defaults; config values override when present"
  - "Zero-valued config fields mean use default (backward compatible)"
  - "Threat model validations added for rate limits (1-100/50), concurrency (1-32/16), retry (1-10/300s)"

patterns-established:
  - "Config-with-fallback: consumer code checks cfg.Field > 0 before using config value, falls back to constant"
  - "Threat model validation: new config fields validated at load time per STRIDE analysis"

requirements-completed: [HARD-01, HARD-02, HARD-03, HARD-04]

# Metrics
duration: 4min
completed: 2026-07-05
---

# Phase 07 Plan 01: Hardcoded Refactoring Summary

**Config schema extension making 40+ hardcoded constants configurable via TOML with zero behavior change on defaults**

## Performance

- **Duration:** 4 min
- **Started:** 2026-07-05T21:21:44Z
- **Completed:** 2026-07-05T21:25:26Z
- **Tasks:** 2
- **Files modified:** 13

## Accomplishments
- Extended Config struct with 40+ new fields across ToolsConfig, FeaturesConfig, UIConfig matching all hardcoded values from the audit
- DefaultConfig() sets defaults matching current hardcoded values exactly (zero behavior change)
- Added threat model validations for rate limits (T-07-01), concurrency (T-07-02), and retry parameters (T-07-04)
- Wired all consumer code to read from config with constant fallbacks: bash timeout, webfetch retries, context truncation threshold, tool concurrency, loop detection window, retry policy, max parallel tasks, toast visibility, coordinator safety timeout

## Task Commits

Each task was committed atomically:

1. **Task 1: Extend Config Structs and DefaultConfig()** - `2d570a0e` (feat)
2. **Task 2: Wire Config Values Into Consumer Code** - `1bd0e165` (feat)

## Files Created/Modified
- `internal/config/types.go` - Added 40+ config fields to ToolsConfig, FeaturesConfig, UIConfig
- `internal/config/loader.go` - Added DefaultConfig() entries and threat model validations
- `internal/tools/bash.go` - Bash reads BashMaxTimeoutSecs from config
- `internal/tools/webfetch.go` - WebFetch reads retry parameters from config
- `internal/tools/defaults.go` - Updated dispatcher to pass config values
- `internal/workflow/engine.go` - Engine reads ContextTruncationThreshold from config
- `internal/workflow/execute.go` - Execute reads MaxToolConcurrency and LoopDetectWindow from config
- `pkg/retry/policy.go` - ConfiguredPolicy accepts config parameters with DefaultPolicy fallback
- `pkg/coordinator/coordinator.go` - SafetyTimeout field replaces hardcoded 5-minute timeout
- `pkg/session/manager.go` - ManagerOpts accepts CoordinatorTimeoutSecs
- `internal/tui/toast.go` - maxVisibleToasts configurable via SetMaxVisibleToasts
- `internal/tui/app_state.go` - NewApp wires ToastMaxVisible from config
- `cmd/m31a/main.go` - Main wires CoordinatorTimeoutSecs to session manager

## Decisions Made
- Constants remain as compiled fallback defaults; config values override when present (zero behavior change)
- Zero-valued config fields mean use default (backward compatible with existing configs)
- Threat model validations enforce bounds: rate_limit_burst 1-100, rate_limit_per_sec 1-50, max_concurrent 1-32, max_tool_concurrency 1-16, retry_max_attempts 1-10, retry_max_delay_ms 1000-300000

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added threat model config validations**
- **Found during:** Task 1 (Extend Config Structs)
- **Issue:** Threat model T-07-01, T-07-02, T-07-04 required config value validation at load time, but validateConfig() had no entries for the new fields
- **Fix:** Added range validation for rate limits (1-100/50), concurrency (1-32/16), and retry parameters (1-10/300s)
- **Files modified:** internal/config/loader.go
- **Verification:** go build and go vet pass
- **Committed in:** 2d570a0e (Task 1 commit)

**2. [Rule 2 - Missing Critical] Wired toast maxVisibleToasts to config**
- **Found during:** Task 2 (Wire Consumer Code)
- **Issue:** Plan acceptance criteria required toast.go to read ToastMaxVisible from config, but maxVisibleToasts was a hardcoded const
- **Fix:** Changed const to var, added SetMaxVisibleToasts function, wired in NewApp from cfg.UI.ToastMaxVisible
- **Files modified:** internal/tui/toast.go, internal/tui/app_state.go
- **Verification:** go build and go vet pass
- **Committed in:** 1bd0e165 (Task 2 commit)

**3. [Rule 2 - Missing Critical] Wired coordinator safety timeout to config**
- **Found during:** Task 2 (Wire Consumer Code)
- **Issue:** Plan action item 8 required coordinator.go 5-minute timeout to be configurable, but it was hardcoded
- **Fix:** Added SafetyTimeout field to Coordinator, updated awaitDone to use it, wired through ManagerOpts from cfg.Features.CoordinatorTimeoutSecs
- **Files modified:** pkg/coordinator/coordinator.go, pkg/session/manager.go, cmd/m31a/main.go
- **Verification:** go build and go vet pass
- **Committed in:** 1bd0e165 (Task 2 commit)

---

**Total deviations:** 3 auto-fixed (3 missing critical)
**Impact on plan:** All auto-fixes necessary for threat model compliance and plan acceptance criteria. No scope creep.

## Issues Encountered
None - plan executed smoothly.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Config schema extended and ready for all subsequent hardcoded refactoring plans (07-02 through 07-05)
- All consumer code reads from config with constant fallbacks
- Zero behavior change when no config overrides are set

---
*Phase: 07-hardcoded-refactoring*
*Completed: 2026-07-05*

## Self-Check: PASSED

- SUMMARY.md: FOUND
- Task 1 commit (2d570a0e): FOUND
- Task 2 commit (1bd0e165): FOUND
- Docs commit (f8d15fca): FOUND
- Shared files (STATE.md, ROADMAP.md) not modified: CONFIRMED
