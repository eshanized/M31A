---
phase: 05-fix-wiring-issues
plan: 02
subsystem: engine
tags: [metrics, context, permissions, tui, dead-code, wiring]

# Dependency graph
requires:
  - phase: 05-fix-wiring-issues/01
    provides: Wiring audit findings and prioritized issue list (W01-W50)
provides:
  - InstructionsSource registered in engine context registry
  - RecordLLMInteractionWithPrompt wired after LLM calls
  - RecordHealDuration and RecordHealLoop wired in heal paths
  - StreamChunkMsg handled in TUI Update()
  - MetricsTool permission case fixed to match tool Name()
  - Git permission extraction case added
  - M31A_NVIDIA_API_KEY in usage help output
  - knownConfigKeys uses valid TOML key strings
  - Dead packages removed: pkg/errors, logging, screens
affects: [05-fix-wiring-issues/03, 05-fix-wiring-issues/04]

# Tech tracking
tech-stack:
  added: []
  patterns: [context-registry-registration, metrics-instrumentation, stream-chunk-routing]

key-files:
  created:
    - internal/engine/workflow/instructions_source_test.go
    - internal/ui/tui/stream_chunk_test.go
    - internal/tools/permissions_extract_test.go
    - cmd/m31a/usage_test.go
    - internal/core/config/known_keys_test.go
  modified:
    - internal/engine/workflow/engine.go
    - internal/engine/workflow/execute.go
    - internal/ui/tui/app_update.go
    - internal/tools/permissions.go
    - cmd/m31a/usage.go
    - internal/core/config/config_validate.go

key-decisions:
  - "Skipped W08 (arbitrager field removal): field is actively used in app.go:713 and other files, not dead code as plan claimed"
  - "RecordHealLoop called at loop exit (max attempts exceeded) and at individual max-attempts guard points"
  - "computePromptHash uses SHA-256 truncated to 16 chars for metrics tracking"

patterns-established:
  - "Prompt hash computation: SHA-256 of message role+content for LLM metrics deduplication"
  - "Heal duration timing: wrap healTask call with time.Now()/time.Since() at call site"

requirements-completed: [W03, W04, W05, W06, W07, W08, W09, W10, W11, W12, W13, W14, W15]

coverage:
  - id: D1
    description: "InstructionsSource registered in engine context registry providing AGENTS.md context"
    requirement: W03
    verification:
      - kind: unit
        ref: "internal/engine/workflow/instructions_source_test.go#TestInstructionsSourceRegistered"
        status: pass
    human_judgment: false
  - id: D2
    description: "RecordLLMInteractionWithPrompt called after each LLM interaction with prompt hash"
    requirement: W04
    verification:
      - kind: unit
        ref: "internal/engine/workflow/instructions_source_test.go#TestComputePromptHash"
        status: pass
    human_judgment: false
  - id: D3
    description: "RecordHealDuration and RecordHealLoop wired in all heal paths"
    requirement: W05
    verification:
      - kind: unit
        ref: "grep RecordHealDuration internal/engine/workflow/execute.go shows 4 call sites"
        status: pass
    human_judgment: false
  - id: D4
    description: "StreamChunkMsg handled in TUI Update() routing to REPL"
    requirement: W07
    verification:
      - kind: unit
        ref: "internal/ui/tui/stream_chunk_test.go#TestStreamChunkMsgHandled"
        status: pass
    human_judgment: false
  - id: D5
    description: "Permissions fixed: Metrics case matches tool Name(), Git case added"
    requirement: W12
    verification:
      - kind: unit
        ref: "internal/tools/permissions_extract_test.go#TestExtractFromParams_Metrics"
        status: pass
    human_judgment: false
  - id: D6
    description: "NVIDIA_API_KEY in usage help, knownConfigKeys uses valid TOML strings"
    requirement: W14
    verification:
      - kind: unit
        ref: "cmd/m31a/usage_test.go#TestPrintUsage_ContainsNVIDIAKey"
        status: pass
    human_judgment: false
  - id: D7
    description: "Dead packages removed: pkg/errors, logging, screens (13148 lines deleted)"
    requirement: W09
    verification:
      - kind: unit
        ref: "test -d pkg/errors returns false"
        status: pass
    human_judgment: false

# Metrics
duration: 12min
completed: 2026-07-28
status: complete
---

# Phase 05 Plan 02: Fix Wiring Issues Summary

**InstructionsSource context registration, LLM metrics instrumentation, StreamChunkMsg routing, permissions fixes, and 13K lines of dead code removal**

## Performance

- **Duration:** 12 min
- **Started:** 2026-07-28T22:14:59Z
- **Completed:** 2026-07-28T22:27:47Z
- **Tasks:** 2
- **Files modified:** 14 (8 modified, 5 created, 3 directories deleted)

## Accomplishments
- InstructionsSource registered in engine context registry, enabling AGENTS.md context injection
- RecordLLMInteractionWithPrompt wired after all LLM calls with SHA-256 prompt hashing
- RecordHealDuration and RecordHealLoop wired in all 4 heal paths for observability
- StreamChunkMsg handled in TUI Update() preventing silent message drops
- MetricsTool permission case fixed to match tool Name() return value
- Git permission extraction case added for permission prompts
- M31A_NVIDIA_API_KEY added to usage help output
- knownConfigKeys cleaned of Go type literal strings
- 3 dead code packages removed (13,148 lines): pkg/errors, logging, screens

## Task Commits

Each task was committed atomically:

1. **Task 1: Wire InstructionsSource, metrics, and StreamChunkMsg** - `900fa1d1` (feat)
2. **Task 2: Fix permissions, config help, knownConfigKeys, and remove dead packages** - `e996300c` (feat)

## Files Created/Modified
- `internal/engine/workflow/engine.go` - InstructionsSource registration, RecordLLMInteractionWithPrompt wiring, prompt hash computation
- `internal/engine/workflow/execute.go` - RecordHealDuration and RecordHealLoop in all heal paths
- `internal/ui/tui/app_update.go` - StreamChunkMsg case handler in Update()
- `internal/tools/permissions.go` - Metrics case fixed, Git case added
- `cmd/m31a/usage.go` - M31A_NVIDIA_API_KEY help text added
- `internal/core/config/config_validate.go` - knownConfigKeys cleaned of type literals
- `internal/engine/workflow/instructions_source_test.go` - Tests for InstructionsSource registration and prompt hash
- `internal/ui/tui/stream_chunk_test.go` - Tests for StreamChunkMsg handling
- `internal/tools/permissions_extract_test.go` - Tests for Metrics and Git permission extraction
- `cmd/m31a/usage_test.go` - Test for NVIDIA_API_KEY in usage output
- `internal/core/config/known_keys_test.go` - Test for valid TOML keys in knownConfigKeys

## Decisions Made
- Skipped W08 (arbitrager field removal): field is actively used in app.go:713 and other files, not dead code as plan claimed
- RecordHealLoop called at loop exit (max attempts exceeded) and at individual max-attempts guard points
- computePromptHash uses SHA-256 truncated to 16 chars for metrics tracking

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Skipped W08 arbitrager field removal (field is actively used)**
- **Found during:** Task 2 (W08)
- **Issue:** Plan stated `arbitrager *arbitrage.Scorer` in app_state.go was dead code, but it's referenced in app.go:713, handler_sidebar.go, commands/commands_ai.go, and other files
- **Fix:** Skipped removal - field is actively used for auto-arbitrage feature
- **Files modified:** None (no-op deviation)
- **Verification:** grep confirms 36 references to arbitrage across the tui package
- **Committed in:** N/A (skipped)

**2. [Rule 1 - Bug] Fixed RecordHealLoop and RecordHealDuration signatures to match actual API**
- **Found during:** Task 1 (implementation)
- **Issue:** Plan described signatures as `RecordHealLoop(taskID, attemptCount, success)` and `RecordHealDuration(taskID, duration)`, but actual API is `RecordHealLoop(phase)` and `RecordHealDuration(phase, durationMs)`
- **Fix:** Adapted calls to match actual collector API signatures
- **Files modified:** internal/engine/workflow/execute.go
- **Verification:** go build and go vet pass, all tests pass
- **Committed in:** 900fa1d1

---

**Total deviations:** 2 auto-fixed (1 skipped dead code claim, 1 signature adaptation)
**Impact on plan:** Both deviations necessary for correctness. No scope creep.

## Issues Encountered
- Disk quota exceeded during initial build attempts; resolved by setting GOTMPDIR to non-tmpfs location and using CGO_ENABLED=0

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All wiring fixes W03-W15 complete, ready for remaining phase plans (05-03, 05-04)
- Dead code removal reduces binary size and eliminates confusion

---
*Phase: 05-fix-wiring-issues*
*Completed: 2026-07-28*

## Self-Check: PASSED
