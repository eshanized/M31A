---
phase: 02
slug: user-experience
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-08-05
---

# Phase 02 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | go test |
| **Config file** | none — uses standard Go test infrastructure |
| **Quick run command** | `go test ./internal/ui/tui/... -count=1` |
| **Full suite command** | `go test ./... -count=1 -timeout 120s` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `go test ./internal/ui/tui/... -count=1`
- **After every plan wave:** Run `go test ./... -count=1 -timeout 120s`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 02-01-01 | 01 | 1 | UX-01 | — | Status bar renders phase, task, elapsed | unit | `go test ./internal/ui/tui/... -run TestFooter -count=1` | ⬜ W0 | ⬜ pending |
| 02-01-02 | 01 | 1 | UX-01 | — | Status bar shows primary operation only | unit | `go test ./internal/ui/tui/... -run TestFooter -count=1` | ⬜ W0 | ⬜ pending |
| 02-02-01 | 02 | 1 | UX-05 | T-02-02-01 | Risk labels render correctly | unit | `go test ./internal/ui/tui/components/ -run TestPermissionRiskLabel -count=1` | ⬜ W0 | ⬜ pending |
| 02-02-02 | 02 | 1 | UX-05, UX-06, UX-07 | T-02-02-01 | Auto-approve safe tools | unit | `go test ./internal/ui/tui/components/ -count=1 && go test ./internal/tools/ -run TestPermission -count=1` | ⬜ W0 | ⬜ pending |
| 02-03-01 | 03 | 2 | UX-02, UX-03 | — | Collapsible sections work | unit | `go test ./internal/ui/tui/... -run TestPlanModel -count=1` | ⬜ W0 | ⬜ pending |
| 02-03-02 | 03 | 2 | UX-04 | — | Current task highlighting | unit | `go test ./internal/ui/tui/... -run TestExecuteModel -count=1` | ⬜ W0 | ⬜ pending |
| 02-04-01 | 04 | 2 | UX-08 | — | Tutorial is skippable | unit | `go test ./internal/ui/tui/... -run TestTourModel -count=1` | ⬜ W0 | ⬜ pending |
| 02-04-02 | 04 | 2 | UX-08 | — | Tutorial flow complete | unit | `go test ./internal/ui/tui/... -run TestTourModel -count=1` | ⬜ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Existing test infrastructure covers all phase requirements

*If none: "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Status bar appearance | UX-01 | Visual layout verification | Launch TUI, verify status bar shows at bottom with phase/task/elapsed |
| Permission modal appearance | UX-05 | Visual layout verification | Trigger permission prompt, verify modal shows risk label |
| Collapsible section animation | UX-02 | Visual transition verification | Press arrow key on plan section, verify smooth collapse/expand |
| Tutorial flow | UX-08 | End-to-end user flow | Run first-time setup, complete tutorial steps |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
