---
phase: 5
phase-slug: fix-wiring-issues
date: 2026-07-29
---

# VALIDATION.md — Phase 5: Fix Wiring Issues

## Validation Architecture

### Dimension 1: Functional Correctness
- Config merge covers all ~30 missing fields across 6 sections
- Rollback.SoftReset called after bisect failure
- InstructionsSource injects into LLM context
- Metrics methods record actual data
- StreamChunkMsg renders in REPL
- Permission extraction matches tool names correctly
- Zen provider retries on transient errors
- FallbackPriority validates provider names

### Dimension 2: Integration
- Each wiring fix verified by integration test
- Tests fail on current code, pass after fix
- CI remains green: go build, go vet, golangci-lint, go test -race

### Dimension 3: Regression
- Existing tests continue to pass
- No new test regressions introduced
- One commit per fix for clean bisect

### Dimension 4: Edge Cases
- Config merge handles zero-value overrides correctly
- Rollback handles missing git history gracefully
- Zen retry respects context cancellation
- Permission extraction handles missing params

### Dimension 5: Performance
- Config merge adds no measurable overhead
- Zen retry delays are bounded (max 2 retries, exponential backoff)
- StreamChunkMsg handling is non-blocking

### Dimension 6: Security
- Config validation rejects invalid provider names
- Permission extraction covers all registered tools
- No new attack surface introduced

### Dimension 7: Documentation
- WIRING_ISSUES.md updated to reflect resolved status
- Each fix documented in commit message

### Dimension 8: Test Coverage
- Integration tests for all wiring fixes (W01-W07, W12-W13, W18-W19, W26-W27, W29)
- Unit tests for dead code removal verification
- make test-fast per task, make test per wave, make check per phase gate
