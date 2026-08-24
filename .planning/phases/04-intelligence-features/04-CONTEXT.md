# Phase 4: Intelligence Features - Context

**Gathered:** 2026-08-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Three intelligence command families operating on top of the Phase 3 symbol graph, Phase 2 provider layer, and Phase 1 EventStore: `m31a explain` (code archaeology — topics, symbols, and file-existence questions with evidence citations), `m31a investigate` (regression root-cause via isolated bisect + repro verification with confidence labeling), and `m31a deps check` (dependency vetting against live registry/vuln sources with cached verdicts and human checkpoint for high-risk findings). All outputs distinguish evidence (citations with file:line, commit SHA) from inference (explicitly marked). Covers EXPLAIN-01..04, REGRESS-01..03, DEPEND-01..04.

</domain>

<decisions>
## Implementation Decisions

### Command Surface Split
- **D-01:** `explain` owns all archaeology/understanding queries — topics ("auth middleware"), symbols, AND files ("why does X exist": introduction commit, original purpose, current consumers, removal impact). `investigate` = regression root-cause only. Resolves the ROADMAP SC#2 vs EXPLAIN-04 conflict in favor of the roadmap success criteria; EXPLAIN-04 is satisfied by explain's file mode — **Reversibility:** reversible — command routing is a thin CLI-layer concern; evidence collectors are shared underneath
- **D-02:** `m31a investigate "<symptom>" [--baseline <ref>] [--repro <cmd>]` — free-text symptom per ROADMAP SC#4; optional flags bound the bisect range and override repro detection; when --baseline omitted, a bounded default window applies (D-11)
- **D-03:** Only `m31a deps check <module>` ships in this phase — no `deps list` / `deps why`. The dependency graph data already exists in the Phase 3 symbol graph, so those views can be added later without schema changes
- **D-04:** All three commands follow the Phase 3 CLI convention: human-readable text/table default + `--format json` emitting structured evidence objects (citations[], confidence, verdict). One scripting convention across intelligence commands

### Explain Grounding & Citations
- **D-05:** Evidence-first single LLM pass — deterministic collector assembles an evidence pack (source excerpt, callers via graph API, git blame/log, ADRs from decisions store, related tests), then ONE LLM call synthesizes the narrative constrained to that pack. Citations are injected structurally from collected data, never model-generated. No dependency on Phase 7 tool infrastructure; honors "model is never sole source of truth"
- **D-06:** Citation format = numbered markers + sections: prose carries [1][2] markers; an Evidence section lists each marker with full citation (file:line, commit SHA, ADR path); statements not backed by any marker go under an explicit "Inference" heading. Structural EXPLAIN-02 compliance; TUI Citation/EvidenceRow components can consume the JSON form directly later
- **D-07:** Shared 3-level confidence enum across explain/investigate/deps: `verified` (machine-checked evidence directly supports claim) / `likely` (strong circumstantial) / `speculative` (weak or contradictory). REGRESS-02's "verified cause" maps to verified, "likely cause" to likely. Single vocabulary for future TUI badges
- **D-08:** EXPLAIN-03 rationale validity is rule-based: verdict class computed from deterministic signals (consumer count via symbol graph, last-touch age, deprecation/TODO/FIXME markers, test coverage presence, ADR staleness); the LLM narrates over computed signals but never invents them. Deterministic and testable

### Bisect & Verification Semantics (investigate)
- **D-09:** Check function = shell command exit code + optional output regex (symptom pattern). Resolution order: `--repro` flag > `[intelligence] repro_command` config > auto-detect per language (`go test ./...` etc.). Fully deterministic; no LLM inside the bisect loop
- **D-10:** Bisect executes in a detached temporary worktree (`git worktree add` at bad ref, removed on exit including interrupt); the user's checkout never moves. This formalizes the WORKTREE-07 direction early for investigate's own use — replaces the legacy in-place `git bisect` behavior of `internal/engine/bisect/bisect.go`
- **D-11:** Default bisect window = last 50 commits (`[intelligence] bisect_max_commits`, configurable). If the symptom doesn't reproduce at the window start, report "not reproducible in window" with evidence and stop — never silently widen. `--baseline <ref>` enables deeper hunts
- **D-12:** Confidence split in root-cause report: commit attribution = `verified` (bisect mechanics prove repro fails at culprit and passes at parent); mechanism explanation = `likely` at best (evidence-first LLM pass over culprit diff + affected symbols from graph Impact()); never auto-promoted to verified

### Dependency Intelligence (deps check)
- **D-13:** Data sources: deps.dev API primary (existence, age, releases, maintenance, license, popularity across ecosystems) + OSV.dev for vulnerabilities + GitHub API for repo-activity enrichment. Transitive impact computed locally from the Phase 3 symbol graph. Read-only HTTP queries inside the command layer — this is NOT the deferred "package registry integrations as agent tools" (§22/ADVINT-04)
- **D-14:** DEPEND-03 human checkpoint: interactive TTY → terminal prompt showing risk evidence (approve/cancel); headless/non-TTY → hard block, exit code 2, verdict + evidence written to EventStore as pending checkpoint record resolvable later via `deps check --approve <module>`. Enforced in both modes with no TUI dependency
- **D-15:** Verdict caching via EventStore events (`DependencyChecked`: module, version, verdict, risk class, source snapshot, timestamp); projections serve cached lookups. Re-evaluation triggers: version change OR `[intelligence] deps_policy` config hash change. Honors Phase 1 "all durable state = events"
- **D-16:** High-risk classification = config-driven rules with safe defaults (`[intelligence.deps_risk]`): known vulnerability = high; license outside allowlist = high; no release in 12 months or archived repo = high; low popularity + young age = medium; API instability signals = medium. Deterministic and adjustable without recompiling

### the agent's Discretion
- Evidence pack size/token budget per explain query (cap context sent to LLM)
- Exact auto-detect heuristics for repro commands per language
- deps.dev/OSV/GitHub request timeouts and retry policy (follow provider-layer resilience patterns from Phase 2)
- Output table column layout for text rendering
- Temp worktree location and naming convention
- Which ADR locations are scanned for the evidence pack (decisions store layout per PERSIST-04)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Architecture & Requirements
- `.planning/CONTEXT_M31A.md` §29 — Intelligence Plane: ExplainEngine, RegressionInvestigator, DependencyAnalyzer responsibilities
- `.planning/CONTEXT_M31A.md` §37 — Six-plane boundaries: Intelligence consumes read-only projections; provides evidence-backed answers upward
- `.planning/REQUIREMENTS.md` — EXPLAIN-01..04, REGRESS-01..03, DEPEND-01..04 (11 requirements)
- `.planning/ROADMAP.md` — Phase 4 success criteria (criteria #1, #2, #4, #5)
- `.planning/PROJECT.md` — Constraints (security model: treat registry responses as untrusted input), out-of-scope list (§22 registry-tools deferral)

### Prior Phase Decisions (binding)
- `.planning/phases/03-code-intelligence-graph/03-CONTEXT.md` — Graph query API (Define/References/Upstream/Downstream/Impact), event vocabulary, multi-repo workspace
- `.planning/phases/02-llm-provider-abstraction/02-CONTEXT.md` — Provider interface, streaming, model profiles, FallbackMode
- `.planning/phases/01-foundation-domain-model-event-store/01-CONTEXT.md` — EventStore patterns, schema_versioning, projection checkpoints

### Research & Risks
- `.planning/research/PITFALLS.md` — Pitfall 10 (provider leakage), Pitfall 29 (intelligence as cache, not authoritative); relevant to evidence-pack construction
- `.planning/research/SUMMARY.md` — Phase-flagged research notes; confidence ratings
- `.planning/codebase/INTEGRATIONS.md` — Existing HTTP client patterns, keychain, webfetch/websearch clients reusable for registry queries

### Current Implementation (Reusable Assets)
- `internal/integrations/codeintel/impact.go` — Impact analysis API consumed by explain/investigate
- `internal/integrations/git/git.go` — Log/LogAll/LogSince/DiffRefs (blame must be added in this phase)
- `internal/engine/bisect/bisect.go` — Legacy in-place bisect (superseded by D-10 temp-worktree approach; reference for checkFn contract)

### External APIs (research targets, not specs)
- deps.dev API (project metadata, license, popularity across ecosystems) — https://docs.deps.dev
- OSV.dev API (vulnerability lookup) — https://osv.dev
- GitHub REST API (repo activity, archive status) — standard auth-free rate-limited tier acceptable

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **CodeIntel query API** (`internal/integrations/codeintel/impact.go`, `graph.go`, `index.go`): Impact(), Upstream/Downstream, Define/References — powers consumer counting (D-08), transitive dependency impact (D-13), and affected-symbol analysis (D-12)
- **Git client** (`internal/integrations/git/git.go`): Log/LogAll/LogSince/DiffRefs/DiffFile ready for archaeology evidence pack; `Run()` generic passthrough available for blame implementation
- **Legacy bisect** (`internal/engine/bisect/bisect.go`): Run(startHash, headHash, checkFn) contract and parseBisectLog — reuse the checkFn callback shape, replace in-place execution with temp worktree (D-10)
- **Provider BaseClient** (`internal/integrations/provider/base_client.go`): retry/backoff/cache patterns to mirror for deps.dev/OSV/GitHub HTTP clients
- **Webfetch client** (`internal/tools/search/webfetch.go`): redirect handling, DNS cache — candidate for registry HTTP calls
- **EventStore** (`internal/memory/eventstore/`): Append, QueryByType/range, subscriptions — DependencyChecked events and pending-checkpoint records land here
- **Config loader** (`internal/core/config/loader.go`): layered TOML — new `[intelligence]` section follows existing section patterns

### Established Patterns
- **CLI convention from Phase 3**: subcommand in cmd/m31a/main.go, table default + `--format json` (impact.go precedent) — D-04 extends it
- **Evidence over prose**: sentinel rule from PROJECT.md out-of-scope list — citations structured, inference labeled (D-05..D-07 enforce mechanically)
- **Events as durable truth**: every persisted artifact becomes an event + projection (D-15)
- **No CGO**: pure-Go HTTP clients only for external sources
- **Structured errors with hints**: ToolError-style error + hint for source-unreachable cases

### Integration Points
- **explain/investigate → CodeIntel**: read-only graph queries for callers, consumers, transitive impact
- **explain/investigate → Git client**: blame (new), log, diff for archaeology and culprit analysis
- **explain → decisions store**: ADR lookup from `.m31a/decisions/` (PERSIST-04 layout) feeds evidence pack
- **investigate → EventStore**: investigation runs recorded as events (start, candidates, verdict)
- **deps check → EventStore**: DependencyChecked events + pending checkpoint records (D-14, D-15)
- **All three → LLMProvider (Phase 2)**: single synthesis calls via ChatCompletion with model profile
- **CLI → main.go**: `explain`, `investigate`, `deps check` subcommands alongside existing index/impact/arch check

</code_context>

<specifics>
## Specific Ideas

- Evidence pack composition for explain: source excerpt (target symbol/file ± context), caller list with file:line, introduction commit + last-touch commit, related ADR excerpts, test files referencing target
- Removal impact output for file mode: consumers count, downstream dependents, tests affected — rendered from graph Impact() data
- Root-cause report structure: Symptom → Bisect range & steps → Culprit commit (verified) → Mechanism hypothesis (likely) → Affected components → Recommended fix direction
- Deps verdict object: { module, version, exists, age, latest_release, maintenance_class, license, vulnerabilities[], transitive_impact, api_stability, popularity, risk_class, confidence, sources[] }
- Pending checkpoint record enables async resolution: headless block writes record; later `--approve` resolves it — decision trail preserved in event log
- Config surface:
  ```toml
  [intelligence]
  repro_command = ""            # override auto-detect
  bisect_max_commits = 50
  [intelligence.deps_risk]      # thresholds per D-16
  stale_months = 12
  [intelligence.deps_policy]    # hash change triggers re-eval per D-15
  ```

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 4-Intelligence Features*
*Context gathered: 2026-08-25*
