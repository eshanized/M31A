# Phase 10: Fix test errors from architecture upgrade - Validation

**Generated:** 2026-07-17
**Phase:** 10-fix-test-errors-from-architecture-upgrade
**Validation standard:** Nyquist (nyquist_validation: true)

## Test Framework

| Property | Value |
|----------|-------|
| Framework | Go stdlib `testing` (no external test frameworks) |
| Config file | None — Go test auto-discovers `*_test.go` files |
| Quick run command | `make test-fast` (no race detector) |
| Full suite command | `make check` (fmt → tidy → vet → lint → test) |

## Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| NFR-4 | Build compiles cleanly after fixes | build | `GOCACHE=/home/snigdha/.cache/go-build GOTMPDIR=/home/snigdha/.tmp go build ./...` | N/A (infrastructure) |
| NFR-4 | Tests pass after centralization | unit | `make test` | ✅ existing |
| NFR-4 | gofmt-clean code | lint | `gofmt -l <modified files>` | N/A (infrastructure) |
| NFR-4 | golangci-lint passes | lint | `make lint` | N/A (infrastructure) |
| NFR-4 | No import cycles introduced | build | `go vet ./...` | N/A (infrastructure) |
| NFR-4 | Coverage maintained at 75%+ | coverage | `go test -coverprofile=... ./...` | N/A (infrastructure) |

## Verification Commands

### Per-Plan Verification (each plan must pass these independently)

**Plan 01 — Fix build errors:**
```bash
# Must succeed — zero compilation errors
GOCACHE=/home/snigdha/.cache/go-build GOTMPDIR=/home/snigdha/.tmp go build ./...

# Session tests must pass
go test ./internal/session/... -count=1 -timeout 30s

# Fileops tests must pass
go test ./internal/tools/fileops/... -count=1 -timeout 30s

# Downstream packages must compile
go build ./internal/tui/...
go build ./internal/workflow/...
go build ./cmd/m31a/...
```

**Plan 02 — Centralize mocks and builders:**
```bash
# Full build must succeed
GOCACHE=/home/snigdha/.cache/go-build GOTMPDIR=/home/snigdha/.tmp go build ./...

# Full vet must pass
go vet ./...

# No duplicate mock definitions
rg "type mockProvider struct" --type go  # should return 1 result
rg "type mockKeychain struct" --type go  # should return 1 result
rg "type mockTool struct" --type go      # should return 1 result

# No duplicate setup functions
rg "func setupTestEngine" --type go      # should return 1 result
rg "func testDispatcher\b" --type go     # should return 1 result

# Import cleanliness
goimports -l ./internal/testutil/... 2>/dev/null | wc -l | grep -q "^0$"

# Tests pass for affected packages
go test ./internal/workflow/... -count=1 -timeout 60s
go test ./internal/tools/... -count=1 -timeout 60s
go test ./internal/provider/... -count=1 -timeout 60s
go test ./internal/keychain/... -count=1 -timeout 30s
go test ./internal/config/... -count=1 -timeout 30s
```

**Plan 03 — Fixtures, e2e, and integration moves:**
```bash
# Full build must succeed
GOCACHE=/home/snigdha/.cache/go-build GOTMPDIR=/home/snigdha/.tmp go build ./...

# testutil tests pass
go test ./internal/testutil/... -count=1 -timeout 60s

# E2E tests pass from new location (or root wrapper)
go test -run TestBinary -count=1 -timeout 30s

# Integration tests pass
go test ./internal/testutil/integration/... -count=1 -timeout 60s

# Full quality gate
make check

# Verify directory structure
ls internal/testutil/mocks/
ls internal/testutil/builders/
ls internal/testutil/fixtures/
ls internal/testutil/e2e/
ls internal/testutil/integration/
```

### Phase Gate Verification

```bash
# Canonical full check — must be green before /gsd-verify-work
make check

# Full test suite with race detector
make test

# Lint standalone
make lint

# Verify no old locations remain
test ! -f internal/tools/edit_integration_test.go && echo "edit_integration_test.go moved"
```

## Sampling Rate

- **Per task commit:** `go build ./...` + `go vet ./...` (fast feedback)
- **Per plan completion:** Full plan-specific verification commands above
- **Per wave merge:** `make check` (full quality gate)
- **Phase gate:** `make check` + `make test` (full suite with race detector)

## Wave 0 Gaps

- [x] `internal/testutil/envtest.go` — existing shared test utilities (keep as-is)
- [x] `go build ./...` — verifies compilation (use GOTMPDIR fix for disk quota)
- [x] `go vet ./...` — verifies no import cycles or type errors
- [x] `make lint` — golangci-lint already configured
- [ ] `internal/testutil/mocks/` — **must be created by Plan 02, Task 1**
- [ ] `internal/testutil/builders/` — **must be created by Plan 02, Task 2**
- [ ] `internal/testutil/fixtures/` — **must be created by Plan 03, Task 1**
- [ ] `internal/testutil/e2e/` — **must be created by Plan 03, Task 2**
- [ ] `internal/testutil/integration/` — **must be created by Plan 03, Task 2**

## Environment Notes

**Disk quota workaround:** The build environment has disk quota constraints on `/tmp`. All build commands must use:
```bash
GOCACHE=/home/snigdha/.cache/go-build GOTMPDIR=/home/snigdha/.tmp go build ./...
```
This is critical — without it, `go build` fails with "disk quota exceeded" on importcfg writes.

## Coverage Targets

| Package | Target | Current | Gate |
|---------|--------|---------|------|
| Overall | 75% | verify with `go test -cover` | Phase gate |
| `pkg/taskrunner` | 90% | verify | Phase gate |
| `pkg/bisect` | 90% | verify | Phase gate |
| `pkg/rollback` | 90% | verify | Phase gate |

Note: Phase 10 restructures test infrastructure — it does not add new test coverage. Coverage should remain at or above pre-Phase-10 levels. If coverage drops, it indicates tests were lost during restructuring.
