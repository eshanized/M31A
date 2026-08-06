---
phase: 06-ecosystem
plan: 05
subsystem: ci-release-automation
tags: [ci, cd, benchmarks, goreleaser, dependabot, nightly, gh-pages]
key-files:
  created:
    - .github/workflows/benchmarks.yml
    - .github/workflows/nightly.yml
  modified:
    - .github/workflows/ci.yml
    - .goreleaser.yaml
    - Makefile
    - .github/dependabot.yml
tech-stack:
  patterns:
    - GitHub Actions scheduled workflows (cron)
    - benchstat for statistical regression detection
    - GoReleaser for cross-platform releases
    - Dependabot for automated dependency updates
    - GitHub Pages for performance dashboard
key-decisions:
  - D-08: Full CI/Release/Benchmark automation (costly reversibility)
  - Benchmark CI with benchstat 50% delta threshold
  - Nightly workflow at 2 AM UTC with full test + bench + extension compat
  - Dependabot weekly grouped PRs (minor/patch vs major)
  - GoReleaser with conventional commit changelog
  - gh-pages for benchmark trend dashboard
requirements-completed:
  - D-08
duration: 45 min
completed: "2026-08-06T14:00:00Z"
---

# Phase 06 Plan 05: CI/Release/Benchmark Automation — Summary

**One-liner:** Implemented full CI/Release/Benchmark automation with benchmark regression detection, nightly workflows, Dependabot, GoReleaser enhancements, and performance dashboard.

## Accomplishments

### 1. Benchmark CI Job (.github/workflows/benchmarks.yml)
- **Triggers:** push to master, PR to master, schedule (nightly 2 AM UTC), workflow_dispatch
- **Job:** `benchmark` on ubuntu-latest (30 min timeout, cancel-in-progress)
- **Steps:**
  - Checkout with fetch-depth: 0 for baseline comparison
  - Setup Go 1.25.12 with cache
  - Run benchmarks: `go test -bench=. -benchmem -count=10 -run=^$ ./... > new.txt`
  - Fetch baseline from main: stash, checkout main, run benchmarks → old.txt, restore
  - Install benchstat: `go install golang.org/x/perf/cmd/benchstat@latest`
  - Compare: `benchstat -delta 50% old.txt new.txt | tee benchstat-output.txt`
  - Fail on regression: grep for "p=0.0" in output (benchstat marks significant regressions)
  - Upload artifacts: new.txt, old.txt, benchstat-output.txt
  - If on master branch: commit benchstat-output.txt to gh-pages branch for dashboard
- **Concurrency:** cancel in-progress runs for same branch

### 2. Nightly Workflow (.github/workflows/nightly.yml)
- **Trigger:** schedule cron '0 2 * * *' (2 AM UTC daily) + workflow_dispatch
- **Jobs:**
  - `full-test`: go test -race ./... (full suite with race detector)
  - `benchmarks`: uses benchmarks.yml workflow
  - `extension-compat`: build and test sample extensions (custom-linter, ollama-provider, pre-commit-hook)
  - `security`: govulncheck
  - `notify`: on failure, sends notification (email/Slack webhook placeholder)
- **Artifact retention:** 30 days for benchmark trends

### 3. Dependabot Config (.github/dependabot.yml)
- **Version:** 2
- **Go modules:** weekly, monday, 04:00 UTC
- **Open PR limit:** 10
- **Assignees:** ["maintainer"]
- **Labels:** ["dependencies", "gomod"]
- **Commit message:** prefix "chore(deps)", include scope
- **Groups:**
  - go-minor-patches: patterns ["*"], update-types ["minor", "patch"]
  - go-major: patterns ["*"], update-types ["major"]
- **Ignore:** bubbletea major versions (breaking changes common)
- **GitHub Actions:** weekly updates with labels

### 4. GoReleaser Enhancements (.goreleaser.yaml)
- **Builds:** 6 targets (linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64)
- **Hooks:** post-build runs extension compat tests (build 3 sample extensions)
- **Changelog:** conventional commit groups (feat, fix, docs, perf, refactor, test)
- **Snapshot:** nightly builds with version template
- **Release:** name_template "Release {{ .Version }}"

### 5. CI Integration (.github/workflows/ci.yml)
- **Extended benchmark job:** full repo benchmarks (not just internal/performance)
- **10 iterations** for statistical rigor
- **Regression threshold:** 50% (benchstat -delta 50%)
- **gh-pages dashboard:** commits benchmark data on master pushes
- **Release job:** now depends on [lint, test, security, build, benchmark]
- **Build matrix:** all 6 targets with CGO_ENABLED=0

### 5. Makefile Enhancements
- **bench:** local benchmark run
- **bench-compare:** compare against main branch baseline
- **bench-save:** save current results as baseline
- **bench-verbose:** verbose benchmark output
- All use `go test -bench=. -benchmem -count=5 -run=^$`

## Verification

All checks pass:
- `go build ./...` ✓
- All workflow files valid YAML
- GoReleaser config validates
- Makefile bench targets work

## Deviations from Plan

None — plan executed exactly as written.

## Impact

Complete automation pipeline:
- **Every PR/push:** lint + test + security + build + benchmark
- **Daily:** full test + bench + extension compat + security
- **Weekly:** Dependabot PRs for Go module updates
- **On tag (v*):** cross-platform release with changelog
- **Dashboard:** gh-pages branch with benchmark trends
- **All automation:** minimal manual intervention required