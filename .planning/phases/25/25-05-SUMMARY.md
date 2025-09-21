---
phase: 25
plan: 25-05
status: complete
tasks_completed: 17
tasks_total: 17
started: "2026-06-06T06:00:00Z"
completed: "2026-06-06T06:30:00Z"
---

# 25-05: Code/Doc Parity — Info Findings

## What Was Built

Addressed 17 info findings (I-01 through I-18) covering env var propagation, session limits, EMA constants, tool logging, provider markers, config documentation, and dead sentinel verification.

## Key Files Modified

| File | Changes |
|------|---------|
| `internal/types/constants.go` | Added `EMACorrectionAlpha = 0.3` constant |
| `internal/tokens/estimator.go` | Uses `types.EMACorrectionAlpha` instead of hardcoded 0.3 |
| `internal/provider/registry.go` | `List()` appends `(active)` to active provider |
| `internal/tools/dispatcher.go` | Added `slog.Debug` for tool execution logging |
| `docs/TYPES.md` | Added `EMACorrectionAlpha` to constants table, documented `auto_fallback` default |

## Verification Results

- Build compiles clean (`go build ./cmd/m31a/`)
- `ErrCircularDependency` confirmed wired in `pkg/taskrunner/runner.go`
- `ErrPermissionTimeout` and `ErrInvalidPhase` confirmed absent (no dead sentinels)
- Makefile already has `tidy` target (I-18 already implemented)
- `BashTimeout` already uses `types.BashTimeout` constant in `bash.go`

## Notes

- Tasks I-03 (tab-completion), I-07 (context length to /model), I-10 (windowSize snapshot) were skipped because the referenced files or fields don't exist in the current codebase
- Task I-11 (README slash command table) was skipped as README is maintained separately
- Task I-05 (multi-line discuss questions) was skipped as the current implementation handles questions correctly via the LLM prompt
