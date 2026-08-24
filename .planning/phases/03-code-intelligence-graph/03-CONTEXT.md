# Phase 03: Code Intelligence Graph - Context

**Gathered:** 2026-08-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Language-agnostic symbol graph built via Tree-sitter + LSP covering 10 languages; incremental indexing on file changes; impact analysis and architecture violation detection operational. The phase delivers `m31a index`, `m31a impact`, `m31a arch check` CLI commands with TUI integration via EventStore projections.
</domain>

<decisions>
## Implementation Decisions

### Tree-sitter + LSP Orchestration
- **D-01:** Hybrid approach — Tree-sitter handles syntax parsing, imports, basic symbol extraction for all 12 supported languages; LSP provides semantic analysis (go to definition, references, call hierarchy, type hierarchy) on-demand for 4 core languages — **Reversibility:** reversible — LSP integration is additive; can disable per language without breaking Tree-sitter graph
- **D-02:** Persistent LSP connections per project per language — one gopls per Go project, one pyright per Python project, etc.; connection pooling with 5-min idle timeout; restart on crash — **Reversibility:** reversible — pooling strategy is implementation detail
- **D-03:** LSP called on-demand for semantic queries (definitions, references, call hierarchy); results cached in graph with TTL; Tree-sitter graph remains primary for structure/imports — **Reversibility:** reversible — cache TTL and on-demand vs pre-compute is tunable
- **D-04:** Tree-sitter parsers: gotreesitter (pure Go, CGO-free) for Go, TypeScript, JavaScript, Python, Rust, Java, C/C++, C#, Ruby, PHP, Swift, Kotlin; regex fallback for Python/Rust where Tree-sitter extraction is weaker — **Reversibility:** costly — parser selection affects all downstream consumers of FileInfo

### Language Coverage
- **D-05:** 10 languages with Tree-sitter parsing: Go, TypeScript, JavaScript, Python, Rust, Java, C/C++, C#, Ruby, PHP — **Reversibility:** one-way — adding languages requires new parser integration and test corpus
- **D-06:** 4 languages with LSP semantic analysis in v1: Go (gopls), TypeScript (typescript-language-server), Python (pyright), Rust (rust-analyzer) — **Reversibility:** reversible — adding LSP for more languages is additive
- **D-07:** Config files (JSON, YAML, TOML) and Markdown/Shell parsed via Tree-sitter for imports/structure only; no LSP — **Reversibility:** reversible — LSP for config formats can be added later

### Incremental Indexing
- **D-08:** fsnotify for file watching with 500ms debounce; change queue processed by background indexer goroutine; incremental re-parse only changed files + downstream dependents — **Reversibility:** reversible — debounce interval and queue processing are tuning parameters
- **D-09:** Cache invalidation via file hash (xxhash) + modtime; deleted files removed from graph/index; cache persists to `.m31a/codeintel/cache.bin` — **Reversibility:** one-way — cache format changes require migration
- **D-10:** Progress reporting via TUI event system: `IndexProgressMsg{Phase, FilesDone, TotalFiles, CurrentFile}` emitted during build; TUI renders progress in S21 Code Intelligence screen — **Reversibility:** reversible — event format and TUI rendering can change independently
- **D-11:** Manual trigger: `m31a index --incremental` forces incremental rebuild; `m31a index` (no flag) does full rebuild — **Reversibility:** reversible — CLI flags can be added/modified

### Graph Storage
- **D-12:** EventStore (SQLite) as authoritative graph store — each symbol, file, import, call, inheritance edge = event; append-only with monotonic SEQ; projections build in-memory adjacency for queries — **Reversibility:** one-way — event schema changes require migration; EventStore is single source of truth
- **D-13:** Event types for code intelligence: `FileIndexed`, `SymbolDefined`, `ImportResolved`, `CallEdgeAdded`, `InheritanceEdgeAdded`, `TypeHierarchyEdgeAdded`, `FileDeleted`, `SymbolRemoved` — **Reversibility:** one-way — event vocabulary is contract for projections
- **D-14:** In-memory adjacency lists rebuilt from events on startup (fast replay via EventStore range queries); incremental updates apply delta events to in-memory graph — **Reversibility:** costly — changing in-memory representation affects all query paths
- **D-15:** Query API: `Define(symbol)`, `References(symbol)`, `Callers(func)`, `Callees(func)`, `Upstream(file)`, `Downstream(file)`, `Neighbors(file)`, `Impact(symbol, depth)` — all read from in-memory graph — **Reversibility:** reversible — API can be extended; EventStore remains source of truth

### LSP Client Management
- **D-16:** Per-project LSP processes — one gopls per Go project, started on first semantic query; 5-min idle shutdown; connection reused across queries — **Reversibility:** reversible — lifecycle policy is implementation detail
- **D-17:** go-lsp client for gopls; typescript-language-server, pyright, rust-analyzer via stdio transport; health check on startup; auto-restart on crash with exponential backoff — **Reversibility:** reversible — transport and health check logic can change
- **D-18:** LSP capabilities negotiated at startup; only use features server supports (definitions, references, call hierarchy, type hierarchy, hover); graceful degradation if unsupported — **Reversibility:** reversible — capability negotiation is standard LSP

### Architecture Violation Rules
- **D-19:** Config-based layer definitions in `config.toml`:
  ```toml
  [arch.layers]
  domain = ["internal/core/**", "pkg/**"]
  engine = ["internal/engine/**"]
  intelligence = ["internal/integrations/**"]
  interaction = ["internal/ui/**"]
  ```
- **D-20:** Forbidden imports = cross-layer imports not in allowed list (config `[arch.allowed_cross_layer]`); circular dependencies = graph cycles detected via Tarjan's algorithm; public API changes = exported symbol signature diff vs prior release tag — **Reversibility:** reversible — rules are config-driven
- **D-21:** Violation severity: `error` (forbidden import, circular dep), `warning` (API change), `info` (deprecated pattern) — **Reversibility:** reversible — severity levels can be adjusted

### Multi-Repo Workspace
- **D-22:** Per-repo indexes stored in each repo's `.m31a/codeintel/`; cross-repo imports tracked via import paths resolving to other workspace repos — **Reversibility:** one-way — cross-repo tracking schema is foundational
- **D-23:** Workspace root detection: directory containing `.m31a/`, `go.work`, `package.json` (with workspaces), or `Cargo.toml` (workspace) — **Reversibility:** reversible — detection logic can be extended
- **D-24:** Unified query API merges results from all repo indexes; `Impact(symbol)` returns cross-repo dependents; `Arch check` runs per-repo + cross-repo violations — **Reversibility:** reversible — query merging is implementation detail

### CLI Commands
- **D-25:** `m31a index [--incremental] [--progress]` — builds/updates symbol graph; `--progress` emits JSON progress events for scripting — **Reversibility:** reversible — CLI interface can be extended
- **D-26:** `m31a impact <symbol> [--depth N] [--format table|json|graphviz]` — direct callers (file:line), indirect dependents (transitive), affected tests, risk categories (API, runtime, test) — **Reversibility:** reversible — output formats and risk categories can be extended
- **D-27:** `m31a arch check [--config arch.toml]` — reports forbidden imports, layer boundary crossings, circular dependencies, public API changes; exit code 1 on errors — **Reversibility:** reversible — new violation types can be added
- **D-28:** TUI screens (S21-S24) consume graph data via EventStore projections; S21 Code Intelligence = project overview, S22 Symbol Explorer = search/browse, S23 Impact Analysis = impact visualization, S24 Dependency Explorer = package deps — **Reversibility:** reversible — TUI screens are projections, can be redesigned without graph changes

### the agent's Discretion
- Exact debounce interval for fsnotify (current: 500ms)
- LSP idle timeout before shutdown (current: 5 min)
- Cache TTL for LSP semantic results (suggest: 10 min)
- Event batch size for graph event replay on startup
- Tarjan's algorithm vs Kahn's for cycle detection
- Graphviz output styling for `m31a impact --format graphviz`

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Architecture & Requirements
- `.planning/CONTEXT_M31A.md` §29 — Intelligence Plane: CodeIntel (Tree-sitter + LSP), ImpactAnalyzer
- `.planning/CONTEXT_M31A.md` §37 — Six-plane architecture boundaries (Intelligence provides read-only projections)
- `.planning/REQUIREMENTS.md` — CODE-01 through CODE-07 (7 requirements)
- `.planning/ROADMAP.md` — Phase 3 success criteria (5 criteria)
- `.planning/PROJECT.md` — Code Intelligence active requirement, constraints

### Current Implementation (Reusable Assets)
- `internal/integrations/codeintel/codeintel.go` — Indexer with Build(), incremental build, caching, query API
- `internal/integrations/codeintel/parser.go` — Tree-sitter parsers (12 languages) + regex fallbacks (Python, Rust)
- `internal/integrations/codeintel/graph.go` — ImportGraph with Upstream/Downstream/Neighbors
- `internal/integrations/codeintel/index.go` — SymbolIndex with Define/References/FileSymbols
- `internal/integrations/codeintel/cache.go` — FileCache, IndexCache, incremental check
- `internal/integrations/codeintel/relevance.go` — RelevanceScorer for task-aware file ranking
- `internal/integrations/codeintel/trie.go` — Trie for symbol prefix matching
- `internal/integrations/codeintel/bench_test.go` — Benchmarks for parsing/indexing

### Research & Risks
- `.planning/research/ARCHITECTURE.md` — CodeIntel orchestration, six-plane boundaries, build order
- `.planning/research/PITFALLS.md` — Pitfall 29: Code Intelligence (re-implementing parsers, intelligence as cache not authoritative)
- `.planning/research/STACK.md` — Tree-sitter grammars, go-lsp, gopls versions
- `.planning/research/SUMMARY.md` — Phase 2 deliverables include CodeIntel
- `.planning/codebase/STACK.md` — gotreesitter v0.20.5 already in deps
- `.planning/codebase/CONCERNS.md` — gotreesitter CGO-free but version mismatch risk

### Integration Points
- **EventStore → CodeIntel**: Graph events appended to EventStore; projections rebuild in-memory graph
- **CodeIntel → ContextEngine**: `RelevantFiles()`, `FormatContext()`, `ProjectSummary()` for agent context assembly
- **CodeIntel → ImpactAnalyzer**: `Upstream()`, `Downstream()`, `Define()`, `References()` for impact analysis
- **CodeIntel → TUI**: S21-S24 screens query projections; `IndexProgressMsg` for progress UI
- **CodeIntel → CLI**: `index`, `impact`, `arch check` commands use same query API
- **Config → CodeIntel**: Layer definitions, LSP server paths, enabled languages from config.toml
- **Git → CodeIntel**: Git history for API change detection; worktree isolation for indexing

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **Indexer** (`internal/integrations/codeintel/codeintel.go`): Full Build/incremental/cache/query API — extend with EventStore persistence, LSP integration, multi-repo support
- **TreeSitterParser** (`parser.go:91-129`): gotreesitter-based multi-language parsing with AST walkers — extend with LSP client calls for semantic enrichment
- **ImportGraph** (`graph.go`): Adjacency list with Upstream/Downstream/Neighbors — extend with call graph, inheritance graph, cross-repo edges
- **SymbolIndex** (`index.go`): Trie-based symbol lookup with Define/References/FileSymbols — extend with LSP-provided definitions/references
- **Cache** (`cache.go`): FileCache with hash/modtime, incremental CheckIncremental — extend with EventStore-backed persistence
- **RelevanceScorer** (`relevance.go`): Task-aware file ranking — keep for ContextEngine integration
- **gotreesitter dependency**: v0.20.5 already in go.mod — CGO-free, supports 12 languages

### Established Patterns
- **EventStore pattern** (from Phase 1): Append-only events, in-memory projections, SQLite durability — CodeIntel follows same pattern
- **Lazy initialization**: LSP clients started on first semantic query — matches provider lazy init pattern
- **Project-local sessions**: `.m31a/codeintel/` per project — matches `.m31a/sessions/` pattern
- **Config layering**: Global → workspace → project → env — layer definitions follow same pattern
- **TUI event subscription**: Screens subscribe to normalized events — `IndexProgressMsg` follows same pattern

### Integration Points
- **EventStore → CodeIntel projections**: `FileIndexed`, `SymbolDefined`, `ImportResolved`, `CallEdgeAdded`, `InheritanceEdgeAdded`, `FileDeleted` events
- **CodeIntel → ContextEngine**: `RelevantFiles(targetFiles, description, topN)`, `FormatContext()`, `ProjectSummary()`
- **CodeIntel → ImpactAnalyzer**: `Upstream(path, depth)`, `Downstream(path, depth)`, `Define(symbol)`, `References(symbol)`
- **CodeIntel → CLI**: `index`, `impact`, `arch check` commands in `cmd/m31a/`
- **CodeIntel → TUI**: S21-S24 screens via projection queries; progress via `IndexProgressMsg`

</code_context>

<specifics>
## Specific Ideas

- LSP client pool: `map[projectRoot]map[language]*LSPClient` with health check and auto-restart
- Cross-repo import resolution: when Tree-sitter finds import like `github.com/org/other-repo`, check if `other-repo` exists in workspace and add cross-repo edge
- API change detection: compare exported symbols between current HEAD and last release tag; report signature changes as `warning` severity
- Risk categories for impact: `API` (public exported symbols), `runtime` (internal functions called by tests), `test` (test-only symbols)
- Graphviz output for `m31a impact --format graphviz`: nodes=symbols, edges=calls/imports, clusters=packages
- Config schema for architecture rules:
  ```toml
  [arch]
  layers = { domain = ["internal/core/**"], engine = ["internal/engine/**"], ... }
  allowed_cross_layer = ["engine -> domain", "interaction -> engine"]
  severity = { forbidden_import = "error", circular_dep = "error", api_change = "warning" }
  ```

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.
</deferred>

---
*Phase: 03-Code Intelligence Graph*
*Context gathered: 2026-08-24*