---
phase: 2
slug: tui-foundation
status: final
nyquist_compliant: true
wave_0_complete: true
created: 2026-05-27
updated: 2026-05-27
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | go test |
| **Config file** | none — standard Go `_test.go` files |
| **Quick run command** | `go test ./internal/tui/... -short` |
| **Full suite command** | `go test -race -cover ./internal/tui/...` |
| **Estimated runtime** | ~5 seconds |
| **Total tests** | ~110 |
| **Statement coverage** | 87.1% (tui), 95.7% (theme) |

---

## Sampling Rate

- **After every task commit:** Run `go test ./internal/tui/... -short`
- **After every plan wave:** Run `go test -race -cover ./internal/tui/...`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 02-01-01 | 01 | 1 | P2.2 | — | N/A | unit | `go test ./internal/tui/theme/... -run TestDarkTheme` | ✅ | ✅ green |
| 02-01-02 | 01 | 1 | P2.2 | — | N/A | unit | `go test ./internal/tui/theme/... -run TestLightTheme` | ✅ | ✅ green |
| 02-01-03 | 01 | 1 | P2.2 | — | N/A | unit | `go test ./internal/tui/theme/... -run "TestManager\|TestToolLabel\|TestDefault"` | ✅ | ✅ green |
| 02-02-01 | 02 | 2 | P2.6 | — | N/A | unit | `go test ./internal/tui/... -run TestRenderHeader` | ✅ | ✅ green |
| 02-02-02 | 02 | 2 | P2.7 | — | N/A | unit | `go test ./internal/tui/... -run TestRenderStatusBar` | ✅ | ✅ green |
| 02-02-03 | 02 | 2 | P2.4 | — | N/A | unit | `go test ./internal/tui/... -run "TestHealthCheck\|TestNextHealth\|TestCalc"` | ✅ | ✅ green |
| 02-03-01 | 03 | 2 | P2.3 | — | N/A | unit | `go test ./internal/tui/... -run TestNewRepl` | ✅ | ✅ green |
| 02-03-02 | 03 | 2 | P2.3 | — | N/A | unit | `go test ./internal/tui/... -run "TestReplModel_Enter\|TestReplModel_Window"` | ✅ | ✅ green |
| 02-03-03 | 03 | 2 | P2.3 | — | N/A | unit | `go test ./internal/tui/... -run "TestReplModel_View\|TestReplModel_Add\|TestReplModel_Get"` | ✅ | ✅ green |
| 02-04-01 | 04 | 2 | P2.5 | T-02-01 | Masked API key input | unit | `go test ./internal/tui/... -run TestFirstRun_KeyInput` | ✅ | ✅ green |
| 02-04-02 | 04 | 2 | P2.5 | T-02-01 | Key validation before storage | unit | `go test ./internal/tui/... -run TestFirstRun_Validation` | ✅ | ✅ green |
| 02-04-03 | 04 | 2 | P2.5 | — | N/A | unit | `go test ./internal/tui/... -run "TestFirstRun_Select\|TestFirstRun_View\|TestFirstRun_Esc"` | ✅ | ✅ green |
| 02-05-01 | 05 | 3 | P2.1 | — | N/A | unit | `go test ./internal/tui/... -run "TestNewApp\|TestApp_Init\|TestApp_CtrlC"` | ✅ | ✅ green |
| 02-05-02 | 05 | 3 | P2.1, P2.7 | — | N/A | unit | `go test ./internal/tui/... -run "TestApp_Screen\|TestApp_FirstRun\|TestApp_AppMsg"` | ✅ | ✅ green |
| 02-05-03 | 05 | 3 | P2.1 | — | N/A | unit | `go test ./internal/tui/... -run "TestApp_View\|TestApp_Terminal\|TestApp_Error"` | ✅ | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements (Resolved)

All Phase 2 implementation files are completed and tested. The following test files exist:

- [x] `internal/tui/theme/theme_test.go` — 8 tests for P2.2 (color values, Manager cycling, tool labels)
- [x] `internal/tui/app_test.go` — 20 tests for P2.1, P2.7 (screen routing, transitions, health tick, ctrl+c)
- [x] `internal/tui/repl_test.go` — 22 tests for P2.3 (input, history, resize, view rendering)
- [x] `internal/tui/health_test.go` — 11 tests for P2.4 (ticker creation, interval calculation)
- [x] `internal/tui/firstrun_test.go` — 23 tests for P2.5 (state machine, validation, keychain prompt)
- [x] `internal/tui/header_test.go` — 16 tests for P2.6 (brand, badges, context bar, health status, truncation)
- [x] `internal/tui/statusbar_test.go` — 10 tests for P2.7 (operation, timestamp, truncation)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Theme auto-detection | P2.2 | Requires terminal with known background | Start TUI with dark terminal, verify dark theme; repeat with light terminal |
| First-run screen layout | P2.5 | Visual layout verification | Start TUI without API key, verify welcome screen renders with all 4 options |
| Health badge live update | P2.4 | Requires real provider connection | Start TUI with valid API key, verify [LIVE] badge appears within 60s |

*The "Terminal too small" behavior was previously manual-only but is now covered by `TestApp_TerminalTooSmall` (verifies the "Terminal too small: 30x5" message and dimension display).*

*All other phase behaviors have automated verification.*

---

## Validation Audit 2026-05-27

| Metric | Count |
|--------|-------|
| Total requirements | 7 (P2.1–P2.7) |
| Automated tests | ~110 |
| Statement coverage | 87.1% |
| Gaps found | 2 |
| Resolved | 2 |
| Escalated | 0 |
| Manual-only | 3 (all correctly classified — require terminal or live connection) |

### Gaps Resolved
- **G-1**: Strengthened `TestApp_TerminalTooSmall` to assert the "Terminal too small" message content and dimension display
- **G-2**: Updated VALIDATION.md from draft to final; marked all tasks ✅ green, set `nyquist_compliant: true`

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 5s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
