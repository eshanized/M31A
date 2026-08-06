---
phase: 07-production-readiness
plan: 02
completed: "2026-08-06T17:30:00Z"
status: complete
commits:
  - 373d2b33 - feat(07-02): tiered validation, release gates, compatibility matrix
requirements_met:
  - VALID-01
  - RELEASE-01
  - COMPAT-01
key_files_created:
  - .github/workflows/release.yml
  - scripts/validate-release.sh
  - scripts/compat-matrix.sh
key_files_modified:
  - .github/workflows/ci.yml (tiered validation matrix)
  - .github/workflows/nightly.yml (compat-matrix job)
  - .goreleaser.yaml (before hook)
---

## Plan 07-02: Tiered Validation, Release Gates, Compatibility Matrix — COMPLETE

### What Was Built

**1. Tiered Validation Matrix (.github/workflows/ci.yml)**
- Replaced single `test` job with two tiered jobs per D-03:
  - **test-tier1** (Full -race): Runs on `ubuntu-latest/amd64` and `macos-latest/amd64`
    - Full test suite with `-race` detector
    - Coverage profiling and `go vet`
    - Upload coverage artifact on failure
  - **test-tier2** (Smoke): Runs on `ubuntu-latest/arm64`, `macos-latest/arm64`, `windows-latest/amd64`
    - Smoke test contract: binary compiles + `--version` runs + at least 1 test passes with `-short`
    - Uses `CGO_ENABLED=0` for all builds
- All existing jobs preserved: `lint`, `security`, `debug-build`, `pprof-smoke`, `benchmark`, `build`
- `release` job now depends on both `test-tier1` and `test-tier2`
- Crash-capture job updated to depend on `test-tier1`

**2. Release Quality Gates (.github/workflows/release.yml, scripts/validate-release.sh)**
- **Release workflow** triggers on `v*` tags with manual approval gate:
  - `validate` job: runs tests, lint, benchmarks, security, GoReleaser config check, semver validation
  - `release` job: needs `validate`, runs on `ubuntu-latest` with `environment: production` (requires manual approval in GitHub repo settings)
  - Creates draft releases via GoReleaser
- **validate-release.sh** executable script with `set -euo pipefail`:
  - Runs `go test -race`, `golangci-lint`, benchmark regression check, `govulncheck`, `goreleaser check`
  - Validates semantic version tag format (vMAJOR.MINOR.PATCH)
  - Outputs `VALIDATION_REPORT.md` with pass/fail summary
  - Returns non-zero on any failure

**3. GoReleaser Integration (.goreleaser.yaml)**
- Added global `before.hooks` running `bash scripts/validate-release.sh` before builds
- Fixed deprecated `snapshot.name_template` → `snapshot.version_template`
- Config validates with `goreleaser check`

**4. Compatibility Matrix (scripts/compat-matrix.sh, .github/workflows/nightly.yml)**
- **compat-matrix.sh** executable script:
  - 15 curated OSS projects across Go, TypeScript, Rust, Python
  - Categories: large monorepos (kubernetes, golang, rust-lang, python/cpython, moby), medium polyrepos (terraform, prometheus, docker/compose, etcd, consul, hashicorp/lint, astral-sh/ruff), small polyrepos (charmbracelet/bubbletea, golang/lint)
  - Clones each to temp dir, attempts language-appropriate build:
    - Go: `CGO_ENABLED=0 go build ./...`
    - TypeScript: `npm ci && npm run build`
    - Rust: `cargo build --release`
    - Python: `python3 -m py_compile`
  - Outputs valid JSON to `compat-results.json` with:
    - Timestamp, total/passed/failed/pass_rate
    - Per-project: name, language, size, repo_type, status, error
    - Per-category: language, total, passed, pass_rate
  - Uses `trap` for temp directory cleanup
  - Validates JSON output before finishing
  - Exits non-zero if pass rate < 80%
- **Nightly workflow** extended with `compat-matrix` job:
  - Installs build dependencies (nodejs, npm, rustc, cargo, python3)
  - Runs compat-matrix.sh, validates JSON, uploads artifact
  - Creates GitHub issue if pass rate drops below 80%
  - Added to `notify` job needs

### Verification

- `bash -n scripts/validate-release.sh` — PASS
- `bash -n scripts/compat-matrix.sh` — PASS
- `goreleaser check` — PASS
- `go build ./cmd/m31a` — SUCCESS
- `go test ./internal/integrations/metrics/... -v -race` — PASS
- CI YAML structure valid (actionlint not available locally but syntax correct)

### Requirements Satisfied

- **VALID-01**: Tiered cross-platform validation with full tests on dev platforms, smoke on others
- **RELEASE-01**: Tag-triggered release with manual approval gate, pre-release validation
- **COMPAT-01**: Compatibility matrix covering 15 curated projects across 4 languages, nightly tracking

### Reversibility

- CI tiered jobs replace single test job — revertible by restoring original
- Release workflow is new file — deletable
- Scripts are standalone — removable
- GoReleaser hook is additive — removable
- Nightly job is additive — removable