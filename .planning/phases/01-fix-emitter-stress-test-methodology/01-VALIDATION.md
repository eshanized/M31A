---
phase: 01
slug: fix-emitter-stress-test-methodology
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-11
---

# Phase 01 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `go test` (stdlib) |
| **Config file** | none — standard `go test` |
| **Quick run command** | `go test -run TestEmitterDropsUnderStreamingLoad ./internal/tui/ -v -count=1` |
| **Full suite command** | `make test` (race-enabled, coverage) |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `go test -run TestEmitterDropsUnderStreamingLoad ./internal/tui/ -count=1`
- **After every plan wave:** Run `go test ./internal/tui/ -count=1`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01 | 1 | REQ-001.1 | — | N/A (research) | analysis | N/A (RESEARCH.md) | ✅ W0 | ⬜ pending |
| 01-01-01 | 01 | 1 | REQ-001.2 | T-01-01, T-01-02 | Drain goroutine runs at calibrated rate, reuses production drainAdaptiveCmd | unit | `go test -run TestEmitterPositiveControl ./internal/tui/ -v` | ❌ W0 | ⬜ pending |
| 01-01-02 | 01 | 1 | REQ-001.3 | T-01-02 | Positive control shows DroppedMessages() > 0 without drain | unit | `go test -run TestEmitterPositiveControl ./internal/tui/ -v` | ❌ W0 | ⬜ pending |
| 01-01-02 | 01 | 1 | REQ-001.4 | T-01-01 | Streaming scenario runs all 7 phases with real-rate drain, reports drops | unit | `go test -run TestEmitterStreamingScenario ./internal/tui/ -v` | ❌ W0 | ⬜ pending |
| 01-02-01 | 02 | 2 | REQ-001.5 | T-02-01 | Tool-burst test runs 8 concurrent tools, reports drops separately | unit | `go test -run TestEmitterToolBurstScenario ./internal/tui/ -v` | ❌ W0 | ⬜ pending |
| 01-03-01 | 03 | 3 | REQ-001.6 | T-02-01 | Audit report v2 created with actual values, supersedes v1 | doc | `cat docs/audits/emitter-stress-test-results-v2.md` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `internal/tui/emitter_stress_test.go` — rewritten with drain goroutine and positive control
- [ ] `internal/tui/emitter_stress_test.go` — new test: TestEmitterStreamingScenario
- [ ] `internal/tui/emitter_stress_test.go` — new test: TestEmitterToolBurstScenario
- [ ] `docs/audits/emitter-stress-test-results-v2.md` — audit report

*If none: "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Audit report supersedes v1 | REQ-001.6 | Document content review | Compare `docs/audits/emitter-stress-test-results-v2.md` against v1; verify all placeholders replaced with actual test output |
| Verdict clarity | REQ-001.6 | Human judgment | Verify report states clear YES/NO on 512/4 adequacy |

*If none: "All phase behaviors have automated verification."*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending