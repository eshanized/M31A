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
