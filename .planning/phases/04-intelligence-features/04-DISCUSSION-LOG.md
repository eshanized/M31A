# Phase 4: Intelligence Features - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-25
**Phase:** 4-Intelligence Features
**Areas discussed:** Command surface split, Explain grounding & citations, Bisect & verification semantics, Deps sources & checkpoint

---

## Area Selection

| Option | Description | Selected |
|--------|-------------|----------|
| Command surface split | Resolve ROADMAP SC#2 vs EXPLAIN-04 conflict; explain vs investigate ownership | ✓ |
| Explain grounding & citations | Evidence-first vs agentic synthesis; citation format; confidence vocabulary | ✓ |
| Bisect & verification semantics | Check function pre-Phase 9; execution isolation; range bounds | ✓ |
| Deps sources & checkpoint | Registry/vuln sources; DEPEND-03 checkpoint UX without Phase 5 gates | ✓ |

**User's choice:** "All" — every presented area selected.
**Notes:** None.

---

## Command surface split

### Q1: Which command owns "why file exists" archaeology?

| Option | Description | Selected |
|--------|-------------|----------|
| explain owns it (Recommended) | `explain` handles all understanding queries (topics, symbols, files incl. removal impact); `investigate` = regression only. Matches ROADMAP SC#1-2; satisfies EXPLAIN-04 via explain's file mode | ✓ |
| investigate routes both | One command classifying intent and routing internally; diverges from ROADMAP SC#2 wording | |
| Both, layered | Narrative explain plus deeper `investigate <file>` archaeology workflow | |

**User's choice:** explain owns it
**Notes:** Verification tests against roadmap success criteria, which drove the recommendation.

### Q2: How does `m31a investigate` receive the regression report?

| Option | Description | Selected |
|--------|-------------|----------|
| Free text + flags (Recommended) | `"symptom after commit X" [--baseline <ref>] [--repro <cmd>]`; bounded default window when baseline omitted | ✓ |
| Structured flags only | Explicit --symptom/--good/--bad/--check required; diverges from SC#4 examples | |
| Interactive fallback | Drops into dialogue asking repro steps when symptom can't be operationalized | |

**User's choice:** Free text + flags
**Notes:** None.

### Q3: How wide should the `deps` command surface be in this phase?

| Option | Description | Selected |
|--------|-------------|----------|
| check only (Recommended) | Only `deps check <module>` per SC#5; list/why deferred — Phase 3 graph already holds dependency data | ✓ |
| Full deps family | check + list + why now; overlaps S24 Dependency Explorer territory | |
| check + explain hook | deps check plus explain integration for third-party packages | |

**User's choice:** check only
**Notes:** None.

### Q4: Output convention for explain / investigate / deps check?

| Option | Description | Selected |
|--------|-------------|----------|
| table + JSON (Recommended) | Text default; `--format json` structured evidence objects; matches Phase 3 impact precedent | ✓ |
| Text-only for now | Inline citation markers only; JSON deferred to TUI phase | |
| Markdown default | Glamour-rendered output; diverges from impact precedent | |

**User's choice:** table + JSON
**Notes:** None.

---

## Explain grounding & citations

### Q1: How does `explain` produce its answer?

| Option | Description | Selected |
|--------|-------------|----------|
| Evidence-first pass (Recommended) | Deterministic evidence pack → ONE LLM call constrained to it; citations injected structurally; no Phase 7 dependency | ✓ |
| Agentic loop | LLM requests more evidence iteratively via tool calls; requires Phase 7 infrastructure | |
| Template-only | No LLM; deterministic report builder; misses EXPLAIN-01 synthesis quality | |

**User's choice:** Evidence-first pass
**Notes:** Constraint surfaced: tool registry/dispatcher lands in Phase 7, making agentic loop premature.

### Q2: How should citations appear in the text output?

| Option | Description | Selected |
|--------|-------------|----------|
| Markers + sections (Recommended) | [1][2] markers; Evidence section with full citations; unbacked statements under "Inference" heading | ✓ |
| Inline parenthetical | Trailing `(file:line @ sha)` per claim; dense | |
| Footnotes | Clean answer first, numbered references at end; weakest claim-evidence binding | |

**User's choice:** Markers + sections
**Notes:** Structural EXPLAIN-02 compliance noted; TUI Citation/EvidenceRow compatibility.

### Q3: What confidence vocabulary do intelligence outputs use?

| Option | Description | Selected |
|--------|-------------|----------|
| 3-level enum (Recommended) | verified / likely / speculative shared across all commands; superset of REGRESS-02 wording | ✓ |
| 2-level literal | Exactly verified/likely per requirement text; no place for weak findings | |
| Numeric score | 0.0–1.0 per finding; false precision concern | |

**User's choice:** 3-level enum
**Notes:** REGRESS-02 mapping: "verified cause"→verified, "likely cause"→likely.

### Q4: How is EXPLAIN-03 rationale validity determined?

| Option | Description | Selected |
|--------|-------------|----------|
| Heuristic signals (Recommended) | Rule-based verdict from consumer count, last-touch age, deprecation markers, test coverage, ADR staleness; LLM narrates only | ✓ |
| LLM judgment | Model weighs raw signals; non-deterministic | |
| Human confirms | Interactive confirmation per run; adds mandatory interaction | |

**User's choice:** Heuristic signals
**Notes:** None.

---

## Bisect & verification semantics

### Q1: What acts as the "verification" check function during bisect in v1?

| Option | Description | Selected |
|--------|-------------|----------|
| Exit code + regex (Recommended) | Shell exit code decides good/bad; optional regex refines symptom match; --repro > config > auto-detect resolution | ✓ |
| Exit code only | Pure exit codes; weak for fuzzy symptoms | |
| LLM judges steps | Per-step LLM output judgment; log2(N) extra calls, non-deterministic | |

**User's choice:** Exit code + regex
**Notes:** Determinism inside bisect loop was the deciding factor.

### Q2: Where does regression bisect execute?

| Option | Description | Selected |
|--------|-------------|----------|
| Temp worktree (Recommended) | Detached `git worktree add` at bad ref; removed on exit incl. interrupt; user checkout untouched | ✓ |
| In-place hardened | Legacy behavior + dirty-refusal + always-reset; cleanup bugs leave tree moved | |
| Temp clone | Fully isolated clone; heavier disk/time cost | |

**User's choice:** Temp worktree
**Notes:** Legacy `internal/engine/bisect` runs git-bisect in the user's worktree — flagged as unsafe during analysis; decision formalizes WORKTREE-07 direction early.

### Q3: What bounds the bisect range when --baseline is omitted?

| Option | Description | Selected |
|--------|-------------|----------|
| Bounded 50 (Recommended) | Last 50 commits configurable via `[intelligence] bisect_max_commits`; not-reproducible reported honestly at window edge | ✓ |
| Merge-base | Window = merge-base with main; fails on long-lived branches/detached HEAD | |
| Full history | Time-budgeted full bisect; unpredictable duration | |

**User's choice:** Bounded 50
**Notes:** Never silently widen — explicit user guidance to pass --baseline for deeper hunts.

### Q4: How are confidence labels assigned in the root-cause report?

| Option | Description | Selected |
|--------|-------------|----------|
| Attribution verified, mechanism likely (Recommended) | Commit attribution verified by bisect mechanics; mechanism narrative from evidence-first LLM pass capped at likely | ✓ |
| All-likely output | Everything likely; discards strongest bisect evidence | |
| Auto-upgrade loop | LLM proposes/runs follow-up checks; requires Phase 7 tools | |

**User's choice:** Attribution verified, mechanism likely
**Notes:** None.

---

## Deps sources & checkpoint

### Q1: Which data sources power `deps check`?

| Option | Description | Selected |
|--------|-------------|----------|
| deps.dev + OSV (Recommended) | deps.dev primary (multi-ecosystem metadata) + OSV.dev vulns + GitHub activity enrichment; transitive impact local from graph; read-only HTTP ≠ deferred §22 agent-tools scope | ✓ |
| Go-only sources | proxy.golang.org + pkg.go.dev + vuln.go.dev; nothing for other indexed languages | |
| Model-memory only | Forbidden by DEPEND-02; ruled out | |

**User's choice:** deps.dev + OSV
**Notes:** Out-of-scope tension ("package registry integrations") resolved as read-only HTTP inside command layer vs agent-tool integrations deferred to ADVINT-04.

### Q2: How does the DEPEND-03 human checkpoint manifest without Phase 5 gates or the TUI?

| Option | Description | Selected |
|--------|-------------|----------|
| Prompt / headless block (Recommended) | TTY: terminal approve/cancel prompt with risk evidence; headless: exit 2 + pending checkpoint record resolvable via `--approve` | ✓ |
| Exit-code gate | Always non-zero exit + `--approve` re-run; approval not recorded as event | |
| Warning only | Prominent text warning; arguably fails DEPEND-03 | |

**User's choice:** Prompt / headless block
**Notes:** Pending-checkpoint record keeps decision trail in EventStore.

### Q3: Where are dependency verdicts cached (DEPEND-04)?

| Option | Description | Selected |
|--------|-------------|----------|
| EventStore events (Recommended) | `DependencyChecked` events + projections; re-eval on version change or policy hash change | ✓ |
| File cache | `.m31a/cache/...json`; second durable-state location contradicts Phase 1 architecture | |
| Events + file | Authority + read-through cache; marginal gain | |

**User's choice:** EventStore events
**Notes:** Honors Phase 1 "all durable state = events".

### Q4: What classifies a dependency as high-risk (checkpoint-triggering)?

| Option | Description | Selected |
|--------|-------------|----------|
| Config rules (Recommended) | `[intelligence.deps_risk]` thresholds, safe defaults: vuln=high, license outside allowlist=high, stale/archived=high, etc. | ✓ |
| Hardcoded rules | Same rules, not adjustable without recompile | |
| LLM risk class | Non-deterministic gate trigger; hard to audit | |

**User's choice:** Config rules
**Notes:** None.

---

## the agent's Discretion

- Evidence pack size/token budget per explain query
- Exact auto-detect heuristics for repro commands per language
- deps.dev/OSV/GitHub timeouts and retry policy (mirror provider-layer resilience)
- Output table column layout for text rendering
- Temp worktree location and naming convention
- ADR locations scanned for the evidence pack (per PERSIST-04 layout)

## Deferred Ideas

None — discussion stayed within phase scope.
