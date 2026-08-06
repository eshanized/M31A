# Phase 7: Production Readiness - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-06
**Phase:** 7-Production Readiness
**Areas discussed:** Observability Stack & Metrics, Crash & Failure Reporting, Cross-Platform Validation, Release Process & Quality Gates, LTS & Deprecation Policy, Compatibility Test Matrix

---

## Observability Stack & Metrics

| Option | Description | Selected |
|--------|-------------|----------|
| GitHub Actions + Pages (benchstat JSON) | Use GitHub Actions + GitHub Pages for dashboards (benchstat JSON trends). Zero infra cost, integrated with CI, version-controlled history. | ✓ |
| Prometheus + Grafana Cloud | Add Prometheus metrics endpoint to M31A, scrape via GitHub Actions or self-hosted Prometheus, visualize in Grafana Cloud (free tier). More metrics types, better alerting. | |
| OpenTelemetry + Vendor (Honeycomb/Datadog) | Use OpenTelemetry in M31A, export to Honeycomb/Datadog/Grafana Cloud. Full traces + metrics + logs, but higher complexity and cost. | |
| Let the agent decide | Let the agent research and recommend based on codebase patterns and constraints. | |

**User's choice:** GitHub Actions + Pages (benchstat JSON)
**Notes:** Extends Phase 4/6 benchmark infrastructure. Zero infra cost, CI-integrated, version-controlled trend history.

---

## Crash & Failure Reporting

| Option | Description | Selected |
|--------|-------------|----------|
| GitHub Actions CI Crash Capture | Use GitHub Actions to capture crashes from CI runs, upload artifacts (stack traces, coredumps), auto-file GitHub issues with template. No external service needed. | ✓ |
| Sentry (or similar) SDK Integration | Integrate Sentry SDK (or similar) in M31A binary for crash reporting with stack traces, breadcrumbs, environment context. Free tier available, better UX for crash analysis. | |
| Local Crash Dumps + Manual Reporting | Add crash handler in M31A that writes minidump/coredump to disk on panic, with reproduction steps template. User manually files issue with dump attached. | |
| Let the agent decide | Let the agent research and recommend based on Go ecosystem and M31A constraints. | |

**User's choice:** GitHub Actions CI Crash Capture
**Notes:** Captures crashes from CI runs, uploads artifacts, auto-files GitHub issues with template. No external service dependency.

---

## Cross-Platform Validation

| Option | Description | Selected |
|--------|-------------|----------|
| Full CI Matrix (6 targets, full test suite) | Test on all 6 targets in CI (GitHub Actions matrix). Use self-hosted runners for macOS ARM64 if needed. Run full test suite including race detector on each. | |
| Full Tests on Linux + Smoke on Others | Full test suite on linux/amd64 (primary). Smoke tests (build + basic tests) on other 5 targets. Reduces CI time/cost. | |
| Tiered: Full on Dev Platforms + Smoke | Full test suite on linux/amd64 + darwin/amd64 (dev platforms). Smoke on linux/arm64, darwin/arm64, windows/amd64. | ✓ |
| Let the agent decide | Let the agent research and recommend based on Go ecosystem and CI constraints. | |

**User's choice:** Tiered: Full on Dev Platforms + Smoke
**Notes:** Full test suite (with race detector) on linux/amd64 + darwin/amd64 (dev platforms); smoke tests (build + basic tests) on linux/arm64, darwin/arm64, windows/amd64.

---

## Release Process & Quality Gates

| Option | Description | Selected |
|--------|-------------|----------|
| Tag-Triggered GoReleaser + Manual Approve | Tag-based: push v* tag → GoReleaser builds all platforms, generates changelog from conventional commits, creates GitHub Release draft. Manual review → publish. | ✓ |
| Release Branch → Tag → GoReleaser | Release branch (e.g., release/v1.2) → CI runs full validation → auto-merge to main → tag → GoReleaser publishes. More formal, supports hotfix branches. | |
| Tag-Triggered + Manual Gate in CI | GoReleaser runs on tag, produces artifacts, but requires manual 'approve' in GitHub Actions before publish. Adds human gate. | |
| Let the agent decide | Let the agent research and recommend based on GoReleaser patterns and M31A constraints. | |

**User's choice:** Tag-Triggered GoReleaser + Manual Approve
**Notes:** Push v* tag, GoReleaser builds all 6 platforms, generates changelog from conventional commits, creates draft release, manual review → publish.

---

## LTS & Deprecation Policy

| Option | Description | Selected |
|--------|-------------|----------|
| Rolling: Current + 1 Minor (6mo) + Major (12mo) | Latest release gets full support. Previous minor version gets security fixes for 6 months. Major versions get 12 months security support. Clear EOL dates published. | ✓ |
| Fixed: 12mo Minor / 24mo Major (LTS every 6mo) | Each minor version supported for 12 months with security patches. Major versions get 24 months. LTS releases every 6 months get 24 months support. | |
| Rolling Window: Only Latest (Go-style) | Only latest release supported. Users must upgrade for fixes. Simple but aggressive. Matches Go release cycle (2 releases/year). | |
| Let the agent decide | Let the agent research and recommend based on Go ecosystem and M31A constraints. | |

**User's choice:** Rolling: Current + 1 Minor (6mo) + Major (12mo)
**Notes:** Current release + 1 previous minor (6 months security fixes) + major versions (12 months security fixes). Clear EOL dates published.

---

## Compatibility Test Matrix

| Option | Description | Selected |
|--------|-------------|----------|
| Curated Real-World Projects (10-20, Nightly) | Curate 10-20 real-world projects (OSS) covering: Go/TS/Python/Rust, small/medium/large, frameworks, mono/polyrepo. Run nightly in CI. Track pass rate per category. | ✓ |
| Synthetic Matrix (Programmatic Generation) | Generate synthetic projects programmatically covering combinatorial matrix: language x size x framework x repo type. More systematic, easier to maintain, but less realistic. | |
| Flagship Projects First (3-5, Expand Later) | Start with 3-5 flagship OSS projects (k8s, cockroachdb, etc.) as canaries. Expand based on adoption. Lower initial investment, grows with ecosystem. | |
| Let the agent decide | Let the agent research and recommend based on Go ecosystem and CI constraints. | |

**User's choice:** Curated Real-World Projects (10-20, Nightly)
**Notes:** Curate 10-20 OSS projects covering languages/sizes/frameworks, run nightly in CI, track pass rate per category.

---

## the agent's Discretion

- Agent may choose specific metrics to collect beyond benchmarks (startup time, memory, CPU, completion rate)
- Agent may design crash handler in M31A binary (panic hook, stack trace capture, minidump)
- Agent may select specific curated projects for compatibility matrix
- Agent may design semantic versioning enforcement in CI (block breaking changes without MAJOR bump)
- Agent may design deprecation warning system (log warnings, docs notices)
- Agent may choose benchmark thresholds for regression detection (currently 50% from Phase 4/6)

---

## Deferred Ideas

None — discussion stayed within phase scope