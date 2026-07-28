---
phase: 05-fix-wiring-issues
plan: 01
subsystem: config, workflow
tags: [config-merge, rollback, bisect, toml, git]

# Dependency graph
requires:
  - phase: 04-dead-code-audit
    provides: codebase cleanup and lint compliance
provides:
  - Complete config merge covering all ~30 missing fields across 6 sections
  - Rollback.SoftReset wired into bisect failure path for automatic offender reversion
affects: [05-fix-wiring-issues]

# Tech tracking
tech-stack:
  added: []
  patterns: [stringMapField helper, execGitRunner for standalone rollback]

key-files:
  created:
    - internal/engine/workflow/bisect_rollback_test.go
  modified:
    - internal/core/config/merge.go
    - internal/core/config/merge_test.go
    - internal/engine/workflow/verify.go
    - internal/engine/rollback/rollback.go

key-decisions:
  - "Created standalone rollback.SoftReset function using git revert --no-commit instead of adapting Rollback.SoftReset method (which does git reset --soft)"
  - "Wired rollback into verify.go tryBisectHeal (where bisect actually lives) instead of execute.go (plan assumed wrong file location)"

patterns-established:
  - "stringMapField: reusable merge helper for map[string]string TOML fields"
  - "execGitRunner: minimal GitRunner implementation for standalone rollback without *git.Git dependency"

requirements-completed: [W01, W02]

# Coverage metadata (#1602)
coverage:
  - id: D1
    description: "Complete config merge for all ~30 missing fields across 6 sections"
    requirement: W01
    verification:
      - kind: unit
        ref: "internal/core/config/merge_test.go#TestMergeConfig_NarrativeConfig"
        status: pass
      - kind: unit
        ref: "internal/core/config/merge_test.go#TestMergeConfig_ModelCapabilitiesConfig"
        status: pass
      - kind: unit
        ref: "internal/core/config/merge_test.go#TestMergeConfig_PromptConfig"
        status: pass
      - kind: unit
        ref: "internal/core/config/merge_test.go#TestMergeConfig_TemplateConfig"
        status: pass
      - kind: unit
        ref: "internal/core/config/merge_test.go#TestMergeConfig_FeaturesMissingFields"
        status: pass
      - kind: unit
        ref: "internal/core/config/merge_test.go#TestMergeConfig_ToolsMissingFields"
        status: pass
      - kind: unit
        ref: "internal/core/config/merge_test.go#TestMergeConfig_CompactionMissingFields"
        status: pass
      - kind: unit
        ref: "internal/core/config/merge_test.go#TestMergeConfig_VerifyLintCommand"
        status: pass
    human_judgment: false
  - id: D2
    description: "Rollback.SoftReset wired into bisect failure path for automatic offender reversion"
    requirement: W02
    verification:
      - kind: unit
        ref: "internal/engine/workflow/bisect_rollback_test.go#TestBisectRollback_SoftResetIntegration"
        status: pass
    human_judgment: false

# Metrics
duration: 14min
completed: 2026-07-29
status: complete
---

# Phase 05 Plan 01: Fix Wiring Issues Summary

**Complete config merge for all ~30 missing fields across 6 sections, and rollback.SoftReset wired into bisect failure path for automatic offender reversion via git revert --no-commit**

## Performance

- **Duration:** 14 min
- **Started:** 2026-07-28T21:58:41Z
- **Completed:** 2026-07-28T22:12:59Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Added 4 new merge section helpers (Narrative, ModelCapabilities, Prompts, Templates) and added ~30 missing field merge calls to existing helpers
- Added stringMapField helper for map[string]string TOML merge operations
- Wired rollback.SoftReset into verify.go tryBisectHeal after offender identification using git revert --no-commit
- Created standalone rollback.SoftReset function with execGitRunner for use without *git.Git dependency
- Added comprehensive test coverage for all new merge functionality and rollback integration

## Task Commits

Each task was committed atomically:

1. **Task 1: Complete config merge for all missing fields** - `939f3e1e` (feat)
2. **Task 2: Wire rollback.SoftReset into bisect failure path** - `3463ae81` (feat)

## Files Created/Modified
- `internal/core/config/merge.go` - Added 4 new merge section helpers, stringMapField, and ~30 missing field merge calls
- `internal/core/config/merge_test.go` - Added 10 new test functions covering all new merge sections
- `internal/engine/workflow/verify.go` - Wired rollback.SoftReset call after bisect offender identification
- `internal/engine/rollback/rollback.go` - Added standalone SoftReset function and execGitRunner
- `internal/engine/workflow/bisect_rollback_test.go` - Integration test validating rollback with real git repo

## Decisions Made
- Created standalone `rollback.SoftReset(offenderHash, workDir)` function using `git revert --no-commit` instead of adapting `Rollback.SoftReset` method (which does `git reset --soft` and has a different signature than what the plan described)
- Wired rollback into `verify.go` `tryBisectHeal` (where bisect actually lives) instead of `execute.go` (the plan assumed the wrong file location)
- Used execGitRunner (minimal os/exec-based GitRunner) for the standalone function to avoid importing the git package into rollback

## Deviations from Plan

### Plan Inaccuracies Adapted

**1. Wrong target file for rollback wiring**
- **Found during:** Task 2
- **Issue:** Plan specified `execute.go` as the file to wire rollback into, but the bisect flow lives in `verify.go` `tryBisectHeal` function
- **Fix:** Wired rollback.SoftReset into verify.go where bisect actually identifies offenders
- **Files modified:** `internal/engine/workflow/verify.go`
- **Verification:** Test passes, go vet clean

**2. Wrong API signature for SoftReset**
- **Found during:** Task 2
- **Issue:** Plan described `func SoftReset(offenderHash string, workDir string) error` as a standalone function doing `git revert --no-commit`, but the existing `Rollback.SoftReset` method has signature `func (r *Rollback) SoftReset(hash string, onReset func(newHead string) error) (*RollbackResult, error)` and does `git reset --soft`
- **Fix:** Created standalone `rollback.SoftReset` function matching the plan's described behavior (`git revert --no-commit`)
- **Files modified:** `internal/engine/rollback/rollback.go`
- **Verification:** Integration test validates revert behavior

---

**Total deviations:** 2 plan inaccuracies adapted (both are plan-vs-reality mismatches, not bugs)
**Impact on plan:** Adaptations necessary to match actual codebase structure. No scope creep.

## Issues Encountered
- Pre-existing test failures due to disk quota in /tmp (`TestExtractWebsiteTemplateTo_*`) and existing rollback test issues (`TestErrorPaths_DestroyedRepo`) — unrelated to this plan's changes

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Config merge now covers all ~30 previously-missing fields across 6 sections
- Rollback is now wired into the bisect failure path for automatic offender reversion
- Ready for next plan in phase 05-fix-wiring-issues

---
*Phase: 05-fix-wiring-issues*
*Completed: 2026-07-29*

## Self-Check: PASSED
