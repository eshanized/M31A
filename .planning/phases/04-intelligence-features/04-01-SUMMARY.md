---
phase: 04-intelligence-features
plan: 01
subsystem: types-config
tags: [confidence-enum, evidence-pack, citations, event-vocabulary, toml-config, layered-merge]

# Dependency graph
requires:
  - phase: 01-foundation-domain-model-event-store
    provides: EventType string-enum pattern, Event envelope, config layering (DefaultConfig/Load/MergeConfig), error sentinel conventions
provides:
  - D-07 three-level Confidence enum (verified/likely/speculative) with ParseConfidence/Valid in internal/core/types
  - EvidenceKind constants plus Evidence/EvidencePack/Citation structs with snake_case json tags and sequential-ID Add()
  - Phase 04 event constants EventInvestigationStarted/EventInvestigationCompleted/EventDependencyChecked
  - Intelligence error sentinels ErrSourceUnreachable/ErrCheckpointPending/ErrNotReproducibleInWindow
  - [intelligence] TOML config section with layered merge, zero-value normalization (50/12/6/license allowlist) and range validation
affects: [04-02-explain-collector, 04-03-explain-render, 04-04-investigate-worktree, 04-05-investigate-bisect, 04-06-deps-clients, 04-07-deps-verdict-cache]

# Actuals (#2632)
actuals:
  tokens: 7507
  tasks: 2
  commits: 4

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "String-enum vocabulary extension (EventType/Confidence) appended as commented phase blocks"
    - "Zero-value config normalization after layered merge (Pitfall 10): absence and explicit zero both normalize post-load"
    - "Safe-by-default bool policy fields: default true in DefaultConfig + defined-key-gated bool merge"

key-files:
  created:
    - internal/core/types/confidence.go
    - internal/core/types/evidence.go
    - internal/core/types/confidence_test.go
    - internal/core/types/evidence_test.go
    - internal/core/config/intelligence_test.go
  modified:
    - internal/core/types/event.go
    - internal/core/types/assurance.go
    - internal/core/errors/errors.go
    - internal/core/config/types.go
    - internal/core/config/merge.go
    - internal/core/config/loader.go
    - internal/core/config/config_validate.go

key-decisions:
  - "Renamed legacy domain type Evidence to VerificationEvidence (assurance.go) so the intelligence evidence-pack owns the shared Evidence name; JSON shape unchanged, zero external consumers"
  - "Kept RequireApprovalHighRisk naming with default-true semantics instead of inverting to DisableApprovalGate: DefaultConfig sets true and the defined-key-gated boolField merge makes explicit false overridable at every layer"
  - "Loader normalization treats <=0 as normalize-to-default; validateConfig separately rejects <1 so programmatic/un-normalized configs still fail fast (T-04-01)"
  - "Omitted env override support for [intelligence] this plan per plan's otherwise-omit clause"

patterns-established:
  - "Phase-commented event constant blocks (// Intelligence events (Phase 04)) appended after prior phase blocks"
  - "New config sections need four touchpoints: Config field, MergeConfig registration+helper, post-load normalization, validateConfig range checks, knownConfigKeys entry"
  - "EvidencePack.Add IDs computed before append so first citation marker is [1]"

requirements-completed: [EXPLAIN-02]

# Coverage metadata (#1602)
coverage:
  - id: D1
    description: "D-07 shared Confidence enum exposing exactly verified/likely/speculative with rejecting parser"
    requirement: EXPLAIN-02
    verification:
      - kind: unit
        ref: "tests/internal/core/types/confidence_test.go#TestParseConfidence"
        status: pass
      - kind: unit
        ref: "tests/internal/core/types/confidence_test.go#TestConfidenceRoundTrip"
        status: pass
      - kind: unit
        ref: "tests/internal/core/types/confidence_test.go#TestConfidenceValid"
        status: pass
    human_judgment: false
  - id: D2
    description: "Evidence/EvidencePack/Citation structs: sequential IDs from 1, Kind/Ref/Snippet preserved, snake_case json tags"
    verification:
      - kind: unit
        ref: "tests/internal/core/types/evidence_test.go#TestEvidencePackAddAssignsSequentialIDs"
        status: pass
      - kind: unit
        ref: "tests/internal/core/types/evidence_test.go#TestEvidenceJSONSnakeCaseKeys"
        status: pass
      - kind: unit
        ref: "tests/internal/core/types/evidence_test.go#TestEvidencePackJSONTags"
        status: pass
      - kind: unit
        ref: "tests/internal/core/types/evidence_test.go#TestCitationJSONSnakeCaseKeys"
        status: pass
    human_judgment: false
  - id: D3
    description: "Phase 04 event vocabulary (InvestigationStarted/InvestigationCompleted/DependencyChecked) with checkpoint pair reused, not duplicated"
    verification:
      - kind: unit
        ref: "tests/internal/core/types/evidence_test.go#TestIntelligenceEventConstants"
        status: pass
      - kind: other
        ref: "grep -c 'EventType = \"CheckpointRequested\"|EventType = \"CheckpointResolved\"' internal/core/types/event.go == 2"
        status: pass
    human_judgment: false
  - id: D4
    description: "Intelligence error sentinels ErrSourceUnreachable/ErrCheckpointPending/ErrNotReproducibleInWindow adjacent to git/bisect group"
    verification:
      - kind: other
        ref: "grep ErrCheckpointPending+ErrNotReproducibleInWindow internal/core/errors/errors.go (AC gate run, count=2)"
        status: pass
      - kind: other
        ref: "go build ./internal/core/errors/"
        status: pass
    human_judgment: false
  - id: D5
    description: "[intelligence] section loads from TOML and normalizes safe defaults when absent or zero (bisect 50, stale 12, young 6, license allowlist)"
    verification:
      - kind: unit
        ref: "tests/internal/core/config/intelligence_test.go#TestIntelligenceLoadSection"
        status: pass
      - kind: unit
        ref: "tests/internal/core/config/intelligence_test.go#TestIntelligenceDefaultsNoSection"
        status: pass
      - kind: unit
        ref: "tests/internal/core/config/intelligence_test.go#TestIntelligenceZeroNormalizes"
        status: pass
    human_judgment: false
  - id: D6
    description: "Config validation rejects bisect_max_commits/stale_months/young_months < 1 naming the field (T-04-01 mitigation)"
    verification:
      - kind: unit
        ref: "tests/internal/core/config/intelligence_test.go#TestValidateConfig_IntelligenceRejectsInvalidRanges"
        status: pass
    human_judgment: false
  - id: D7
    description: "Layered merge: workspace-level [intelligence] values override global while unoverridden global values persist"
    verification:
      - kind: unit
        ref: "tests/internal/core/config/intelligence_test.go#TestIntelligenceMergeWorkspaceOverridesGlobal"
        status: pass
    human_judgment: false

# Metrics
duration: 30 min
completed: 2026-08-25
status: complete
---

# Phase 4 Plan 1: Shared Types & Config Vocabulary Summary

**D-07 Confidence enum, EvidencePack/Citation contract, Phase 04 event vocabulary, intelligence error sentinels, and a fully merged/normalized/validated [intelligence] TOML section — the import surface for plans 04-02 through 04-07**

## Performance

- **Duration:** 30 min
- **Started:** 2026-08-25T09:14:55Z
- **Completed:** 2026-08-25T09:44:42Z
- **Tasks:** 2
- **Files modified:** 12

## Accomplishments
- Shared three-level Confidence enum (verified/likely/speculative) with strict ParseConfidence and Valid, per D-07
- Evidence pack contract: EvidenceKind taxonomy, sequential-ID Add starting at [1], non-nil Sections constructor, snake_case json tags on Evidence/EvidencePack/Citation ready for --format json (D-04/D-06)
- Event vocabulary extended with InvestigationStarted/InvestigationCompleted/DependencyChecked; existing checkpoint pair reused untouched (D-14)
- Three intelligence error sentinels placed beside git/bisect group following ErrBisectFailed style
- [intelligence] config: schema structs (flat/stable for the D-15 policy hash), layered merge registration, Pitfall-10 zero-value normalization (bisect 50 / stale 12 / young 6 / license allowlist), range validation rejecting <1, typo-warning key registration

## Task Commits

Each task was committed atomically:

1. **Task 1 (RED): failing tests for confidence enum + evidence pack** - `9c09ee46` (test)
2. **Task 1 (GREEN): implement types, events, sentinels** - `45929563` (feat)
3. **Task 2: [intelligence] config schema/merge/normalization/validation** - `0ab2fc2b` (feat)

_Plan metadata commit follows this summary._

## Files Created/Modified
- `internal/core/types/confidence.go` - D-07 Confidence enum, ParseConfidence, Valid
- `internal/core/types/evidence.go` - EvidenceKind constants, Evidence/EvidencePack/Citation with snake_case tags
- `internal/core/types/event.go` - Phase 04 intelligence event constants block
- `internal/core/types/assurance.go` - legacy Evidence renamed to VerificationEvidence (JSON unchanged)
- `internal/core/errors/errors.go` - ErrSourceUnreachable, ErrCheckpointPending, ErrNotReproducibleInWindow
- `internal/core/config/types.go` - IntelligenceConfig/DepsRiskConfig/DepsPolicyConfig + Config.Intelligence field
- `internal/core/config/merge.go` - mergeIntelligenceConfig registration + DepsRisk/DepsPolicy helpers
- `internal/core/config/loader.go` - defaults in DefaultConfig, normalizeIntelligence post-load step, DefaultLicenseAllowlist
- `internal/core/config/config_validate.go` - >=1 range checks + knownConfigKeys intelligence entry
- `internal/core/types/{confidence,evidence}_test.go`, `internal/core/config/intelligence_test.go` - behavior coverage

## Decisions Made
- Renamed the Phase 1 domain type `Evidence` to `VerificationEvidence` to free the shared name mandated by this plan's acceptance criteria for the evidence-pack struct. The old type had zero consumers outside assurance.go and its JSON serialization is byte-identical.
- Kept `RequireApprovalHighRisk` (default true via DefaultConfig) rather than the inversion fallback: the defined-key-gated boolField merge already distinguishes explicit-false from unset across all layers, keeping zero-value semantics safe-by-default.
- Normalization uses `<= 0 -> default` in loader.go while validation rejects `< 1`: Load is safe-by-default (absence, explicit zero, and negatives all normalize), and validateConfig still rejects invalid ranges when invoked on un-normalized configs (unit-tested).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Name collision: Evidence already declared in types package**
- **Found during:** Task 1 (GREEN compile)
- **Issue:** Plan mandates new `Evidence` struct in evidence.go, but `internal/core/types/assurance.go` already declared a different domain-model `Evidence` (verification evidence entries); package would not compile.
- **Fix:** Renamed the legacy type to `VerificationEvidence` with doc comment; updated its 3 intra-package references. Zero consumers existed outside the types package; JSON tags unchanged so no serialization impact. Downstream plans 04-02..04-07 keep the planned `Evidence` name.
- **Files modified:** internal/core/types/assurance.go
- **Verification:** go build ./internal/core/... green; go test ./internal/core/... green
- **Committed in:** 45929563 (part of Task 1 GREEN commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Rename was the minimal-risk resolution preserving the plan's downstream import contract. No scope creep.

## Issues Encountered

- `go build ./...` and full-suite runs cannot go green in this repo state: four PRE-EXISTING legacy packages (`internal/engine/session`, `internal/engine/taskrunner`, `internal/integrations/ledger`, `internal/tools/todo`) reference symbols absent from the canonical domain model. Verified identical failure set at HEAD before any change; logged to `.planning/phases/04-intelligence-features/deferred-items.md`. Scoped verification substituted: `go build ./internal/core/...`, `go test ./internal/core/...`, plus consumer-package spot checks (`internal/memory/eventstore`, `internal/engine/bisect` pass).
- golangci-lint v2.12.2 crashes typechecking under system Go 1.27.0 (toolchain mismatch, fails identically on untouched packages). `gofmt -l` clean + `go vet` clean substitute for lint guarantees this plan.
- `TestDetectWorkspaceRoot_None` (codeintel) fails pre-existing: it requires `<repo>/test-tmp/` to exist for os.MkdirTemp parent; unrelated to this plan, logged to deferred-items.md.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- All must-have truths hold: enum exposes exactly three values; parse errors name valid values; Add assigns IDs 1..n; snake_case tags verified by marshal tests; event constants exact-string checked; checkpoint pair single-declared; [intelligence] loads/merges/normalizes/validates; empty Sections remains an explicit distinguishable state for the EXPLAIN-02 no-evidence edge.
- Downstream plans can import `types.Confidence/Evidence/EvidencePack/Citation`, `types.EventInvestigation*`/`EventDependencyChecked`, `coreerrors.Err*` sentinels, and `config.Intelligence` without modifying them.
- Note for 04-07: policy hash should serialize `DepsPolicyConfig` only (fields kept flat and stable for this purpose).
- Environment caveats above (legacy broken packages, golangci-lint mismatch) do not block Phase 4 scoped verification but should be revisited before any whole-repo gate.

---
*Phase: 04-intelligence-features*
*Completed: 2026-08-25*

## Self-Check: PASSED

- All 12 created/modified files verified present on disk (`[ -f ]` checks passed).
- All 3 task commits verified in history: 9c09ee46, 45929563, 0ab2fc2b.
- All task acceptance criteria re-run and passing (Task 1: 8/8 greps+tests, Task 2: 6/6 incl. gofmt/vet substitutes for env-broken lint).
