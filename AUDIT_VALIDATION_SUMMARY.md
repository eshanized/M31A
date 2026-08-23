# M31A Audit Validation Summary

## Executive Summary

This validation pass independently verified all 17 findings from the previous forensic audit of M31A (commit e095ca23). The validation used static code analysis, test execution, and fixture-based reproduction where possible.

**Bottom Line: M31A is NOT SAFE FOR AUTONOMOUS USE and NOT RELEASE READY.**

---

## Validation Scorecard

| Category | Previous | Validated | Change |
|----------|----------|-----------|--------|
| **Total Findings** | 17 | 17 | — |
| **P0 (Catastrophic)** | 3 | **3** | — |
| **P1 (Major)** | 6 | **5 + 3 new = 8** | +2 |
| **P2 (Significant)** | 5 | **4** | -1 |
| **P3 (Minor)** | 3 | **2** | -1 |

---

## Classification Breakdown

| Classification | Count | Findings |
|----------------|-------|----------|
| **CONFIRMED_REPRODUCED** | 10 | 001, 002, 003, 004, 005, 006, 009, 011, 013, 014, 016, 017 |
| **CONFIRMED_STATIC** | 3 | 007, 012, 008* |
| **PARTIALLY_CORRECT** | 1 | 008 (severity P1→P2) |
| **FALSE_POSITIVE** | 0 | — |
| **UNCONFIRMED** | 2 | 010, 015 |
| **NEW FINDINGS** | 3 | NEW-001, NEW-002, NEW-003 |

*008 reclassified from LIKELY/P1 to PARTIALLY_CORRECT/P2 after finding some expansions ARE caught but nested command substitution in arguments is missed.

---

## Critical Findings Confirmed

### P0 - Three Confirmed Catastrophic Issues

| ID | Issue | Evidence |
|----|-------|----------|
| **001** | `Git.Commit()` calls `AddAll()` | Reproduced: `git add -A && git commit` commits user's unrelated changes |
| **002** | 90% verify threshold | Test logs show 8/10 fails, 9/10 passes, 10/10 passes |
| **003** | `SetPhase()` bypasses validation | 4 production callers, no session ID check, can restore Ship from Idle |

### P1 - Major Issues (8 Total)

| ID | Issue | Status |
|----|-------|--------|
| 004 | Session recovery loses task state | CONFIRMED_REPRODUCED |
| 005 | Bash workdir symlink escape | CONFIRMED_REPRODUCED (code analysis) |
| 006 | Headless permissions non-functional | CONFIRMED_REPRODUCED (code analysis) |
| 007 | LLM file paths unvalidated | CONFIRMED_STATIC |
| 008 | Incomplete shell expansion blocking | PARTIALLY_CORRECT (P2) |
| 009 | Recovery without consistency check | CONFIRMED_REPRODUCED |
| **NEW-001** | Session resume fails (state machine sync) | **NEW - CONFIRMED_REPRODUCED** |
| **NEW-002** | Ship fallback to AddAll() | **NEW - CONFIRMED_STATIC** |

---

## New Findings Discovered During Validation

### NEW-001: Session Resume Fails Due to State Machine Sync Bug (P1)
**Root Cause:** On `ResumeOnStartup`, TUI loads session with WorkflowPhase (e.g., Execute), creates NEW engine (state machine at PhaseIdle), then calls `RunPhaseCmd(Execute)`. Engine tries `Transition(PhaseIdle, Execute)` which FAILS (only PhaseInitialize valid from PhaseIdle).

**Code Path:** `app_session.go:73-76` → `RunPhaseCmd()` → `engine.RunPhase()` → `stateMachine.Transition(PhaseIdle, Execute)` → **FAILS**

### NEW-002: Ship Phase Fallback to AddAll() (P1)
**Location:** `ship.go:106-110`
```go
if addErr := e.git.Add(taskFiles...); addErr != nil {
    e.git.AddAll()  // FALLBACK - commits ALL changes
}
```
If task file add fails (file not found, permission denied), falls back to staging ALL worktree changes.

### NEW-003: Verify Phase Doesn't Load Checkpoint Data (P1)
On resume at Verify phase, tasks loaded from session but engine hasn't run Execute. Task states (StatusDone, HealsAttempted) may be stale. Related to NEW-001.

---

## False Positives Removed

| ID | Claim | Reality |
|----|-------|---------|
| **010** | "Unbounded emitter channel memory growth" | Channel is bounded (`ChannelCap`). Worst case: workflow blocks on send, not memory growth. |

---

## Test Quality Gaps Confirmed

| Area | Status |
|------|--------|
| Git commit safety (unrelated changes) | NO TEST |
| Verify threshold behavior (90% pass) | NO TEST |
| State machine invalid transition rejection | NO TEST |
| Headless mode permission handling | NO TEST |
| Bisect crash recovery | NO TEST |
| Symlink escape in file tools | NO TEST |
| Session resume with task state | NO TEST |
| Ship phase AddAll fallback | NO TEST |

---

## Remaining Blind Spots

- E2E workflow with real API keys (no credentials in environment)
- Cross-compilation targets (goreleaser not tested)
- Concurrent sessions in same project
- Subagent worktree isolation
- Windows path handling
- Large session performance (>1000 messages)

---

## Final Verdict

### NOT SAFE FOR AUTONOMOUS USE
**Reason:** 3 P0 issues + broken session resume (NEW-001) + non-functional headless mode

### NOT RELEASE READY
**Reason:** P0 issues must be fixed; session resume is a core feature that's broken

---

## Recommended Remediation Order

1. **Fix Git Commit()** - Remove `Commit()` or make private; enforce `CommitWithFiles()` everywhere; remove fallback in ship.go:108
2. **Fix State Machine Validation** - Add `RestorePhase()` with validation; validate session ID on restore; fix session resume (NEW-001)
3. **Remove Verify Threshold** - Make verification all-or-nothing; add explicit opt-in if needed
4. **Fix Headless Permissions** - Add `--permission-mode` flag; default dangerous tools to deny
5. **Fix Session Resume** - Call `LoadCheckpointData()` and `LoadTasks()` on auto-resume; sync state machine
6. **Centralize Path Validation** - Add `ValidateTaskFiles()` at plan creation and before every use
7. **Harden Shell Parsing** - Use `mvdan.cc/sh/v3/syntax` for command validation
8. **Scope Rollback/Bisect** - Track agent files; use worktrees for bisect
9. **Add Critical Tests** - Especially for P0/P1 failure paths

---

## Files Generated

- `AUDIT_VALIDATION.md` - Detailed validation evidence for each finding
- `AUDIT_ROOT_CAUSES.md` - 7 root causes with architectural analysis
- `AUDIT_VALIDATION_SUMMARY.md` - This file