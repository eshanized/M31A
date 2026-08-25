---
phase: 4
slug: intelligence-features
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-08-25
---

# Phase 4 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | go test (standard library); race coverage via targeted `go test -race ./internal/intelligence/<pkg>/` — NOTE `make test-fast` runs WITHOUT `-race`; final gates `make test` / `make check` ARE race-enabled |
| **Config file** | none — Wave 0 installs |
| **Quick run command** | `make test-fast` |
| **Full suite command** | `make test` |
| **Estimated runtime** | ~60 seconds |

---

## Sampling Rate

- **After every task commit:** Run `make test-fast`
- **After every plan wave:** Run `make test`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 90 seconds

---

## Per-Task Verification Map

Waves reflect the post-revision dependency graph (04-04 gained `depends_on: ["04-02"]` for `git.ValidateRef`, cascading 04-05 → wave 4 and 04-07 → wave 5 via `cmd/m31a/main.go` file ownership).

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 04-01-T1 | 01 | 1 | EXPLAIN-02 | — | N/A (shared types) | unit | `go test ./internal/core/types/ ./internal/core/errors/` | ❌ W0 | ⬜ pending |
| 04-01-T2 | 01 | 1 | EXPLAIN-02 | — | Config validation bounds (bisect_max_commits ≥ 1) | unit | `go test ./internal/core/config/ -run TestIntelligence -v && go test ./internal/core/config/` | ❌ W0 | ⬜ pending |
| 04-02-T1 | 02 | 2 | EXPLAIN-01 | T-04-02b | Hostile git refs rejected before exec; exported ValidateRef guard | unit (real git fixture) | `go test ./internal/integrations/git/ -run TestBlamePorcelain -v && go test ./internal/integrations/git/` | ❌ W0 | ⬜ pending |
| 04-02-T2 | 02 | 2 | EXPLAIN-01 | T-04-02b | Read-only evidence collection; deterministic dedup + budget trim | unit (seeded repo) | `go test ./internal/intelligence/explain/ -run TestCollector -v` | ❌ W0 | ⬜ pending |
| 04-02-T3 | 02 | 2 | EXPLAIN-02 | T-04-02a | Pack-as-data prompt; marker validation strips unknown citations | e2e tracer (mock LLM) | `go test ./internal/intelligence/explain/ ./cmd/m31a/ && make build && ./m31a explain 2>&1 \| grep -ci usage && { ./m31a explain DefinitelyNotASymbolQxz99 2>&1 \|\| true; } \| grep -ci "not found\|scope"` | ❌ W0 | ⬜ pending |
| 04-03-T1 | 03 | 3 | EXPLAIN-03 | — | Verdict class from deterministic signals only (D-08) | unit | `go test ./internal/intelligence/explain/ -run "TestSignals\|TestVerdictClass" -v` | ❌ W0 | ⬜ pending |
| 04-03-T2 | 03 | 3 | EXPLAIN-04 | T-04-02b | Read-only archaeology; ADR scan degrades silently when absent | integration (temp repo) | `go test ./internal/intelligence/explain/ -run "TestFileMode\|TestTopicMode\|TestScanADRs\|TestResolveMode" -v` | ❌ W0 | ⬜ pending |
| 04-03-T3 | 03 | 3 | EXPLAIN-03, EXPLAIN-04 | T-04-02a | Help text documents disambiguation precedence | e2e smoke (credential-gated) | `go test ./internal/intelligence/explain/ ./cmd/m31a/ && make build && if [ -n "$NVIDIA_API_KEY" ]; then ./m31a explain internal/core/types/event.go --format json \| head -8; else echo "SKIP: NVIDIA_API_KEY unset"; fi` | ❌ W0 | ⬜ pending |
| 04-04-T1 | 04 | 3 | REGRESS-01 | T-04-04b, T-04-04c | User checkout never moves; hostile refs rejected pre-exec; orphan prune sweep | integration (real git) | `go test ./internal/intelligence/investigate/ -run TestWorktreeLifecycle -v && go test ./internal/intelligence/investigate/` | ❌ W0 | ⬜ pending |
| 04-04-T2 | 04 | 3 | REGRESS-01 | T-04-04a | No shell spawned; arg-splitting only; bounded output capture | unit | `go test ./internal/intelligence/investigate/ -run "TestRepro\|TestResolveRepro" -v` | ❌ W0 | ⬜ pending |
| 04-04-T3 | 04 | 3 | REGRESS-01 | T-04-04c | Window never silently widened; deterministic attribution | integration + race | `go test ./internal/intelligence/investigate/ -run TestBisect -v && go test -race ./internal/intelligence/investigate/ && make test-fast` | ❌ W0 | ⬜ pending |
| 04-05-T1 | 05 | 4 | REGRESS-02 | T-04-05a | verified only after two independent confirmation observations; mechanism ≤ likely | unit | `go test ./internal/intelligence/investigate/ -run TestReport -v && go test ./internal/intelligence/investigate/` | ❌ W0 | ⬜ pending |
| 04-05-T2 | 05 | 4 | REGRESS-03 | — | Honest empty/degraded rendering in both formats | unit | `go test ./internal/intelligence/investigate/ -run TestRenderInvestigation -v` | ❌ W0 | ⬜ pending |
| 04-05-T3 | 05 | 4 | REGRESS-01..03 | T-04-05b | Documented exit codes 0/1/3/4; events land via nil-safe emitters | e2e tracer | `go test ./cmd/m31a/ ./internal/intelligence/investigate/ && make build && ./m31a investigate 2>&1 \| grep -ci usage && ./m31a investigate --help 2>&1 \| grep -c baseline` | ❌ W0 | ⬜ pending |
| 04-06-T1 | 06 | 2 | DEPEND-01 | T-04-06a | LimitReader cap; typed 404 sentinel; Retry-After honored | unit (httptest fixtures) | `go test ./internal/intelligence/deps/ -run TestDepsDev -v && go test ./internal/intelligence/deps/` | ❌ W0 | ⬜ pending |
| 04-06-T2 | 06 | 2 | DEPEND-01, DEPEND-02 | T-04-06b, T-04-06c | Fixed-host SSRF guard; token never logged; unknown ≠ clean | unit (httptest fixtures) | `go test ./internal/intelligence/deps/ -run "TestOSV\|TestGitHub" -v && go test ./internal/intelligence/deps/` | ❌ W0 | ⬜ pending |
| 04-06-T3 | 06 | 2 | DEPEND-02 | — | Pure deterministic classification with source provenance per rule | unit | `go test ./internal/intelligence/deps/ -run TestClassifyRisk -v && make test-fast` | ❌ W0 | ⬜ pending |
| 04-07-T1 | 07 | 5 | DEPEND-01, DEPEND-04 | T-04-07a | Speculative confidence cap on any source outage; no duplicate cache events | unit (httptest + eventstore) | `go test ./internal/intelligence/deps/ -run "TestAssembleVerdict\|TestVerdictCache" -v && go test -race ./internal/intelligence/deps/` | ❌ W0 | ⬜ pending |
| 04-07-T2 | 07 | 5 | DEPEND-03 | T-04-07b | Headless block exit 2 after persist; idempotent approve; zero install capability | unit | `go test ./internal/intelligence/deps/ -run TestCheckpoint -v && go test ./internal/intelligence/deps/` | ❌ W0 | ⬜ pending |
| 04-07-T3 | 07 | 5 | DEPEND-01..04 | T-04-07b | Checkpoint gate enforced both modes; transitive impact from graph when index available | e2e tracer | `go test ./cmd/m31a/ ./internal/intelligence/deps/ && make build && ./m31a deps 2>&1 \| grep -ci usage && ./m31a deps check --approve 2>&1 \| grep -ci module` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

*All 20 tasks carry their own `<automated>` verify inside their PLAN files (tdd-flagged tasks create the test files inline); this map mirrors them for sampling continuity. File Exists flips to ✅ as each plan executes.*

---

## Wave 0 Requirements

- [x] Test stubs for intelligence packages are created inline by tdd-flagged tasks in plans 04-01 through 04-07 (Wave 0 absorbed into the plans — no separate scaffold plan needed)
- [x] Existing infrastructure covers framework needs (`go test` stdlib)

*If none: "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Live deps.dev/OSV/GitHub queries | DEPEND-01 | External network services not faked in CI | Run `m31a deps check github.com/pkg/errors` with network access and inspect report fields |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 90s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
