# Phase 4: Intelligence Features - Pattern Map

**Mapped:** 2026-08-25
**Files analyzed:** 21 new + 3 modified
**Analogs found:** 24 / 24 (all files have in-repo analogs; 3 are role-match only)

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `internal/core/types/confidence.go` | model (type vocabulary) | transform | `internal/core/types/event.go` EventType constants + `codeintel/impact.go` RiskCategory | exact |
| `internal/core/types/evidence.go` | model (Citation/EvidencePack) | transform | `internal/integrations/codeintel/impact.go` ImpactResult structs | exact |
| `internal/core/types/event.go` (modify) | model (event vocabulary) | pub-sub | self — extend Phase 03 block (lines 63-71) | exact |
| `internal/intelligence/explain/collector.go` | service | transform (read-only orchestration) | `codeintel/impact.go` AnalyzeImpact (lines 133-235) | exact |
| `internal/intelligence/explain/signals.go` | service (rule engine) | transform | `codeintel/impact.go` CategorizeSymbolRisk (lines 42-71) | exact |
| `internal/intelligence/explain/synthesize.go` | service | request-response (LLM) | `provider/interface.go` LLMProvider.ChatCompletion | exact |
| `internal/intelligence/explain/render.go` | utility (renderer) | transform | `cmd/m31a/impact.go` outputImpactTable/outputImpactJSON | exact |
| `internal/intelligence/investigate/worktree.go` | infrastructure | file-I/O + process | `integrations/git/git.go` Run wrappers + `engine/bisect/bisect.go` deferred-cleanup contract | exact |
| `internal/intelligence/investigate/repro.go` | utility | process execution | `engine/bisect/bisect.go` checkFn contract (lines 57-61, 116-125) | exact |
| `internal/intelligence/investigate/bisect.go` | service (search engine) | batch over commits | `engine/bisect/bisect.go` Run loop + GitRunner interface | exact |
| `internal/intelligence/investigate/report.go` | service + model | transform + event append | `codeintel/events.go` emitEvent (lines 88-110) | exact |
| `internal/intelligence/deps/depsdev.go` | HTTP client | request-response | `provider/base_client.go` CatalogClient + RetryStream | role-match |
| `internal/intelligence/deps/osv.go` | HTTP client | request-response | `provider/base_client.go` + `tools/search/webfetch.go` client construction | role-match |
| `internal/intelligence/deps/github_enrich.go` | HTTP client | request-response | `provider/base_client.go` HandleChatHTTPError rate-limit mapping | role-match |
| `internal/intelligence/deps/risk.go` | service (config-driven rules) | transform | `internal/integrations/archcheck.DetectViolations` (consumed at `cmd/m31a/arch.go:42`) | exact |
| `internal/intelligence/deps/cache.go` | store/projection | CRUD via events | `memory/eventstore/projection.go` Projection iface + `query.go` type filter | exact |
| `internal/intelligence/deps/checkpoint.go` | service | event-driven persistence | `codeintel/events.go` emitEvent + existing CheckpointRequested/Resolved pair | exact |
| `internal/integrations/git/blame.go` | integration method | streaming text parse | `git/git.go` StatusPorcelain parser (lines 441-563) + parseLog (239-286) | exact |
| `cmd/m31a/explain.go` | CLI controller | request-response | `cmd/m31a/impact.go` runImpact (lines 14-62) | exact |
| `cmd/m31a/investigate.go` | CLI controller | request-response | `cmd/m31a/impact.go` runImpact | exact |
| `cmd/m31a/deps.go` | CLI controller (nested subcommand + TTY gate) | request-response | `cmd/m31a/arch.go` runArchCheck ("arch check" nested form) | exact |
| `cmd/m31a/main.go` (modify) | route/dispatch | request-response | self — subcommand block lines 806-829 | exact |
| `internal/core/config/types.go` (modify) | config schema | transform | self — VerifyConfig (lines 192-198) small-section template | exact |
| `internal/core/config/loader.go` + `merge.go` (modify) | config loading | layered merge | self — Load() layers (250-334); mergeHelper registration (`merge.go:97-114`) | exact |

---

## Pattern Assignments

### `internal/core/types/confidence.go` + `evidence.go` (model, transform)

**Analog:** `internal/core/types/event.go` (string-enum vocabulary home) and `internal/integrations/codeintel/impact.go` (typed result structs with doc comments on every field).

**String enum pattern** (event.go lines 21-23, 43-44):
```go
type EventType string

const (
    EventCheckpointRequested   EventType = "CheckpointRequested"
    EventCheckpointResolved    EventType = "CheckpointResolved"
```

D-07 Confidence follows identically: `type Confidence string` with constants `ConfidenceVerified Confidence = "verified"`, `ConfidenceLikely = "likely"`, `ConfidenceSpeculative = "speculative"`.

**Struct-with-documented-fields pattern** (impact.go lines 24-39):
```go
// CallerInfo represents a direct caller of a symbol with location and risk category.
type CallerInfo struct {
    File     string       // relative file path where the call occurs
    Line     int          // line number of the call
    Symbol   string       // name of the calling symbol
    Category RiskCategory // risk category of the caller
}
```
Copy this field-comment discipline for EvidencePack/Citation structs (AGENTS.md requires doc comments on exported types). Note impact.go uses no json tags (CLI renders directly); evidence.go SHOULD add json tags per D-04 (`--format json` emits citations[]) — use the tag style from `codeintel/events.go` lines 13-20 (snake_case: `json:"mod_time"`).

---

### `internal/core/types/event.go` modification (model, pub-sub)

**Analog:** self — Phase 03 block (lines 62-72):

```go
    // Code Intelligence events (Phase 03)
    EventFileIndexed           EventType = "FileIndexed"
    ...
    EventSymbolRemoved         EventType = "SymbolRemoved"
)
```

Add a Phase 04 block adjacent, same alignment style:

```go
    // Intelligence events (Phase 04)
    EventInvestigationStarted   EventType = "InvestigationStarted"
    EventInvestigationCompleted EventType = "InvestigationCompleted"
    EventDependencyChecked      EventType = "DependencyChecked"
```

Reuse (never duplicate) the existing checkpoint pair for D-14 pending records: `EventCheckpointRequested` / `EventCheckpointResolved` (event.go lines 43-44).

---

### `internal/integrations/git/blame.go` (integration method, streaming text parse)

**Analog:** `internal/integrations/git/git.go`. Three precedents to copy:

**1. Command execution via unexported run + typed error wrapping** (git.go lines 30-44):
```go
func (g *Git) run(args ...string) (string, error) {
    cmd := exec.Command("git", args...)
    cmd.Dir = g.workDir
    out, err := cmd.CombinedOutput()
    if err != nil {
        return string(out), fmt.Errorf("git %s: %w: %s", strings.Join(args, " "), err, string(out))
    }
    return string(out), nil
}
```
Blame becomes: `out, err := g.run("blame", "--porcelain", "--", path)` (path must be repo-relative; optional rev arg validated by validateGitRef before appending).

**2. Porcelain parser with per-record state** — StatusPorcelain (git.go lines 441-563) is the direct template for stateful line-oriented parsing: split output on "\n", skip short/malformed lines, build structured records, tolerate unrecognized fields. For blame porcelain's suppressed-metadata behavior (metadata printed once per commit), maintain `map[sha]*CommitMeta` keyed on the 40-byte header SHA (RESEARCH Pitfall 1).

**3. Ref validation before interpolation** (git.go lines 288-338) — mandatory for any ref-taking command:
```go
func validateGitRef(ref string) bool {
    if strings.HasPrefix(ref, "--") { return false }
    if strings.ContainsAny(ref, ";|&$`<>(){}[]!#~") { return false }
    if strings.Contains(ref, "..") { return false }
    // ... full body at git.go:291-338
}
```
Currently **unexported** — blame.go lives in the same package and calls it directly; the investigate package cannot, so export a wrapper (e.g., `ValidateRef`) or route all ref-taking calls through package methods that validate internally (DiffRefs precedent, lines 340-360).

**4. Existing helpers to reuse, not rebuild:** `RevParse` (601-608), `CountCommits` (749-761), `DiffRefs` (340-360), `HeadHash` (574-581), `Log/LogAll/LogSince` (191-222, format `%H§%h§%an§%s§%aI` — rare-char separator convention worth imitating for any new multi-field log parsing).

---

### `internal/intelligence/explain/collector.go` (service, transform)

**Analog:** `internal/integrations/codeintel/impact.go` AnalyzeImpact (lines 133-157):

```go
func AnalyzeImpact(graph *CodeGraph, index *SymbolIndex, symbol string, depth int) *ImpactResult {
    result := &ImpactResult{
        Symbol:        symbol,
        DirectCallers: []CallerInfo{},
        Indirect:      []string{},
        AffectedTests: []string{},
        Risk:          CategorizeSymbolRisk(index, symbol),
    }

    // 1. Find direct callers using the call graph
    callEdges := graph.Callers(symbol)
    ...
```

Pattern to copy: one exported orchestration function taking already-built dependencies (graph, index — never constructs them), initializing all result slices non-nil, numbered stage comments, returning a fully populated struct. Collector mirrors this: takes graph/index/git client/ADR dir, fills an EvidencePack where each section gets a pre-assigned numeric ID via `pack.Add(kind, ref, snippet)` (RESEARCH.md Pattern 1 sketch). Mode selection (topic/symbol/file per D-01): file-exists → file mode; exact symbol in index.Define → symbol mode; else topic mode.

Token budgeting uses the existing estimator (estimator.go lines 96-101, 289):
```go
est := tokens.NewEstimator(modelID)
n := est.Estimate(text)
```
Trim whole lowest-priority sections rather than mid-truncating snippets; set `Truncated` flag when budget dropped anything.

---

### `internal/intelligence/explain/signals.go` (rule engine, transform)

**Analog:** `codeintel/impact.go` CategorizeSymbolRisk (lines 42-71):

```go
func CategorizeSymbolRisk(index *SymbolIndex, symbol string) RiskCategory {
    locs := index.Define(symbol)
    if len(locs) == 0 {
        return RiskRuntime // unknown symbol defaults to runtime
    }
    for _, loc := range locs { ... }
    return RiskRuntime
}
```

Pattern: pure function over deterministic inputs, early-return default verdict, no I/O inside. D-08 signal computation copies this shape: consumer count from `graph.Callers`, last-touch age from blame max author-time, TODO/FIXME scan, test presence from `AnalyzeImpact(...).AffectedTests`. signals.go must stay LLM-free and side-effect-free — the LLM only narrates computed values.

---

### `internal/intelligence/explain/synthesize.go` (service, request-response LLM)

**Analog:** `internal/integrations/provider/interface.go` (lines 15-26):

```go
type LLMProvider interface {
    Name() string
    ...
    ChatCompletion(ctx context.Context, req ChatRequest) (*types.ChatResponse, error)
    ChatCompletionStream(ctx context.Context, req ChatRequest) (*types.StreamIterator, error)
    ...
}
```

Callers accept interfaces, never concrete clients. The minimal-interface idiom is proven in bisect.go lines 18-22:
```go
type GitRunner interface {
    Run(args ...string) (string, error)
}
```
Define a narrow `Synthesizer` interface (just ChatCompletion) in explain so tests inject mocks without the full provider registry.

Contract per D-05 + RESEARCH Pitfalls 8/9:
- Exactly ONE ChatCompletion call per query; no agentic loops.
- System prompt states the evidence pack is DATA, not instructions.
- After response, regex-scan prose for `\[\d+\]`; markers not present in pack IDs are stripped and their sentences demoted under "Inference".
- Dependency direction: intelligence imports provider interface/types only — never talks HTTP to model vendors directly (Pitfall 10 in PITFALLS.md).

---

### `internal/intelligence/explain/render.go` (utility, transform)

**Analog:** `cmd/m31a/impact.go` renderers (lines 52-61, 65-110) — but placed in-package so CLI and future TUI both reuse it:

Format switch (lines 52-61):
```go
switch *format {
case "json":
    return outputImpactJSON(result)
case "graphviz":
    return outputImpactGraphviz(result)
case "table":
    fallthrough
default:
    return outputImpactTable(result)
}
```

Table style (lines 65-77):
```go
fmt.Printf("Impact Analysis for: %s\n", result.Symbol)
fmt.Println("Direct Callers:")
fmt.Println("  FILE:LINE        SYMBOL           CATEGORY")
fmt.Printf("  %-20s %-15s %s\n", fmt.Sprintf("%s:%d", caller.File, caller.Line), caller.Symbol, caller.Category)
```

JSON style (lines 102-110):
```go
encoder := json.NewEncoder(os.Stdout)
encoder.SetIndent("", "  ")
if err := encoder.Encode(result); err != nil { ...; return 1 }
return 0
```

D-06 rendering contract: prose paragraphs carry `[n]` markers; an Evidence section lists each marker with its full citation (file:line, SHA, ADR path); unbacked statements go under an explicit "Inference" heading.

---

### `internal/intelligence/investigate/worktree.go` + `repro.go` + `bisect.go` (engine)

**Primary analog:** `internal/engine/bisect/bisect.go` — reuse contracts, replace mechanics (D-10). Legacy file REMAINS untouched (verify.go still calls it).

**Interface injection for testability** (bisect.go lines 17-29):
```go
type GitRunner interface {
    Run(args ...string) (string, error)
}

type Bisect struct {
    workDir string
    logger  *slog.Logger
    git     GitRunner
}
```
The worktree manager should accept git behind an equivalent narrow interface so tests stub it without exec.

**checkFn callback contract to preserve** (bisect.go lines 57-61, 116-125):
```go
// Run performs a git bisect between sessionStartHash (good) and headHash (bad)
// using checkFn to determine pass/fail at each step.
// checkFn returns true if the commit passes verification, false if it fails.
...
if checkFn() { /* good */ } else { /* bad */ }
```
repro.go implements checkFn as shell-command exit code + optional output regex (D-09), executed with `exec.CommandContext` with cmd.Dir set to the worktree. Resolution chain: `--repro` flag > `[intelligence] repro_command` config > per-language auto-detect (`go test ./...` for Go).

**Iteration cap guard** (bisect.go lines 89-95) — carry into the manual binary search:
```go
const maxIterations = 50
iteration := 0
for {
    iteration++
    if iteration > maxIterations {
        return nil, fmt.Errorf("bisect did not converge after %d iterations", maxIterations)
    }
```

**Guaranteed-cleanup defer shape** (bisect.go lines 66-73) — worktree.go replaces `bisect reset` with remove --force + prune, plus signal handler (RESEARCH.md Pattern 3):
```go
defer func() { ... }()                       // runs on every return path
wtDir := filepath.Join(os.TempDir(), fmt.Sprintf("m31a-investigate-%d", os.Getpid()))
signalCh := make(chan os.Signal, 1)
signal.Notify(signalCh, os.Interrupt, syscall.SIGTERM)
```
Startup hygiene: opportunistically prune stale `m31a-investigate-*` registrations before creating a new worktree (RESEARCH Pitfall 2).

**Candidate enumeration:** window bound = first N entries of `git rev-list --reverse <baseline>..<head>` truncated at `bisect_max_commits` (D-11); pre-check size with existing `CountCommits` (git.go 749-761). If symptom does not reproduce at window start → report "not reproducible in window" and STOP; never widen silently. Checkout candidates via `git.New(wtDir).Run("checkout", "--detach", sha)` after ref validation. Attribution = verified ONLY when culprit fails AND parent passes (two independent observations, D-12 / Pitfall 7).

---

### `internal/intelligence/investigate/report.go` + `deps/checkpoint.go` (events)

**Analog:** `internal/integrations/codeintel/events.go` — complete emission pattern (lines 88-110):

```go
func emitEvent(ctx context.Context, store types.EventStore, eventType types.EventType, payload interface{}) error {
    if store == nil {
        return nil   // nil-safe: callers without a store skip persistence
    }
    data, err := json.Marshal(payload)
    if err != nil {
        return err
    }
    evt := types.Event{
        ID:        uuid.New(),
        Type:      eventType,
        Timestamp: time.Now(),
        Payload:   data,
        Metadata: types.EventMetadata{
            SchemaVersion: 1,
            Source:        "codeintel",
        },
    }
    return store.Append(ctx, evt)
}
```

Copy verbatim with `Source: "intelligence"` plus thin typed wrappers (`EmitInvestigationStarted(...)`, etc., mirroring lines 112-150). Payload structs follow the flat snake_case-tagged style of events.go lines 12-37.

Checkpoint records for D-14 REUSE `EventCheckpointRequested` / `EventCheckpointResolved` (event.go lines 43-44) — payload `{module, version, verdict, risk_class, sources_snapshot}`; `--approve <module>` appends CheckpointResolved referencing the original event ID. EventStore.Append signature (append.go line 77): `Append(ctx context.Context, events ...types.Event) error`.

Report structure per D-12: Symptom → Range & steps → Culprit (attribution=verified after confirmation run) → Mechanism hypothesis (confidence ≤ likely, from single LLM pass over culprit diff + graph Impact()) → Affected components (AnalyzeImpact output) → Fix direction.

---

### `internal/intelligence/deps/depsdev.go` / `osv.go` / `github_enrich.go` (HTTP clients)

**Primary analog:** `internal/integrations/provider/base_client.go`. No external-registry JSON clients exist yet (role-match); mirror Phase 2 resilience semantics exactly:

**Shared transport + hard-timeout catalog client** (base_client.go lines 27-39, 106-115):
```go
sharedTransport = &http.Transport{
    DialContext:           (&net.Dialer{Timeout: types.HTTPDialTimeout}).DialContext,
    TLSHandshakeTimeout:   10 * time.Second,
    ResponseHeaderTimeout: 10 * time.Second,
    MaxIdleConns:          100,
    MaxIdleConnsPerHost:   10,
    IdleConnTimeout:       90 * time.Second,
}
...
CatalogClient: &http.Client{
    Transport: transport,
    Timeout:   types.FetchModelsTimeout,   // hard wall-clock timeout
},
```

**Retry defaults** (base_client.go lines 88-97): mode `initial_only`, MaxAttempts 3, BaseDelay 1s. Backoff loop with ctx cancellation (lines 453-459):
```go
delay := b.RetryConfig.BaseDelay * time.Duration(1<<uint(attempt))
select {
case <-ctx.Done():
    return nil, ctx.Err()
case <-time.After(delay):
    continue
}
```
Honor `Retry-After` header on deps.dev 429s (pattern: base_client.go lines 222-227).

**Rate-limit typed error** (base_client.go lines 222-227) — GitHub 403 + `X-RateLimit-Remaining: 0` maps like:
```go
case http.StatusTooManyRequests:
    if retryAfter != "" {
        return fmt.Errorf("%w (retry-after: %s)", m31errors.ErrRateLimited, retryAfter)
    }
    return m31errors.ErrRateLimited
```
Surface with a hint ("set GITHUB_TOKEN to raise limit"); degrade gracefully — verdict continues with deps.dev scorecard data when GitHub unavailable.

**Untrusted-body posture** (base_client.go line 196; webfetch.go lines 60-70; websearch.go:211):
```go
_, _ = ReadBodyLimited(resp, types.MaxLLMResponseBytes)
body, err := io.ReadAll(io.LimitReader(resp.Body, types.MaxFileSize+1))
CheckRedirect: func(req *http.Request, via []*http.Request) error {
    if len(via) >= MaxRedirects { return http.ErrUseLastResponse }
    return nil
},
```
Cap responses ~2MB via `io.LimitReader`; decode into minimal structs; validate required fields; NO DisallowUnknownFields (schemas evolve). Only fixed hosts queried (api.deps.dev, api.osv.dev, api.github.com) — links from responses displayed, never fetched (SSRF guard).

**Endpoint mechanics (from research):** deps.dev system enum uppercase `"GO"`, module paths via `url.PathEscape` (handles nested `/v2` major-suffix paths); OSV POST `/v1/query` body `{"package":{"name":"...","ecosystem":"Go"},"version":"v1.2.3"}` (normalize leading `v`); GitHub GET `/repos/{owner}/{repo}` optional `Authorization: Bearer $GITHUB_TOKEN` env var; label `open_issues_count` as issues+PRs in output. Empty-vulns ≠ source-unavailable — distinguish explicitly, cap confidence at speculative when any source unreachable (DEPEND-02).

---

### `internal/intelligence/deps/risk.go` (config-driven rules, transform)

**Analog:** `archcheck.DetectViolations(graph, rules)` consumed by `cmd/m31a/arch.go` (lines 40-62):
```go
violations := archcheck.DetectViolations(graph, rules)
for _, v := range violations {
    fmt.Printf("%s: %s: %s -> %s: %s\n", v.Severity, v.Type, v.From, v.To, v.Message)
}
if hasErrors { return 1 }
```
Pattern: config/rules struct in → deterministic classified results out; severity-to-exit-code coupling lives in the CLI layer, not the engine. D-16 thresholds come from `[intelligence.deps_risk]`; every triggered rule records which source snapshot fired it (DEPEND-02 provenance in `sources[]`).

---

### `internal/intelligence/deps/cache.go` (projection-backed cache)

**Analog:** `internal/memory/eventstore/projection.go` Projection interface (lines 21-27) and registration flow (lines 42-46):
```go
type Projection interface {
    Name() string
    Apply(event types.Event) error
    State() any
    Checkpoint() ([]byte, error)
    Restore(data []byte) error
}
pm.projections[p.Name()] = p
```
Cache key `(module, version, deps_policy_hash)` per D-15. Lookup replays DependencyChecked events — type-filter idiom from query.go lines 26-30:
```go
if q.Type != nil {
    query += " AND type = ?"
    args = append(args, string(*q.Type))
}
```
Simplest compliant form: query events by type via the public Query API, json.Unmarshal payloads, index latest-per-key in memory — no new SQL tables ("all durable state = events", Phase 1 binding decision). Re-evaluation triggers: version change OR policy-hash change.

---

### `cmd/m31a/explain.go` / `investigate.go` (CLI controllers)

**Analog:** `cmd/m31a/impact.go` runImpact (lines 14-62):

```go
// runImpact handles the `m31a impact` command.
func runImpact(args []string, workDir string, logger *slog.Logger) int {
    fs := flag.NewFlagSet("impact", flag.ExitOnError)
    depth := fs.Int("depth", 3, "Traversal depth for transitive dependents")
    format := fs.String("format", "table", "Output format: table, json, graphviz")
    fs.Usage = func() {
        fmt.Fprintln(os.Stderr, "Usage: m31a impact <symbol> [--depth N] [--format table|json|graphviz]")
        ...
    }
    fs.Parse(args)

    if fs.NArg() < 1 {
        fmt.Fprintln(os.Stderr, "Error: symbol argument required")
        fs.Usage()
        return 1
    }
    ...
    indexer := codeintel.NewIndexerWithStore(workDir, nil)
    if err := indexer.Build(context.Background()); err != nil {
        logger.Error("index build failed", "error", err)
        fmt.Fprintf(os.Stderr, "Error building index: %v\n", err)
        return 1
    }
```

Copy this exact shape: `runXxx(args []string, ...) int` returning process exit code; FlagSet named for the command; Usage override; positional-count check; slog + stderr dual reporting. explain/investigate need cfg and provider registry too — follow the wider signature style of main.go line 813: `runModelsList(flag.Args()[2:], cfg, logger, registry)`.

explain target disambiguation order (research Open Question 2): file-exists → file mode; exact index.Define hit → symbol mode; else topic mode; document in help text.

---

### `cmd/m31a/deps.go` (CLI controller, nested subcommand + TTY gate)

**Analog:** `cmd/m31a/arch.go` — the nested-subcommand precedent (`arch check`) that `deps check` mirrors:

```go
func runArchCheck(args []string, workDir string, logger *slog.Logger) int {
    fs := flag.NewFlagSet("arch check", flag.ExitOnError)   // note nested name
    ...
}
```
Registered in main.go as a two-token match (see dispatch section below).

D-14 checkpoint gate (new behavior, no direct precedent — nearest patterns):
- TTY detect: `fi, _ := os.Stdin.Stat(); (fi.Mode() & os.ModeCharDevice) != 0`.
- Interactive: print risk evidence, prompt approve/cancel.
- Headless/non-TTY: hard block, exit code 2, write pending checkpoint record first (checkpoint.go), resolvable later via `deps check --approve <module>` — mirror arch.go's exit-code-on-severity pattern (lines 58-62) but with code 2.

---

### `cmd/m31a/main.go` modification (dispatch)

**Analog:** self — existing subcommand block (lines 806-829):

```go
    // Migration command: migrate .planning/ to .m31a/
    if flag.Arg(0) == "migrate" {
        return runMigrate(flag.Args()[1:], workDir, logger)
    }

    // Models list command: m31a models list [--provider <name>] [--json]
    if flag.Arg(0) == "models" && flag.Arg(1) == "list" {
        return runModelsList(flag.Args()[2:], cfg, logger, registry)
    }

    // Index command: m31a index [--incremental] [--progress]
    if flag.Arg(0) == "index" {
        return runIndex(flag.Args()[1:], workDir, logger)
    }

    // Impact command: m31a impact <symbol> [--depth N] [--format table|json|graphviz]
    if flag.Arg(0) == "impact" {
        return runImpact(flag.Args()[1:], workDir, logger)
    }

    // Arch check command: m31a arch check [--config arch.toml]
    if flag.Arg(0) == "arch" && flag.Arg(1) == "check" {
        return runArchCheck(flag.Args()[2:], workDir, logger)
    }
```

Add three blocks in the same style (before the TUI-launch path): `explain` (single token), `investigate` (single token), `deps check` / `deps check --approve` (two tokens, arch-check pattern). Note these early-dispatch sites receive `workDir`, `cfg`, `logger`, `registry` already in scope.

---

### `internal/core/config/types.go` + `loader.go` + `merge.go` modifications

**Analog:** self — small-section template VerifyConfig (types.go lines 192-198):
```go
// VerifyConfig holds configurable verification commands.
// Empty values trigger auto-detection (existing behavior).
type VerifyConfig struct {
    BuildCommand string `toml:"build_command"`
    TestCommand  string `toml:"test_command"`
    LintCommand  string `toml:"lint_command"`
}
```
Registration points to touch for a new `[intelligence]` section:
1. `Config` struct field (types.go lines 13-35, e.g., `Intelligence IntelligenceConfig \`toml:"intelligence"\``).
2. `MergeConfig` call list (merge.go lines 97-114: one line per section, e.g., `h.mergeIntelligenceConfig(&base.Intelligence, &overlay.Intelligence, "intelligence")`) plus a mergeHelper method using `h.stringField/h.intField/h.sliceField`.
3. Unknown-key warning needs nothing extra IF the section name matches the struct tag (loader warns on unrecognized top-level keys, loader.go lines 268-281).

**Zero-value normalization is mandatory (RESEARCH Pitfall 10):** TOML absence leaves zero values after layered merge — post-load normalize like other sections do downstream: `if cfg.Intelligence.BisectMaxCommits == 0 { cfg.Intelligence.BisectMaxCommits = 50 }`. Validate ranges at load per config_validate conventions (bisect_max_commits >= 1; risk thresholds sane).

Planned section shape (CONTEXT.md specifics):
```toml
[intelligence]
repro_command = ""
bisect_max_commits = 50
[intelligence.deps_risk]
stale_months = 12
[intelligence.deps_policy]
```

---

## Shared Patterns

### Event emission (nil-safe, source-tagged)
**Source:** `internal/integrations/codeintel/events.go` lines 88-150
**Apply to:** investigate/report.go, deps/cache.go, deps/checkpoint.go
```go
evt := types.Event{ID: uuid.New(), Type: eventType, Timestamp: time.Now(),
    Payload: data, Metadata: types.EventMetadata{SchemaVersion: 1, Source: "intelligence"}}
return store.Append(ctx, evt)   // store==nil → return nil
```

### Error handling
**Source:** `internal/core/errors/errors.go` — sentinels (e.g., ErrBisectFailed line 56), ToolError struct (73-91), Wrap/Wrapf (152-170)
**Apply to:** all intelligence packages
Return wrapped errors, never panic: `coreerrors.Wrap(err, "context")` / `Wrapf`. New sentinels (source-unreachable, checkpoint-pending, not-reproducible-in-window) belong adjacent to existing ones in errors.go. User-facing failures carry a hint (ToolError-style tool+op context).

### HTTP resilience
**Source:** `provider/base_client.go` (transport 22-39; retry config 46-50, defaults 88-97; RetryStream backoff 453-459; rate-limit mapping 216-242) + `tools/search/webfetch.go` (timeout client + redirect cap 60-70)
**Apply to:** all three deps clients. initial_only / 3 attempts / 1s base; hard client Timeout; LimitReader caps; Retry-After honored; typed sentinel errors.

### Git command discipline
**Source:** `integrations/git/git.go` — run wrapper (30-38), validateGitRef (288-338), porcelain parsers (239-286, 441-563)
**Apply to:** blame.go, investigate/worktree.go. Validate refs before interpolation; wrap errors as `git <subcommand>: %w`; parse statefully with malformed-line tolerance.

### CLI convention (Phase 3, extended by D-04)
**Source:** `cmd/m31a/impact.go` + `cmd/m31a/arch.go` + main.go dispatch (806-829)
**Apply to:** explain.go, investigate.go, deps.go. FlagSet-per-command; table default; `--format json` structured objects carrying citations[]/confidence/verdict; int exit codes; usage text on stderr.

### Interface-seam testability
**Source:** `engine/bisect/bisect.go` lines 18-22 (GitRunner), `provider/interface.go` (LLMProvider)
**Apply to:** explain/synthesize.go (narrow Synthesizer iface), investigate (narrow git iface), deps clients (injectable base URL/http.Client for httptest fixtures per RESEARCH validation map).

### Untrusted-input posture
**Source:** PROJECT.md security model via RESEARCH Security Domain; enforced mechanically by webfetch/base_client excerpts above
**Apply to:** registry JSON decoding, evidence-pack contents into LLM prompts, repro command execution (confined to disposable worktree, exec.CommandContext arg-splitting).

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `internal/intelligence/deps/*` HTTP clients | HTTP client | request-response | No external-registry JSON clients exist yet; role-match to BaseClient/webfetch resilience patterns (documented above) |
| `cmd/m31a/deps.go` TTY interactive prompt | CLI controller | request-response | No interactive stdin prompt precedent in cmd/; headless path follows arch.go exit-code pattern; TTY detect is stdlib os.Stdin.Stat idiom |

Everything else maps to a strong in-repo analog.

## Metadata

**Analog search scope:** `internal/core/` (types, config, errors), `internal/integrations/` (codeintel, git, provider, archcheck), `internal/engine/` (bisect, tokens), `internal/memory/eventstore/`, `cmd/m31a/`, `internal/tools/search/`
**Files scanned:** ~30 (12 read in full, remainder grepped for API surfaces)
**Pattern extraction date:** 2026-08-25

**Key cross-cutting notes for planner:**
1. Legacy `internal/engine/bisect/bisect.go` STAYS (verify.go depends on it) — new worktree bisect lives beside it.
2. `validateGitRef` is unexported — same-package blame.go uses it freely; investigate must get refs validated through exported package methods or an exported wrapper.
3. All events reuse SchemaVersion 1 + `Metadata.Source = "intelligence"`; checkpoint pair is reused, not duplicated.
4. Every new config field needs merge.go registration AND post-load zero-value normalization (Pitfall 10).
5. deps clients must be testable against httptest servers — inject base URL/client constructor (RESEARCH validation map requires fixture-based tests, no network in CI).

