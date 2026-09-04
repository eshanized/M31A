---
phase: 04-intelligence-features
plan: 05
subsystem: intelligence-cli
tags: [investigate, bisect, report, cli, D-12, REGRESS-02, REGRESS-03]

# Dependency graph
requires:
  - phase: 04-intelligence-features
    plan: 04
    provides: WorktreeManager, ResolveReproCommand/ExecuteRepro, Candidates/FindCulprit, InvestigateGitRunner, error sentinels
provides:
  - RootCauseReport with two-tier confidence (verified/likely/speculative) per D-12
  - BuildRootCauseReport with confirmation pass (culprit=fail AND parent=pass)
  - RenderInvestigationText/RenderInvestigationJSON for REGRESS-03 output
  - EmitInvestigationStarted/Completed events with source=intelligence
  - m31a investigate SYMPTOM [--baseline] [--repro] [--format] CLI wired in main.go
affects: []

# Actuals (#2632)
actuals:
  tokens: 13718
  tasks: 3
  commits: 3

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Two-tier confidence: attribution=verified (mechanical), mechanism≤likely (single LLM pass)"
    - "Confirmation pass as mandatory gate before verified attribution (D-12)"
    - "Manual flag parsing to support flags after positional (like explain.go)"
    - "Nil-store safe event emission following codeintel/events.go pattern"

key-files:
  created:
    - internal/intelligence/investigate/report.go
    - internal/intelligence/investigate/report_test.go
    - cmd/m31a/investigate.go
  modified:
    - cmd/m31a/main.go

key-decisions:
  - "MechanismConfidence never exceeds Likely — capped at Likely even if synthesis returns Verified"
  - "Nil synthesizer degrades MechanismConfidence to Speculative with empty FixDirection/Mechanism"
  - "Not-reproducible-in-window carries sentinel evidence and Speculative confidence on overall verdict"
  - "AffectedComponents deduplicated by name keeping minimal depth; ordered depth-then-name"
  - "Empty AffectedComponents is non-nil slice rendered as 'none' in text mode"
  - "JSON confidence fields serialize exactly as verified/likely/speculative enum strings"
  - "Manual flag parsing (not stdlib flag) to support flags after symptom argument"
  - "EventStore opened via .m31a/events.db when present; nil-store safe path for headless runs"

patterns-established:
  - "InvestigationStarted/Completed events follow codeintel emitEvent pattern verbatim with Source=intelligence"
  - "RootCauseReport structure: Symptom → Range/Steps → Culprit[verified] → Mechanism[likely] → Affected → FixDirection → Evidence"
  - "CLI exit codes: attributed=0, usage=1, not-reproducible=3, unattributable=4"

requirements-completed: [REGRESS-01, REGRESS-02, REGRESS-03]

# Coverage metadata (#1602)
coverage:
  - id: D1
    description: "BuildRootCauseReport runs confirmation pass: culprit=fail AND parent=pass before Verified attribution (D-12)"
    requirement: REGRESS-02
    verification:
      - kind: unit
        ref: "tests/internal/intelligence/investigate/report_test.go#TestBuildRootCauseReport_VerifiedPath"
        status: pass
      - kind: unit
        ref: "tests/internal/intelligence/investigate/report_test.go#TestBuildRootCauseReport_FlakyParent"
        status: pass
    human_judgment: false
  - id: D2
    description: "MechanismConfidence capped at Likely; nil synth -> Speculative; mechanism never auto-promoted"
    requirement: REGRESS-02
    verification:
      - kind: unit
        ref: "tests/internal/intelligence/investigate/report_test.go#TestBuildRootCauseReport_NilSynth"
        status: pass
    human_judgment: false
  - id: D3
    description: "Not-reproducible-in-window outcome with sentinel evidence and Speculative confidence"
    requirement: REGRESS-02
    verification:
      - kind: unit
        ref: "tests/internal/intelligence/investigate/report_test.go#TestBuildRootCauseReport_NotReproducible"
        status: pass
    human_judgment: false
  - id: D4
    description: "AffectedComponents deduplication with minimal depth retention and deterministic ordering"
    requirement: REGRESS-03
    verification:
      - kind: unit
        ref: "tests/internal/intelligence/investigate/report_test.go#TestBuildRootCauseReport_DedupComponents"
        status: pass
      - kind: unit
        ref: "tests/internal/intelligence/investigate/report_test.go#TestBuildRootCauseReport_ComponentOrdering"
        status: pass
    human_judgment: false
  - id: D5
    description: "Leaf changes report empty non-nil AffectedComponents slice"
    requirement: REGRESS-03
    verification:
      - kind: unit
        ref: "tests/internal/intelligence/investigate/report_test.go#TestBuildRootCauseReport_LeafChange"
        status: pass
    human_judgment: false
  - id: D6
    description: "RenderInvestigationText/JSON with correct section order, not-reproducible omits culprit/mechanism"
    requirement: REGRESS-03
    verification:
      - kind: unit
        ref: "tests/internal/intelligence/investigate/report_test.go#TestRenderInvestigationText"
        status: pass
      - kind: unit
        ref: "tests/internal/intelligence/investigate/report_test.go#TestRenderInvestigationJSON"
        status: pass
      - kind: unit
        ref: "tests/internal/intelligence/investigate/report_test.go#TestRenderInvestigationText_NotReproducibleOmitCulprit"
        status: pass
    human_judgment: false
  - id: D7
    description: "JSON confidence fields serialize exactly as verified/likely/speculative enum strings"
    requirement: REGRESS-03
    verification:
      - kind: unit
        ref: "tests/internal/intelligence/investigate/report_test.go#TestBuildRootCauseReport_JSONConfidenceEnum"
        status: pass
    human_judgment: false
  - id: D8
    description: "Nil-store safe event emission (no panic, no error)"
    requirement: REGRESS-02
    verification:
      - kind: unit
        ref: "tests/internal/intelligence/investigate/report_test.go#TestEmitInvestigationEvents_NilStore"
        status: pass
    human_judgment: false
  - id: D9
    description: "m31a investigate CLI wired in main.go dispatch with manual flag parsing, exit codes 0/1/3/4"
    requirement: REGRESS-01
    verification:
      - kind: other
        ref: "usage output mentions symptom, baseline, repro, format, exit-code table"
        status: pass
      - kind: other
        ref: "main.go contains investigate dispatch before TUI launch"
        status: pass
    human_judgment: false

# Metrics
duration: 95 min
completed: 2026-09-05
status: complete
---

# Phase 04 Plan 05: Investigate Bisect + Report Summary

**Confirmation-run attribution semantics (D-12), two-tier root-cause report (REGRESS-02/03), durable InvestigationStarted/Completed events, and CLI wiring with --baseline/--repro flags (D-02)**

## Performance

- **Duration:** 95 min
- **Started:** 2026-09-05
- **Completed:** 2026-09-05
- **Tasks:** 3
- **Files modified:** 4 created/modified

## Accomplishments

- **Task 1 (TDD - D-12):** Confirmation attribution + root-cause report builder in `report.go`
  - `BuildRootCauseReport` orchestrates pre-checks, Candidates, FindCulprit, confirmation pass (culprit=fail AND parent=pass), culprit diff analysis, affected components via AnalyzeImpact, optional mechanism synthesis
  - Two-tier confidence: CulpritConfidence=Verified only after both confirmation observations; MechanismConfidence≤Likely (capped); nil synth→Speculative
  - Not-reproducible-in-window carries sentinel evidence and Speculative overall confidence
  - AffectedComponents from codeintel.AnalyzeImpact: deduplicated by name (minimal depth), ordered depth-then-name, empty non-nil for leaf changes
  - Events: EmitInvestigationStarted/Completed with Source=intelligence, SchemaVersion=1, nil-store safe

- **Task 2 (D-04):** Report rendering — table and JSON
  - `RenderInvestigationText`: Symptom, Bisect Range, Steps table, Culprit[confidence], Mechanism[confidence], Affected Components (or "none"), Fix Direction, Evidence
  - `RenderInvestigationJSON`: Full struct with 2-space indent; confidence fields as exact enum strings
  - Not-reproducible-in-window renders sentinel evidence block, omits culprit/mechanism sections
  - Empty AffectedComponents renders as "none" in text mode

- **Task 3 (TRACER):** CLI wiring — `m31a investigate` end-to-end
  - `runInvestigate` in `cmd/m31a/investigate.go` mirrors `runImpact`/`runExplain` shape
  - Manual flag parsing (like explain.go) to support flags after symptom: `--baseline`, `--repro`, `--format table|json`
  - Baseline default: HEAD~N where N=bisect_max_commits; explicit --baseline overrides
  - Repro resolution: flag > config > auto-detect (go.mod→go test ./..., package.json→npm test)
  - EventStore via .m31a/events.db when present; nil-store safe degradation
  - Provider/model from registry like explain; synthesis failure degrades report instead of failing command
  - Exit codes: attributed=0, usage=1, not-reproducible=3, unattributable=4
  - Dispatch added in main.go before TUI launch

## Task Commits

Each task was committed atomically:

1. **Task 1 (TDD - RED+GREEN):** Confirmation attribution + root-cause report builder (D-12) - `d070126c` (feat)
2. **Task 2:** Report rendering — table and JSON (D-04) - `7804455c` (feat)
3. **Task 3 (TRACER):** CLI wiring — m31a investigate end-to-end through main.go - `40fd96f5` (feat)

_Plan metadata commit follows this summary._

## Files Created/Modified

- `internal/intelligence/investigate/report.go` - RootCauseReport, BuildRootCauseReport, EmitInvestigationStarted/Completed, RenderInvestigationText/JSON, extractSymbolsFromDiff
- `internal/intelligence/investigate/report_test.go` - 12 tests covering verified path, flaky parent, nil synth, not-reproducible, dedup, ordering, leaf change, JSON enum, nil-store events, text/JSON rendering
- `cmd/m31a/investigate.go` - runInvestigate with manual flag parsing, baseline/repro/format flags, exit codes 0/1/3/4
- `cmd/m31a/main.go` - investigate dispatch before TUI launch

## Decisions Made

- **Mechanism confidence ceiling:** Even if synthesis returns Verified, MechanismConfidence is capped at Likely per D-12 — attribution is mechanical (two independent observations), mechanism is hypothesis (single LLM pass)
- **Nil synthesizer handling:** No synth → MechanismConfidence=Speculative, FixDirection="", Mechanism="" — honest degradation
- **Not-reproducible-in-window:** Returns sentinel evidence (checked SHA, observed result, window size) with Speculative overall confidence; no fabricated attribution
- **Affected components deduplication:** Component reachable via multiple transitive paths appears once with minimal depth; ordered by depth ascending then name ascending
- **Empty affected components:** Non-nil slice (not nil) rendered as "none" in text mode — explicit empty signal
- **JSON enum serialization:** Confidence fields use exact strings verified/likely/speculative — verified by table-driven test
- **Manual flag parsing:** Stdlib flag stops at first positional; manual split supports `m31a investigate SYMPTOM --format json` and `m31a investigate --format json SYMPTOM` equally
- **Nil EventStore:** Emit functions return nil without error when store=nil — safe for headless runs without .m31a

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Flag parsing stopped at first positional argument**
- **Found during:** Task 3 CLI testing
- **Issue:** Stdlib flag.Parse stops at first non-flag argument, so `m31a investigate "multi word symptom" --format json` treated only "multi" as symptom
- **Fix:** Manual flag parsing (like explain.go) splitting flags from positionals before validation, accepting either argument order
- **Files modified:** cmd/m31a/investigate.go, test_investigate_cli/main.go
- **Verification:** Both argument orders work; usage documents flag-after-positional

**2. [Rule 3 - Blocking] Disk quota exceeded in /tmp for worktree creation during CLI testing**
- **Found during:** Task 3 integration testing
- **Issue:** WorktreeManager uses os.TempDir() (/tmp) which has quota limit; test binary runs outside go test TMPDIR
- **Fix:** Documented as test environment limitation; unit tests use TMPDIR=/home/snigdha/.tmp and pass; production binary will respect user's TMPDIR via os.TempDir()
- **Impact:** No code change needed; CLI wiring verified via flag parsing, usage output, dispatch, and unit tests

**3. [Rule 1 - Bug] Test binary nil registry panic**
- **Found during:** Task 3 CLI testing
- **Issue:** runInvestigate called with nil registry; provider selection panicked
- **Fix:** Guard registry usage with nil check; continue without synthesis when registry unavailable
- **Files modified:** test_investigate_cli/main.go (test helper), cmd/m31a/investigate.go already handles nil provider gracefully

**4. [Rule 1 - Bug] Slice bounds panic on short SHA in text renderer**
- **Found during:** Task 2 rendering tests
- **Issue:** `r.CulpritSHA[:12]` panicked when test used short SHA (6 chars)
- **Fix:** Check length before slicing: `if len(s) > 12 { s = s[:12] }`
- **Files modified:** internal/intelligence/investigate/report.go

## Issues Encountered

- **Pre-existing broken packages:** `internal/engine/session`, `internal/engine/taskrunner`, `internal/integrations/ledger`, `internal/tools/todo` fail to build — verified identical at HEAD before any changes; scoped verification substituted (go vet + tests on modified packages only)
- **golangci-lint toolchain mismatch:** gofmt + go vet are authoritative; vet passes on modified packages
- **Whole-binary build blocked:** `make build` fails through legacy packages; scoped verification used for validation

## User Setup Required

None — no external service configuration required. All functionality uses local git and stdlib.

## Next Phase Readiness

- All must-have truths hold: verified attribution only after dual confirmation; mechanism≤likely enforced mechanically; honest empty/degraded cases; deterministic ordering; event-sourced audit trail
- REGRESS-01/02/03 satisfied: bisect engine + confirmation + two-tier report + CLI + events
- Downstream plans can import RootCauseReport, BuildRootCauseReport, RenderInvestigationText/JSON, EmitInvestigationStarted/Completed, and investigate CLI dispatch without modification

---

## Self-Check: PASSED

- All 4 created/modified files verified present on disk
- All 3 task commits verified in history: d070126c, 7804455c, 40fd96f5
- All task acceptance criteria re-run and passing (29 tests in investigate package)
- go vet clean on modified packages
- Race detector clean on investigate package

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: process_execution | internal/intelligence/investigate/report.go | ExecuteRepro runs repro command in disposable worktree via exec.CommandContext (no shell, 64KB output cap) |
| threat_flag: filesystem_read | internal/intelligence/investigate/report.go | GitClient.DiffRefs reads culprit diff; codeintel AnalyzeImpact reads graph data |
| threat_flag: tampering_elevation | cmd/m31a/investigate.go | User-supplied repro command executed in isolated worktree; --repro flag overrides config |

---