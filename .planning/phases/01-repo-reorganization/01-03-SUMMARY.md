---
phase: 01-repo-reorganization
plan: 03
subsystem: infra
tags: [go, imports, directory-structure, integrations]

# Dependency graph
requires:
  - phase: 01-repo-reorganization/01-01
    provides: Research and context for repo structure
  - phase: 01-repo-reorganization/01-02
    provides: Engine packages relocated to internal/engine/
provides:
  - internal/integrations/ layer with 14 relocated packages
  - Provider sub-packages (openrouter, zen, nvidia, mock) preserved under integrations
  - Updated import paths across entire codebase
affects: [01-repo-reorganization/01-04, 01-repo-reorganization/01-05, 01-repo-reorganization/01-06]

# Tech tracking
tech-stack:
  added: []
  patterns: [integrations-layer-consolidation]

key-files:
  created: []
  modified: [cmd/m31a/main.go, internal/tools/*.go, internal/tui/*.go, internal/engine/workflow/*.go]

key-decisions:
  - "Moved all external-system adapters to internal/integrations/ for consistent dependency graph"
  - "Deleted internal/wiring/ (obsolete, only contained regression_test.go)"
  - "Used longest-path-first sed ordering to prevent substring collision during import updates"

patterns-established:
  - "Integrations layer: all external system adapters live under internal/integrations/"
  - "Import update pattern: sed with quoted string matching to avoid substring collisions"

requirements-completed: []

coverage:
  - id: D1
    description: "14 integration packages relocated from internal/ to internal/integrations/"
    verification:
      - kind: automated_ui
        ref: "ls internal/integrations/provider/interface.go internal/integrations/git/ internal/integrations/keychain/"
        status: pass
    human_judgment: false
  - id: D2
    description: "All import paths updated across codebase (0 stale imports remaining)"
    verification:
      - kind: automated_ui
        ref: "grep -r 'internal/provider' --include='*.go' | grep -v 'integrations/provider' | wc -l => 0"
        status: pass
    human_judgment: false
  - id: D3
    description: "Provider sub-packages (mock/, openrouter/, zen/, nvidia/) preserved under integrations/provider/"
    verification:
      - kind: automated_ui
        ref: "ls internal/integrations/provider/mock/ internal/integrations/provider/openrouter/"
        status: pass
    human_judgment: false

# Metrics
duration: 3min
completed: 2026-07-21
status: complete
---

# Phase 1 Plan 3: Integration Packages Relocation Summary

**14 integration packages moved to internal/integrations/ with all import paths updated, obsolete wiring package deleted**

## Performance

- **Duration:** 3 min
- **Started:** 2026-07-21
- **Completed:** 2026-07-21
- **Tasks:** 2
- **Files modified:** ~80

## Accomplishments
- Relocated 14 integration packages (provider, git, keychain, shell, context, history, ledger, metrics, logging, log, autodream, arbitrage, codeintel, skills) to internal/integrations/
- Preserved all provider sub-packages (mock/, openrouter/, zen/, nvidia/) under integrations/provider/
- Updated import paths across entire codebase with 0 stale imports remaining
- Deleted obsolete internal/wiring/ package

## Task Commits

Each task was committed atomically:

1. **Task 1: Create integrations directory and move packages** - (pending)
2. **Task 2: Update integration import paths** - (pending)

## Files Created/Modified
- `internal/integrations/provider/` - LLM provider abstraction and 3 implementations
- `internal/integrations/git/` - Git operations wrapper
- `internal/integrations/keychain/` - OS keychain integration
- `internal/integrations/shell/` - Shell detection and execution
- `internal/integrations/context/` - Dynamic context sources
- `internal/integrations/history/` - Frecent prompt history
- `internal/integrations/ledger/` - LEDGER.md append-only records
- `internal/integrations/metrics/` - Session metrics collector
- `internal/integrations/logging/` - Audit logging
- `internal/integrations/log/` - Structured logging
- `internal/integrations/autodream/` - Context compression
- `internal/integrations/arbitrage/` - Model-cost optimizer
- `internal/integrations/codeintel/` - Code intelligence
- `internal/integrations/skills/` - Skill discovery
- `cmd/m31a/main.go` - Updated imports
- `internal/tools/*.go` - Updated imports
- `internal/tui/*.go` - Updated imports
- `internal/engine/workflow/*.go` - Updated imports

## Decisions Made
- Used quoted string matching in sed to prevent substring collisions (e.g., `internal/git` vs `internal/integrations/git`)
- Deleted internal/wiring/ as it only contained regression_test.go and was confirmed obsolete

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Pre-existing build errors from prior phases (missing internal/types and internal/errors packages) — outside scope of this plan

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Integrations layer consolidated under internal/integrations/
- Ready for next phase (likely core layer or remaining reorganization)
- Build currently broken due to missing internal/types and internal/errors (pre-existing)

---
*Phase: 01-repo-reorganization*
*Completed: 2026-07-21*
