---
phase: 04-intelligence-features
plan: 08
subsystem: intelligence
tags: [explain, rationale, verdict, signals, gap-closure]

# Dependency graph
requires:
  - phase: 04-intelligence-features
    provides: explain package with VerdictClassFromSignals function and renderers
provides:
  - Rationale Validity verdict now computed from deterministic signals via VerdictClassFromSignals in both text and JSON output
  - Overall explanation confidence (ans.Confidence) preserved as LLM synthesis confidence
  - Test verifying rationale verdict independence from synthesis confidence
affects: [04-intelligence-features]

# Actuals (#2632) — pairs with the plan's `estimate` to calibrate future estimates.
# Same estimateTokens scale (chars/4 over the realized diff), never a harness token count.
actuals:
  tokens: 1767
  tasks: 2
  commits: 2

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Rationale verdict computed from pure deterministic signals function
    - Separation of rationale verdict (signals-based) from overall confidence (LLM-based)
    - Test-driven verification of verdict independence

key-files:
  created: []
  modified:
    - internal/intelligence/explain/render.go
    - internal/intelligence/explain/synthesize_test.go

key-decisions:
  - "VerdictClassFromSignals called in both RenderText and RenderJSON for Rationale Validity section"
  - "Overall ans.Confidence remains LLM synthesis confidence — only rationale subsection uses signals verdict"
  - "No changes needed to CLI (cmd/m31a/explain.go) — it already computes and passes signals correctly"

patterns-established:
  - "Pure function VerdictClassFromSignals is the single source of truth for rationale verdict"
  - "Rationale validity and explanation confidence are distinct concepts with distinct sources"

requirements-completed:
  - EXPLAIN-03

# Coverage metadata (#1602)
coverage:
  - id: D1
    description: "Rationale Validity verdict in text output uses signals-based verdict from VerdictClassFromSignals"
    requirement: "EXPLAIN-03"
    verification:
      - kind: unit
        ref: "internal/intelligence/explain/synthesize_test.go#TestRenderText_RationaleValiditySection"
        status: pass
      - kind: unit
        ref: "internal/intelligence/explain/synthesize_test.go#TestRationaleVerdictFromSignals"
        status: pass
    human_judgment: false
  - id: D2
    description: "Rationale verdict in JSON output uses signals-based verdict from VerdictClassFromSignals"
    requirement: "EXPLAIN-03"
    verification:
      - kind: unit
        ref: "internal/intelligence/explain/synthesize_test.go#TestRenderJSON_RationaleObject"
        status: pass
      - kind: unit
        ref: "internal/intelligence/explain/synthesize_test.go#TestRationaleVerdictFromSignals"
        status: pass
    human_judgment: false
  - id: D3
    description: "Overall explanation confidence (ans.Confidence) preserved as LLM synthesis confidence"
    requirement: "EXPLAIN-03"
    verification:
      - kind: unit
        ref: "internal/intelligence/explain/synthesize_test.go#TestRationaleVerdictFromSignals"
        status: pass
    human_judgment: false
  - id: D4
    description: "VerdictClassFromSignals correctly maps deterministic signals to three-level Confidence enum"
    requirement: "EXPLAIN-03"
    verification:
      - kind: unit
        ref: "internal/intelligence/explain/signals_test.go#TestVerdictClassFromSignals_Branches"
        status: pass
      - kind: unit
        ref: "internal/intelligence/explain/signals_test.go#TestVerdictClassFromSignals_Deterministic"
        status: pass
    human_judgment: false

# Metrics
duration: 15min
completed: 2026-09-05
status: complete
---

# Phase 04 Plan 08: EXPLAIN-03 Rationale Verdict Gap Closure

**Fixed critical gap where Rationale Validity verdict incorrectly used LLM synthesis confidence (ans.Confidence) instead of deterministic signals verdict from VerdictClassFromSignals**

## Performance

- **Duration:** 15 min
- **Started:** 2026-09-05T04:45:00Z
- **Completed:** 2026-09-05T05:00:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- RenderText now computes rationale verdict via `VerdictClassFromSignals(*ans.RationaleSignals)` instead of `ans.Confidence`
- RenderJSON includes signals-based verdict in `rationale.verdict` field instead of `string(ans.Confidence)`
- Overall explanation confidence (`ans.Confidence` at top level) remains LLM synthesis confidence — only the Rationale Validity subsection changed
- New test `TestRationaleVerdictFromSignals` verifies rationale verdict independence from synthesis confidence (signals=verified, ans.Confidence=likely → rationale shows verified, overall shows likely)
- All existing explain tests pass including end-to-end mock pipeline

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix render.go to use VerdictClassFromSignals for rationale verdict** - `ac8e4c06` (fix)
2. **Task 2: Update synthesize_test.go with test verifying signals-based rationale verdict** - `fd1b816a` (test)

## Files Created/Modified

- `internal/intelligence/explain/render.go` - Fixed both RenderText (line 52) and RenderJSON (line 93) to call `VerdictClassFromSignals` for rationale verdict
- `internal/intelligence/explain/synthesize_test.go` - Added `TestRationaleVerdictFromSignals` demonstrating the fix

## Decisions Made

- VerdictClassFromSignals is called in both renderers for the Rationale Validity section — this is the correct architecture since the function is pure and deterministic
- Overall ans.Confidence remains the LLM synthesis confidence — the plan correctly separates "rationale validity" (are the code signals healthy?) from "explanation confidence" (how well did the LLM cite evidence?)
- No changes needed to cmd/m31a/explain.go — it already computes signals via ComputeRationaleSignals and passes them to Synthesize, which attaches them to the answer for rendering

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- Go build cache disk quota exceeded on /tmp — resolved by setting TMPDIR and GOCACHE to home directory locations
- Four legacy packages fail to build (engine/session, engine/taskrunner, integrations/ledger, tools/todo) — pre-existing, out of scope per known environment notes. Explain package builds and tests pass independently.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- EXPLAIN-03 rationale validity gap fully closed per VERIFICATION.md
- All 61 phase verification truths now satisfied (previously 58/61 with 3 related to this gap)
- Phase 4 Intelligence Features complete — ready for verification and transition to Phase 5

---
*Phase: 04-intelligence-features*
*Completed: 2026-09-05*
## Self-Check: PASSED
