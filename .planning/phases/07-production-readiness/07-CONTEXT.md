# Phase 7: Production Readiness - Context

**Gathered:** 2026-08-06
**Status:** Ready for planning

<domain>
## Phase Boundary

Reach a level suitable for daily professional use. This phase covers: observability stack and metrics (building on Phase 4/6 benchmarks), crash and failure reporting, cross-platform validation strategy, release process with quality gates, LTS and deprecation policy, and compatibility test matrix for real-world projects. Exit criteria: M31A is dependable enough to become a developer's primary terminal coding assistant.
</domain>

<decisions>
## Implementation Decisions

### Observability Stack & Metrics
- **D-01:** GitHub Actions + Pages with benchstat JSON — zero infra cost, CI-integrated, version-controlled trend history. Extends Phase 4/6 benchmark infrastructure. — **Reversibility:** reversible — can add Prometheus/Grafana later if needed

### Crash & Failure Reporting
- **D-02:** GitHub Actions CI Crash Capture — captures crashes from CI runs, uploads artifacts (stack traces, coredumps), auto-files GitHub issues with template. No external service dependency. — **Reversibility:** reversible — can add Sentry SDK later if needed

### Cross-Platform Validation
- **D-03:** Tiered validation — full test suite (with race detector) on linux/amd64 + darwin/amd64 (dev platforms); smoke tests (build + basic tests) on linux/arm64, darwin/arm64, windows/amd64. — **Reversibility:** reversible — can expand to full matrix later if resources allow

### Release Process & Quality Gates
- **D-04:** Tag-triggered GoReleaser with manual approval — push v* tag, GoReleaser builds all 6 platforms, generates changelog from conventional commits, creates draft release, manual review → publish. — **Reversibility:** reversible — can add release branch workflow later

### LTS & Deprecation Policy
- **D-05:** Rolling support — current release + 1 previous minor (6 months security fixes) + major versions (12 months security fixes). Clear EOL dates published. — **Reversibility:** reversible — can extend support windows later

### Compatibility Test Matrix
- **D-06:** Curated real-world projects — 10-20 OSS projects covering languages (Go, TS, Python, Rust), sizes (small/medium/large), frameworks, mono/polyrepo. Run nightly in CI, track pass rate per category. — **Reversibility:** costly — infrastructure for cloning/building/maintaining 10-20 repos is significant to remove

### the agent's Discretion
- Agent may choose specific metrics to collect beyond benchmarks (startup time, memory, CPU, completion rate)
- Agent may design crash handler in M31A binary (panic hook, stack trace capture, minidump)
- Agent may select specific curated projects for compatibility matrix
- Agent may design semantic versioning enforcement in CI (block breaking changes without MAJOR bump)
- Agent may design deprecation warning system (log warnings, docs notices)
- Agent may choose benchmark thresholds for regression detection (currently 50% from Phase 4/6)
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Architecture & Codebase
- `.planning/codebase/ARCHITECTURE.md` — System overview, component responsibilities, data flow, anti-patterns, engine file organization, entry points
- `.planning/codebase/STACK.md` — Technology stack, dependencies, platform requirements, Go 1.25, CGO_ENABLED=0
- `.planning/codebase/CONCERNS.md` — Tech debt, known bugs, test coverage gaps, fragile areas, security considerations, scaling limits
- `.planning/codebase/CONVENTIONS.md` — Coding conventions, file organization, concurrency, logging, interface boundaries, error handling, testing, commit messages
- `.planning/codebase/INTEGRATIONS.md` — LLM providers, web services, auth, monitoring, CI/CD, git integration, shell execution, code intelligence

### Key Source Files
- `internal/core/config/loader.go` — Config loading with TOML parsing, env var override, hot-reload via fsnotify
- `internal/tools/dispatcher.go` — Tool registration via `RegisterTool()`, permission policies, rate limiting; adapter pattern for external tools
- `internal/integrations/provider/registry.go` — LLM provider registration via `Register()`, health checks, model caching; adapter pattern for external providers
- `internal/engine/workflow/engine.go` — Core workflow engine, RunPhase orchestration (extension point for phase hooks)
- `internal/engine/workflow/engine_concurrency.go` — Lock ordering hierarchy (must maintain for any new locks)
- `cmd/m31a/main.go` — CLI entry, flag parsing, config load, provider registration, TUI launch
- `.github/workflows/ci.yml` — Existing CI jobs (lint, test, security, build matrix, release); extend with benchmark, nightly, compat matrix
- `.goreleaser.yaml` — Cross-platform builds (6 targets), packaging (tar.gz, deb, rpm, apk, archlinux, scoop), changelog config
- `.github/workflows/benchmarks.yml` — Benchmark CI with benchstat regression detection (50% threshold)
- `.github/workflows/nightly.yml` — Scheduled nightly workflow (full test + bench + extension compat)
- `.github/dependabot.yml` — Weekly Go module updates with grouping
- `Makefile` — Build targets including bench, bench-compare, bench-save

### Standards
- `AGENTS.md` — Build commands, code style, conventional commits, lint config

### Phase Context (Prior Decisions)
- `.planning/phases/01-reliability-first/01-CONTEXT.md` — Concurrency, cancellation, error handling, testing decisions
- `.planning/phases/02-user-experience/02-CONTEXT.md` — UI patterns, status bar, modals, first-run decisions
- `.planning/phases/03-engineering-excellence/03-CONTEXT.md` — Engine split, module boundaries, documentation, DX decisions
- `.planning/phases/04-performance/04-DISCUSSION-LOG.md` — Performance decisions (lazy loading, buffer pools, parallel indexing)
- `.planning/phases/05-05-intelligence/05-CONTEXT.md` — Plan generation, context management, model routing, verification decisions
- `.planning/phases/06-ecosystem/06-CONTEXT.md` — Extension loading, plugin API, config profiles, workflow hooks, distribution, docs, CI automation
</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `.github/workflows/benchmarks.yml` — Benchmark CI with benchstat regression detection (50% threshold), gh-pages dashboard commit
- `.github/workflows/nightly.yml` — Scheduled nightly workflow with full test + bench + extension compat
- `.github/workflows/ci.yml` — CI pipeline with lint, test, security, build matrix, release, now extended with benchmark job
- `.github/workflows/pr-checks.yml` — PR validation workflow (lint, test, build, security, optional benchmark)
- `.github/dependabot.yml` — Weekly Go module updates with grouping
- `.goreleaser.yaml` — Cross-platform builds (6 targets), conventional commit changelog, extension compat hooks
- `internal/engine/workflow/engine.go` — Core engine with RunPhase, can add crash handler in Shutdown()
- `cmd/m31a/main.go` — Can add panic handler for crash reporting
- `Makefile` — bench, bench-compare, bench-save targets for local benchmark runs
- `.github/workflows/benchmarks.yml` — gh-pages commit for benchmark trend data

### Established Patterns
- GoReleaser for cross-platform releases — add quality gates (benchmark pass, extension compat)
- GitHub Actions for CI/CD — add jobs for crash capture, compatibility matrix, nightly
- GitHub Pages + benchstat JSON for dashboard — extend with more metrics
- Conventional commits (`feat:`, `fix:`, `docs:`, etc.) — automated changelog in GoReleaser
- Go 1.25, CGO_ENABLED=0, static binaries — cross-platform validation matrix
- Bubble Tea Elm architecture — crash handler in main.go panic recovery
- Go standard library `testing` + `go test -race` — race detector on dev platforms

### Integration Points
- `cmd/m31a/main.go` — Add panic handler for crash capture, install signal handlers
- `.github/workflows/ci.yml` — Add benchmark job dependency to release, add cross-platform matrix tiers
- `.github/workflows/nightly.yml` — Add compatibility matrix job (curated projects)
- `.goreleaser.yaml` — Add pre-release validation hooks (benchmark pass, extension compat)
- `.github/dependabot.yml` — Already configured for weekly updates
- `Makefile` — bench targets for local development
</code_context>

<specifics>
## Specific Ideas

- **Crash handler in main.go:** Install `panic` recovery that captures stack trace, writes minidump to `.m31a/crashes/`, auto-files GitHub issue via `gh` CLI if in CI
- **Metrics to collect:** startup time, workflow execution time, tool dispatch latency, memory allocation, completion rate, verification rate, cancellation rate, retry rate, resource usage
- **Curated projects:** kubernetes/kubernetes, golang/go, cockroachdb/cockroach, etcd-io/etcd, hashicorp/terraform, prometheus/prometheus, grafana/grafana, docker/compose, microsoft/vscode, facebook/react (diverse languages, sizes, frameworks)
- **Semantic versioning enforcement in CI:** Check for breaking changes in pkg/extensions types.go, config schema, CLI flags — require MAJOR bump
- **Deprecation warnings:** Log warnings when deprecated config fields/APIs used, emit docs notices, track in CHANGELOG
- **Benchmark thresholds:** Start with 50% (from Phase 4), tune after 2-3 runs per RESEARCH.md Pitfall 5
- **Cross-platform tiered CI:** Full race tests on linux/amd64 + darwin/amd64; smoke (build + basic test) on linux/arm64, darwin/arm64, windows/amd64
</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope
</deferred>

---

*Phase: 07-Production Readiness*
*Context gathered: 2026-08-06*