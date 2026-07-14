# Phase 3: Stabilization — Quick Reference

## Overview
- **Phase:** 03-stabilization
- **Total Tasks:** 16 across 4 waves
- **Estimated Time:** 55-85 minutes
- **Goal:** Resolve all blockers from HIGH_PRIORITY_VERIFICATION_REPORT.md

---

## Wave Summary

| Wave | Tasks | Focus | Parallel? | Time |
|------|-------|-------|-----------|------|
| 1 | 1-4 | Critical Regressions + Security | YES | 15-25 min |
| 2 | 5-8 | Security + High Correctness | NO (file overlaps) | 15-25 min |
| 3 | 9-12 | Lint + Vet + Medium Issues | YES | 10-15 min |
| 4 | 13-16 | Dead Code + Tests + Cleanup | YES | 15-20 min |

---

## Critical Issues (Must Fix)

### REGR-1: Duplicate Task Rendering
- **File:** `internal/tui/execute_model.go:285-357`
- **Issue:** Two loops render pending tasks twice
- **Fix:** Remove second loop, consolidate into single loop

### REGR-2: Data Race in Pause/Resume
- **File:** `internal/workflow/engine.go:270-289`
- **Issue:** `resumeCh` read without mutex
- **Fix:** Capture `resumeCh` under `pauseMu.Lock()`

### REGR-3: Double Permission Load
- **File:** `internal/tools/defaults.go:16-46`
- **Issue:** Permissions loaded twice
- **Fix:** Remove second load (lines 40-46)

### REGR-4: ModeAuto Termination
- **File:** `internal/tui/app_update_phase.go:17-53`
- **Issue:** `ModeAuto` not handled, workflow terminates
- **Fix:** Add `ModeAuto` handling to follow full workflow

---

## Security Issues (Must Fix)

### NEW-1: Git Argument Injection
- **File:** `internal/tools/git.go`
- **Issue:** No validation on git args
- **Fix:** Create `validateGitArgs()` with per-operation allowlists

### NEW-2: workdir Sandbox Bypass
- **File:** `internal/tools/bash.go:122-125`
- **Issue:** User-supplied workdir added to sandbox
- **Fix:** Validate workdir is within project directory

---

## Correctness Issues (Must Fix)

### NEW-3: workdir Path Resolution
- **File:** `internal/tools/bash.go:124`
- **Issue:** Relative paths resolve against CWD
- **Fix:** Resolve against `t.workDir`

### NEW-4: Commit Message Truncation
- **File:** `internal/tools/git.go:527-554`
- **Issue:** Only first word after `-m` captured
- **Fix:** Join all tokens after `-m`

---

## Lint/Vet Issues

### LINT-1: copylocks
- **File:** `emitter_stress_test.go:61`
- **Fix:** Use pointer receiver

### LINT-2: ineffassign
- **File:** `phase_transition_model.go:127`
- **Fix:** Remove unused `w` variable

### LINT-3,4,5: staticcheck
- **File:** `git.go:485,487,491`
- **Fix:** Use `fmt.Fprintf` instead of `WriteString(fmt.Sprintf(...))`

---

## Medium Issues

### NEW-5: Validation Error Color
- **File:** `config_model.go` or `settings_model.go`
- **Fix:** Change green to red

### NEW-6: Corrupt JSON Handling
- **File:** `persistent_permissions.go:66-71`
- **Fix:** Log error, backup file, return error

### NEW-7: Workflow Mode Race
- **File:** `engine.go:876`
- **Fix:** Protect reads with `RLock`

### NEW-8: esc/q Confirmation
- **File:** `execute_model.go:199-202`
- **Fix:** Show confirmation dialog

---

## Verification Commands

```bash
# Full verification
make check

# Lint only
make lint

# Tests with race detector
make test

# Build only
go build ./...

# Quick test
make test-fast
```

---

## Key Files

| File | Purpose |
|------|---------|
| `PLAN.md` | Full task details with acceptance criteria |
| `CONTEXT.md` | Implementation decisions and context |
| `EXECUTION_PLAN.md` | Detailed execution strategy |
| `QUICK_REFERENCE.md` | This file |

---

## Emergency Recovery

### If build fails:
```bash
goimports -w .
go build ./...
```

### If tests fail:
```bash
go test ./... -count=1 -v | head -100
```

### If lint fails:
```bash
golangci-lint run ./... 2>&1 | head -50
```

### If race detector fails:
```bash
go test ./... -race -count=1 2>&1 | grep -A5 "DATA RACE"
```

### To restart from specific wave:
```bash
/gsd-execute-phase 3 --wave 1  # Start from Wave 1
/gsd-execute-phase 3 --wave 2  # Start from Wave 2
/gsd-execute-phase 3 --wave 3  # Start from Wave 3
/gsd-execute-phase 3 --wave 4  # Start from Wave 4
```

### To run interactively:
```bash
/gsd-execute-phase 3 --interactive
```

---

*Quick Reference created: 2026-07-14*
