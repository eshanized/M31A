---
phase: 01
slug: reliability-first
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-08-05
---

# Phase 01 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Go testing (stdlib) |
| **Config file** | go.mod |
| **Quick run command** | `make test-fast` |
| **Full suite command** | `make test` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `make test-fast`
- **After every plan wave:** Run `make test`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01 | 1 | REL-01 | — | N/A | unit | `go test -race ./internal/engine/workflow/...` | ✅ | ⬜ pending |
| 01-02-01 | 01 | 1 | REL-02 | — | N/A | integration | `go test -race ./internal/tools/...` | ❌ Wave 0 | ⬜ pending |
| 01-03-01 | 01 | 2 | REL-03 | — | N/A | integration | `go test -race ./internal/tools/exec/...` | ❌ Wave 0 | ⬜ pending |
| 01-04-01 | 01 | 2 | REL-04 | — | N/A | unit | `go test ./internal/...` | ❌ Wave 0 | ⬜ pending |
| 01-05-01 | 01 | 3 | REL-05 | — | N/A | integration | `go test -race ./internal/engine/workflow/... -run Pause` | ❌ Wave 0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `internal/engine/workflow/pause_resume_test.go` — covers REL-05 (pause/resume integration)
- [ ] `internal/tools/dispatcher_edge_test.go` — covers dispatcher edge cases
- [ ] `internal/engine/workflow/cancellation_test.go` — covers REL-02, REL-03 (context propagation)
- [ ] `internal/engine/workflow/error_handling_test.go` — covers REL-04 (error wrapping)

*Existing infrastructure covers mutex refactoring tests (REL-01) via engine_race_test.go.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| None | — | — | — |

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
