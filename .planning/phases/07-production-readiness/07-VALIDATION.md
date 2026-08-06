---
phase: 7
slug: production-readiness
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-08-06
---

# Phase 7 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Go standard library `testing` |
| **Config file** | none — use `go test` directly |
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
| 07-01-01 | 01 | 1 | OBS-01 | — | N/A | unit | `go test ./internal/integrations/metrics/...` | ✅ existing | ⬜ pending |
| 07-01-02 | 01 | 1 | CRASH-01 | T-07-01 | Panic recovery prevents DoS | unit | `go test ./internal/observability/...` | ❌ Wave 0 | ⬜ pending |
| 07-01-03 | 01 | 1 | VALID-01 | — | N/A | integration | `make test` | ✅ existing | ⬜ pending |
| 07-01-04 | 01 | 1 | RELEASE-01 | — | N/A | integration | `goreleaser release --snapshot` | ✅ existing | ⬜ pending |
| 07-01-05 | 01 | 1 | LTS-01 | — | N/A | unit | `go test ./internal/core/...` | ✅ existing | ⬜ pending |
| 07-01-06 | 01 | 1 | COMPAT-01 | — | N/A | e2e | `scripts/compat-matrix.sh` | ❌ Wave 0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `internal/observability/crash.go` — panic recovery handler stubs for CRASH-01
- [ ] `internal/observability/crash_test.go` — crash handler tests
- [ ] `scripts/compat-matrix.sh` — compatibility test runner stub
- [ ] `.github/ISSUE_TEMPLATE/crash-report.md` — crash report template

*Existing infrastructure covers most phase requirements. Wave 0 adds crash handler and compat matrix runner.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Benchmark dashboard renders correctly | OBS-01 | Requires browser verification | Push to gh-pages, verify chart renders |
| Release draft looks correct | RELEASE-01 | Requires manual review | Run `goreleaser release --snapshot`, inspect dist/ |

*If none: "All phase behaviors have automated verification."*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
