---
gsd_state_version: 1.0
status: unknown
stopped_at: Completed 04-05-PLAN.md
last_updated: "2026-09-04T22:18:15.527Z"
state_head: 40fd96f52bf7f9de7e20bf422e2490cb3f82dbe5
progress:
  total_phases: 12
  completed_phases: 2
  total_plans: 23
  completed_plans: 22
  percent: 17
current_phase: 4
current_phase_name: Intelligence Features
---

# State: M31A

**Project:** M31A  
**Core Value:** A persistent software-engineering agent runtime that turns natural-language intent into verified, resumable engineering work.  
**Milestone:** 1 (Architectural Foundation)  
**Last Updated:** 2026-08-24  

---

## Project Reference

| Field | Value |
|-------|-------|
| **Core Value** | Persistent software-engineering agent runtime turning natural-language intent into verified, resumable engineering work |
| **Current Focus** | Phase 1: Foundation — Domain Model & Event Store |
| **Mode** | yolo (auto-approve) |
| **Granularity** | fine (12 phases) |
| **Parallelization** | enabled |
| **Model Profile** | adaptive (role-based cost optimization) |

---

## Current Position

| Field | Value |
|-------|-------|
| **Current Phase** | 4 — Intelligence Features |
| **Current Plan** | 04-06 (Deps clients) |
| **Phase Status** | In progress (6/7 plans complete) |
| **Progress** | ██████████ 96% (22/23 plans) |
| **Active Workstream** | main |
| **Git Branch** | main |
| **Last Commit** | 40fd96f5 |

---

## Performance Metrics

| Metric | Value |
|--------|-------|
| **Phases Completed** | 0/12 |
| **Plans Executed** | 2 |
| **Tests Passing** | ✓ |
| **Coverage** | - |
| **Verification Verdicts** | - |
| **Open UAT Items** | - |
| **Blockers** | None |

---
**Per-Plan Metrics:**

| Plan | Duration | Tasks | Files |
|------|----------|-------|-------|
| Phase 04 P01 | 30 min | 2 tasks | 12 files |
| Phase 04 P02 | 48 min | 3 tasks | 9 files |
| Phase 04 P03 | 90 min | 3 tasks | 8 files |
| Phase 04 P04 | 55 min | 3 tasks | 6 files |
| Phase 04 P05 | 95 min | 3 tasks | 4 files |
| Phase 04 P06 | 15 min | 3 tasks | 12 files |
| Phase 4 P5 | 95 | 3 tasks | 4 files |

## Accumulated Context

### Decisions Logged

| ID | Decision | Rationale | Phase |
|----|----------|-----------|-------|
| D-001 | Architectural reset (not incremental refactor) | Current implementation has fundamental architectural problems conflicting with canonical design | Pre-milestone |
| D-002 | Fine granularity (12 phases) | Many distinct subsystems need focused phases | Pre-milestone |
| D-003 | YOLO mode (auto-approve) | Speed up iteration during architectural migration; checkpoints protect irreversible actions | Pre-milestone |
| D-004 | Parallel execution | Independent subsystems can be built in parallel | Pre-milestone |
| D-005 | Git-tracked planning docs | `.planning/` committed for team visibility and history | Pre-milestone |
| D-006 | Adaptive model profile | Role-based cost optimization: heavy roles use highest-tier, light roles use cheapest | Pre-milestone |
| D-007 | Research before each phase | Investigate domain, find patterns, surface gotchas before planning | Pre-milestone |
| D-008 | Plan checker enabled | Catch gaps before execution starts | Pre-milestone |
| D-009 | Verifier enabled | Confirm deliverables match phase goals | Pre-milestone |
| D-010 | Drift guard enabled | Plan review enforces source-grounding | Pre-milestone |
| D-011 | TUI as projection only | TUI owns presentation state only; never domain truth | Pre-milestone |
| D-012 | SQLite for durable state | Embedded, no separate server, ACID, good for event sourcing | Pre-milestone |
| D-013 | Event-driven architecture | Append-only events enable recovery, replay, audit, debugging | Pre-milestone |
| D-014 | Six-plane architecture | Interaction, Intelligence, Engineering, Execution, Assurance, Memory — clear separation | Pre-milestone |
| D-015 | deps.dev v3 PathEscape for module paths | Preserves nested paths and /v2 major suffixes exactly as API expects (uppercase GO system enum) | 04-06 |
| D-016 | OSV leading-v normalization with build-metadata preservation | Go ecosystem requires v-prefix; build metadata makes v1.0.0+build distinct from v1.0.0 | 04-06 |
| D-017 | GitHub OpenIssuesAndPRs field naming | open_issues_count includes PRs per Pitfall 6; struct name reflects true semantics | 04-06 |
| D-018 | Risk classification Source provenance on every rule | DEPEND-02 requires citing which snapshot (osv|depsdev|github) fired each rule | 04-06 |
| D-019 | Release age >= stale_months boundary is high | D-16 greater-or-equal convention; age exactly equal to threshold classifies high | 04-06 |
| D-020 | Case-exact SPDX license allowlist matching | Ecosystem case rules (Go module paths case-sensitive) — "MIT" matches only "MIT" | 04-06 |
| D-021 | Rationale-validity verdict from deterministic signals | VerdictClassFromSignals maps ConsumerCount/LastTouchAgeDays/HasDeprecationMarkers/HasTestCoverage/ADRStale to verified/likely/speculative; LLM narrates only | 04-03 |
| D-022 | File mode archaeology via git log --diff-filter=A | Introduction commit SHA + subject from first commit adding the file; consumers from graph.Callers; removal impact from AnalyzeImpact | 04-03 |
| D-023 | Topic mode via token-ranked source excerpts | Query tokenized; files scored by token occurrences in filename + content (first 200 lines); top 3 with excerpts, symbol hits, recent commits | 04-03 |
| D-024 | ADR scanning with silent degradation | ScanADRs returns CandidateADR[] from .m31a/decisions/; token overlap filter; returns nil (not error) when directory absent | 04-03 |
| D-025 | ResolveMode precedence: file → symbol → topic | File existence checked first via workDir; then exact Index.Define hit; else topic fallback | 04-03 |
| D-026 | Rationale Validity section in all explain outputs | RenderText: "Rationale Validity:" with verdict + signal flags; RenderJSON: rationale object with verdict + boolean/count signals; no floats | 04-03 |
| D-027 | Confirmation-run attribution: verified only after dual observation | CulpritConfidence=Verified only after culprit=fail AND parent=pass; MechanismConfidence≤Likely by construction | 04-05 |
| D-028 | Mechanism confidence ceiling at Likely | Even if synthesis returns Verified, MechanismConfidence capped at Likely; nil synth→Speculative | 04-05 |
| D-029 | Not-reproducible-in-window with sentinel evidence | Returns checked SHA, observed result, window size; Speculative overall confidence; no fabricated attribution | 04-05 |
| D-030 | Affected components dedup by name with minimal depth | Multi-path reachable components appear once at minimal depth; ordered depth-then-name | 04-05 |
| D-031 | Manual flag parsing for flags-after-positional | Stdlib flag stops at first positional; manual split supports both argument orders like explain.go | 04-05 |

### Active Todos

- [x] Run `/gsd-plan-phase 1` to create detailed plan for Foundation phase
- [x] Execute Phase 1 Plan 01-01: Domain types (18 types), event envelope, EventStore interface, serialization
- [ ] Execute Phase 1 Plan 01-02: Config system with EventStore/Migration sections, layered loading, keychain integration
- [ ] Execute Phase 1 Plan 01-03: Event store core: schema, WAL mode, transactional append, range queries
- [ ] Execute Phase 1 Plan 01-04: Event subscription (TUI real-time), hot backup (non-blocking)
- [ ] Execute Phase 1 Plan 01-05: Projection manager with checkpoints, artifact writers (project.md, decisions/, research/), .gitignore
- [ ] Execute Phase 1 Plan 01-06: Migration engine: parse .planning/, emit events, rebuild projections, archive .planning/, CLI command
- [ ] Validate migration from `.planning/` to `.m31a/` works end-to-end

### Blockers

None

### Notes

- Research complete (confidence: HIGH) — see `.planning/research/SUMMARY.md`
- 118 v1 requirements mapped to 12 phases with 100% coverage
- Key risks identified: Distributed state ownership (Pitfall 1), Git safety (Pitfall 2), State machine validation (Pitfall 3), Verification threshold (Pitfall 4), Permission/TUI coupling (Pitfall 5), Task recovery (Pitfall 6), Path validation (Pitfall 7), Shell safety (Pitfall 8), Event store durability (Pitfall 9), Provider leakage (Pitfall 10)
- Phases flagged for deeper research during planning: Phase 3 (Code Intelligence Tree-sitter+LSP orchestration), Phase 6 (Adversarial review loop mechanics), Phase 6 (Decision intelligence heuristics), Phase 10 (TUI performance at 160+ cols)

---

## Session Continuity

**Last session:** 2026-09-04T22:18:08.057Z
**Stopped at:** Completed 04-05-PLAN.md
**Resume file:** None

### Last Session Summary

Executed Plan 04-05 (Investigate Bisect + Report): Completed confirmation-run attribution semantics (D-12) with two-tier root-cause report (REGRESS-02/03), durable InvestigationStarted/Completed events, and CLI wiring with --baseline/--repro flags (D-02). All 3 tasks completed (1 TDD, 1 rendering, 1 TRACER): 3 commits, 4 files created/modified, 29 tests passing.

### Next Actions

1. **Immediate**: Execute Phase 4 Plan 04-06 (Deps clients) - DONE
2. **Then**: Execute Phase 4 Plan 04-07 (Deps verdict cache + checkpoint)
3. **Verify**: Run `/gsd-verify-work 4` after phase completes

### Context for Resume

- Working directory: `/home/snigdha/Desktop/M31A`
- Planning directory: `.planning/`
- Config: `.planning/config.json` (yolo mode, fine granularity, parallelization)
- Research flags: Phase 3, 6, 10 need `--research-phase` during planning
- Key files: `CONTEXT_M31A.md` (canonical architecture), `UI-SPEC.md` (36-screen TUI spec), `.planning/codebase/` (current implementation analysis)

---

## Decisions

- [Phase 04]: Shared intelligence type vocabulary: 3-level Confidence enum per D-07 — Single authoritative citation/confidence contract for EXPLAIN-02 compliance across plans 04-02..04-07
- [Phase 04]: Blame collapses to ONE last-touch citation (max author-time SHA) per explain query — keeps packs in budget while preserving archaeology value
- [Phase 04]: Structural citation validation demotes unknown-marker sentences wholesale to Inference; marker-less sentences stay in prose — paragraph-level grounding invariant without gutting natural prose
- [Phase 04]: runExplain collects evidence BEFORE provider selection so not-found targets fail fast without API keys
- [Phase 04]: Restored corrupted func main() in cmd/m31a/main.go (pre-existing syntax breakage blocked all package-main compiles)
- [Phase 04 P06]: deps.dev v3 module paths use url.PathEscape preserving nested paths and /v2 suffixes exactly (uppercase GO system enum)
- [Phase 04 P06]: OSV version normalization adds leading 'v' for Go ecosystem; build-metadata suffix preserved verbatim making distinct queries
- [Phase 04 P06]: GitHub open_issues_count labeled OpenIssuesAndPRs — includes PRs per Pitfall 6
- [Phase 04 P06]: Risk classification every TriggeredRule carries Source provenance (osv|depsdev|github) per DEPEND-02
- [Phase 04 P06]: Release age boundary: >= stale_months classifies high (greater-or-equal convention)
- [Phase 04 P06]: Case-exact SPDX license allowlist matching — "MIT" matches only "MIT", not "mit"
- [Phase 04 P04]: Worktree temp dir uses pid+nanosecond timestamp (m31a-investigate-<pid>-<ts>) under os.TempDir() — unique per run, enables orphan detection
- [Phase 04 P04]: ValidateRef exported from investigate package — all ref-taking git calls gated through it pre-exec
- [Phase 04 P04]: ExecuteRepro uses exec.CommandContext with strings.Fields arg-splitting — no shell, no metacharacter interpretation; 64KB output cap
- [Phase 04 P04]: FindCulprit pre-checks mandatory: symptom must FAIL at window end and PASS at window start; violation returns NotReproducibleInWindowError with evidence — window never silently widened
- [Phase 04 P04]: Confirmation pass requires culprit fails AND parent passes (two independent observations) before verified attribution (D-12)
- [Phase 4]: Worktree temp dir uses pid+nanosecond timestamp (m31a-investigate-<pid>-<ts>) under os.TempDir() — unique per run, enables orphan detection
- [Phase 4]: ValidateRef exported from investigate package — all ref-taking git calls gated through it pre-exec
- [Phase 4]: ExecuteRepro uses exec.CommandContext with strings.Fields arg-splitting — no shell, no metacharacter interpretation; 64KB output cap
- [Phase 4]: FindCulprit pre-checks mandatory: symptom must FAIL at window end and PASS at window start; violation returns NotReproducibleInWindowError with evidence — window never silently widened
- [Phase 4]: Confirmation pass requires culprit fails AND parent passes (two independent observations) before verified attribution (D-12)
- [Phase 4]: Confirmation-run attribution: verified only after dual observation (D-27)
- [Phase 4]: Mechanism confidence ceiling at Likely (D-28)
- [Phase 4]: Not-reproducible-in-window with sentinel evidence (D-29)
- [Phase 4]: Affected components dedup by name with minimal depth (D-30)
- [Phase 4]: Manual flag parsing for flags-after-positional (D-31)
