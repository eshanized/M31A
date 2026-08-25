---
phase: 04-intelligence-features
plan: 02
subsystem: intelligence-cli
tags: [git-blame, porcelain-parser, evidence-pack, llm-synthesis, citation-validation, cli]

# Dependency graph
requires:
  - phase: 04-intelligence-features (04-01)
    provides: Confidence enum, Evidence/EvidencePack/Citation contract with sequential IDs, EvidenceKind taxonomy, snake_case json tags
provides:
  - internal/integrations/git BlamePorcelain stateful parser (suppressed-metadata safe) plus exported ValidateRef injection guard
  - internal/intelligence/explain package: CollectorDeps/Collect symbol mode, ErrTargetNotFound sentinel, deterministic deduped budget-trimmed packs
  - Synthesizer narrow interface + Synthesize one-call pipeline with structural marker validation demoting unknown-marker sentences to Inference
  - RenderText/RenderJSON dual-format output per D-04/D-06 including explicit no-evidence report naming scopes
  - m31a explain SYMBOL [--format table|json] CLI wired into main.go dispatch before TUI launch
affects: [04-03-explain-file-topic-modes, 04-04-investigate-worktree, 04-05-investigate-bisect, 04-06-deps-clients, 04-07-deps-verdict-cache]

# Actuals (#2632)
actuals:
  tokens: 20788
  tasks: 3
  commits: 5

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Stateful porcelain parsing keyed on header SHA token; suppressed repeat groups resolve from cache"
    - "Narrow Synthesizer interface over provider types — intelligence never dials vendors directly (Pitfall 10)"
    - "Post-synthesis structural citation validation: sentence-level demotion of unknown markers into Inference"
    - "Collect-before-provider-resolution CLI ordering so target-not-found works without API keys"

key-files:
  created:
    - internal/integrations/git/blame.go
    - internal/integrations/git/blame_test.go
    - internal/intelligence/explain/collector.go
    - internal/intelligence/explain/collector_test.go
    - internal/intelligence/explain/synthesize.go
    - internal/intelligence/explain/render.go
    - internal/intelligence/explain/synthesize_test.go
    - cmd/m31a/explain.go
  modified:
    - cmd/m31a/main.go

key-decisions:
  - "Collector reduces whole-file blame to ONE last-touch evidence item (max author-time commit SHA + summary) instead of per-line rows, keeping packs small"
  - "Budget trimming drops WHOLE sections lowest-priority-first (tests then blame then callers) and never mid-truncates snippets; always keeps at least one section"
  - "Marker-less synthesis sentences stay in prose; only sentences carrying unknown markers are demoted wholesale to Inference with just the unknown markers stripped"
  - "runExplain collects evidence BEFORE provider selection so ErrTargetNotFound renders a search-scope note without any API key configured"

requirements-completed: [EXPLAIN-02]

# Coverage metadata (#1602)
coverage:
  - id: D1
    description: "Git blame porcelain parser: suppressed-metadata groups resolve Author/AuthorTime from SHA cache; continuation headers, boundary/unknown tags tolerated; hostile revs rejected pre-execution; ValidateRef exported"
    verification:
      - kind: unit
        ref: "tests/internal/integrations/git/blame_test.go#TestParseBlamePorcelain_FixtureTwoHunks"
        status: pass
      - kind: unit
        ref: "tests/internal/integrations/git/blame_test.go#TestBlamePorcelain_HostileRevFailsFast"
        status: pass
      - kind: integration
        ref: "tests/internal/integrations/git/blame_test.go#TestBlamePorcelain_RealRepoSuppressedMetadata"
        status: pass
      - kind: unit
        ref: "tests/internal/integrations/git/blame_test.go#TestValidateRef"
        status: pass
    human_judgment: false
  - id: D2
    description: "Symbol-mode evidence collector: pack carries source/callers/blame/tests kinds with file:line and SHA refs, kind+ref dedupe first-wins with renumbering, deterministic ordering, budget trims tests before source, unknown symbol wraps ErrTargetNotFound with zero sections"
    requirement: EXPLAIN-01
    verification:
      - kind: integration
        ref: "tests/internal/intelligence/explain/collector_test.go#TestCollectSymbol_AssemblesAllKinds"
        status: pass
      - kind: integration
        ref: "tests/internal/intelligence/explain/collector_test.go#TestCollect_DeterministicOrder"
        status: pass
      - kind: integration
        ref: "tests/internal/intelligence/explain/collector_test.go#TestCollect_BudgetTrimsTestsBeforeSource"
        status: pass
      - kind: integration
        ref: "tests/internal/intelligence/explain/collector_test.go#TestCollect_UnknownSymbolReturnsSentinel"
        status: pass
      - kind: unit
        ref: "tests/internal/intelligence/explain/collector_test.go#TestDedupeSections_FirstWins"
        status: pass
    human_judgment: false
  - id: D3
    description: "Single-call synthesis seam: exactly one ChatCompletion per Synthesize, pack-as-data system prompt, unknown-marker sentences demoted to Inference with markers stripped, confidence verified only with citations and empty inference"
    requirement: EXPLAIN-02
    verification:
      - kind: unit
        ref: "tests/internal/intelligence/explain/synthesize_test.go#TestSynthesize_SingleCallValidMarkers"
        status: pass
      - kind: unit
        ref: "tests/internal/intelligence/explain/synthesize_test.go#TestSynthesize_RequestShape"
        status: pass
      - kind: unit
        ref: "tests/internal/intelligence/explain/synthesize_test.go#TestValidateMarkers_MixedDemotesUnknownSentence"
        status: pass
      - kind: unit
        ref: "tests/internal/intelligence/explain/synthesize_test.go#TestValidateMarkers_AllUnknownGoesToInference"
        status: pass
    human_judgment: false
  - id: D4
    description: "Dual-format rendering per D-04/D-06: text output with aligned Evidence listing and explicit Inference heading; JSON object with query/prose/citations[]/inference[]/confidence snake_case keys; explicit no-evidence report naming scopes"
    requirement: EXPLAIN-02
    verification:
      - kind: unit
        ref: "tests/internal/intelligence/explain/synthesize_test.go#TestRenderText_EvidenceSectionAndInferenceHeading"
        status: pass
      - kind: unit
        ref: "tests/internal/intelligence/explain/synthesize_test.go#TestRenderText_NoEvidencePackNamesScopes"
        status: pass
      - kind: unit
        ref: "tests/internal/intelligence/explain/synthesize_test.go#TestRenderJSON_Fields"
        status: pass
    human_judgment: false
  - id: D5
    description: "CLI wiring: m31a explain routed in main.go dispatch before TUI launch; usage exit 1 on no args; missing-symbol exits nonzero printing search scope; flags accepted before or after symbol"
    verification:
      - kind: other
        ref: "command: throwaway harness compiled cmd/m31a/explain.go verbatim against real config/provider/codeintel/git APIs; explain-with-no-args printed Usage and exited 1; explain DefinitelyNotASymbolQxz99 on this repo printed 'no symbol ... found' + 'Searched scopes: indexed workspace symbols (4878 symbols)' and exited 1; real-symbol run collected evidence then failed cleanly at provider selection"
        status: pass
      - kind: other
        ref: "grep: explain dispatch block present in cmd/m31a/main.go between impact dispatch and TUI launch path"
        status: pass
    human_judgment: true
    rationale: "In-repo make build / go test ./cmd/m31a/ remain blocked by PRE-EXISTING legacy compile failures (engine/session, engine/taskrunner, integrations/ledger, tools/todo — extending through workflow and the TUI stack). Behavior was proven via a disposable harness that compiled the new CLI file against the same real APIs and executed it on this repository; a human should confirm the command end-to-end once the architectural reset restores whole-binary builds."

# Metrics
duration: 48 min
completed: 2026-08-25
status: complete
---

# Phase 4 Plan 2: Explain Symbol Mode Tracer Summary

**Git blame porcelain parser plus the full explain pipeline — deterministic evidence collection, single constrained LLM synthesis with structural citation validation, table/json rendering, and m31a explain CLI wiring**

## Performance

- **Duration:** 48 min
- **Started:** 2026-08-25T09:55:25Z
- **Completed:** 2026-08-25T10:43:10Z
- **Tasks:** 3 (Task 1 TDD, Task 2 TDD, Task 3 tracer)
- **Files modified:** 9

## Accomplishments
- Blame porcelain parser handles real captures: metadata suppression across multi-hunk files resolved via SHA-keyed cache (verified against both a captured fixture and a live two-commit repo), continuation headers without num-lines parsed correctly, boundary/previous/unknown tags skipped silently, hostile revs rejected before any git exec; ValidateRef exported for plan 04-04
- Deterministic evidence packs: source excerpt with line anchors, graph callers as file:line refs, blame reduced to one last-touch item, sorted affected tests; kind+ref dedupe (first wins) with sequential renumbering; whole-section budget trimming tests-first via the tokens estimator; ErrTargetNotFound sentinel for unknown symbols
- One-call synthesis behind a narrow Synthesizer interface with a pack-as-data system prompt (T-04-02a); post-synthesis marker validation keeps known-marker sentences, demotes unknown-marker sentences wholesale to Inference with markers stripped, and never emits dangling citations (EXPLAIN-02)
- Dual-format renderers following impact.go/D-04 conventions including an explicit no-evidence report naming searched scopes; m31a explain wired into main.go beside impact, collecting evidence before provider resolution so missing targets fail fast without API keys

## Task Commits

Each task was committed atomically:

1. **Task 1 (RED): failing blame porcelain parser tests** - `c551533e` (test)
2. **Task 1 (GREEN): implement git blame porcelain parser** - `bda191bd` (feat)
3. **Task 2 (RED): failing evidence collector tests** - `9a22ee0e` (test)
4. **Task 2 (GREEN): implement evidence collector for symbol mode** - `eb02af84` (feat)
5. **Task 3 (TRACER): explain symbol mode end-to-end** - `686f7208` (feat)

_Plan metadata commit follows this summary._

## Files Created/Modified
- `internal/integrations/git/blame.go` - BlamePorcelain parser, BlameLine/CommitMeta, ValidateRef export
- `internal/integrations/git/blame_test.go` - Real porcelain fixture, hostile-ref and live-repo suppressed-metadata coverage
- `internal/intelligence/explain/collector.go` - CollectorDeps, Collect symbol mode, ErrTargetNotFound, dedupe + budget trim
- `internal/intelligence/explain/collector_test.go` - Seeded two-commit repo fixture covering every collector behavior
- `internal/intelligence/explain/synthesize.go` - Synthesizer interface, Synthesize, validateMarkers, ExplainAnswer
- `internal/intelligence/explain/render.go` - RenderText / RenderJSON with no-evidence report
- `internal/intelligence/explain/synthesize_test.go` - Mock synthesizer, request-shape, marker validation, render, end-to-end pipeline tests
- `cmd/m31a/explain.go` - runExplain CLI: flag handling, integrations assembly, collect→synthesize→render flow
- `cmd/m31a/main.go` - explain dispatch beside impact before TUI launch; restored corrupted func main()

## Decisions Made
- Whole-file blame collapses to a single last-touch citation (max author-time SHA + summary) rather than per-line rows — keeps packs inside budget while preserving EXPLAIN-01 archaeology value.
- Budget trimming drops whole lowest-priority sections (tests → blame → callers), never mid-truncates snippets, and always retains at least one section: an oversized top-priority item beats an empty pack.
- Marker-less sentences remain in prose; only sentences containing unknown markers are demoted wholesale (with just the unknown markers stripped) — this satisfies the paragraph-level invariant that non-Inference prose references at least one known marker without gutting natural connective sentences.
- runExplain resolves the provider AFTER Collect so `explain <unknown>` prints its search-scope note on machines with no API keys; provider failure surfaces only for genuinely collectable queries.
- explain.go splits flags from positionals manually because stdlib flag stops at the first positional, which would have broken the documented `m31a explain SYMBOL --format json` order.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Restored corrupted func main() in cmd/m31a/main.go**
- **Found during:** Task 3 (CLI wiring compile check)
- **Issue:** main.go at HEAD was syntactically invalid: the `func main() {` opener had been deleted, leaving orphaned `os.Exit(run())` + a stray closing brace after truncate(); package main could not parse at all (pre-existing, predates this plan).
- **Fix:** Re-added `func main() {` above the existing `os.Exit(run())` body — the unique mechanical restoration.
- **Files modified:** cmd/m31a/main.go
- **Verification:** gofmt clean; go vet attributes zero errors to main.go's own lines; standalone typecheck of the new CLI code passes.
- **Committed in:** 686f7208 (Task 3 commit)

**2. [Rule 1 - Bug] Flags after the symbol were swallowed as positionals**
- **Found during:** Task 3 (harness behavioral checks)
- **Issue:** stdlib flag.Parse stops at the first positional argument, so the documented `m31a explain Bar --format json` left format=table and NArg=3 — usage error despite correct user input (same latent quirk exists in impact.go, out of scope there).
- **Fix:** runExplain splits `--format[=| ]value` flags from positionals manually before validating, accepting either argument order.
- **Files modified:** cmd/m31a/explain.go
- **Verification:** harness runs: `explain Bar --format xml` → unsupported-format usage exit 1; `explain --format json X` and `explain X --format json` both route correctly.
- **Committed in:** 686f7208 (Task 3 commit)

**3. [Rule 2 - Missing Critical] Call-edge assembly added to test helper and CLI**
- **Found during:** Task 2 GREEN iteration
- **Issue:** codeintel.BuildGraph populates only the import graph — Graph.Callers returns nothing unless AddCallEdge was fed separately (production receives edges via event-store replay). With the planner-assumed indexer-only setup, caller evidence could never appear.
- **Fix:** Both the seeded-repo test helper and runExplain's buildExplainIntegrations attach CallEdges from parsed CallSites, mirroring the projection's assembly discipline.
- **Files modified:** internal/intelligence/explain/collector_test.go, cmd/m31a/explain.go
- **Verification:** TestCollectSymbol_AssemblesAllKinds asserts callers-kind refs exist; harness CASE D collected evidence for a real symbol end-to-end.
- **Committed in:** eb02af84 (helper), 686f7208 (CLI)

---

**Total deviations:** 3 auto-fixed (1 blocking, 1 bug, 1 missing critical)
**Impact on plan:** All fixes were prerequisites for the tracer's compile/behavioral guarantees; no scope creep beyond the plan's own acceptance criteria.

## Issues Encountered

- **Whole-binary build remains impossible at HEAD (pre-existing, out of scope):** `go build ./cmd/m31a` fails through the four documented legacy packages (engine/session, engine/taskrunner, integrations/ledger, tools/todo). A disposable sandbox exploration additionally showed the cascade extends further than documented — internal/engine/workflow has its own Plan-vs-PlanDocument type errors once deps compile, and the TUI packages (tuitypes/commands/components/streaming/tui) transitively fail. Migrating these belongs to the deferred architectural-reset work; nothing outside cmd/m31a/main.go's func-main restoration was touched.
- **Verification substitution for the tracer gate:** since package main cannot link, cmd/m31a/explain.go was compiled VERBATIM in a throwaway harness against the real config/provider/codeintel/git/explain APIs (typecheck proof), and the harness binary exercised the actual CLI paths on this repository: no-args usage exit 1; DefinitelyNotASymbolQxz99 → "no symbol ... found (searched indexed symbols)" + "Searched scopes: indexed workspace symbols (4878 symbols)" exit 1; real symbol BlamePorcelain → successful collection, then a clean provider-selection failure (exit 1) proving stage ordering. LLM-call behavior is mock-proven in unit tests. Logged to the broken-windows ledger for ship-gate visibility.
- `go vet` silently skips typechecking a package whose imports fail — the earlier "vet clean" signal for package main was meaningless until proven otherwise by the harness typecheck. Worth remembering for scoped verification in later plans.
- Pre-existing test failures unrelated to this plan observed in the broad scoped run: TestDetectWorkspaceRoot_None (codeintel, already in deferred-items), pkg/extensions stale test mock, lsp suite timeout (>300s, environment), narrative/subagent/tui-components failing via the broken dependency chain.

## User Setup Required

None - no external service configuration required. (`explain` uses the configured provider registry; API keys follow the existing Phase 2 conventions.)

## Next Phase Readiness
- The tracer skeleton is proven at package level: plans 04-03..04-07 can reuse Synthesizer, CollectorDeps injection, EvidencePack IDs, validateMarkers-style structural validation, and the RenderText/RenderJSON D-04 convention without modification.
- File/topic modes land in 04-03 behind the already-defined ModeFile/ModeTopic constants; ResolveMode precedence (file → symbol → topic) is documented there.
- EXPLAIN-02 marked complete; EXPLAIN-01 is shared with 04-03 and will be marked when that plan finishes (shared-ID gate).
- Whole-binary gates (make build/check, go test ./cmd/m31a/) stay blocked on the legacy reset migration; scoped verification remains `go test ./internal/intelligence/... ./internal/integrations/git/ ./internal/core/...` plus targeted consumer spot-checks until then.

---
*Phase: 04-intelligence-features*
*Completed: 2026-08-25*

## Self-Check: PASSED

- All 9 created/modified files verified present on disk (`[ -f ]` checks passed).
- All 5 task commits verified in history: c551533e, bda191bd, 9a22ee0e, eb02af84, 686f7208.
- Acceptance criteria re-run: Task 1 greps+tests 5/5; Task 2 signature/kinds/determinism/budget/sentinel/vet 6/6; Task 3 interface/validateMarkers/call-count/mixed-fixture/render exports/dispatch-block greps and behavioral checks 8/8 automated (binary-level items covered by harness substitution, see D5 rationale).
