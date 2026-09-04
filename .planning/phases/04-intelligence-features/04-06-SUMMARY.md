---
phase: 04-intelligence-features
plan: 06
subsystem: intelligence
tags: [deps.dev, osv, github, risk-classification, resilience, http-clients]

# Dependency graph
requires:
  - phase: 04-intelligence-features
    plan: 01
    provides: "Confidence enum, Evidence/EvidencePack/Citation, intelligence event vocabulary, error sentinels, [intelligence] config schema with DepsRiskConfig (stale_months, young_months, license_allowlist)"
provides:
  - "deps.dev v3 client: GetPackage/GetVersion/GetProject with PathEscape, 404 sentinel, Retry-After 429 handling, 2MB body cap, UTC timestamp normalization"
  - "OSV.dev client: QueryVulns with leading-v normalization, build-metadata distinct queries, severity-descending then ID sorting, QueriedOK vs ErrSourceUnreachable distinction"
  - "GitHub enrichment client: RepoActivity with PushedAt/Archived/OpenIssuesAndPRs/Stars, auth header only when token present, 403+zero-remaining rate-limit mapping with GITHUB_TOKEN hint"
  - "Config-driven risk classification engine (D-16): ClassifyRisk pure function applying 6 rules (vuln-present, license-external, release-stale, repo-archived, popularity-young, api-instability) with Source provenance (osv|depsdev|github) and highest-severity collapse"
affects: [04-07-deps-verdict-cache]

# Actuals (#2632)
actuals:
  tokens: 14814
  tasks: 3
  commits: 6

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "TDD per-task: RED (failing test) → GREEN (implementation) → commit pair"
    - "httptest.Server fixtures for zero-network CI; recorded JSON testdata committed"
    - "Layered resilience: hard client timeout + initial_only retry (3 attempts, 1s base) + Retry-After honoring + 2MB LimitReader + required-field validation"
    - "Typed error sentinels (ErrPackageNotFound, ErrSourceUnreachable) over generic errors for honest degradation"
    - "Source provenance on every risk rule (DEPEND-02): TriggeredRule.Source names originating snapshot"
    - "Case-exact SPDX license matching per ecosystem case rules (Go module paths case-sensitive)"
    - "Boundary convention: release age >= stale_months classifies high (greater-or-equal)"
    - "Pure deterministic classifiers: no I/O, no clock, precomputed ages passed in"

key-files:
  created:
    - internal/intelligence/deps/depsdev.go
    - internal/intelligence/deps/depsdev_test.go
    - internal/intelligence/deps/osv.go
    - internal/intelligence/deps/osv_test.go
    - internal/intelligence/deps/github_enrich.go
    - internal/intelligence/deps/risk.go
    - internal/intelligence/deps/risk_test.go
    - internal/intelligence/deps/testdata/depsdev_package.json
    - internal/intelligence/deps/testdata/depsdev_version.json
    - internal/intelligence/deps/testdata/depsdev_project.json
    - internal/intelligence/deps/testdata/osv_query.json
    - internal/intelligence/deps/testdata/github_repo.json
  modified: []

key-decisions:
  - "Module path percent-encoding via url.PathEscape preserves nested paths and /v2 major suffixes exactly as deps.dev v3 expects (uppercase GO system enum)"
  - "OSV version normalization: v-less input gains leading 'v' prefix for Go ecosystem; build-metadata suffix preserved verbatim making v1.0.0+build a distinct query from v1.0.0"
  - "GitHub open_issues_count labeled OpenIssuesAndPRs in struct name and rendering per Pitfall 6 (includes PRs)"
  - "Risk classification rules evaluate in fixed D-16 order; multiple hits collapse to highest severity while all rules stay listed with Source provenance"
  - "Zero triggered rules yields RiskClass low with empty-but-non-nil TriggeredRules slice (JSON consumers see [] not null)"
  - "All three clients share package-local http helper (LimitReader cap + required-field validation) to avoid duplication without importing provider internals"

patterns-established:
  - "httptest fixtures with server-side path inspection prove url.PathEscape application"
  - "Retry-After test injects short delays via client hook asserting second attempt timing without exceeding attempt budget"
  - "QueriedOK flag distinguishes empty-result-from-successful-query vs transport-failure-yielding-zero-vulns"
  - "Rate-limit error hints mention GITHUB_TOKEN for actionable operator guidance"

requirements-completed: [DEPEND-01, DEPEND-02]

# Coverage metadata (#1602)
coverage:
  - id: D1
    description: "deps.dev v3 client with GetPackage/GetVersion/GetProject, PathEscape, 404 sentinel, 429 Retry-After, 2MB cap, UTC timestamps"
    requirement: DEPEND-01
    verification:
      - kind: unit
        ref: "internal/intelligence/deps/depsdev_test.go#TestDepsDevGetPackageHappyPath"
        status: pass
      - kind: unit
        ref: "internal/intelligence/deps/depsdev_test.go#TestDepsDevGetPackageEscapesModulePath"
        status: pass
      - kind: unit
        ref: "internal/intelligence/deps/depsdev_test.go#TestDepsDevGetPackageNotFoundSentinel"
        status: pass
      - kind: unit
        ref: "internal/intelligence/deps/depsdev_test.go#TestDepsDevGetPackageRetryAfterOn429"
        status: pass
      - kind: unit
        ref: "internal/intelligence/deps/depsdev_test.go#TestDepsDevGetPackageOversizedResponseCapped"
        status: pass
      - kind: unit
        ref: "internal/intelligence/deps/depsdev_test.go#TestDepsDevGetPackageMissingVersionKeyRejected"
        status: pass
    human_judgment: false
  - id: D2
    description: "OSV.dev client with QueryVulns ordering severity-descending then ID, leading-v normalization, build-metadata distinct queries, QueriedOK vs ErrSourceUnreachable"
    requirement: DEPEND-01
    verification:
      - kind: unit
        ref: "internal/intelligence/deps/osv_test.go#TestOSVQueryVulnsOrderingSeverityDescendingThenID"
        status: pass
      - kind: unit
        ref: "internal/intelligence/deps/osv_test.go#TestOSVVersionNormalizationDistinctBodies"
        status: pass
      - kind: unit
        ref: "internal/intelligence/deps/osv_test.go#TestOSVBuildMetadataPreservedVerbatim"
        status: pass
      - kind: unit
        ref: "internal/intelligence/deps/osv_test.go#TestOSVQueriedEmptyDistinctFromUnreachable"
        status: pass
    human_judgment: false
  - id: D3
    description: "GitHub enrichment client with RepoActivity, auth header conditional, 403+zero-remaining rate-limit hint"
    requirement: DEPEND-02
    verification:
      - kind: unit
        ref: "internal/intelligence/deps/osv_test.go#TestGitHubRepoActivityParsesArchivedAndCounts"
        status: pass
      - kind: unit
        ref: "internal/intelligence/deps/osv_test.go#TestGitHubAuthHeaderPresenceFollowsToken"
        status: pass
      - kind: unit
        ref: "internal/intelligence/deps/osv_test.go#TestGitHubRateLimitMapsToTypedListedErrorWithHint"
        status: pass
    human_judgment: false
  - id: D4
    description: "Config-driven risk classification engine: 6 D-16 rules, Source provenance, highest-severity collapse, case-exact SPDX, >= stale boundary, pure deterministic"
    requirement: DEPEND-02
    verification:
      - kind: unit
        ref: "internal/intelligence/deps/risk_test.go#TestClassifyRiskVulnPresentIsHighFromOSV"
        status: pass
      - kind: unit
        ref: "internal/intelligence/deps/risk_test.go#TestClassifyRiskLicenseAllowlistCaseExact"
        status: pass
      - kind: unit
        ref: "internal/intelligence/deps/risk_test.go#TestClassifyRiskStaleBoundaryGreaterOrEqual"
        status: pass
      - kind: unit
        ref: "internal/intelligence/deps/risk_test.go#TestClassifyRiskRepoArchivedHighFromGitHub"
        status: pass
      - kind: unit
        ref: "internal/intelligence/deps/risk_test.go#TestClassifyRiskPopularityYoungMedium"
        status: pass
      - kind: unit
        ref: "internal/intelligence/deps/risk_test.go#TestClassifyRiskAPIInstabilityMedium"
        status: pass
      - kind: unit
        ref: "internal/intelligence/deps/risk_test.go#TestClassifyRiskMultipleRulesCollapseToMaxSeverityAllListed"
        status: pass
      - kind: unit
        ref: "internal/intelligence/deps/risk_test.go#TestClassifyRiskZeroRulesLowWithNonNilSlice"
        status: pass
      - kind: unit
        ref: "internal/intelligence/deps/risk_test.go#TestClassifyRiskDeterministicAcrossRuns"
        status: pass
      - kind: unit
        ref: "internal/intelligence/deps/risk_test.go#TestClassifyRiskEveryRuleCarriesProvenanceSource"
        status: pass
    human_judgment: false

# Metrics
duration: 15 min
completed: 2026-09-05
status: complete
---

# Phase 04 Plan 06: Dependency Intelligence Data Layer Summary

**deps.dev v3, OSV.dev, GitHub enrichment clients with Phase 2 resilience semantics + config-driven D-16 risk classification engine — all fixture-driven, zero-network CI, deterministic ClassifyRisk over cited source snapshots**

## Performance

- **Duration:** 15 min (core TDD execution on 2026-08-25; risk GREEN completed 2026-09-05)
- **Started:** 2026-08-25T10:57:03Z
- **Completed:** 2026-09-05T18:52:53Z
- **Tasks:** 3
- **Files modified:** 12 (7 Go files + 5 JSON fixtures)

## Accomplishments

- **deps.dev v3 client production-ready:** GET v3/systems/GO/packages/{PathEscape(module)} + versions/{version} + projects/{id} with hardened transport (hard timeout, initial_only retry 3/1s, Retry-After on 429, 2MB LimitReader, required-field validation, UTC timestamps). 404 maps to typed ErrPackageNotFound (exists=false path), not a crash.
- **OSV.dev client with honest unknown semantics:** POST /v1/query with Go ecosystem + leading-v normalization; build-metadata suffix makes distinct queries; severity-descending then ID ascending sort; QueriedOK flag distinguishes queried-empty from transport failure (ErrSourceUnreachable).
- **GitHub enrichment with rate-limit transparency:** RepoActivity parses pushed_at/archived/open_issues_count (labeled OpenIssuesAndPRs)/stars; Authorization header only when GITHUB_TOKEN set; 403 with X-RateLimit-Remaining=0 maps to typed rate-limit error hinting GITHUB_TOKEN.
- **D-16 risk classification engine complete:** ClassifyRisk pure function applying 6 config-driven rules (vuln-present high/osv, license-external high/depsdev, release-stale high/depsdev, repo-archived high/github, popularity-young medium/depsdev, api-instability medium/depsdev). Every TriggeredRule carries Source provenance. Boundary: age >= stale_months is high. Case-exact SPDX allowlist. Deterministic byte-equal across runs.

## Task Commits

Each task was committed atomically (TDD RED → GREEN pairs):

1. **Task 1 RED: failing tests for deps.dev v3 client** - `d982f796` (test)
2. **Task 1 GREEN: implement deps.dev v3 client** - `12f683da` (feat)
3. **Task 2 RED: failing tests for OSV and GitHub enrichment clients** - `630328a3` (test)
4. **Task 2 GREEN: implement OSV and GitHub enrichment clients** - `8e484076` (feat)
5. **Task 3 RED: failing tests for config-driven risk classification (D-16)** - `f8e02c4a` (test)
6. **Task 3 GREEN: implement config-driven risk classification engine (D-16)** - `d4c773e4` (feat)

_Note: 4 earlier commits (78d9611d, ba485fb1, e862c63a, e5fca7ce) from prior exploratory work also carry 04-06 tag but are not part of this plan's TDD execution._

## Files Created/Modified

- `internal/intelligence/deps/depsdev.go` - deps.dev v3 client (NewDepsDevClient, GetPackage, GetVersion, GetProject, ErrPackageNotFound)
- `internal/intelligence/deps/depsdev_test.go` - 10 test cases covering happy paths, escaping, 404, 429 retry-after, 2MB cap, missing fields, empty versions, transport retries, context cancellation
- `internal/intelligence/deps/osv.go` - OSV client (NewOSVClient, QueryVulns, Vuln struct, version normalization, severity sorting)
- `internal/intelligence/deps/osv_test.go` - 7 test cases: ordering, fixed-in collection, v-prefix normalization, build-metadata distinct, queried-empty vs unreachable, GitHub activity/auth/rate-limit
- `internal/intelligence/deps/github_enrich.go` - GitHub enricher (NewGitHubEnricher, RepoActivity, repo-from-module heuristic, rate-limit mapping)
- `internal/intelligence/deps/risk.go` - Risk engine (RiskInput, RiskAssessment, TriggeredRule, ClassifyRisk, licenseAllowlisted, joinSignals)
- `internal/intelligence/deps/risk_test.go` - 10 table-driven tests: all 6 rules, boundary exact-threshold, provenance, case-exact SPDX, max-severity collapse, zero-rule low, determinism
- `internal/intelligence/deps/testdata/depsdev_package.json` - deps.dev package fixture
- `internal/intelligence/deps/testdata/depsdev_version.json` - deps.dev version fixture
- `internal/intelligence/deps/testdata/depsdev_project.json` - deps.dev project fixture
- `internal/intelligence/deps/testdata/osv_query.json` - OSV multi-vuln fixture with severity ordering
- `internal/intelligence/deps/testdata/github_repo.json` - GitHub repo fixture with archived/pushed_at/stars

## Decisions Made

- Module path percent-encoding via url.PathEscape preserves nested paths and /v2 major suffixes exactly as deps.dev v3 expects (uppercase GO system enum)
- OSV version normalization: v-less input gains leading 'v' prefix for Go ecosystem; build-metadata suffix preserved verbatim making v1.0.0+build a distinct query from v1.0.0
- GitHub open_issues_count labeled OpenIssuesAndPRs in struct name and rendering per Pitfall 6 (includes PRs)
- Risk classification rules evaluate in fixed D-16 order; multiple hits collapse to highest severity while all rules stay listed with Source provenance
- Zero triggered rules yields RiskClass low with empty-but-non-nil TriggeredRules slice (JSON consumers see [] not null)
- All three clients share package-local http helper (LimitReader cap + required-field validation) to avoid duplication without importing provider internals

## Deviations from Plan

### Auto-fixed Issues

None - plan executed exactly as written. All must-have truths verified by passing tests.

---

**Total deviations:** 0 auto-fixed
**Impact on plan:** No deviations required; implementation matches all acceptance criteria and must-have truths.

## Issues Encountered

- Pre-existing broken packages in repo (internal/engine/session, internal/engine/taskrunner, internal/integrations/ledger, internal/tools/todo) cause full `make test` failures — scoped verification used (`go test ./internal/intelligence/deps/ ./internal/core/...`) which passes completely.
- golangci-lint toolchain mismatch (v2.12.2 vs Go 1.27.0) — gofmt + go vet clean substitute for lint guarantees on this package.
- TestDetectWorkspaceRoot_None (codeintel) fails pre-existing (requires test-tmp directory) — unrelated, logged to deferred-items.md.

## User Setup Required

None - no external service configuration required. All clients work against httptest fixtures with zero network access in CI. GITHUB_TOKEN is optional at runtime for GitHub enrichment; clients degrade honestly without it.

## Next Phase Readiness

- All must-have truths hold: clients produce cited, ordered, honestly-degraded source snapshots; risk engine is deterministic and config-driven.
- Downstream plan 04-07 (deps verdict cache + checkpoint) can import NewDepsDevClient, NewOSVClient, NewGitHubEnricher, ClassifyRisk, and all types without modification.
- Registry data layer feeds verdict assembly with provenance-tracked snapshots per DEPEND-02.
- Environment caveats above (legacy broken packages, golangci-lint mismatch) do not block Phase 4 scoped verification but should be revisited before any whole-repo gate.

---

*Phase: 04-intelligence-features*
*Completed: 2026-09-05*

## Self-Check: PASSED

- All 12 created files verified present on disk (`[ -f ]` checks passed).
- All 6 task commits verified in history: d982f796, 12f683da, 630328a3, 8e484076, f8e02c4a, d4c773e4.
- All 27 acceptance criteria tests re-run and passing (deps: 10, osv/github: 7, risk: 10).
- go vet clean on ./internal/intelligence/deps/
- gofmt clean on all created files.