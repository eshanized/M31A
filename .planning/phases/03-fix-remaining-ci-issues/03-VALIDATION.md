---
phase: 3
slug: fix-remaining-ci-issues
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-27
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Go stdlib `testing` (Go 1.26.5) |
| **Config file** | None (Makefile targets) |
| **Quick run command** | `make test-specific TEST=<TestName>` |
| **Full suite command** | `make test` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `make lint` (lint fix) or `make test-specific TEST=<name>` (test fixes)
- **After every plan wave:** Run `go test -race -timeout 30s ./...`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 03-01-01 | 01 | 1 | REQ-01 | — | N/A | lint | `make lint` | N/A (CI job) | ⬜ pending |
| 03-01-02 | 01 | 1 | REQ-02 | — | N/A | unit | `make test-specific TEST=TestRegistry_Execute_PhaseAliases` | ✅ | ⬜ pending |
| 03-01-03 | 01 | 1 | REQ-02 | — | N/A | unit | `make test-specific TEST=TestAskUserQuestion_ChannelFull` | ✅ | ⬜ pending |
| 03-01-04 | 01 | 1 | REQ-04 | — | N/A | security | `govulncheck ./...` | N/A (CI job) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `internal/core/types/fileutil.go` — lint fix (replace `os.SEEK_SET` with `io.SeekStart`)
- [x] `internal/ui/tui/commands/commands_all_test.go` — test fix (initialize session.Manager properly)
- [x] `internal/tools/extra_test.go` — test fix (context cancellation for channel-full test)
- [x] `go.mod` — security fix (update goldmark dependency)

*Existing infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| CI pipeline passes on GitHub Actions | REQ-05 | Requires push to GitHub and CI run | Push to master, observe CI workflow completion |

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
