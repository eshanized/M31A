---
phase: 07-production-readiness
plan: 01
completed: "2026-08-06T16:45:00Z"
status: complete
commits:
  - 3816f781 - feat(07-01): crash handler end-to-end — panic recovery through crash report file
  - d266a744 - feat(07-01): CI crash capture and observability metrics collection
  - a008602a - feat(07-01): benchstat JSON output and GitHub Pages dashboard
requirements_met:
  - OBS-01
  - CRASH-01
key_files_created:
  - internal/observability/crash.go
  - internal/observability/crash_test.go
  - .github/ISSUE_TEMPLATE/crash-report.md
  - .github/workflows/ci.yml (crash-capture job)
  - .github/workflows/benchmarks.yml (JSON output + dashboard)
key_files_modified:
  - cmd/m31a/main.go (wired crash handler)
  - internal/integrations/metrics/types.go (new metric types)
  - internal/integrations/metrics/collector.go (new Record* methods)
---

## Plan 07-01: Crash Handler, Observability Metrics, Benchstat Dashboard — COMPLETE

### What Was Built

**1. Crash Handler (internal/observability/crash.go)**
- `CrashReport` struct capturing: timestamp, version, commit, Go version, OS, arch, stack trace, panic value
- `RecoverAndCapture(version, commit string)` function that:
  - Recovers from panic
  - Builds crash report with full context
  - Writes JSON crash report to `~/.m31a/crashes/crash-YYYYMMDD-HHMMSS.json` with 0600 permissions
  - Logs to stderr for CI capture (`CRASH: <panic>\n<stack>`)
  - Re-panics to preserve exit behavior
- Wired as FIRST defer in `cmd/m31a/main.go` run() function
- Signal handler updated to use crash handler for consistent reporting

**2. CI Crash Capture (.github/workflows/ci.yml)**
- Added `crash-capture` job that runs after `test` job (always)
- Uploads crash artifacts via actions/upload-artifact@v4
- Auto-files GitHub issues using crash-report.md template when crashes detected
- Uses gh CLI with GITHUB_TOKEN for issue creation
- Issue includes crash information and full stack trace

**3. Observability Metrics Collection (internal/integrations/metrics/)**
- Extended `SessionMetrics` with new metric types:
  - `StartupMetric` (DurationMs, ConfigLoadMs, ProviderMs, TUIMs)
  - `CompletionMetric` (Phase, Success, Failure, Timeout)
  - `CancellationMetric` (Phase, Reason)
- Added thread-safe collection methods to `Collector`:
  - `RecordStartup(durationMs, configLoadMs, providerMs, tuiMs int64)`
  - `RecordCompletion(phase types.WorkflowPhase, success, timeout bool)`
  - `RecordCancellation(phase types.WorkflowPhase, reason string)`
- Updated `Snapshot()` for deep-copy of new fields
- All existing tests pass with -race flag

**4. Benchstat JSON Output & GitHub Pages Dashboard (.github/workflows/benchmarks.yml)**
- Added JSON output step: `benchstat -format json old.txt new.txt > benchstat-results.json`
- Generated self-contained `dashboard.html` with:
  - Fetch and render benchstat-results.json
  - Summary table with old/new/delta for ns/op, B/op, allocs/op
  - Color-coded regressions (red) and improvements (green)
  - p=0.0 highlighted
  - Timestamp and commit hash display
- Commits both JSON and dashboard to gh-pages branch
- Outputs dashboard URL to GitHub Actions step summary

### Verification

- `go test ./internal/observability/... -v -race` — PASS (3 tests)
- `go test ./internal/integrations/metrics/... -v -race` — PASS (all existing tests + new fields)
- `go build ./cmd/m31a` — SUCCESS
- Crash handler tests verify: file creation, 0600 permissions, JSON fields, re-panic behavior

### Requirements Satisfied

- **OBS-01**: Session metrics track startup time, completion rate, cancellation rate via collector methods
- **CRASH-01**: Binary captures panics with full context; CI uploads artifacts and auto-files GitHub issues

### Reversibility

All changes are additive:
- New package `internal/observability/` can be deleted
- New defer in main.go can be removed
- New metric types/fields/methods can be reverted
- CI job and benchmarks workflow steps can be removed
- Dashboard is new files on gh-pages branch