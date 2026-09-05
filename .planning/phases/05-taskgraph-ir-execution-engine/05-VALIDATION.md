---
phase: "05"
slug: "taskgraph-ir-execution-engine"
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: "2026-09-05"
---

# Phase 05 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Go standard library `testing` + `github.com/stretchr/testify` (assert) |
| **Config file** | None — see Wave 0 |
| **Quick run command** | `go test ./internal/engine/taskrunner/... ./internal/core/types/... -count=1` |
| **Full suite command** | `make test` (race-enabled with coverage) |
| **Estimated runtime** | ~60 seconds |

---

## Sampling Rate

- **After every task commit:** Run `go test ./internal/engine/taskrunner/... ./internal/core/types/... -count=1`
- **After every plan wave:** Run `make test`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 01-01 | 05-01 | 1 | TASKGRAPH-01 | — | TaskGraphIR compiles with all fields | unit | `go test ./internal/core/types/... -run TestTaskGraphIR -count=1` | ❌ W0 | ⬜ pending |
| 01-02 | 05-01 | 1 | TASKGRAPH-01 | — | Plan.CompileToIR() produces valid IR | unit | `go test ./internal/core/types/... -run TestCompileToIR -count=1` | ❌ W0 | ⬜ pending |
| 01-03 | 05-01 | 1 | TASKGRAPH-02 | V5, V11 | ExecutionState.TransitionTo() validates transitions | unit | `go test ./internal/core/types/... -run TestExecutionState -count=1` | ❌ W0 | ⬜ pending |
| 01-04 | 05-01 | 1 | TASKGRAPH-02 | V5, V11 | Invalid transition returns error | unit | `go test ./internal/core/types/... -run TestInvalidTransition -count=1` | ❌ W0 | ⬜ pending |
| 02-01 | 05-02 | 2 | TASKGRAPH-03 | V11 | RETRY resets to READY, increments retry_count | unit | `go test ./internal/engine/taskrunner/... -run TestRetry -count=1` | ❌ W0 | ⬜ pending |
| 02-02 | 05-02 | 2 | TASKGRAPH-03 | V4, V7 | REPAIR calls provider, applies patch via FileEdit | integration | `go test ./internal/engine/taskrunner/... -run TestRepair -count=1` | ❌ W0 | ⬜ pending |
| 02-03 | 05-02 | 2 | TASKGRAPH-03 | V7 | REPLAN emits ReplanRequested event | unit | `go test ./internal/engine/taskrunner/... -run TestReplan -count=1` | ❌ W0 | ⬜ pending |
| 02-04 | 05-02 | 2 | TASKGRAPH-03 | V7 | ESCALATE emits EscalationRequested event | unit | `go test ./internal/engine/taskrunner/... -run TestEscalate -count=1` | ❌ W0 | ⬜ pending |
| 03-01 | 05-03 | 3 | TASKGRAPH-04 | — | Schedule() produces valid topological waves | unit | `go test ./internal/engine/taskrunner/... -run TestSchedule -count=1` | ✅ existing | ⬜ pending |
| 03-02 | 05-03 | 3 | TASKGRAPH-04 | V11 | ExecuteGroup respects max_parallel_tasks | unit | `go test ./internal/engine/taskrunner/... -run TestParallelism -count=1` | ❌ W0 | ⬜ pending |
| 03-03 | 05-03 | 3 | TASKGRAPH-04 | V12 | LockNames serialize same-lock tasks | unit | `go test ./internal/engine/taskrunner/... -run TestLockNames -count=1` | ❌ W0 | ⬜ pending |
| 04-01 | 05-04 | 4 | TASKGRAPH-05 | V4 | Checkpoint gate blocks wave until approval | integration | `go test ./internal/engine/taskrunner/... -run TestCheckpointGate -count=1` | ❌ W0 | ⬜ pending |
| 04-02 | 05-04 | 4 | TASKGRAPH-05 | V7, V8 | CheckpointApproved/Denied event persisted | integration | `go test ./internal/memory/eventstore/... -run TestCheckpointEvents -count=1` | ❌ W0 | ⬜ pending |
| 05-01 | 05-05 | 5 | TASKGRAPH-06 | V8, V11 | EventStore captures every TransitionTo() | integration | `go test ./internal/engine/taskrunner/... -run TestEventEmission -count=1` | ❌ W0 | ⬜ pending |
| 05-02 | 05-05 | 5 | TASKGRAPH-06 | V7 | Resume reconstructs exact task states | integration | `go test ./internal/engine/taskrunner/... -run TestResume -count=1` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `internal/core/types/planning_test.go` — TaskGraphIR, CompileToIR, ExecutionState, RecoveryAction tests
- [ ] `internal/engine/taskrunner/runner_test.go` — Extended Runner tests (ExecutionState, checkpoints, recovery, locks, events)
- [ ] `internal/engine/taskrunner/recovery_test.go` — RETRY, REPAIR, REPLAN, ESCALATE strategy tests
- [ ] `internal/engine/taskrunner/checkpoint_test.go` — Checkpoint gate, approval channel tests
- [ ] `internal/memory/eventstore/projection_test.go` — TaskGraphExecutionProjection for resume
- [ ] Framework: testify already in go.mod; no additional installs needed

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Human approval blocks execution at ONE_WAY_DOOR checkpoint via TUI S13 | TASKGRAPH-05 | Requires TUI interaction | Run `m31a execute` with plan containing ONE_WAY_DOOR checkpoint; verify TUI shows S13, blocks until approve, then continues |
| REPAIR patch applied correctly and retried | TASKGRAPH-03 | Requires provider API key + LLM behavior observation | Run `m31a execute` with failing task; observe REPAIR patch generation, application, retry |
| Resume after crash restores exact task states | TASKGRAPH-06 | Requires process kill/restart | Start execution, kill process mid-wave, restart `m31a`, verify exact task states restored |

*All other phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending