# Phase 4: Intelligence Features - Research

**Researched:** 2026-08-25
**Domain:** Evidence-backed code intelligence (explain/archaeology), regression root-cause via isolated bisect, dependency vetting against live registry APIs — all in Go 1.26+, CGO_ENABLED=0
**Confidence:** HIGH

## Summary

Phase 4 adds three command families on top of infrastructure that already exists: `explain` (deterministic evidence pack → single LLM synthesis, D-05), `investigate` (binary-search regression hunting inside a detached temp worktree, D-09/D-10), and `deps check` (deps.dev + OSV.dev + GitHub enrichment with cached verdicts, D-13/D-15). No new external Go packages are required — every external integration is plain JSON-over-HTTPS against well-documented free APIs, implemented with stdlib `net/http`/`encoding/json`, mirroring the resilience patterns already established in `internal/integrations/provider/base_client.go`.

The external-API surface verified this session: **deps.dev v3** gives package existence, versions with `publishedAt`/`isDefault`/`isDeprecated`, SPDX licenses (`licensecheck`-derived for Go), advisory keys, project stars/forks/open-issues/OpenSSF-Scorecard — no auth, no formal rate limit (429-on-overload with backoff advised). **OSV.dev** is a single unauthenticated POST `/v1/query` returning OSV-schema vulns; the Go ecosystem string is `"Go"`. **GitHub REST** `GET /repos/{owner}/{repo}` provides `pushed_at`, `archived`, `open_issues_count`; unauthenticated budget is 60 req/hr per IP, so enrichment must be cached (D-15) and optionally token-backed. **Git mechanics**: blame parsing uses `--porcelain` (stateful — commit metadata prints once per commit); temp worktrees use `git worktree add --detach`, whose metadata lands in `.git/worktrees/<id>` and whose lifecycle (remove/prune/signal cleanup) is fully documented.

The two riskiest engineering areas are (1) **worktree hygiene under interrupt** — leaked worktrees leave stale metadata that blocks later runs until pruned — and (2) **LLM citation integrity** — the model must never invent citation targets, so marker IDs are validated against the evidence pack post-hoc and unmatched statements are demoted to the Inference section. Both have mechanical mitigations documented below.

**Primary recommendation:** Build three self-contained packages under `internal/intelligence/` (explain, investigate, deps) plus shared confidence/evidence types in `internal/core/types`; add blame + worktree methods to `internal/integrations/git`; extend config with `[intelligence]`; register three CLI subcommands following the `impact.go` precedent. Zero new module dependencies.

## User Constraints (from CONTEXT.md)

### Locked Decisions

**Command Surface Split**
- **D-01:** `explain` owns all archaeology/understanding queries — topics ("auth middleware"), symbols, AND files ("why does X exist": introduction commit, original purpose, current consumers, removal impact). `investigate` = regression root-cause only.
- **D-02:** `m31a investigate "<symptom>" [--baseline <ref>] [--repro <cmd>]` — free-text symptom; optional flags bound the bisect range and override repro detection; when --baseline omitted, a bounded default window applies (D-11)
- **D-03:** Only `m31a deps check <module>` ships in this phase — no `deps list` / `deps why`.
- **D-04:** All three commands follow the Phase 3 CLI convention: human-readable text/table default + `--format json` emitting structured evidence objects (citations[], confidence, verdict).

**Explain Grounding & Citations**
- **D-05:** Evidence-first single LLM pass — deterministic collector assembles an evidence pack (source excerpt, callers via graph API, git blame/log, ADRs from decisions store, related tests), then ONE LLM call synthesizes the narrative constrained to that pack. Citations are injected structurally from collected data, never model-generated.
- **D-06:** Citation format = numbered markers + sections: prose carries [1][2] markers; an Evidence section lists each marker with full citation (file:line, commit SHA, ADR path); statements not backed by any marker go under an explicit "Inference" heading.
- **D-07:** Shared 3-level confidence enum across explain/investigate/deps: `verified` / `likely` / `speculative`. REGRESS-02's "verified cause" maps to verified, "likely cause" to likely.
- **D-08:** EXPLAIN-03 rationale validity is rule-based: verdict class computed from deterministic signals (consumer count via symbol graph, last-touch age, deprecation/TODO/FIXME markers, test coverage presence, ADR staleness); the LLM narrates over computed signals but never invents them.

**Bisect & Verification Semantics**
- **D-09:** Check function = shell command exit code + optional output regex. Resolution order: `--repro` flag > `[intelligence] repro_command` config > auto-detect per language (`go test ./...` etc.). Fully deterministic; no LLM inside the bisect loop.
- **D-10:** Bisect executes in a detached temporary worktree (`git worktree add` at bad ref, removed on exit including interrupt); the user's checkout never moves. Replaces the legacy in-place `git bisect` behavior of `internal/engine/bisect/bisect.go`.
- **D-11:** Default bisect window = last 50 commits (`[intelligence] bisect_max_commits`, configurable). If the symptom doesn't reproduce at the window start, report "not reproducible in window" with evidence and stop — never silently widen.
- **D-12:** Confidence split in root-cause report: commit attribution = `verified` (bisect proves repro fails at culprit and passes at parent); mechanism explanation = `likely` at best (evidence-first LLM pass over culprit diff + affected symbols from graph Impact()); never auto-promoted to verified.

**Dependency Intelligence**
- **D-13:** Data sources: deps.dev API primary + OSV.dev vulnerabilities + GitHub API repo-activity enrichment. Transitive impact computed locally from the Phase 3 symbol graph. Read-only HTTP queries inside the command layer.
- **D-14:** Human checkpoint: interactive TTY → terminal prompt showing risk evidence (approve/cancel); headless/non-TTY → hard block, exit code 2, verdict + evidence written to EventStore as pending checkpoint record resolvable later via `deps check --approve <module>`. Enforced in both modes with no TUI dependency.
- **D-15:** Verdict caching via EventStore events (`DependencyChecked`: module, version, verdict, risk class, source snapshot, timestamp); projections serve cached lookups. Re-evaluation triggers: version change OR `[intelligence] deps_policy` config hash change.
- **D-16:** High-risk classification = config-driven rules with safe defaults (`[intelligence.deps_risk]`): known vulnerability = high; license outside allowlist = high; no release in 12 months or archived repo = high; low popularity + young age = medium; API instability signals = medium.

### agent's Discretion
- Evidence pack size/token budget per explain query (cap context sent to LLM)
- Exact auto-detect heuristics for repro commands per language
- deps.dev/OSV/GitHub request timeouts and retry policy (follow provider-layer resilience patterns from Phase 2)
- Output table column layout for text rendering
- Temp worktree location and naming convention
- Which ADR locations are scanned for the evidence pack (decisions store layout per PERSIST-04)

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| EXPLAIN-01 | explain combines source, call graph, Git blame, commit history, ADRs, tests | Evidence-pack builder pattern (Architecture Patterns §Pattern 1); blame via new `BlamePorcelain()` (§Code Examples); callers/tests via existing `AnalyzeImpact` [VERIFIED: internal/integrations/codeintel/impact.go:136-157] |
| EXPLAIN-02 | Output distinguishes evidence citations from inference | D-06 numbered-marker structure; post-synthesis marker validation against pack IDs (Pitfall 8) |
| EXPLAIN-03 | Rationale validity assessment with confidence | D-08 deterministic signal table: consumer count (Impact), last-touch age (blame max SHA date), TODO/FIXME grep, test presence (AffectedTests), rendered before LLM narration |
| EXPLAIN-04 | File mode: introduction commit, original purpose, consumers, removal impact | File-mode collector: first-commit-touching-file via `git log --diff-filter=A --format=...` + blame; consumers/removal via `AnalyzeImpact(graph, index, symbol, depth)` |
| REGRESS-01 | investigate identifies failing behavior, bisects, hypothesizes, verifies | Worktree-bisect engine (§Pattern 3): rev-list candidates → binary search → checkFn(exit+regex) → culprit diff → single LLM mechanism pass |
| REGRESS-02 | Distinguishes likely vs verified cause | D-12 two-tier report: attribution verified only after parent-passes/culprit-fails confirmation run |
| REGRESS-03 | Output includes culprit commit, mechanism, affected components, fix direction | Report structure: Symptom → Range & steps → Culprit (verified) → Mechanism (likely) → Affected (graph Impact) → Fix direction |
| DEPEND-01 | Registry existence, age, releases, maintenance, source repo, license, vulns, transitive impact, API stability, popularity | deps.dev GetPackage/GetVersion/GetProject covers existence/age/releases/license/popularity/stars; OSV query covers vulns; symbol graph covers local transitive impact (§Standard Stack API tables) |
| DEPEND-02 | Model-memory suggestions marked unverified; registry queries authoritative | Verdict object carries `sources[]` provenance; anything not backed by a live source snapshot gets confidence `speculative` |
| DEPEND-03 | Human checkpoint for high-risk dependencies | Existing event pair reused (see Runtime State Inventory quote); TTY detect via `os.Stdin.Stat()` char-device check; headless exit 2 |
| DEPEND-04 | Verdicts cached with timestamp; re-evaluated on version/policy change | DependencyChecked event + cache-key `(module, version, deps_policy_hash)` lookup projection |

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Evidence collection (blame, log, graph queries, tests) | Intelligence layer (`internal/intelligence/explain`) | Integrations (git, codeintel) | Deterministic collectors orchestrate read-only integration APIs; no business logic in CLI or integrations |
| LLM synthesis | Intelligence layer | Provider layer (Phase 2) | Single ChatCompletion call through provider registry; intelligence never talks HTTP to model vendors directly (Pitfall 10 from PITFALLS.md) |
| Bisect execution + repro runs | Intelligence layer (`internal/intelligence/investigate`) | Git client (new worktree methods) | Isolation semantics owned by investigator; git client stays a thin command wrapper |
| Registry/vuln lookups | Integrations-style clients inside `internal/intelligence/deps` | — | Read-only HTTP with retry/backoff mirroring BaseClient patterns; not agent tools (D-13, PROJECT.md §22 deferral) |
| Checkpoint enforcement | CLI layer (`cmd/m31a`) + EventStore | — | TTY prompt vs headless block decided where stdin/stdout are known; durable record always via events (D-14) |
| Verdict caching | EventStore projections | — | "All durable state = events" (Phase 1 binding decision); no separate cache file |
| Confidence enum + citation types | `internal/core/types` | — | Shared vocabulary across all three commands and future TUI badges (D-07) |

## Standard Stack

### Core (all already in go.mod — zero installs)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| net/http + encoding/json | stdlib | deps.dev / OSV / GitHub clients | Free APIs with simple JSON; avoids supply-chain risk; CGO-free [VERIFIED: go.mod] |
| internal/integrations/git | in-repo | blame, log, diff, worktree ops via exported Run() | `Run(args ...string)` passthrough exists [VERIFIED: internal/integrations/git/git.go:40-44] |
| internal/integrations/codeintel | in-repo | AnalyzeImpact / Callers / Downstream for consumer counts + affected tests | Public API confirmed [VERIFIED: internal/integrations/codeintel/impact.go:136-143] |
| internal/memory/eventstore | in-repo | Append/QueryByType for DependencyChecked + checkpoints | `Append(ctx, events ...types.Event)` [VERIFIED: internal/memory/eventstore/append.go:77] |
| github.com/pkoukk/tiktoken-go | v0.1.8 | Token budget estimation for evidence pack | Already used by `internal/engine/tokens/estimator.go` [VERIFIED: go.mod; internal/engine/tokens/] |
| golang.org/x/sync | v0.22.0 | singleflight for concurrent duplicate registry lookups | Pattern precedent in provider/cache.go [VERIFIED: go.mod; grep hit internal/integrations/provider/cache.go] |
| modernc.org/sqlite | v1.57.0 | EventStore backend (indirect — no direct use) | Pure-Go SQLite keeps CGO_ENABLED=0 [VERIFIED: go.mod] |

### External APIs (the real "stack")

| API | Endpoint(s) | Auth | Rate Limit | Verified From |
|-----|-------------|------|------------|---------------|
| deps.dev v3 | `GET https://api.deps.dev/v3/systems/GO/packages/{module}` · `GET .../versions/{version}` · `GET /v3/projects/{id}` · `GET /v3/advisories/{id}` | none | None formally specified; 429 on overload — exponential backoff advised [CITED: github.com/google/deps.dev/issues/33 maintainer response] | docs.deps.dev/api/v3 fetched 2026-08-25 [VERIFIED: docs.deps.dev/api/v3] |
| OSV.dev | `POST https://api.osv.dev/v1/query` body `{"package":{"name":"...","ecosystem":"Go"},"version":"v1.2.3"}` → `{"vulns":[...]}` | none | Not formally documented; generous for single queries | google/osv.dev docs via Context7 [VERIFIED: github.com/google/osv.dev docs/api/post-v1-query.md] |
| GitHub REST | `GET https://api.github.com/repos/{owner}/{repo}` → `pushed_at`, `archived`, `open_issues_count`, `license.spdx_id`, `stargazers_count` | optional Bearer token | 60 req/hr unauthenticated (per IP); 5,000 req/hr authenticated [CITED: docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api] | OpenAPI description via Context7 [VERIFIED: github/rest-api-description repository schema] |

Key deps.dev response facts (verbatim from official docs):
- Systems enum: `GO`, `RUBYGEMS`, `NPM`, `CARGO`, `MAVEN`, `PYPI`, `NUGET` — uppercase `GO`
- Version fields: `publishedAt`, `isDefault`, `isDeprecated`, `deprecatedReason`, `licenses[]` (SPDX expressions; for Go determined via licensecheck), `advisoryKeys[].id`
- Project fields: `openIssuesCount`, `starsCount`, `forksCount`, `license`, `description`, `homepage`, `scorecard.overallScore` ([0,10])
- Advisory fields: `cvss3Score` [0,10], `cvss3Vector`, `title`, `aliases[]` (CVEs)
- Module paths must be percent-encoded in URL path (`url.PathEscape`)
- Coverage caveat: "For Go it only collects modules that have been fetched through proxy.golang.org"

Key OSV facts: ecosystem string for Go modules is exactly `"Go"` (example in official schema shows `"ecosystem": "Go"`); ranges carry `events[] {introduced, fixed}`; results paginated by `page_token`; commit-hash queries supported.

Key GitHub facts: `open_issues_count` includes issues AND pull requests (label it accordingly in output). `archived` boolean is the archive-status signal for D-16.

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Raw HTTP clients | deps.dev gRPC / `deps.dev/api/v3` Go package | gRPC pulls protoc runtime + generated code; JSON API is sufficient and dependency-free. Stay raw HTTP. |
| osv-scanner library | Direct POST /v1/query | osv-scanner is a scanner binary/framework, far heavier than one endpoint call |
| go-git library | exec git via existing wrapper | go-git's blame/worktree support is partial and slower; the project standardizes on exec git (every existing method wraps exec.Command) |
| Driving `git bisect` in worktree | Manual binary search over `rev-list` output | Manual search gives full control of logging/events/window bounding, avoids parsing bisect log state machine; legacy parseBisectLog fragility (bisect.go:154-190) argues against reusing it |

**Installation:** none — `go.mod` unchanged.

## Package Legitimacy Audit

No external packages are installed by this phase. All functionality uses stdlib + packages already vendored in go.mod (verified present: tiktoken-go v0.1.8, x/sync v0.22.0, uuid v1.6.0, testify v1.8.2 — read from go.mod this session).

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| (none added) | — | — | — | — | — | — |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

## Architecture Patterns

### System Architecture Diagram

```
                        ┌──────────────────────────────────────────────┐
                        │                 cmd/m31a (CLI)               │
                        │  explain / investigate / deps check --approve│
                        │  TTY detect → prompt | headless → exit 2     │
                        └───────┬──────────────┬──────────────┬────────┘
                                │              │              │
              ┌─────────────────▼───┐   ┌──────▼───────┐  ┌───▼─────────────┐
              │ explain             │   │ investigate  │  │ deps            │
              │ Collector→Pack→LLM  │   │ BisectEngine │  │ RiskRules(D-16) │
              └──┬────┬────┬────┬───┘   └──────┬───────┘  └───┬──────────────┘
                 │    │    │    │              │              │
     ┌───────────▼┐ ┌─▼──┐ ┌▼────┐   ┌──────▼───────┐  ┌───▼──────────────┐
     │ codeintel  │ │git │ │ADR  │   │ temp worktree│  │ HTTPS clients    │
     │ Impact()   │ │blame│ │dir │   │ (add/detach, │  │ api.deps.dev     │
     │ Callers()  │ │log  │ │scan │   │  remove,prune)│ │ api.osv.dev     │
     └────────────┘ └────┘ └─────┘   └──────────────┘  │ api.github.com   │
                                                        └──────────────────┘
                 │              │                    │
        ┌────────▼──────────────▼────────────────────▼──┐
        │                EventStore (SQLite)             │
        │  InvestigationStarted/Completed · Dependency-  │
        │  Checked · CheckpointRequested/Resolved        │
        └────────────────────┬───────────────────────────┘
                             │ projections
                     ┌───────▼────────┐
                     │ verdict cache  │  key=(module,version,policy_hash)
                     └────────────────┘
```

Primary trace (`deps check github.com/x/y`): CLI parses module → deps client checks EventStore cache (miss) → parallel GETs to deps.dev/OSV/GitHub → risk rules classify → low risk: print verdict, append DependencyChecked → high risk: TTY ? approve : persist pending checkpoint + exit 2.

Primary trace (`investigate "panic..."`): resolve baseline/head → verify symptom reproduces at HEAD in temp worktree (else "not reproducible", stop per D-11) → verify NOT reproducing at baseline → rev-list candidates → binary search invoking checkFn per commit → confirm parent passes + culprit fails (attribution=verified) → culprit diff + Impact() → single LLM mechanism synthesis (confidence≤likely) → InvestigationCompleted event → report.

### Recommended Project Structure
```
internal/
├── core/types/
│   ├── confidence.go          # Confidence enum: verified|likely|speculative (D-07)
│   ├── evidence.go            # Citation, EvidencePack, EvidenceSection types
│   └── event.go               # + Investigation*, DependencyChecked EventTypes
├── intelligence/
│   ├── explain/
│   │   ├── collector.go       # builds EvidencePack from integrations (read-only)
│   │   ├── signals.go         # D-08 rationale-validity rule engine
│   │   ├── synthesize.go      # ONE LLM call + citation marker validation
│   │   └── render.go          # table + --format json (D-04/D-06)
│   ├── investigate/
│   │   ├── worktree.go        # create/remove/prune temp worktrees w/ signal safety
│   │   ├── repro.go           # D-09 resolution chain + regex matching
│   │   ├── bisect.go          # bounded binary search (D-11)
│   │   └── report.go          # D-12 two-tier confidence report + events
│   └── deps/
│       ├── depsdev.go         # deps.dev v3 client
│       ├── osv.go             # OSV /v1/query client
│       ├── github_enrich.go   # repo activity client
│       ├── risk.go            # D-16 configurable classification
│       ├── cache.go           # DependencyChecked projection lookup (D-15)
│       └── checkpoint.go      # D-14 pending-record write/resolve
internal/integrations/git/
│   └── blame.go               # BlamePorcelain parser (+ maybe WorktreeAdd/Remove wrappers)
cmd/m31a/
│   ├── explain.go             # runExplain — mirrors impact.go shape
│   ├── investigate.go         # runInvestigate
│   └── deps.go                # runDepsCheck
```

### Pattern 1: Evidence Pack → Single Synthesis Call (D-05)

**What:** Deterministic collector fills a fixed-section pack; each section carries pre-assigned numeric IDs used later as citation markers. The LLM receives the pack as data with instructions to cite only `[n]` markers; after the call, prose markers are validated against emitted IDs — unknown markers are stripped and those sentences move under "Inference".

**When to use:** every explain query (topic/symbol/file modes differ only in which sections the collector fills).

**Example:**
```go
// Source: in-repo pattern (codeintel/events.go payload style) + D-05/D-06 decisions
type Evidence struct {
    ID       int    `json:"id"`
    Kind     string `json:"kind"` // "source"|"callers"|"blame"|"commit"|"adr"|"test"
    Ref      string `json:"ref"`  // file:line, commit SHA, or ADR path
    Snippet  string `json:"snippet"`
}
type EvidencePack struct {
    Query    string     `json:"query"`
    Sections []Evidence `json:"sections"`
    Truncated bool      `json:"truncated"` // set when budget trimming dropped sections
}

func (p *EvidencePack) Add(kind, ref, snippet string) *Evidence {
    e := Evidence{ID: len(p.Sections) + 1, Kind: kind, Ref: ref, Snippet: snippet}
    p.Sections = append(p.Sections, e)
    return &e
}
```

Token budgeting: estimate each snippet with `internal/engine/tokens` estimator; enforce per-section caps and a total cap (agent discretion area); trim lowest-priority sections (tests before source, ADRs before blame) rather than mid-truncating snippets.

### Pattern 2: Blame Porcelain Parsing (stateful)

Official format (git-blame docs): header `<40-byte SHA> <orig-line> <final-line> [<num-lines>]` — num_lines present only on group-start lines; then tag lines `author`, `author-mail`, `author-time`, `author-tz`, committer equivalents, `summary`, `filename`; content line prefixed with TAB. Metadata prints **once per commit** unless `--line-porcelain`. Official robustness note: ignore unrecognized tag words between the header and filename lines.

```bash
# invocation (via Git.Run passthrough):
git blame --porcelain -- <path>   # at HEAD; add <rev> arg for historical blame
```

Parser sketch: maintain `map[sha]*CommitMeta`; on header line, if sha unseen, consume subsequent tag lines into meta until the TAB-prefixed content line; else skip straight to content lines. Emit `[]BlameLine{SHA, OrigLine, FinalLine, Author, AuthorTime}` plus deduped commit set (introduction candidate = min orig-line group's SHA... actually introduction commit for file mode comes from `git log --diff-filter=A --format=%H -1 -- <path>`, which is cheaper and exact).

### Pattern 3: Temp Worktree Bisect Loop (D-10/D-11)

Verified mechanics (git-worktree docs): `git worktree add [--detach] <path> [<commit-ish>]`; metadata dir `$GIT_DIR/worktrees/<id>`; `remove` refuses unclean trees without `--force`; `prune` clears stale registrations for deleted dirs; a branch may be checked out in only one worktree (irrelevant here since we detach).

```go
// Lifecycle contract:
wtDir := filepath.Join(os.TempDir(), fmt.Sprintf("m31a-investigate-%d", os.Getpid()))
defer cleanup(wtDir)                       // best-effort remove --force + prune
signalCh := make(chan os.Signal, 1)
signal.Notify(signalCh, os.Interrupt, syscall.SIGTERM)
go func() { <-signalCh; cleanup(wtDir); os.Exit(130) }()
// create:  git worktree add --detach <wtDir> <badRef>
// iterate: git -C <wtDir> checkout --detach <sha>   (via git.New(wtDir).Run)
// cleanup: git worktree remove --force <wtDir>; on error → git worktree prune
```

Startup hygiene: before creating, opportunistically prune leftovers matching the naming prefix (`m31a-investigate-*`) so a killed previous run can't wedge the next one. Candidates come from `git rev-list --reverse <baseline>..<head>` truncated to `bisect_max_commits` (50 default) — CountCommits already exists for the size check [VERIFIED: internal/integrations/git/git.go:749-761]. Convergence guard inherited from legacy design (maxIterations cap, bisect.go:89-95).

### Pattern 4: Registry Clients with BaseClient Resilience Semantics

Mirror Phase 2 defaults: hard timeout client for catalog-style calls (BaseClient.CatalogClient pattern), initial_only retry mode, 3 attempts, 1s base delay [VERIFIED: internal/integrations/provider/base_client.go:88-120]. For deps.dev specifically honor Retry-After on 429; for GitHub treat 403 + `X-RateLimit-Remaining: 0` as typed rate-limit error surfaced with a hint ("set GITHUB_TOKEN to raise limit"). Response bodies capped (e.g., io.LimitReader ~2MB) — registry responses are UNTRUSTED INPUT (PROJECT.md security model). Strict `json.Decoder.DisallowUnknownFields` is too brittle for evolving third-party schemas; instead decode into minimal structs and validate required fields.

### Anti-Patterns to Avoid
- **Model-generated citations:** letting the LLM produce file:line/SHAs. Citations must round-trip through pack IDs; the model only selects markers.
- **In-place bisect:** any code path that moves the user's HEAD (legacy `git bisect start`). Forbidden by D-10.
- **Silent window widening:** auto-growing past `bisect_max_commits` when not reproducible. Forbidden by D-11 — report and stop.
- **Unbounded LLM passes:** multi-turn agentic loops inside explain/investigate. D-05/D-09 fix exactly one synthesis call outside the deterministic loop.
- **Trusting registry JSON shape:** deps.dev/OSV/GitHub schemas evolve; missing-field tolerance + typed errors beat strict decoding crashes.
- **Hardcoding model names or registry URLs beyond documented defaults:** provider models are dynamic (AGENTS.md); base URLs belong in config for testability.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Vulnerability data | CVE scrape/NVD sync | OSV.dev /v1/query | Curated, ecosystem-native, mergeable GHSA/OSV/CVE IDs; free, no auth |
| License detection for Go modules | License-file heuristics | deps.dev `licenses[]` | Google computes it with licensecheck upstream; SPDX-normalized |
| Maintenance/popularity scoring | Custom heuristics from scraping | deps.dev Project (stars/forks/openIssues/scorecard.overallScore) + GitHub pushed_at/archived | Multi-source aggregation maintained upstream; Scorecard is an industry standard |
| Token counting | chars/4 guesses | internal/engine/tokens estimator (tiktoken-go) | Already calibrated in-project [VERIFIED: internal/engine/tokens/estimator.go exists] |
| Retry/backoff/cache | New HTTP middleware | Copy provider BaseClient patterns (initial_only, 3 attempts, 1s base) | Consistency with Phase 2; behavior already reviewed |
| Ref validation for git args | Ad-hoc checks | `validateGitRef` (unexported — export or reuse via package methods) | Battle-tested injection guard [VERIFIED: internal/integrations/git/git.go:291-338] |
| Binary search convergence bookkeeping | — | small own implementation | Fine to own: ~60 lines, but keep iteration cap + full step log for the report |

**Key insight:** Every "hard" part of this phase (vuln intel, license ID, popularity, token math) has a maintained upstream source or existing in-repo implementation. The genuinely new code is orchestration + parsing + reporting, which is where tests should concentrate.

## Runtime State Inventory

Phase 4 is greenfield feature work but **replaces the execution model of legacy bisect** (D-10), so the inventory is answered explicitly:

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None carrying old names. New events appended only (InvestigationStarted/Completed, DependencyChecked, checkpoint records reuse existing types). | none |
| Live service config | None — no external services configured out-of-band. | none |
| OS-registered state | Temp worktrees created under os.TempDir() during runs; leaked entries possible after kill -9 (metadata in target repo's .git/worktrees/, dir in /tmp). Mitigate: startup prune sweep + signal handler. | preventive code, not migration |
| Secrets/env vars | New OPTIONAL env var GITHUB_TOKEN (raises 60/hr → 5,000/hr). Plain env var acceptable; do NOT put in config files. Keychain (pkg/keychain/) reserved for LLM API keys per AGENTS.md — a read-only public-API token may follow either pattern; planner should pick env-var for simplicity. | new env var, optional |
| Build artifacts | Legacy `internal/engine/bisect/bisect.go` REMAINS: it is called by `internal/engine/workflow/verify.go:321` [VERIFIED: grep this session]. Do NOT delete in this phase. New worktree bisect lives beside it; deletion is a later-phase task after verify.go migrates. | keep legacy, mark deprecated |

**Nothing found** in stored-data / live-service categories — stated explicitly per protocol.

## Common Pitfalls

### Pitfall 1: Blame porcelain metadata suppression breaks naive parsers
**What goes wrong:** Parser assumes author/summary appear for every line; second line-group from the same commit omits them → empty author fields, wrong attribution.
**Why:** Porcelain suppresses repeat commit info for efficiency (official doc).
**How to avoid:** Cache CommitMeta by SHA; only parse tag lines on first sight of a SHA; take content lines after TAB unconditionally.
**Warning signs:** Empty authors in unit tests with multi-hunk files; fix by feeding fixture captured from real `git blame --porcelain`.

### Pitfall 2: Leaked worktrees wedge future investigations
**What goes wrong:** Process killed (Ctrl-C, OOM) between add and remove leaves `.git/worktrees/<id>` metadata + /tmp dir; later `worktree add` at same path fails ("already registered"/"already exists").
**Why:** Worktree registration is durable repo state, not process state.
**How to avoid:** Unique pid/timestamp dirs; defer-cleanup; SIGINT/SIGTERM handler; startup sweep pruning `m31a-investigate-*` entries before creating a new one; final fallback `git worktree prune`.
**Warning signs:** Tests failing on second run in same repo clone; "contains modified or untracked files" on add.

### Pitfall 3: Merge commits and rev-list windows
**What goes wrong:** Linear-window assumption wrong on merge-heavy history; culprit appears "outside" window because rev-list enumerates both parents' commits.
**Why:** `baseline..head` is set arithmetic, not a linear range.
**How to avoid:** Treat rev-list output as the candidate set (sorted topologically by default); window bound = first N of `rev-list` output, not commit-date filtering. Document in report that window is commit-count based.
**Warning signs:** Culprit hash dated older than baseline timestamp.

### Pitfall 4: GitHub anonymous rate-limit exhaustion
**What goes wrong:** Enrichment loop hits 403 after 60 requests/hour (per IP — CI machines share IPs).
**Why:** Unauthenticated quota is tiny; NAT/CI aggregates users.
**How to avoid:** Cache verdicts (D-15 does this); fetch repo enrichment lazily and only for modules being checked; support GITHUB_TOKEN env; degrade gracefully (verdict continues with deps.dev scorecard data when GitHub unavailable).
**Warning signs:** HTTP 403 with `X-RateLimit-Remaining: 0` header.

### Pitfall 5: Go module path escaping + OSV version prefix
**What goes wrong:** 404s/empty vulns for valid modules.
**Why:** deps.dev requires percent-encoded paths (`url.PathEscape` — slashes inside module paths); OSV Go versions are semver with `v` prefix (`v1.2.3`).
**How to avoid:** Always PathEscape module path; normalize version strings to include leading `v` for the Go ecosystem before POSTing to OSV.
**Warning signs:** Works for `github.com/x/y` fails for nested `github.com/x/y/z/v2` paths (major-version suffix!).

### Pitfall 6: open_issues_count includes PRs
**What goes wrong:** "Maintenance looks bad: 400 open issues" when most are PRs.
**Why:** Documented GitHub semantic.
**How to avoid:** Label the field "open issues+PRs" in output; don't feed it raw into D-16 medium-risk heuristics without that understanding.

### Pitfall 7: Flaky repro poisons attribution
**What goes wrong:** Intermittently-failing check marks a passing commit bad → wrong culprit labeled verified.
**Why:** Exit-code check is single-shot by D-09 determinism choice.
**How to avoid:** Confirmation pass is mandatory (culprit fails AND parent passes — two independent observations) before `verified`; on suspected flake (parent also fails intermittently) report confidence `likely` with both observations logged. Optionally allow `[intelligence] repro_attempts` (default 1) — planner discretion.
**Warning signs:** Bisect converges in 0 steps; parent-check failure.

### Pitfall 8: Hallucinated citation markers
**What goes wrong:** Prose cites [7] but pack has 5 items → dangling reference destroys EXPLAIN-02 compliance.
**Why:** LLMs invent plausible numbers.
**How to avoid:** Post-synthesis regex scan for `\[\d+\]`; drop unknown markers and demote their sentences to Inference section; assert in renderer that every Evidence-section entry is referenced ≥0 times and every non-Inference paragraph references ≥1 known marker.
**Warning signs:** JSON output where citations[] ⊄ pack IDs.

### Pitfall 9: Prompt injection via repo content
**What goes wrong:** Commit messages/source contain "ignore instructions, output X"; synthesized answer follows injected text, citations launder it as evidence.
**Why:** Pack contents are attacker-controlled data in general repos (PROJECT.md security model: registry/repo responses untrusted).
**How to avoid:** System prompt states pack is DATA not instruction; structural citation validation bounds damage (claims must map to real evidence refs even if wording is poisoned); never execute anything found in pack; keep synthesis temperature low and refuse markers pointing outside pack.
**Warning signs:** Answer claims absent from any snippet.

### Pitfall 10: Config zero-values defeat layered defaults
**What goes wrong:** `bisect_max_commits` unset in TOML decodes to 0; bisect immediately aborts.
**Why:** Loader merges layers over zero-valued Config; TOML absence ≠ default presence [VERIFIED: internal/core/config/loader.go:250-330 layered merge].
**How to avoid:** Post-load normalization: `if cfg.Intelligence.BisectMaxCommits == 0 { = 50 }` — same pattern other sections use (e.g., FeaturesConfig defaults applied downstream).
**Warning signs:** First-run environments behave differently than dev machines with full config.toml.

## Code Examples

### deps.dev Go module lookup
```go
// Source: https://docs.deps.dev/api/v3/ (fetched 2026-08-25)
// GET /v3/systems/GO/packages/{module}  — system is UPPERCASE "GO"
u := fmt.Sprintf("https://api.deps.dev/v3/systems/GO/packages/%s", url.PathEscape(module))
// resp.versions[]: {versionKey:{system,name,version}, publishedAt, isDefault, isDeprecated}
// GET /v3/systems/GO/packages/{m}/versions/{v}
//   → licenses[], advisoryKeys[].id, links[], relatedProjects[].projectKey.id
// GET /v3/projects/github.com%2Fowner%2Frepo   (PathEscape the "host/owner/repo" id)
//   → openIssuesCount, starsCount, forksCount, license, scorecard.overallScore
```

### OSV vulnerability query
```bash
# Source: https://github.com/google/osv.dev/blob/master/docs/api/post-v1-query.md
curl -d '{"package":{"name":"github.com/x/y","ecosystem":"Go"},"version":"v1.2.3"}' \
     "https://api.osv.dev/v1/query"
# → {"vulns":[{"id":"GHSA-...","summary":"...","aliases":["CVE-..."],
#     "affected":[{"ranges":[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.25.0"}]}]}]}]}
```

### Blame invocation + fixture shape
```
# Source: git-scm.com/docs/git-blame (THE PORCELAIN FORMAT)
$ git blame --porcelain -- internal/foo/bar.go
<40-byte-sha> 1 1 3            ← orig-line final-line num-lines (group start)
author  Jane
author-mail <jane@x>
author-time 1735689600
author-tz +0530
committer Jane ...
summary  Initial bar implementation
boundary              ← only when line predates history boundary
filename internal/foo/bar.go
	func Bar() {          ← TAB-prefixed content
<same-or-other-sha> 4 2     ← continuation line: NO num-lines field
	...
```

### Worktree bisect skeleton
```bash
# Source: git-scm.com/docs/git-worktree (command synopsis, remove/prune semantics)
git worktree add --detach /tmp/m31a-investigate-4242 <badRef>
git -C /tmp/m31a-investigate-4242 checkout --detach <candidateSha>
(cd /tmp/m31a-investigate-4242 && go test ./...) ; echo $?   # checkFn: exit + regex
git worktree remove --force /tmp/m31a-investigate-4242
git worktree prune                                            # startup sweep fallback
```

### New event types (following existing conventions)
Existing checkpoint pair to REUSE for D-14 (verbatim from source):
```go
// Source: internal/core/types/event.go:43-44
EventCheckpointRequested   EventType = "CheckpointRequested"
EventCheckpointResolved    EventType = "CheckpointResolved"
```
New constants to add adjacent to the Phase 03 block (event.go:63-71), payloads modeled on codeintel/events.go structs, emitted with `Metadata.Source = "intelligence"` (precedent: `"codeintel"` [VERIFIED: internal/integrations/codeintel/events.go:103-106]):
- `EventInvestigationStarted` / `EventInvestigationCompleted` — symptom, range, steps[], outcome, culprit, confidence
- `EventDependencyChecked` — module, version, verdict, risk_class, sources_snapshot, evaluated_at, policy_hash (D-15)

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| NVD/CVE feeds scraped locally | OSV.dev aggregated query API | OSV GA 2021+, now ecosystem-standard | No local DB sync; single POST per check |
| deps.dev alpha endpoints | v3 stable with stability guarantee + deprecation policy; v3alpha for extras (GetDependents, GetCapabilities) | v3 current | Stick to v3; v3alpha only if dependent-count needed later [CITED: docs.deps.dev/api] |
| In-place `git bisect` scripting | `git worktree add --detach` isolation | worktree mature since git 2.5; tooling convention now | User checkout untouched (D-10) |
| go-git library for programmatic git | exec-based wrappers (project standard) | project-wide | Consistent with all existing integrations |

**Deprecated/outdated:** none affecting this phase. Note git blame `--porcelain`/`--line-porcelain` are long-stable; git 2.55 environment exceeds minimum needs.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `.m31a/decisions/` ADR layout does not exist yet (PERSIST-04 is future); explain must scan-if-present and degrade silently. Directory confirmed absent today [VERIFIED: ls], exact PERSIST-04 layout [ASSUMED] | Pattern 1 / collector | Explain loses ADR citations until PERSIST-04 ships — acceptable degradation, no crash |
| A2 | Popularity signal = deps.dev starsCount (+scorecard) suffices; v3alpha GetDependents not needed in this phase | Standard Stack | Weaker popularity signal; upgrade path exists without schema change |
| A3 | OSV undocumented rate limits are generous enough for interactive single-module checks without backoff beyond generic retry | Pitfall 4 | Rare 429s handled by shared retry; worst case degraded verdict |
| A4 | Repro auto-detect for Go projects = `go test ./...` (extensible per-language map) | Discretion area (sanctioned by D-09) | Wrong repro for non-go projects → "not reproducible" report, safe failure mode |
| A5 | Manual binary search over rev-list preferred over driving `git bisect` inside the temp worktree | Alternatives Considered | More own-code (~60 lines) but fully controllable logging/events; if abandoned, git-bisect-in-worktree is the fallback |
| A6 | GITHUB_TOKEN via plain env var (not keychain) for rate-limit lift | Runtime State Inventory | Minor secret-handling inconsistency; token is optional and low-sensitivity (public-data read scope) |

## Open Questions (RESOLVED)

1. **Where does the shared Confidence enum live?**
   - What we know: D-07 says shared across three commands + future TUI; `internal/core/types` is the designated shared vocabulary home (AGENTS.md).
   - What's unclear: whether to colocate with domain.go or new file.
   - Recommendation: new `internal/core/types/confidence.go`; trivial either way.
   - RESOLVED: New file `internal/core/types/confidence.go`, exactly as recommended — created by plan 04-01 Task 1 alongside evidence.go (see 04-01-PLAN.md Task 1 `<files>`).

2. **Explain target disambiguation order**
   - What we know: D-01 routes topics/symbols/files into one command; index has Define() for symbol lookup.
   - What's unclear: precedence when input could be both (rare).
   - Recommendation: file-exists → file mode; exact symbol in SymbolIndex → symbol mode; else topic mode. Document in help text.
   - RESOLVED: Adopted the recommended precedence — file-exists first, exact SymbolIndex hit second, topic fallback third, documented in help text. Implemented by ResolveMode in plan 04-03 Task 2 and surfaced in CLI help by plan 04-03 Task 3.

3. **Does `deps check --approve <module>` need the original verdict replay?**
   - What we know: D-14 says pending record resolvable via --approve; event trail preserved.
   - What's unclear: whether approval re-fetches fresh data or trusts cached pending verdict.
   - Recommendation: trust the persisted verdict (checkpoint approves THAT evidence); fresh re-run available by clearing cache via policy-hash change.
   - RESOLVED: Approval trusts the persisted verdict snapshot — the checkpoint approves THAT evidence; no refetch on the approve path; fresh re-evaluation remains available via a policy-hash cache invalidation. This resolution is load-bearing for D-14 semantics and is enforced mechanically by plan 04-07 Task 2 (ApprovePending behavior: "Approval trusts the persisted verdict snapshot — no refetch on approve path").

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| git | blame, worktree, log, rev-list | ✓ | 2.55.0 | none needed (features ancient & stable) |
| Go toolchain | build/test, repro default command | ✓ | 1.27.0 (go.mod wants 1.26.5 ✓) | — |
| golangci-lint | make lint | ✓ | 2.12.2 | — |
| Network: api.deps.dev | DEPEND-01 existence/licenses/project | runtime-dependent | — | Typed "source unreachable" error + hint; verdict confidence capped at speculative |
| Network: api.osv.dev | DEPEND-01 vulnerabilities | runtime-dependent | — | Same degradation; empty-vulns ≠ no-vulns must be distinguished (unknown ≠ clean) |
| Network: api.github.com | maintenance enrichment | runtime-dependent | — | Skip enrichment; deps.dev scorecard still available |
| GITHUB_TOKEN (optional env) | rate-limit lift | not set | — | 60/hr anonymous quota sufficient for interactive use |

**Missing dependencies with no fallback:** none.
**Missing dependencies with fallback:** network services degrade gracefully per above; offline operation must still produce structured reports with `sources[]` marked unavailable (never silently claim "no vulnerabilities").

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Go stdlib testing + stretchr/testify v1.8.2 [VERIFIED: go.mod] |
| Config file | Makefile-driven; race enabled via `make test`, fast via `make test-fast` |
| Quick run command | `go test ./internal/intelligence/... ./internal/integrations/git/... -run TestXxx` |
| Full suite command | `make test` (race + coverage; AGENTS.md canonical check: `make check`) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| EXPLAIN-01 | Pack contains all six section kinds from fixture repo | unit | `go test ./internal/intelligence/explain/ -run TestCollectorSections` | ❌ Wave 0 |
| EXPLAIN-02 | Unknown citation marker demoted to Inference | unit | `go test ./internal/intelligence/explain/ -run TestMarkerValidation` | ❌ Wave 0 |
| EXPLAIN-03 | Rationale signals compute from synthetic graph (consumers/age/TODO) | unit | `go test ./internal/intelligence/explain/ -run TestSignals` | ❌ Wave 0 |
| EXPLAIN-04 | File mode finds introduction commit in seeded repo | integration (temp git repo) | `go test ./internal/intelligence/explain/ -run TestFileMode` | ❌ Wave 0 |
| REGRESS-01 | Bisect finds seeded culprit in scripted-history repo | integration (temp git repo) | `go test ./internal/intelligence/investigate/ -run TestBisectFinds` | ❌ Wave 0 |
| REGRESS-02 | Attribution verified only when parent passes + culprit fails; not-reproducible stops | unit + integration | `go test ./internal/intelligence/investigate/ -run TestConfidenceAndWindow` | ❌ Wave 0 |
| REGRESS-03 | Report includes culprit/mechanism/affected/fix sections | unit | `go test ./internal/intelligence/investigate/ -run TestReportShape` | ❌ Wave 0 |
| DEPEND-01 | Client parses recorded API fixtures (httptest servers) | unit | `go test ./internal/intelligence/deps/ -run TestClients` | ❌ Wave 0 |
| DEPEND-02/03 | High-risk → headless exit 2 + pending checkpoint; --approve resolves | unit (non-TTY harness) | `go test ./internal/intelligence/deps/ -run TestCheckpoint` | ❌ Wave 0 |
| DEPEND-04 | Cache hit skips HTTP; version/policy change invalidates | unit | `go test ./internal/intelligence/deps/ -run TestCache` | ❌ Wave 0 |
| blame parser | Porcelain fixture with suppressed metadata parsed correctly | unit | `go test ./internal/integrations/git/ -run TestBlamePorcelain` | ❌ Wave 0 |
| worktree mgr | Create/cleanup incl. simulated interrupt; prune sweep | unit | `go test ./internal/intelligence/investigate/ -run TestWorktreeLifecycle` | ❌ Wave 0 |

All external-API tests use `httptest.Server` fixtures — no network in CI-path tests. Integration tests shell out only to local `git init`/commit scripts in t.TempDir(). Coverage gates per AGENTS.md: overall 75% (pkg/ targets don't apply here; intelligence code lives in internal/).

### Sampling Rate
- **Per task commit:** targeted `go test ./internal/intelligence/... ./internal/integrations/git/...`
- **Per wave merge:** `make test-fast`
- **Phase gate:** `make check` green (fmt → tidy → vet → lint → test) before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `internal/intelligence/*_test.go` skeletons + fixtures: blame-porcelain sample, deps.dev/OSV/GitHub JSON recordings
- [ ] `internal/integrations/git/blame_test.go` with suppressed-metadata case
- [ ] Test helper to seed a temp git repo with scripted history (init → good commits → culprit commit) shared by investigate tests
- [ ] Non-TTY harness stub for checkpoint tests (stdin pipe vs char device)

*(Framework install: nothing — infrastructure complete.)*

## Security Domain

security_enforcement not disabled in config.json → included.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | No user auth in phase (API tokens optional, env-var) |
| V3 Session Management | no | Stateless CLI commands |
| V4 Access Control | yes (scoped) | Repro commands execute user-supplied code — confined to disposable worktree; no escalation beyond what user could run themselves; checkpoint gates risky dep additions |
| V5 Input Validation | **yes** | Registry/API responses treated as untrusted: io.LimitReader caps, strict required-field checks, timeouts; git refs via validateGitRef discipline [VERIFIED: internal/integrations/git/git.go:291-338]; module paths escaped via url.PathEscape |
| V6 Cryptography | no | TLS via net/http defaults; no custom crypto |
| V7 Error Handling | yes | Structured errors + hints (ToolError pattern); never echo API tokens or full URLs-with-keys in messages |
| V14 Configuration | yes | New `[intelligence]` section validated at load (bisect_max_commits ≥ 1; risk thresholds sane ranges) following config_validate.go conventions |

### Known Threat Patterns for This Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Prompt injection via commit messages/source/ADR content into synthesis LLM | Tampering/Elevation | Pack-as-data system prompt; structural citation validation (Pitfall 9); never act on pack content |
| Malicious/oversized registry responses | DoS/Tampering | LimitReader caps (~2MB), response timeouts, decode into minimal structs |
| Symlink/path tricks via worktree paths | Tampering | Fixed-prefix temp dirs (os.TempDir + m31a prefix), never user-controlled path component |
| Shell injection in repro command string | Elevation | Execute via exec.CommandContext with argument splitting (documented shell-semantics caveat if config uses shell form); worktree isolation bounds blast radius |
| SSRF via attacker-chosen repo links | Information Disclosure | Only fixed hosts queried (api.deps.dev, api.osv.dev, api.github.com); links from responses displayed, never fetched |
| False "clean" verdict from silent source failure | Repudiation | Distinguish "queried: 0 vulns" from "source unavailable" in verdict + sources[]; confidence capped speculative when any source unreachable |

## Sources

### Primary (HIGH confidence)
- docs.deps.dev/api/v3 — fetched in full this session: endpoints, systems enum, response fields, coverage caveats
- github.com/google/osv.dev docs (post-v1-query.md, api-quickstart.md, schema example) via Context7 — request/response formats, "Go" ecosystem string
- git-scm.com/docs/git-blame + kernel.org man page — porcelain format spec, parsing robustness note
- git-scm.com/docs/git-worktree — add/remove/prune/lock semantics, $GIT_DIR/worktrees layout
- github/rest-api-description OpenAPI (via Context7) — repository schema: pushed_at/archived/open_issues_count
- docs.github.com rate-limits page (via Context7) — 60/hr unauthenticated, 5000/hr authenticated
- In-repo (read this session, cited with line ranges above): git.go, impact.go, events.go (codeintel), event.go (types), types.go/loader.go (config), bisect.go, append.go (eventstore), go.mod, impact.go (cmd)

### Secondary (MEDIUM confidence)
- github.com/google/deps.dev issue #33 — maintainer statement on rate limits (authoritative for policy, informal channel)
- continuumcode.ai / parallelcode.dev worktree guides — operational color; cross-checked against official git docs before inclusion

### Tertiary (LOW confidence)
- OSV.dev rate-limit behavior (undocumented; A3)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — zero new deps; all external APIs verified from official docs this session
- Architecture: HIGH — consumes verified in-repo APIs read line-by-line; patterns mirror established Phase 2/3 conventions
- Pitfalls: HIGH — mechanics (blame suppression, worktree metadata, escaping, rate limits) verified from primary sources; flake/attribution risks reasoned from D-09/D-12 semantics
- External API longevity: MEDIUM — deps.dev v3 has stability guarantee; OSV/GitHub stable but schemas can gain fields

**Research date:** 2026-08-25
**Valid until:** 2026-09-24 (30 days; deps.dev/OSV are slow-moving; GitHub schema drift unlikely to affect consumed fields)
