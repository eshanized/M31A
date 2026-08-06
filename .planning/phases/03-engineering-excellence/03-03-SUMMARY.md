---
phase: 03-engineering-excellence
plan: 03
subsystem: infra
tags: [slog, pprof, debug, profiling, ci, goreleaser]

# Dependency graph
requires:
  - phase: 03-engineering-excellence
    provides: Engine decomposition into focused files
provides:
  - Debug logging with slog across all engine files
  - pprof profiling endpoint on localhost:6060
  - Enhanced Makefile with debug/profile/lint-full targets
  - CI workflow with debug-build and pprof-smoke jobs
affects: [04-ux, 05-security]

# Tech tracking
tech-stack:
  added: [net/http/pprof]
  patterns: [structured-logging, debug-gating, localhost-only-profiling]

key-files:
  created: []
  modified:
    - cmd/m31a/main.go
    - internal/engine/workflow/engine.go
    - internal/engine/workflow/engine_pause.go
    - internal/engine/workflow/engine_streaming.go
    - internal/engine/workflow/engine_checkpoint.go
    - internal/engine/workflow/engine_model.go
    - internal/engine/workflow/engine_helpers.go
    - .github/workflows/ci.yml
    - Makefile

key-decisions:
  - "pprof bound to localhost only to prevent external access"
  - "Log level priority: --log-level flag > --debug flag > M31A_LOG_LEVEL env > default (info)"
  - "Standalone helper functions retain bare slog calls (no Engine receiver access)"

patterns-established:
  - "Structured logging: all Engine methods use e.logger with consistent camelCase field names"
  - "Debug gating: pprof and debug logging gated behind --debug flag or M31A_DEBUG=1"

requirements-completed: [DX-01, DX-02, DX-03]

coverage:
  - id: D1
    description: "pprof debug endpoint on localhost:6060 gated behind --debug flag"
    requirement: DX-02
    verification:
      - kind: integration
        ref: "M31A_DEBUG=1 timeout 5 go run ./cmd/m31a/ --debug 2>&1 | grep pprof"
        status: pass
    human_judgment: false
  - id: D2
    description: "Structured slog logging across all engine files with consistent fields"
    requirement: DX-01
    verification:
      - kind: unit
        ref: "go test -race ./internal/engine/workflow/... -count=1"
        status: pass
    human_judgment: false
  - id: D3
    description: "Enhanced Makefile with debug, profile, and lint-full targets"
    requirement: DX-03
    verification:
      - kind: automated_ui
        ref: "make help | grep -E 'debug|profile|lint-full'"
        status: pass
    human_judgment: false
  - id: D4
    description: "CI workflow with debug-build and pprof-smoke jobs"
    requirement: DX-03
    verification:
      - kind: other
        ref: "grep -c 'debug-build|pprof-smoke' .github/workflows/ci.yml"
        status: pass
    human_judgment: false

# Metrics
duration: 10min
completed: 2026-08-06
status: complete
---

# Phase 3 Plan 3: Developer Experience Summary

**pprof debug endpoint on localhost:6060, structured slog logging across all engine files, enhanced Makefile with debug/profile/lint-full targets, CI workflow with debug-build and pprof-smoke jobs**

## Performance

- **Duration:** 10 min
- **Started:** 2026-08-06T00:55:53Z
- **Completed:** 2026-08-06T01:06:05Z
- **Tasks:** 3
- **Files modified:** 9

## Accomplishments
- pprof profiling endpoint available on localhost:6060 when --debug flag or M31A_DEBUG=1 is set
- pprof bound to localhost only to prevent external access in production
- Structured slog logging with consistent camelCase fields across all engine files
- Debug-level logging for pause/resume, streaming retry, model selection, project caching
- Info-level logging for checkpoint save/load, recovery, rollback
- Enhanced Makefile with profile and lint-full targets
- CI workflow now verifies debug binary compiles and pprof endpoint responds

## Task Commits

Each task was committed atomically:

1. **Task 1: Tracer - Add pprof debug endpoint and log level configuration** - `3eb2389d` (feat)
2. **Task 2: Add structured slog logging across all engine files** - `937311f7` (feat)
3. **Task 3: Enhance release automation and CI workflow** - `131debe5` (feat)

## Files Created/Modified
- `cmd/m31a/main.go` - Added --debug, --log-level flags, pprof server startup, log level configuration
- `internal/engine/workflow/engine.go` - Replaced bare slog calls with e.logger
- `internal/engine/workflow/engine_pause.go` - Added debug logging for pause/resume/skip/cancel
- `internal/engine/workflow/engine_streaming.go` - Updated stream iterator close logging to use e.logger
- `internal/engine/workflow/engine_checkpoint.go` - Added info logging for checkpoint save/load/recovery/rollback
- `internal/engine/workflow/engine_model.go` - Added debug logging for model selection
- `internal/engine/workflow/engine_helpers.go` - Added debug logging for project cache loading
- `.github/workflows/ci.yml` - Added debug-build and pprof-smoke CI jobs
- `Makefile` - Added profile and lint-full targets

## Decisions Made
- pprof bound to localhost only to prevent external access (threat T-03-05 mitigation)
- Log level priority: --log-level flag > --debug flag > M31A_LOG_LEVEL env > default (info)
- Standalone helper functions (finalizeToolCalls) retain bare slog calls since they lack Engine receiver access

## Deviations from Plan

### Auto-fixed Issues

None - plan executed exactly as written.

**Total deviations:** 0 auto-fixed
**Impact on plan:** No scope creep. All three DX improvements implemented per D-07.

## Issues Encountered
- Test failures in TestExtractWebsiteTemplateTo due to disk quota exceeded (pre-existing, unrelated to this plan)

## User Setup Required
None - no external service configuration required.

## Known Stubs
- `finalizeToolCalls` in engine_streaming.go uses bare `slog.Warn` instead of `e.logger` because it's a standalone function without Engine receiver access. This is intentional and documented.

## Next Phase Readiness
- Debug logging, profiling, and release automation complete
- Contributors can now debug issues using structured logs and profiles
- CI verifies debug builds and pprof endpoint respond correctly

---
*Phase: 03-engineering-excellence*
*Completed: 2026-08-06*

## Self-Check: PASSED
