---
phase: 1
slug: foundation-domain-model-event-store
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-08-23
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Go standard library `testing` + `testify/assert` + `testify/require` |
| **Config file** | None — tests use `testing.T` directly |
| **Quick run command** | `go test ./internal/core/types/... ./internal/core/config/... ./internal/memory/eventstore/... -short` |
| **Full suite command** | `make test` (race-enabled, coverage) |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `go test ./internal/core/types/... ./internal/core/config/... ./internal/memory/eventstore/... -short`
- **After every plan wave:** Run `make test`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 01-01 | 01 | 1 | DOMAIN-01 | — | 18 domain types defined with JSON tags | unit | `go test ./internal/core/types/... -run TestDomainTypes` | ❌ W0 | ⬜ pending |
| 01-02 | 01 | 1 | DOMAIN-02 | — | Each type in separate file/group; no mega-struct | unit | `go test ./internal/core/types/... -run TestOwnership` | ❌ W0 | ⬜ pending |
| 01-03 | 01 | 1 | DOMAIN-03 | — | All types marshal/unmarshal JSON round-trip | unit | `go test ./internal/core/types/... -run TestSerialization` | ❌ W0 | ⬜ pending |
| 01-04 | 01 | 1 | DOMAIN-04 | — | Domain mutations emit events via EventStore | integration | `go test ./internal/memory/eventstore/... -run TestEventEmission` | ❌ W0 | ⬜ pending |
| 01-05 | 01 | 1 | PERSIST-01 | — | events.db created with WAL mode | unit | `go test ./internal/memory/eventstore/... -run TestWALMode` | ❌ W0 | ⬜ pending |
| 01-06 | 01 | 1 | PERSIST-02 | — | Append-only writes, monotonic SEQ, range queries | unit | `go test ./internal/memory/eventstore/... -run TestAppendQuery` | ❌ W0 | ⬜ pending |
| 01-07 | 01 | 1 | PERSIST-03 | — | config.toml loaded with layered precedence | unit | `go test ./internal/core/config/... -run TestLayeredConfig` | ❌ W0 | ⬜ pending |
| 01-08 | 01 | 1 | PERSIST-04 | — | project.md, decisions/, research/ written | integration | `go test ./internal/memory/artifacts/... -run TestProjections` | ❌ W0 | ⬜ pending |
| 01-09 | 01 | 1 | PERSIST-05 | — | .gitignore excludes caches, secrets, embeddings | unit | `go test ./internal/memory/... -run TestGitIgnorePolicy` | ❌ W0 | ⬜ pending |
| 01-10 | 01 | 2 | PERSIST-06 | T-010 | Migration replays all .planning/ data to events | integration | `go test ./internal/memory/eventstore/... -run TestMigration` | ❌ W0 | ⬜ pending |
| 01-11 | 01 | 2 | PERSIST-07 | — | Hot backup completes without blocking writers | integration | `go test ./internal/memory/eventstore/... -run TestHotBackup` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `internal/core/types/domain_test.go` — stubs for DOMAIN-01, DOMAIN-02, DOMAIN-03
- [ ] `internal/core/types/event_test.go` — stubs for DOMAIN-04, event serialization
- [ ] `internal/core/config/config_test.go` — stubs for PERSIST-03
- [ ] `internal/memory/eventstore/eventstore_test.go` — stubs for PERSIST-01, PERSIST-02, PERSIST-07
- [ ] `internal/memory/eventstore/migration_test.go` — stubs for PERSIST-06
- [ ] `internal/memory/artifacts/artifacts_test.go` — stubs for PERSIST-04, PERSIST-05
- [ ] Framework: testify already in go.mod (indirect), no additional install needed

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Migration preserves traceability for all .planning/ artifacts | PERSIST-06 | Requires visual inspection of migrated data | 1. Run `m31a migrate` in repo with existing .planning/ 2. Verify `.m31a/events.db` contains events for all requirements, decisions, research 3. Verify human-readable projections in `.m31a/` match source |
| OS keychain stores/retrieves NVIDIA_API_KEY without disk persistence | PERSIST-03 | Requires platform-specific verification | 1. Run `m31a config set provider.key <key>` 2. Verify key NOT in `.m31a/config.toml` 3. Verify key retrievable via `m31a config get provider.key` 4. Restart M31A, verify key still works |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending