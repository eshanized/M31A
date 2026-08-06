---
plan: 04-05
name: Parallel CodeIntel Indexing
phase: 04
subsystem: codeintel
tags: [performance, parallelism, indexing]
requires: [04-02]
provides: [parseFilesParallel]
affects: [internal/integrations/codeintel]
tech-stack:
  added: []
  patterns: [semaphore, fan-out]
key-files:
  created: []
  modified:
    - internal/integrations/codeintel/codeintel.go
    - internal/integrations/codeintel/bench_test.go
key-decisions:
  - D-09: All operations get concurrent execution
  - D-10: Semaphore-based concurrency control
  - D-11: Reuse existing task runner patterns
  - D-12: Collect all errors, report together
duration: 5min
completed: "2026-08-06T03:21:00Z"
coverage:
  - deliverable: parseFilesParallel with semaphore-bounded goroutines
    verification:
      - kind: test
        ref: go test ./internal/integrations/codeintel/... -count=1
        status: pass
        human_judgment: false
  - deliverable: Error collection via errors.Join
    verification:
      - kind: test
        ref: go vet ./internal/integrations/codeintel/...
        status: pass
        human_judgment: false
---

# Phase 4 Plan 05: Parallel CodeIntel Indexing Summary

Adds concurrent file parsing to CodeIntel with semaphore-bounded goroutines (NumCPU workers) — automatically uses parallel path for 10+ files, sequential fallback for smaller sets.

## Accomplishments

- Added `parseFilesParallel()` using goroutines bounded by `runtime.NumCPU()`
- All parse errors collected via `errors.Join` and returned together (D-12)
- Parallel parsing for file sets >= 10, sequential fallback for smaller sets
- Updated `buildFromCache` and `buildIncremental` to dispatch to parallel path
- Added `BenchmarkParseFilesSequential/Parallel` benchmarks

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None

## Self-Check: PASSED
