---
phase: 04-release-audit
plan: 01
subsystem: architecture
tags: [pkg, internal, types, interfaces, import-cycles, go-modules]

# Dependency graph
requires:
  - phase: 03-stabilization
    provides: "Codebase with working tests and lint passing"
provides:
  - "pkg/types/ — shared type vocabulary importable without internal/"
  - "pkg/errors/ — sentinel errors importable by pkg/"
  - "Consumer-side interfaces in 10 pkg/ packages"
  - "WorkflowEvent interface for narrative bridge"
  - "ChatRequest and ToolDefinition in pkg/types/"
affects: [04-release-audit]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Type aliases for backward compatibility (internal/types/ wraps pkg/types/)"
    - "Consumer-side interfaces to break import cycles"
    - "WorkflowEvent interface for cross-package event conversion"

key-files:
  created:
    - pkg/types/types.go
    - pkg/types/constants.go
    - pkg/types/toolcall.go
    - pkg/types/plan.go
    - pkg/types/git.go
    - pkg/types/fileutil.go
    - pkg/errors/errors.go
    - pkg/narrative/event.go
    - pkg/session/fileutil.go
    - internal/workflow/engine_event_methods.go
  modified:
    - internal/types/types.go
    - internal/types/constants.go
    - internal/types/toolcall.go
    - internal/types/plan.go
    - internal/types/git.go
    - internal/errors/errors.go
    - internal/provider/interface.go
    - pkg/taskrunner/runner.go
    - pkg/arbitrage/arbitrage.go
    - pkg/bisect/bisect.go
    - pkg/ledger/ledger.go
    - pkg/rollback/rollback.go
    - pkg/autodream/autodream.go
    - pkg/session/manager.go
    - pkg/session/checkpoint.go
    - pkg/session/planning.go
    - pkg/session/session.go
    - pkg/session/session_info.go
    - pkg/metrics/collector.go
    - pkg/metrics/types.go
    - pkg/compaction/compaction.go
    - pkg/compaction/serialize.go
    - pkg/narrative/bridge.go

key-decisions:
  - "Moved ChatRequest and ToolDefinition to pkg/types/ (pure data structs) instead of defining adapter wrappers"
  - "Used consumer-side interfaces for provider, git, tokens, workflow dependencies"
  - "WorkflowEvent interface uses EventType()/EventData() pattern for clean conversion"
  - "pkg/types/fileutil.go provides FileLock and AtomicWrite with Unix build tags"

patterns-established:
  - "Consumer-side interface pattern: define interface in pkg/, concrete type in internal/"
  - "Type alias pattern: internal/types/ wraps pkg/types/ for backward compatibility"
  - "Event interface pattern: message types implement EventType()/EventData() for cross-package conversion"

requirements-completed: [C1]

coverage:
  - id: D1
    description: "pkg/types/ package with all shared types importing only stdlib"
    requirement: C1
    verification:
      - kind: unit
        ref: "go build ./pkg/types/... && go vet ./pkg/types/..."
        status: pass
    human_judgment: false
  - id: D2
    description: "pkg/errors/ package with sentinel errors importing only stdlib"
    requirement: C1
    verification:
      - kind: unit
        ref: "go build ./pkg/errors/... && go vet ./pkg/errors/..."
        status: pass
    human_judgment: false
  - id: D3
    description: "internal/types/ thin wrapper with type aliases preserving backward compatibility"
    requirement: C1
    verification:
      - kind: unit
        ref: "go test ./internal/types/... -count=1"
        status: pass
    human_judgment: false
  - id: D4
    description: "All 10 pkg/ packages import pkg/types instead of internal/types"
    requirement: C1
    verification:
      - kind: unit
        ref: "grep -r 'internal' pkg/ --include='*.go' | grep -v '_test.go' | wc -l returns 0"
        status: pass
    human_judgment: false
  - id: D5
    description: "Consumer-side interfaces for provider, git, tokens, workflow"
    requirement: C1
    verification:
      - kind: unit
        ref: "go test ./pkg/compaction/... ./pkg/narrative/... -count=1"
        status: pass
    human_judgment: false

duration: 35min
completed: 2026-07-14
status: complete
---

# Phase 4 Plan 01: Fix pkg/ to internal/ Architectural Boundary Violation Summary

**Extracted 50+ shared types/errors from internal/ to pkg/ with type aliases for backward compatibility, eliminating all pkg/ to internal/ imports across 10 packages**

## Performance

- **Duration:** 35 min
- **Started:** 2026-07-14T10:00:00Z
- **Completed:** 2026-07-14T10:35:00Z
- **Tasks:** 2
- **Files modified:** 30+

## Accomplishments
- Created pkg/types/ with all shared type vocabulary (Task, Message, ToolCall, etc.) importing only stdlib
- Created pkg/errors/ with all sentinel errors and error types importing only stdlib
- Converted internal/types/ to thin wrapper using type aliases to pkg/types/
- Updated all 10 pkg/ packages to import pkg/types and pkg/errors
- Defined consumer-side interfaces (Provider, GitRunner, TokenEstimator, WorkflowEvent)
- Moved ChatRequest and ToolDefinition to pkg/types/ to satisfy interface compatibility
- Verified zero pkg/ to internal/ imports across entire codebase

## Task Commits

Each task was committed atomically:

1. **Task 1: Extract shared types from internal/types/ to pkg/types/ and create pkg/errors/** - `f01a2dce` (feat)
2. **Task 2: Update all pkg/ consumer imports and define interfaces** - `8b633aa2` (feat)

## Files Created/Modified

### Created
- `pkg/types/types.go` — All shared types (Task, Message, ToolCall, WorkflowPhase, etc.)
- `pkg/types/constants.go` — Shared constants (BashTimeout, MaxHealAttempts, etc.)
- `pkg/types/toolcall.go` — ToolCallAcc and BuildToolCallsFromAcc
- `pkg/types/plan.go` — Plan, ReviewNote, OpenQuestion types
- `pkg/types/git.go` — CommitInfo, GitClient interface
- `pkg/types/fileutil.go` — FileLock, AtomicWrite (Unix build-tagged)
- `pkg/errors/errors.go` — All sentinel errors and error types
- `pkg/narrative/event.go` — WorkflowEvent interface definition
- `pkg/session/fileutil.go` — Local fileLock/atomicWrite delegating to pkg/types
- `internal/workflow/engine_event_methods.go` — EventType()/EventData() on all message types

### Modified
- `internal/types/types.go` — Thin wrapper with type aliases to pkg/types/
- `internal/types/constants.go` — Constant re-exports
- `internal/types/toolcall.go` — Type aliases
- `internal/types/plan.go` — Type aliases
- `internal/types/git.go` — Type aliases
- `internal/errors/errors.go` — Variable aliases to pkg/errors/
- `internal/provider/interface.go` — ChatRequest/ToolDefinition aliases from pkg/types/
- All 10 pkg/ consumer files — Updated imports

## Decisions Made
- Moved ChatRequest and ToolDefinition to pkg/types/ instead of creating adapter wrappers (simpler, no runtime overhead)
- Used consumer-side interface pattern for all internal/ dependencies (Provider, GitRunner, TokenEstimator, WorkflowEvent)
- WorkflowEvent interface uses EventType()/EventData() pattern for clean conversion without type switches

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] ChatRequest and ToolDefinition type mismatch**
- **Found during:** Task 2 (compaction.Provider interface)
- **Issue:** compaction.Provider.ChatCompletionStream expected CompletionRequest but provider.LLMProvider uses provider.ChatRequest — structurally identical but different types
- **Fix:** Moved ChatRequest and ToolDefinition to pkg/types/, added aliases in internal/types/ and internal/provider/
- **Files modified:** pkg/types/types.go, internal/types/types.go, internal/provider/interface.go, pkg/compaction/compaction.go
- **Verification:** go build ./... passes
- **Committed in:** 8b633aa2 (Task 2 commit)

**2. [Rule 1 - Bug] Narrative bridge event type name mismatch**
- **Found during:** Task 2 (bridge tests)
- **Issue:** SelfHealStart/SelfHealComplete event types returned "self_heal_start" but constants expect "selfheal_start"
- **Fix:** Updated engine_event_methods.go to match exact constant values
- **Files modified:** internal/workflow/engine_event_methods.go
- **Verification:** go test ./pkg/narrative/... passes
- **Committed in:** 8b633aa2 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both auto-fixes necessary for correctness. ChatRequest move was required for interface compatibility. Event type fix matched existing constants.

## Issues Encountered
None beyond the auto-fixed deviations above.

## Known Stubs
None — all implementations are complete and wired.

## Threat Flags
None — no new security-relevant surface introduced.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- C1 architectural violation fully resolved
- All pkg/ packages now importable without pulling in internal/ code
- Type identity preserved via aliases (pkg/types.Message == internal/types.Message)
- Ready for remaining release audit fixes (C2, C3, H1-H8)

---
*Phase: 04-release-audit*
*Completed: 2026-07-14*
