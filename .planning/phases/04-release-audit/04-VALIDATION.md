---
phase: 4
slug: release-audit
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-14
---

# Phase 4 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Go standard `testing` package |
| **Config file** | none — uses `go test` flags |
| **Quick run command** | `make test-fast` (no race detector) |
| **Full suite command** | `make test` (race-enabled with coverage) |
| **Estimated runtime** | ~120 seconds |

---

## Sampling Rate

- **After every task commit:** Run `go test -short -count=1 -timeout=60s ./...`
- **After every plan wave:** Run `make check`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 04-01-01 | 01 | 1 | C1 | — | Zero `pkg/` → `internal/` imports | grep | `grep -r '"github.com/eshanized/M31A/internal' pkg/ --include='*.go'` | N/A | ⬜ pending |
| 04-01-02 | 01 | 1 | C1 | — | `pkg/errors/` exists with sentinel errors | grep | `grep -r '"github.com/eshanized/M31A/internal/errors' pkg/ --include='*.go'` | N/A | ⬜ pending |
| 04-01-03 | 01 | 1 | C1 | — | `pkg/narrative` does not import `internal/workflow` | grep | `grep -r '"github.com/eshanized/M31A/internal/workflow' pkg/ --include='*.go'` | N/A | ⬜ pending |
| 04-01-04 | 01 | 1 | C1 | — | All `pkg/` packages independently importable | build | `go build ./...` | N/A | ⬜ pending |
| 04-02-01 | 02 | 2 | C3 | T-4-01 | No data race on `e.provider` | race | `go test -race -count=1 -timeout=60s ./internal/workflow/...` | N/A | ⬜ pending |
| 04-02-02 | 02 | 2 | C2 | — | Test suites complete in 60s | integration | `go test -short -timeout=60s ./internal/tools/... ./pkg/bisect/...` | N/A | ⬜ pending |
| 04-02-03 | 02 | 2 | H1 | T-4-02 | Command blocklist comprehensive | unit | `go test -run TestBash_Dangerous -v ./internal/tools/...` | N/A | ⬜ pending |
| 04-02-04 | 02 | 2 | H2 | T-4-03 | Prompt injection defense | unit | `go test -run TestPromptInjection -v ./internal/tools/...` | N/A | ⬜ pending |
| 04-02-05 | 02 | 2 | H3 | T-4-04 | Sandbox failure surfaces warning | unit | `go test -run TestBash_Sandbox -v ./internal/tools/...` | N/A | ⬜ pending |
| 04-02-06 | 02 | 2 | H4 | T-4-05 | Subagent isolation defaults to worktree | unit | `go test -run TestSubagent_Isolation -v ./internal/tools/subagent/...` | N/A | ⬜ pending |
| 04-03-01 | 03 | 3 | H5 | T-4-06 | Error chains preserved | unit | `go test -run TestErrorChaining -v ./...` | N/A | ⬜ pending |
| 04-03-02 | 03 | 3 | H7 | — | Provider names use constants | grep | `grep -r '"openrouter"\|"zen"\|"nvidia"' --include='*.go' internal/ pkg/ | grep -v 'const\|Provider'` | N/A | ⬜ pending |
| 04-04-01 | 04 | 4 | H8 | — | `cmd/m31a` test coverage | coverage | `go test -cover ./cmd/m31a/...` | N/A | ⬜ pending |
| 04-04-02 | 04 | 4 | H8 | — | `internal/decision` test coverage | coverage | `go test -cover ./internal/decision/...` | N/A | ⬜ pending |
| 04-04-03 | 04 | 4 | M1 | — | Permission rule expiry/revocation | unit | `go test -run TestPermission_Expiry -v ./internal/tools/...` | N/A | ⬜ pending |
| 04-04-04 | 04 | 4 | M2 | T-4-07 | Unprotected engine fields fixed | race | `go test -race -count=1 -timeout=60s ./internal/workflow/...` | N/A | ⬜ pending |
| 04-04-05 | 04 | 4 | M3 | — | `LoadWorkflowState` file lock | race | `go test -race -count=1 -timeout=60s ./pkg/session/...` | N/A | ⬜ pending |
| 04-05-01 | 05 | 5 | ALL | — | Full verification suite | integration | `make check && make lint && make test` | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] DNS mock infrastructure for `internal/tools/websearch_test.go`
- [ ] DNS mock infrastructure for `internal/tools/webfetch_test.go`
- [ ] Mock git runner for `pkg/bisect/bisect_test.go` happy path
- [ ] Provider name constants in `internal/types/providers.go`
- [ ] `pkg/types/` package creation with shared types
- [ ] `pkg/errors/` package creation with sentinel errors

*If none: "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Prompt injection defense visible in system prompt | H2 | Requires LLM interaction to verify | Send tool output with injection attempt, verify delimiter wrapping |
| Command blocklist catches `$()` and backticks | H1 | Requires bash execution context | Run `$(echo test)` and verify blocked |
| Sandbox failure shows visible warning | H3 | Requires platform-specific testing | Test on unsupported platform, verify warning appears |

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
