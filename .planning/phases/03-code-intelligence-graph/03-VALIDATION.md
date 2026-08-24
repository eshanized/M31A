---
phase: 03
slug: code-intelligence-graph
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-08-24
---

# Phase 03 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Go standard testing + testify |
| **Config file** | go.mod (existing) |
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
| 03-01-01 | 01 | 1 | CODE-01 | T-03-01 | Validate all paths through `ValidateTaskFiles()` | unit | `go test ./internal/integrations/codeintel/... -run TestParser -x` | ✅ | ⬜ pending |
| 03-01-02 | 01 | 1 | CODE-03 | — | N/A | unit | `go test ./internal/integrations/codeintel/... -run TestGraphEntities -x` | ❌ W0 | ⬜ pending |
| 03-02-01 | 02 | 1 | CODE-02 | T-03-02 | Validate LSP server command from config | integration | `go test ./internal/integrations/lsp/... -run TestLSPClient -x` | ❌ W0 | ⬜ pending |
| 03-02-02 | 02 | 2 | CODE-04 | — | N/A | unit | `go test ./internal/integrations/codeintel/... -run TestIncremental -x` | ✅ | ⬜ pending |
| 03-03-01 | 03 | 2 | CODE-05 | — | N/A | unit | `go test ./internal/integrations/codeintel/... -run TestImpact -x` | ❌ W0 | ⬜ pending |
| 03-03-02 | 03 | 2 | CODE-06 | — | N/A | unit | `go test ./internal/integrations/archcheck/... -run TestViolation -x` | ❌ W0 | ⬜ pending |
| 03-04-01 | 04 | 3 | CODE-07 | — | N/A | integration | `go test ./internal/integrations/codeintel/... -run TestMultiRepo -x` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `internal/integrations/lsp/` — new package, all tests needed
- [ ] `internal/integrations/archcheck/` — new package, all tests needed
- [ ] `internal/integrations/codeintel/events.go` — new file, event emission tests
- [ ] `internal/integrations/codeintel/projection.go` — new file, projection rebuild tests
- [ ] `internal/integrations/codeintel/impact.go` — new file, impact analysis tests
- [ ] `cmd/m31a/index.go` — new CLI command, integration tests
- [ ] `cmd/m31a/impact.go` — new CLI command, integration tests
- [ ] `cmd/m31a/arch.go` — new CLI command, integration tests

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| LSP server process injection prevention | CODE-02 | Security-critical behavior requiring config validation | Verify LSP server command validated from config; reject arbitrary commands |
| Path traversal via malicious import paths | CODE-01 | Security-critical behavior requiring path validation | Verify all paths go through `ValidateTaskFiles()` with `EvalSymlinks` |

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
