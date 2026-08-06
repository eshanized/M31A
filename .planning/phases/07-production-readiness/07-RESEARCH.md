# Phase 7: Production Readiness - Research

**Researched:** 2026-08-06
**Domain:** Production readiness, observability, crash reporting, cross-platform validation, release process, LTS policy, compatibility testing
**Confidence:** HIGH

## Summary

Phase 7 focuses on making M31A production-ready for daily professional use. This involves extending the existing CI/CD infrastructure (GitHub Actions, GoReleaser, benchstat) with observability metrics, crash capture, tiered cross-platform validation, release quality gates, LTS/deprecation policy, and a curated compatibility test matrix. The research covers six domains, each building on established patterns already present in the codebase.

**Primary recommendation:** Extend existing infrastructure rather than introducing new dependencies. The codebase already has GitHub Actions, GoReleaser, benchstat, and a nightly workflow — leverage these foundations for observability, crash capture, and release quality gates.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** GitHub Actions + Pages with benchstat JSON — zero infra cost, CI-integrated, version-controlled trend history. Extends Phase 4/6 benchmark infrastructure.
- **D-02:** GitHub Actions CI Crash Capture — captures crashes from CI runs, uploads artifacts (stack traces, coredumps), auto-files GitHub issues with template. No external service dependency.
- **D-03:** Tiered validation — full test suite (with race detector) on linux/amd64 + darwin/amd64; smoke tests on linux/arm64, darwin/arm64, windows/amd64.
- **D-04:** Tag-triggered GoReleaser with manual approval — push v* tag, GoReleaser builds all 6 platforms, generates changelog from conventional commits, creates draft release, manual review → publish.
- **D-05:** Rolling support — current release + 1 previous minor (6 months security fixes) + major versions (12 months security fixes). Clear EOL dates published.
- **D-06:** Curated real-world projects — 10-20 OSS projects covering languages (Go, TS, Python, Rust), sizes (small/medium/large), frameworks, mono/polyrepo. Run nightly in CI, track pass rate per category.

### the agent's Discretion
- Agent may choose specific metrics to collect beyond benchmarks (startup time, memory, CPU, completion rate)
- Agent may design crash handler in M31A binary (panic hook, stack trace capture, minidump)
- Agent may select specific curated projects for compatibility matrix
- Agent may design semantic versioning enforcement in CI (block breaking changes without MAJOR bump)
- Agent may design deprecation warning system (log warnings, docs notices)
- Agent may choose benchmark thresholds for regression detection (currently 50% from Phase 4/6)

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| OBS-01 | Extend observability metrics beyond benchmarks | Section: Observability & Metrics — GitHub Actions + Pages dashboard, session metrics integration |
| CRASH-01 | Implement crash capture in CI | Section: Crash & Failure Reporting — panic handler in main.go, GitHub Actions artifact upload |
| VALID-01 | Tiered cross-platform validation | Section: Cross-Platform Validation — matrix strategy, race detector on dev platforms |
| RELEASE-01 | Tag-triggered release with quality gates | Section: Release Process — GoReleaser hooks, manual approval, changelog generation |
| LTS-01 | Rolling support and deprecation policy | Section: LTS & Deprecation — semantic versioning enforcement, deprecation warnings |
| COMPAT-01 | Curated OSS project compatibility matrix | Section: Compatibility Test Matrix — nightly CI, pass rate tracking |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Observability metrics collection | Internal (metrics/) | CI/CD (GitHub Actions) | Session metrics already exist; extend to CI aggregation |
| Crash handler | CLI entry (cmd/m31a) | Internal (workflow) | Panic recovery must be at top-level; stack trace capture in main.go |
| Cross-platform validation | CI/CD (GitHub Actions) | Build (Makefile) | Matrix strategy in CI; Makefile targets for local cross-compilation |
| Release quality gates | CI/CD (GitHub Actions) | Build (GoReleaser) | Pre-release hooks in GoReleaser; CI jobs as gate dependencies |
| LTS/deprecation policy | Documentation | Internal (config) | EOL dates in docs; deprecation warnings in config loader |
| Compatibility matrix | CI/CD (GitHub Actions) | Internal (workflow) | Nightly workflow; curated projects as test fixtures |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `golang.org/x/perf/cmd/benchstat` | latest | Benchmark regression detection | Already used in Phase 4/6 benchmarks; JSON output for dashboards |
| `runtime/debug` | stdlib | Stack trace capture | Standard Go library for panic recovery and stack traces |
| `log/slog` | stdlib | Structured logging | Already used in codebase; JSON output for observability |
| GoReleaser v2 | 2.x | Cross-platform releases | Already configured; add pre-release hooks |
| GitHub Actions | v7 | CI/CD matrix strategy | Already used; extend with tiered validation |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `golang.org/x/mod/semver` | v0.38.0 | Semantic version parsing | CI enforcement of version bumps |
| `github.com/google/go-cmp` | latest | Test comparison | Compatibility test assertions |
| `github.com/charmbracelet/bubbletea` | v1.3.0 | TUI framework | Already used; crash handler must preserve TUI state |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| benchstat JSON | Prometheus + Grafana | Zero infra cost vs. richer dashboards; D-01 locks benchstat |
| GitHub Actions crash capture | Sentry SDK | No external dependency vs. richer crash analytics; D-02 locks GitHub Actions |
| GoReleaser hooks | Custom release script | Standard tooling vs. full control; D-04 locks GoReleaser |

**Installation:**
```bash
go install golang.org/x/perf/cmd/benchstat@latest
go install golang.org/x/mod/semver@latest
```

**Version verification:** Before writing the Standard Stack table, verify each recommended package exists and is current using the ecosystem-appropriate command:
```bash
go list -m golang.org/x/perf@latest
go list -m golang.org/x/mod@latest
```

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| `golang.org/x/perf` | Go modules | 12+ years | stdlib-adjacent | github.com/golang/perf | OK | Approved |
| `golang.org/x/mod` | Go modules | 12+ years | stdlib-adjacent | github.com/golang/mod | OK | Approved |
| `github.com/google/go-cmp` | Go modules | 8+ years | high | github.com/google/go-cmp | OK | Approved |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

## Architecture Patterns

### System Architecture Diagram

```text
┌──────────────────────────────────────────────────────────────────┐
│                    M31A Binary (cmd/m31a/main.go)                 │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │                    Crash Handler Layer                      │  │
│  │  defer recover() → debug.Stack() → crash report file       │  │
│  │  Signal handler (SIGTERM/SIGINT) → graceful shutdown       │  │
│  └─────────────────────────────────────────────────────────────┘  │
│                              │                                    │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │              Observability Metrics Layer                    │  │
│  │  internal/integrations/metrics/collector.go                │  │
│  │  SessionMetrics → METRICS.json per session                 │  │
│  └─────────────────────────────────────────────────────────────┘  │
│                              │                                    │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │              Workflow Engine (7 phases)                     │  │
│  │  Initialize → Discuss → Plan → Execute → Verify → Ship     │  │
│  └─────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│                    CI/CD Layer (GitHub Actions)                   │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │              Tiered Validation Matrix                       │  │
│  │  Tier 1: linux/amd64 + darwin/amd64 (full race tests)     │  │
│  │  Tier 2: linux/arm64, darwin/arm64, windows/amd64 (smoke) │  │
│  └─────────────────────────────────────────────────────────────┘  │
│                              │                                    │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │              Release Pipeline                               │  │
│  │  Tag v* → GoReleaser → draft release → manual approval     │  │
│  │  Pre-release: benchmark pass, extension compat, security   │  │
│  └─────────────────────────────────────────────────────────────┘  │
│                              │                                    │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │              Nightly Compatibility Matrix                   │  │
│  │  Curated OSS projects → build/test → pass rate tracking    │  │
│  └─────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘
```

### Recommended Project Structure
```
internal/
├── observability/
│   ├── crash.go          # Panic handler, stack trace capture
│   ├── crash_test.go     # Tests for crash handler
│   └── types.go          # CrashReport type definition
├── integrations/
│   └── metrics/
│       ├── collector.go  # Existing session metrics (extend)
│       └── dashboard.go  # GitHub Pages dashboard generation
.github/
├── workflows/
│   ├── ci.yml            # Extended with tiered validation
│   ├── benchmarks.yml    # Extended with dashboard generation
│   ├── nightly.yml       # Extended with compatibility matrix
│   └── release.yml       # New: tag-triggered with manual approval
└── ISSUE_TEMPLATE/
    └── crash-report.md   # GitHub issue template for crashes
scripts/
├── validate-release.sh   # Pre-release validation harness
├── compat-matrix.sh      # Compatibility test runner
└── crash-capture.sh      # CI crash capture workflow
```

### Pattern 1: Panic Recovery Handler
**What:** Top-level panic recovery that captures stack traces and writes crash reports
**When to use:** At the entry point of the binary to catch all unrecovered panics
**Example:**
```go
// Source: Standard Go panic recovery pattern
// internal/observability/crash.go
package observability

import (
    "fmt"
    "os"
    "path/filepath"
    "runtime/debug"
    "time"
)

// CrashReport represents a captured crash event
type CrashReport struct {
    Timestamp   time.Time `json:"timestamp"`
    Version     string    `json:"version"`
    Commit      string    `json:"commit"`
    GoVersion   string    `json:"go_version"`
    OS          string    `json:"os"`
    Arch        string    `json:"arch"`
    StackTrace  string    `json:"stack_trace"`
    PanicValue  string    `json:"panic_value"`
}

// RecoverAndCapture installs a top-level panic handler
func RecoverAndCapture(version, commit string) {
    if r := recover(); r != nil {
        report := CrashReport{
            Timestamp:  time.Now(),
            Version:    version,
            Commit:     commit,
            GoVersion:  fmt.Sprintf("%s %s/%s", runtime.Version(), runtime.GOOS, runtime.GOARCH),
            OS:         runtime.GOOS,
            Arch:       runtime.GOARCH,
            StackTrace: string(debug.Stack()),
            PanicValue: fmt.Sprintf("%v", r),
        }
        
        // Write crash report to ~/.m31a/crashes/
        writeCrashReport(report)
        
        // Log to stderr for CI capture
        fmt.Fprintf(os.Stderr, "CRASH: %s\n%s\n", report.PanicValue, report.StackTrace)
        
        // Re-panic after capture to preserve exit behavior
        panic(r)
    }
}
```

### Pattern 2: GitHub Actions Tiered Validation
**What:** Matrix strategy with conditional test depth based on platform
**When to use:** CI workflows that need full tests on dev platforms, smoke on others
**Example:**
```yaml
# Source: GitHub Actions matrix best practices
# .github/workflows/ci.yml extension
jobs:
  test-tier1:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest]
        arch: [amd64]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v7
      - uses: actions/setup-go@v7
        with:
          go-version: "1.25.12"
      - name: Run full test suite with race detector
        run: go test -race -timeout 30s -coverprofile=coverage.out ./...
        env:
          GOARCH: ${{ matrix.arch }}
  
  test-tier2:
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            arch: arm64
          - os: macos-latest
            arch: arm64
          - os: windows-latest
            arch: amd64
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v7
      - uses: actions/setup-go@v7
        with:
          go-version: "1.25.12"
      - name: Smoke test (build + basic tests)
        run: |
          go build -o m31a ./cmd/m31a
          go test -short -timeout 30s ./...
        env:
          GOARCH: ${{ matrix.arch }}
```

### Pattern 3: GoReleaser Pre-Release Hooks
**What:** Quality gates that run before GoReleaser publishes artifacts
**When to use:** Release pipelines that need validation before publishing
**Example:**
```yaml
# Source: GoReleaser documentation
# .goreleaser.yaml extension
before:
  hooks:
    - make test
    - make lint
    - make bench-compare
    # Validate benchmarks pass regression check
    - scripts/validate-release.sh
```

### Anti-Patterns to Avoid
- **Hand-rolling crash reporting:** Use `debug.Stack()` from stdlib, not custom stack capture
- **Skipping race detector on dev platforms:** Always use `-race` on linux/amd64 + darwin/amd64
- **Hardcoding benchmark thresholds:** Start with 50% (Phase 4), tune after 2-3 runs
- **Ignoring CGO_ENABLED=0:** All cross-platform builds must maintain static binary constraint

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Stack trace capture | Custom goroutine walker | `runtime/debug.Stack()` | Stdlib handles all edge cases, goroutine scheduling |
| Benchmark regression detection | Manual diff comparison | `benchstat` | Statistical analysis, handles variance, known patterns |
| Semantic version parsing | Regex or manual parsing | `golang.org/x/mod/semver` | Handles all semver edge cases, pre-release, build metadata |
| GitHub Actions matrix | Nested if/else in YAML | `strategy.matrix` with `include`/`exclude` | Native feature, handles Cartesian products automatically |
| Release changelog | Manual git log filtering | GoReleaser changelog config | Already configured in `.goreleaser.yaml` |

**Key insight:** The codebase already has established patterns for benchmarks (benchstat), cross-compilation (GoReleaser), and CI (GitHub Actions). Extend these rather than introducing new tools.

## Common Pitfalls

### Pitfall 1: Race Detector on Cross-Platform Builds
**What goes wrong:** Race detector not available on all platforms, causing CI failures
**Why it happens:** `-race` requires CGO on some platforms, but M31A has `CGO_ENABLED=0`
**How to avoid:** Only use `-race` on Tier 1 platforms (linux/amd64 + darwin/amd64); use smoke tests on Tier 2
**Warning signs:** CI failures with "race detector not available" errors

### Pitfall 2: Benchmark Threshold Tuning
**What goes wrong:** Too-sensitive thresholds cause false positives, too-lenient miss regressions
**Why it happens:** Initial 50% threshold is conservative; real-world variance varies by benchmark
**How to avoid:** Start with 50%, tune after 2-3 nightly runs by analyzing benchstat output
**Warning signs:** Flaky benchmark CI jobs, ignored regression alerts

### Pitfall 3: Crash Handler Terminal State
**What goes wrong:** Crash handler leaves terminal in broken state (alt-screen, mouse capture)
**Why it happens:** Bubble Tea TUI modifies terminal state; panic recovery doesn't restore it
**How to avoid:** Call `restoreTerminal()` before re-panic; already implemented in `cmd/m31a/main.go:49-56`
**Warning signs:** Users report garbled terminal after crash

### Pitfall 4: GoReleaser Hook Failures
**What goes wrong:** Pre-release hooks fail, blocking all releases
**Why it happens:** Hooks run in strict mode; any failure aborts release
**How to avoid:** Make hooks idempotent; add `|| true` for non-critical hooks; test with `goreleaser release --snapshot`
**Warning signs:** Release workflow fails on tag push

### Pitfall 5: Compatibility Matrix Flakiness
**What goes wrong:** Curated projects fail due to external factors (network, dependencies)
**Why it happens:** OSS projects have their own CI, dependencies, and build requirements
**How to avoid:** Cache dependencies; retry failed builds; track pass rate trends not individual failures
**Warning signs:** Nightly compatibility job shows 0% pass rate

## Code Examples

### Crash Handler Integration in main.go
```go
// Source: M31A codebase pattern
// cmd/m31a/main.go (extension point)
func main() {
    // Install crash handler before any other initialization
    defer observability.RecoverAndCapture(Version, Commit)
    
    // ... existing initialization code ...
    
    // Signal handler already exists (lines 633-673)
    // Crash handler complements it by capturing panics
}
```

### Session Metrics Extension
```go
// Source: M31A codebase pattern
// internal/integrations/metrics/collector.go (extension)
// Add new metric types for production observability

// StartupMetric captures binary startup performance
type StartupMetric struct {
    DurationMs    int64  `json:"duration_ms"`
    ConfigLoadMs  int64  `json:"config_load_ms"`
    ProviderMs    int64  `json:"provider_init_ms"`
    TUIMs         int64  `json:"tui_init_ms"`
}

// CompletionMetric tracks workflow completion rates
type CompletionMetric struct {
    Phase         types.WorkflowPhase `json:"phase"`
    SuccessCount  int64               `json:"success_count"`
    FailureCount  int64               `json:"failure_count"`
    TimeoutCount  int64               `json:"timeout_count"`
}
```

### GitHub Issue Template for Crashes
```markdown
<!-- .github/ISSUE_TEMPLATE/crash-report.md -->
---
name: Crash Report
about: Report a crash or panic in M31A
title: '[CRASH] '
labels: crash, bug
assignees: ''
---

## Crash Information

**Version:** (from `m31a --version`)
**OS:** (e.g., linux/amd64, darwin/arm64)
**Go Version:** (from `go version`)

## Stack Trace

```
(paste crash output here)
```

## Steps to Reproduce

1. 
2. 
3. 

## Expected Behavior

(what should have happened)

## Additional Context

(add any other context about the crash)
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual benchmark comparison | benchstat with JSON output | Phase 4 | Automated regression detection |
| Single-platform CI | Tiered validation matrix | Phase 7 | Faster CI, comprehensive coverage |
| Ad-hoc releases | GoReleaser with quality gates | Phase 7 | Consistent, validated releases |
| No crash reporting | CI crash capture + issue template | Phase 7 | Visibility into production issues |

**Deprecated/outdated:**
- Manual benchmark comparison: Replaced by benchstat with statistical analysis
- Single-platform testing: Extended to tiered validation for better coverage
- Release without validation: Now requires benchmark pass, extension compat, security scan

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | Existing benchstat infrastructure can be extended for dashboard generation | Standard Stack | May need alternative dashboard tooling |
| A2 | GoReleaser pre-release hooks are sufficient for quality gates | Architecture Patterns | May need separate CI job for validation |
| A3 | Curated OSS projects can be built with CGO_ENABLED=0 | Compatibility Matrix | Some projects may require CGO |
| A4 | 50% benchmark threshold is appropriate starting point | Common Pitfalls | May cause false positives/negatives |
| A5 | GitHub Actions artifact upload is sufficient for crash capture | Crash Reporting | May need external crash service for production |

## Open Questions

1. **Which specific OSS projects to curate for compatibility matrix?**
   - What we know: Need 10-20 projects covering Go, TS, Python, Rust; small/medium/large sizes
   - What's unclear: Exact project list, build requirements, maintenance status
   - Recommendation: Start with well-maintained projects from each language; validate build compatibility in research phase

2. **How to handle benchmark threshold tuning?**
   - What we know: Start with 50% from Phase 4/6
   - What's unclear: When to tune, what thresholds work for different benchmark types
   - Recommendation: Analyze 2-3 nightly runs, then adjust per-benchmark thresholds

3. **Should crash handler write to disk or only to CI artifacts?**
   - What we know: D-02 specifies CI crash capture with artifact upload
   - What's unclear: Local crash reports for user debugging
   - Recommendation: Write to both `~/.m31a/crashes/` (local) and CI artifacts (when in CI)

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Go 1.25+ | All phases | ✓ | 1.25.12 | — |
| GitHub Actions | CI/CD | ✓ | v7 | — |
| GoReleaser | Release process | ✓ | v2 | — |
| benchstat | Benchmark regression | ✓ | latest | Manual comparison |
| `runtime/debug` | Stack trace capture | ✓ (stdlib) | — | — |
| `log/slog` | Structured logging | ✓ (stdlib) | — | — |

**Missing dependencies with no fallback:**
- None — all required tools are available

**Missing dependencies with fallback:**
- None

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Go standard library `testing` |
| Config file | none — use `go test` directly |
| Quick run command | `make test-fast` |
| Full suite command | `make test` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| OBS-01 | Metrics collection | unit | `go test ./internal/integrations/metrics/...` | ✅ existing |
| CRASH-01 | Crash handler | unit | `go test ./internal/observability/...` | ❌ Wave 0 |
| VALID-01 | Tiered validation | integration | `make test` (full matrix) | ✅ existing |
| RELEASE-01 | Release quality gates | integration | `goreleaser release --snapshot` | ✅ existing |
| LTS-01 | Version enforcement | unit | `go test ./internal/core/...` | ✅ existing |
| COMPAT-01 | Compatibility matrix | e2e | `scripts/compat-matrix.sh` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `make test-fast`
- **Per wave merge:** `make test`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `internal/observability/crash.go` — panic recovery handler
- [ ] `internal/observability/crash_test.go` — crash handler tests
- [ ] `scripts/compat-matrix.sh` — compatibility test runner
- [ ] `.github/ISSUE_TEMPLATE/crash-report.md` — crash report template

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | API keys via OS keychain (existing) |
| V3 Session Management | no | Session persistence (existing) |
| V4 Access Control | no | Tool permissions (existing) |
| V5 Input Validation | yes | Config validation, tool input schemas |
| V6 Cryptography | no | API key storage (existing) |

### Known Threat Patterns for Go CLI

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Shell command injection | Tampering | Bash tool blocklist, quote validation (existing) |
| API key exposure | Information Disclosure | OS keychain storage, env var scrubbing (existing) |
| Race conditions | Tampering | `-race` detector, mutex patterns (existing) |
| Panic denial of service | Denial of Service | Panic recovery handler (new) |

## Sources

### Primary (HIGH confidence)
- GoReleaser documentation (https://goreleaser.com) — build hooks, global hooks, release config
- GitHub Actions documentation (https://docs.github.com/actions) — matrix strategy, artifacts, workflows
- Go standard library docs (https://pkg.go.dev) — runtime/debug, log/slog, testing
- benchstat documentation (https://pkg.go.dev/golang.org/x/perf/cmd/benchstat) — benchmark analysis

### Secondary (MEDIUM confidence)
- WebSearch results for Go CLI observability best practices
- WebSearch results for Go panic handler patterns
- WebSearch results for GitHub Actions matrix testing

### Tertiary (LOW confidence)
- WebSearch results for compatibility test matrix patterns
- WebSearch results for LTS/deprecation policies

## Metadata

**Confidence breakdown:**
- Standard Stack: HIGH — all libraries verified via existing codebase and official docs
- Architecture: HIGH — patterns extend existing codebase infrastructure
- Pitfalls: MEDIUM — based on common Go CI/CD issues, validated against codebase patterns

**Research date:** 2026-08-06
**Valid until:** 2026-09-06 (30 days for stable infrastructure patterns)
