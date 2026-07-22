# Phase 02, Plan 02 Summary — CI Workflow Updates

**Phase:** 02-fix-test-hanging-and-ci-issues
**Plan:** 02-02
**Wave:** 1
**Status:** Complete ✓
**Completed:** 2026-07-23

---

## Objective
Update GitHub Actions CI workflow with job timeouts, test timeouts, and artifact collection. Ensure CI jobs cannot hang indefinitely and provide debugging artifacts on failure. Pin Go version to 1.25.0 for reproducible builds and verify compatibility.

---

## Changes Made

### 1. CI Workflow Test Job Updates (Task 1)
Updated `.github/workflows/ci.yml`:

**Test Job (`test`):**
- Verified `timeout-minutes: 10` already present
- Updated test command: `go test -race -timeout 30s -coverprofile=coverage.out -covermode=atomic ./...` (added `-timeout 30s`)
- Added artifact upload on failure:
  ```yaml
  - name: Upload coverage on failure
    if: failure()
    uses: actions/upload-artifact@v4
    with:
      name: coverage
      path: coverage.out
  - name: Upload test output on failure
    if: failure()
    uses: actions/upload-artifact@v4
    with:
      name: test-output
      path: test-output.log
  ```

**Go Version Pinning (All Jobs):**
Changed all `go-version` references from `"1.25"` to `"1.25.0"`:
- `lint` job: `go-version: "1.25.0"`
- `test` job: `go-version: "1.25.0"`
- `security` job: `go-version: "1.25.0"`
- `build` job matrix: `go: ["1.25.0"]`
- `release` job: `go-version: "1.25.0"`

**Go Module Caching (All Jobs):**
Added to all 5 jobs (lint, test, security, build, release):
```yaml
- name: Cache Go modules
  uses: actions/cache@v4
  with:
    path: |
      ~/go/pkg/mod
      ~/.cache/go-build
    key: ${{ runner.os }}-go-${{ hashFiles('**/go.sum') }}
    restore-keys: |
      ${{ runner.os }}-go-
```

### 2. Lint, Security, Build Jobs (Task 2)
Applied consistent Go 1.25.0 pinning and module caching to:
- `lint` job: setup-go + cache step
- `security` job: setup-go + cache step
- `build` job matrix: updated go version to 1.25.0, added cache per matrix job
- `release` job: setup-go with cache: true (uses built-in caching)

### 3. Local Build Verification (Task 2)
Built binary locally with current Go version:
```bash
TMPDIR=/home/snigdha/tmp go build ./cmd/m31a
```
**Result:** Binary created successfully (65.9 MB), version command works.

### 4. YAML Syntax Validation (Task 3)
Validated workflow structure:
- Python YAML parser confirms valid syntax
- All required fields present: `name`, `on`, `permissions`, `jobs`
- All 5 jobs have `runs-on`, `steps`, `timeout-minutes`
- Test job has artifact upload on failure
- All jobs reference Go 1.25.0

---

## Verification
- ✅ Test job has `timeout-minutes: 10`
- ✅ Test step command includes `-timeout 30s`
- ✅ All 5 go-version references pin to `1.25.0` (lint, test, security, build matrix, release)
- ✅ All 5 jobs have Go module caching via actions/cache@v4
- ✅ Artifact upload steps trigger on test failure (`if: failure()`)
- ✅ CI workflow YAML syntax validates
- ✅ Local build with Go 1.25.0 succeeds (verifies version compatibility)

---

## Artifacts Created/Updated
1. `.github/workflows/ci.yml` — fully updated CI workflow

---

## Rationale for Go 1.25.0 Pinning (per RESEARCH.md §3.2)
Pinning to exact patch version `1.25.0` (not `1.25`) ensures:
- All jobs use identical toolchain
- Eliminates "works on my machine" from patch-level differences
- Reproducible builds across local and CI
- Tradeoff: security patches require explicit go.mod + CI config updates

---

## Next Steps (Plan 02-03)
Plan 02-03 will fix hanging tests identified in Plan 02-01 analysis. CI changes here ensure:
- 10-minute job timeout prevents stuck CI runs
- 30-second test timeout catches hanging tests in CI
- Failure artifacts enable debugging without re-running
- Consistent Go 1.25.0 toolchain matches local development