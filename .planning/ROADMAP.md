# ROADMAP — M31A

## Project Title

M31A — AI-powered CLI agent with TUI, workflow engine, and multi-provider support

## Phase 1: Audit Bug Fixes

**Goal:** Resolve all 30 confirmed bugs from the logical bug audit (BUGS.md), organized into 4 batches by severity, with test-driven fixes and CI-clean verification.

**Success Criteria:**

- All 30 bugs (B01–B30) fixed with passing tests
- `go build ./...`, `go vet ./...`, `golangci-lint run`, `go test -race ./...` all clean
- Each fix has a test that fails on current code and passes after fix
- FIXES.md documents each fix with bug ID, change, test, and residual risk

**Batches:**

1. Critical/Security (B01, B02, B03, B07, B08, B09)
2. Data Races (B04, B05, B06, B12, B13, B20, B23, B24)
3. Correctness (B10, B11, B14, B15, B16, B17, B18, B19, B21, B22)
4. Low Severity Cleanup (B25, B26, B27, B28, B29, B30)

**Plans:** 1/4 plans executed

Plans:

- [ ] 01-audit-fixes/01-01-PLAN.md — Batch 1: Critical/Security (B01, B02, B03, B06, B07, B08, B09)
- [ ] 01-audit-fixes/01-02-PLAN.md — Batch 2: Data Races (B04, B05, B12, B13, B20, B23, B24)
- [ ] 01-audit-fixes/01-03-PLAN.md — Batch 3: Correctness (B10, B11, B14, B15, B16, B17, B18, B19, B21, B22)
- [ ] 01-audit-fixes/01-04-PLAN.md — Batch 4: Low Severity (B25, B26, B27, B28, B29, B30)

**Canonical refs:** `BUGS.md`

## Phase 2: Fix CI Test Regressions

**Goal:** Fix all 59 test failures across 6 root causes introduced during Phase 01 bug-fix batches, restoring CI to green.

**Success Criteria:**

- All 59 failing tests pass (`go test -race ./...` clean)
- `go vet ./...` and `golangci-lint run` clean (including the execute.go:571 ineffassign)
- No new test regressions introduced
- Each fix validated by running the specific failing test in isolation

**Root Causes (from TEST_FAILURES.md):**

- A: Config merge `intField`/`float6Field` regression (6 tests) — `merge.go` missing non-zero fallback
- B: Session manager `sessionMetadata` missing `Label` field (3 tests)
- C: Workflow engine `RunPhase` transition enforcement (19 tests) — state machine blocks legal test paths
- D: TestIsCI race condition (1 test) — parallel env-var mutation
- E: Bash security patterns lost in directory restructure (23 tests) — `exec/bash.go` missing expanded blocklist
- F: TestAskUserQuestion_Timeout assertion (1 test) — checks wrong error channel

**Plans:** 4/4 plans executed

Plans:

- [x] 02-fix-ci-regressions/02-01-PLAN.md — Root Cause A: Config merge int/float regression (6 tests) + Root Cause B: Session Label field (3 tests)
- [x] 02-fix-ci-regressions/02-02-PLAN.md — Root Cause C: Workflow engine RunPhase transitions (19 tests) + execute.go:571 lint
- [x] 02-fix-ci-regressions/02-03-PLAN.md — Root Cause D: TestIsCI race (1 test) + Root Cause F: AskUserQuestion timeout (1 test)
- [x] 02-fix-ci-regressions/02-04-PLAN.md — Root Cause E: Bash security patterns restore + regex upgrade (23 tests)

**Canonical refs:** `TEST_FAILURES.md`

## Phase 3: Fix Remaining CI Issues

**Goal:** Fix all remaining lint errors, test failures, and security issues blocking CI from passing.

**Success Criteria:**

- `golangci-lint run` clean (0 issues)
- `go test -race ./...` clean (all tests pass)
- `go vet ./...` clean
- No security vulnerabilities (CodeQL/govulncheck clean)
- CI pipeline passes on GitHub Actions

**Known Issues:**

- Lint: 3 staticcheck SA1019 warnings in `internal/core/types/fileutil.go` (deprecated `os.SEEK_SET`)
- Tests: `TestRegistry_Execute_PhaseAliases` nil pointer dereference in `FileLock.Lock`
- Tests: `TestAskUserQuestion_ChannelFull` timeout issues in `internal/tools`
- Security: GO-2026-5320 XSS in goldmark@v1.5.2

**Plans:** 1/1 plans executed

Plans:

- [x] 03-fix-remaining-ci-issues/03-01-PLAN.md — Lint fixes (os.SEEK_SET), test fixes (2 tests), security upgrade (goldmark)

**Canonical refs:** 03-CONTEXT.md, 03-RESEARCH.md

### Phase 4: Audit and remove unused/irrelevant code from M31A codebase

**Goal:** Remove clearly dead code (deprecated functions, unused utilities, wrapper/re-exports, unreferenced packages) verified via grep to have zero production callers, reducing codebase maintenance surface.
**Requirements**: D-01 through D-07 from CONTEXT.md
**Depends on:** Phase 3
**Plans:** 2/2 plans executed

Plans:

- [x] 04-audit-and-remove-unused-irrelevant-code-from-m31a-codebase/04-01-PLAN.md — Wave 1: Remove deprecated theme functions, buffer pool utilities, truncate wrappers, tools re-exports
- [x] 04-audit-and-remove-unused-irrelevant-code-from-m31a-codebase/04-02-PLAN.md — Wave 2: Remove provider functions, tuitypes/theme utilities, a11y package, final verification

## Phase 5: Fix Wiring Issues

**Goal:** Resolve all 50 wiring issues (W01-W50) from the wiring audit (WIRING_ISSUES.md), connecting dead producers to consumers, fixing config drift, completing partial registrations, and closing message routing gaps.

**Success Criteria:**

- All 50 wiring issues (W01-W50) fixed with integration tests
- `go build ./...`, `go vet ./...`, `golangci-lint run`, `go test -race ./...` all clean
- Each wiring fix has a test that fails on current code and passes after fix
- Dead code with zero references removed (~40 items)
- WIRING_ISSUES.md updated to reflect resolved status

**Batches (by severity):**

1. Critical (W01, W02) — Config merge, rollback integration
2. High (W03-W15) — InstructionsSource, metrics, StreamChunkMsg, permissions, config help
3. Medium (W16-W32) — Narrative events, exec constants, IP dedup, Zen parity, dead code
4. Low (W33-W50) — Test utilities, mock cleanup, unused methods/interfaces

**Plans:** 0/4 plans executed

Plans:

- [ ] 05-fix-wiring-issues/05-01-PLAN.md — Wave 1: Config merge (~30 fields) + Rollback SoftReset integration
- [ ] 05-fix-wiring-issues/05-02-PLAN.md — Wave 2: InstructionsSource, metrics wiring, StreamChunkMsg, permissions, dead packages
- [ ] 05-fix-wiring-issues/05-03-PLAN.md — Wave 2: WorkflowEvent methods, dead constants/errors, Zen parity, FallbackPriority, layout cleanup
- [ ] 05-fix-wiring-issues/05-04-PLAN.md — Wave 3: Dead test utilities, production methods/interfaces, full CI verification

**Canonical refs:** WIRING_ISSUES.md, 05-CONTEXT.md
