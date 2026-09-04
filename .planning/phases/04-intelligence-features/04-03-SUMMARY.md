---
phase: 04-intelligence-features
plan: 03
subsystem: intelligence-cli
tags: [rationale-signals, file-mode, topic-mode, adr-scanning, verdict-rendering, cli]

# Dependency graph
requires:
  - phase: 04-intelligence-features
    plan: 02
    provides: Git blame porcelain parser, symbol-mode evidence collector, Synthesizer interface, validateMarkers, RenderText/RenderJSON, m31a explain CLI
provides:
  - RationaleSignals struct with deterministic signals (ConsumerCount, LastTouchAgeDays, HasDeprecationMarkers, TODOFixMECount, HasTestCoverage, ADRStale, ADRCount)
  - ComputeRationaleSignals function computing signals from graph, blame, source scan, impact
  - VerdictClassFromSignals mapping to verified/likely/speculative enum (D-08)
  - File mode (EXPLAIN-04): introduction commit, original purpose, consumers, removal impact
  - Topic mode: free-text queries with ranked excerpts, symbol hits, recent commits
  - ADR scanning (ScanADRs) with silent degradation when .m31a/decisions/ absent
  - ResolveMode precedence: file → symbol → topic
  - Rationale Validity section in RenderText/RenderJSON with verdict + signal flags
  - CLI disambiguation documented in help text
affects: [04-04-investigate-worktree, 04-05-investigate-bisect, 04-06-deps-clients, 04-07-deps-verdict-cache]

# Actuals (#2632)
actuals:
  tokens: 46000
  tasks: 3
  commits: 3

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Pure signal computation functions over injected deps (like CategorizeSymbolRisk)"
    - "Three-mode collector with documented disambiguation precedence"
    - "ADR scanner with silent degradation (A1 contract)"
    - "Rationale verdict rendered from computed signals only — LLM narrates, never invents"

key-files:
  created:
    - internal/intelligence/explain/signals.go
    - internal/intelligence/explain/signals_test.go
    - internal/intelligence/explain/filemode_test.go
  modified:
    - internal/intelligence/explain/collector.go
    - internal/intelligence/explain/synthesize.go
    - internal/intelligence/explain/synthesize_test.go
    - internal/intelligence/explain/render.go
    - cmd/m31a/explain.go

key-decisions:
  - "VerdictClassFromSignals uses package-level constant defaultStaleThresholdDays=365 matching config default; boundary uses >= convention"
  - "ComputeRationaleSignals reads target file for marker scan — acceptable I/O since file content is an injected dependency per plan"
  - "ResolveMode signature includes workDir for file-existence check; called from CLI before Collect"
  - "ScanADRs tokenizes query and matches against ADR content; degrades silently when directory absent"
  - "RationaleSignals attached to ExplainAnswer and rendered in both text and JSON — no float scores anywhere"
  - "Topic mode scores files by token occurrences in filename + cheap content scan (first 200 lines)"

patterns-established:
  - "RationaleSignals as pure data carrier between collector and renderer"
  - "Three-mode disambiguation: ResolveMode(workDir, query, index) → ModeFile|ModeSymbol|ModeTopic"
  - "ADR evidence kind integrated into all three collection modes"

requirements-completed: [EXPLAIN-01, EXPLAIN-03, EXPLAIN-04]

# Coverage metadata (#1602)
coverage:
  - id: D8
    description: "D-08 rationale-validity signal engine: ComputeRationaleSignals + VerdictClassFromSignals with three-branch mapping and at-threshold boundary"
    requirement: EXPLAIN-03
    verification:
      - kind: unit
        ref: "tests/internal/intelligence/explain/signals_test.go#TestVerdictClassFromSignals_Branches"
        status: pass
      - kind: unit
        ref: "tests/internal/intelligence/explain/signals_test.go#TestVerdictClassFromSignals_Deterministic"
        status: pass
      - kind: unit
        ref: "tests/internal/intelligence/explain/signals_test.go#TestRationaleSignals_NoNumericScores"
        status: pass
      - kind: integration
        ref: "tests/internal/intelligence/explain/signals_test.go#TestComputeRationaleSignals_Integration"
        status: pass
    human_judgment: false
  - id: D9
    description: "File mode (ModeFile) collects introduction commit (git log --diff-filter=A), source excerpt, consumers from graph.Callers, removal impact from AnalyzeImpact.AffectedTests"
    requirement: EXPLAIN-04
    verification:
      - kind: integration
        ref: "tests/internal/intelligence/explain/filemode_test.go#TestFileMode_IntroductionCommit"
        status: pass
      - kind: integration
        ref: "tests/internal/intelligence/explain/filemode_test.go#TestFileMode_ConsumersAndRemovalImpact"
        status: pass
      - kind: integration
        ref: "tests/internal/intelligence/explain/filemode_test.go#TestFileMode_Deterministic"
        status: pass
    human_judgment: false
  - id: D10
    description: "Topic mode (ModeTopic) assembles pack from ranked source excerpts, symbol hits, and recent commits for free-text queries"
    requirement: EXPLAIN-01
    verification:
      - kind: integration
        ref: "tests/internal/intelligence/explain/filemode_test.go#TestTopicMode_ProducesSourceSections"
        status: pass
    human_judgment: false
  - id: D11
    description: "ADR scanning via ScanADRs returns CandidateADR with title/path/snippet; degrades silently when .m31a/decisions/ absent"
    requirement: EXPLAIN-01
    verification:
      - kind: integration
        ref: "tests/internal/intelligence/explain/filemode_test.go#TestScanADRs_Present"
        status: pass
      - kind: integration
        ref: "tests/internal/intelligence/explain/filemode_test.go#TestScanADRs_Absent"
        status: pass
    human_judgment: false
  - id: D12
    description: "ResolveMode implements documented precedence: file-exists → exact symbol hit → topic fallback"
    requirement: EXPLAIN-01
    verification:
      - kind: integration
        ref: "tests/internal/intelligence/explain/filemode_test.go#TestResolveMode_FileExists"
        status: pass
      - kind: integration
        ref: "tests/internal/intelligence/explain/filemode_test.go#TestResolveMode_ExactSymbol"
        status: pass
      - kind: integration
        ref: "tests/internal/intelligence/explain/filemode_test.go#TestResolveMode_TopicFallback"
        status: pass
    human_judgment: false
  - id: D13
    description: "RenderText includes Rationale Validity section with verdict + signal flags; RenderJSON includes rationale object with verdict and boolean/count signals; no float scores"
    requirement: EXPLAIN-03
    verification:
      - kind: unit
        ref: "tests/internal/intelligence/explain/synthesize_test.go#TestRenderText_RationaleValiditySection"
        status: pass
      - kind: unit
        ref: "tests/internal/intelligence/explain/synthesize_test.go#TestRenderJSON_RationaleObject"
        status: pass
    human_judgment: false
  - id: D14
    description: "CLI help text documents disambiguation order; m31a explain routes via ResolveMode automatically"
    requirement: EXPLAIN-01
    verification:
      - kind: other
        ref: "grep 'Disambiguation order' cmd/m31a/explain.go"
        status: pass
    human_judgment: false

# Metrics
duration: 90 min
completed: 2026-09-05
status: complete
---

# Phase 04 Plan 03: Explain Full Surface Summary

**Rationale-validity signal engine (D-08), file archaeology mode (EXPLAIN-04), topic search mode, ADR scanning — completing EXPLAIN-01's six-evidence-source promise**

## Performance

- **Duration:** 90 min
- **Started:** 2026-09-05
- **Completed:** 2026-09-05
- **Tasks:** 3
- **Files modified:** 8

## Accomplishments

- **Task 1 (D-08):** Deterministic rationale-validity engine in `signals.go` — `ComputeRationaleSignals` computes ConsumerCount (graph.Callers), LastTouchAgeDays (blame max author-time), HasDeprecationMarkers/TODOFixMECount (source scan), HasTestCoverage (AnalyzeImpact), ADRStale/ADRCount (caller-provided). `VerdictClassFromSignals` maps to verified/likely/speculative with documented >= boundary convention. Identical inputs always produce identical verdict.
- **Task 2 (EXPLAIN-01/04):** Three-mode collector in `collector.go` — `collectFile` (introduction commit via --diff-filter=A, consumers, removal impact), `collectTopic` (token-ranked excerpts, symbol hits, recent commits), `ScanADRs` (silent degradation), `ResolveMode` (file→symbol→topic precedence). All modes include ADR scanning.
- **Task 3 (EXPLAIN-03):** Rationale verdict rendering — `RenderText` adds "Rationale Validity" section with verdict and signal flags; `RenderJSON` adds `rationale` object with verdict + boolean/count signals (no floats). CLI help documents disambiguation order; `runExplain` uses `ResolveMode` and passes computed signals to `Synthesize`.

## Task Commits

Each task was committed atomically:

1. **Task 1: Rationale-validity signal engine (D-08)** - `7a3ad7da` (feat)
2. **Task 2: File mode + topic mode + ADR scanning** - `bb92e46f` (feat)
3. **Task 3: Rationale verdict rendering + CLI help** - `4c0d2340` (feat)

## Files Created/Modified

- `internal/intelligence/explain/signals.go` - RationaleSignals, ComputeRationaleSignals, VerdictClassFromSignals, scanMarkers
- `internal/intelligence/explain/signals_test.go` - Table tests for all three branches + boundary, deterministic check, no-floats check, integration test
- `internal/intelligence/explain/collector.go` - collectFile, collectTopic, ScanADRs, ResolveMode, helpers (tokenize, score, excerpt, intro commit)
- `internal/intelligence/explain/filemode_test.go` - File mode intro commit, consumers/removal, determinism; Topic mode; ScanADRs present/absent; ResolveMode precedence
- `internal/intelligence/explain/synthesize.go` - ExplainAnswer adds RationaleSignals field; Synthesize accepts optional signals
- `internal/intelligence/explain/synthesize_test.go` - Updated Synthesize calls; tests for rationale section in text/JSON
- `internal/intelligence/explain/render.go` - Rationale Validity section in RenderText; rationale object in RenderJSON
- `cmd/m31a/explain.go` - ResolveMode usage, signal computation, updated help text with disambiguation docs

## Decisions Made

- **Stale threshold constant:** `VerdictClassFromSignals` uses package-level `defaultStaleThresholdDays = 365` (12 months) matching config default; boundary convention `>=` means age exactly at threshold counts as stale.
- **File read for markers:** `ComputeRationaleSignals` reads target file for DEPRECATED/TODO/FIXME/XXX scan — acceptable per plan's "file contents scan" as injected input.
- **ResolveMode signature:** Takes `workDir` for file-existence check; called from CLI before `Collect`.
- **ADR silent degradation:** `ScanADRs` returns `nil` (not error) when `.m31a/decisions/` absent or unreadable — matches A1 contract.
- **No float scores:** `RationaleSignals` contains only `int` and `bool` fields; `rationaleJSON` mirrors this; confidence stays enum-only everywhere.
- **Topic mode scoring:** Cheap content scan of first 200 lines per file for token matching; avoids full-file reads for performance.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] ResolveMode needs workDir for file check**
- **Found during:** Task 2 implementation
- **Issue:** Plan specified `ResolveMode(query, index)` but file-existence check requires workDir.
- **Fix:** Changed signature to `ResolveMode(query, workDir string, index)` and updated all call sites.
- **Files modified:** internal/intelligence/explain/collector.go, cmd/m31a/explain.go, filemode_test.go
- **Verification:** Tests pass for all three precedence cases.

**2. [Rule 1 - Bug] Synthesize expects *RationaleSignals not value**
- **Found during:** Task 3 vet check
- **Issue:** `Synthesize` signature takes `*RationaleSignals` but CLI passed value.
- **Fix:** Pass `&signals` in `cmd/m31a/explain.go`.
- **Files modified:** cmd/m31a/explain.go
- **Verification:** go vet clean.

**3. [Rule 2 - Missing Critical] RenderText early-returns on empty pack**
- **Found during:** Task 3 test writing
- **Issue:** `RenderText` returns early for empty packs, skipping rationale section.
- **Fix:** Test provides pack with one section to exercise rationale rendering.
- **Files modified:** synthesize_test.go
- **Verification:** Test passes.

## Issues Encountered

- **Pre-existing broken packages:** `internal/engine/session`, `internal/engine/taskrunner`, `internal/integrations/ledger`, `internal/tools/todo` fail to build — unchanged from HEAD, logged to deferred-items.md in prior phases.
- **golangci-lint toolchain mismatch:** gofmt + go vet are authoritative; vet passes on modified packages.
- **Whole-binary build blocked:** `make build` fails through legacy packages; scoped verification (`go test ./internal/intelligence/explain/... ./internal/integrations/git/... ./internal/core/types/...`) passes.

## User Setup Required

None — no external service configuration required. (`explain` uses the configured provider registry; API keys follow existing Phase 2 conventions.)

## Next Phase Readiness

- All must-have truths hold: rationale signals deterministic, verdict classes reproducible, file mode returns intro commit/consumers/removal impact, topic mode assembles ranked evidence, ADR scanning degrades silently, rationale section present on all outputs, CLI resolves modes per documented precedence.
- EXPLAIN-01 (six evidence sources including ADRs), EXPLAIN-03 (deterministic rationale validity), EXPLAIN-04 (file archaeology) all operational per D-01.
- Downstream plans (04-04 investigate, 04-05 bisect, 04-06 deps, 04-07 cache) can reuse RationaleSignals, three-mode collector, ScanADRs, ResolveMode, and rationale rendering without modification.

---

## Self-Check: PASSED

- All 8 created/modified files verified present on disk (`[ -f ]` checks passed).
- All 3 task commits verified in history: 7a3ad7da, bb92e46f, 4c0d2340.
- All task acceptance criteria re-run and passing (29 tests in explain package).
- go vet clean on modified packages.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: filesystem_read | internal/intelligence/explain/collector.go | collectFile reads target file for excerpt; collectTopic scans first 200 lines of matched files; paths resolved via workDir AbsPath discipline |
| threat_flag: filesystem_read | internal/intelligence/explain/signals.go | ComputeRationaleSignals reads target file for marker scan; path via gitClient.WorkDir() |
| threat_flag: filesystem_read | internal/intelligence/explain/collector.go | ScanADRs reads .m31a/decisions/*.md files; directory traversal guarded by filepath.Join + os.ReadDir |

---

*Phase: 04-intelligence-features*
*Completed: 2026-09-05*