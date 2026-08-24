# Phase 03: Code Intelligence Graph - Discussion Log

**Gathered:** 2026-08-24
**Status:** Complete

---

## Discussion Summary

**Phase:** 3 — Code Intelligence Graph
**Domain:** Language-agnostic symbol graph via Tree-sitter + LSP with incremental indexing, impact analysis, and architecture violation detection

---

## Areas Discussed

### 1. Tree-sitter + LSP Orchestration — Integration Strategy
**Selected:** Hybrid: Tree-sitter for structure, LSP on-demand (Recommended)

**Rationale:**
- Tree-sitter handles syntax parsing, imports, basic symbol extraction for all 12 supported languages
- LSP provides semantic analysis (go to definition, references, call hierarchy, type hierarchy) on-demand
- Persistent LSP connections per project per language with connection pooling
- LSP called on-demand for semantic queries; results cached in graph with TTL
- Tree-sitter graph remains primary for structure/imports

**Decisions:**
- D-01: Hybrid approach — Tree-sitter for structure, LSP on-demand for semantic analysis
- D-02: Persistent LSP connections per project per language; 5-min idle timeout
- D-03: LSP on-demand for semantic queries; cached with TTL
- D-04: gotreesitter for 12 languages; regex fallback for Python/Rust

---

### 2. Language Coverage — Which 10 Languages Get LSP Semantic Analysis
**Selected:** Core 4 LSP languages: Go, TS, Python, Rust (Recommended)

**Rationale:**
- Tree-sitter covers all 10+ languages for syntax/structure
- LSP semantic analysis only for 4 core languages where mature servers exist
- Other languages (Java, C/C++, C#, Ruby, PHP, Swift, Kotlin) use Tree-sitter only in v1
- Config files (JSON, YAML, TOML) and Markdown/Shell use Tree-sitter only

**Decisions:**
- D-05: 10 languages with Tree-sitter parsing
- D-06: 4 languages with LSP in v1 (Go, TypeScript, Python, Rust)
- D-07: Config/Markdown/Shell via Tree-sitter only

---

### 3. Incremental Indexing — File Watching, Cache Invalidation, Progress Reporting
**Selected:** fsnotify + debounce + TUI progress events (Recommended)

**Rationale:**
- fsnotify for file watching with 500ms debounce
- Change queue processed by background indexer goroutine
- Cache invalidation via file hash (xxhash) + modtime
- Progress reporting via TUI event system
- Manual trigger with `m31a index --incremental`

**Decisions:**
- D-08: fsnotify + 500ms debounce + background indexer
- D-09: Cache invalidation via hash + modtime; persists to `.m31a/codeintel/cache.bin`
- D-10: Progress via `IndexProgressMsg` events for TUI
- D-11: Manual `m31a index --incremental` trigger

---

### 4. Graph Storage — SQLite Persistence, Query Performance, EventStore Integration
**Selected:** EventStore (SQLite) as authoritative graph store (Recommended)

**Rationale:**
- EventStore (SQLite) as single source of truth for graph
- Each symbol/file/relationship = append-only event
- In-memory adjacency rebuilt from events on startup
- Query API reads from in-memory graph

**Decisions:**
- D-12: EventStore as authoritative graph store
- D-13: Event types: FileIndexed, SymbolDefined, ImportResolved, CallEdgeAdded, InheritanceEdgeAdded, TypeHierarchyEdgeAdded, FileDeleted, SymbolRemoved
- D-14: In-memory adjacency rebuilt from events; incremental delta updates
- D-15: Query API: Define, References, Callers, Callee, Upstream, Downstream, Neighbors, Impact

---

### 5. LSP Client — Subprocess Management, Pooling, Crash Recovery
**Selected:** Per-project LSP processes: One gopls per project (Recommended)

**Rationale:**
- One LSP server per language per project
- Started on first semantic query; 5-min idle shutdown
- go-lsp for gopls; stdio transport for others
- Health check on startup; auto-restart on crash with exponential backoff

**Decisions:**
- D-16: Per-project LSP processes; lazy start, 5-min idle shutdown
- D-17: go-lsp for gopls; stdio for TS/Python/Rust; health check + auto-restart
- D-18: Capability negotiation at startup; graceful degradation

---

### 6. Architecture Violation Rules — Layer Definitions, Forbidden Imports, API Change Detection
**Selected:** Config-based layer definitions (Recommended)

**Rationale:**
- Layer definitions in config.toml with glob patterns
- Forbidden imports = cross-layer imports not in allowed list
- Circular dependencies = graph cycles via Tarjan's algorithm
- Public API changes = exported symbol signature diff vs prior release tag

**Decisions:**
- D-19: Config-based layers in config.toml with glob patterns
- D-20: Forbidden imports, circular deps (Tarjan), API changes (Git history diff)
- D-21: Severity: error (forbidden/circular), warning (API change), info (deprecated)

---

### 7. Multi-Repo Workspace — Workspace Detection, Cross-Repo Imports, Unified Queries
**Selected:** Per-repo indexes + cross-repo import tracking (Recommended)

**Rationale:**
- Per-repo indexes in each repo's `.m31a/codeintel/`
- Cross-repo imports tracked via import path resolution
- Workspace root detection via `.m31a/`, `go.work`, `package.json`, `Cargo.toml`
- Unified query API merges results from all repo indexes

**Decisions:**
- D-22: Per-repo indexes + cross-repo import tracking
- D-23: Workspace root detection via config files
- D-24: Unified query API merges cross-repo results

---

### 8. CLI Commands — index, impact, arch check Interfaces and TUI Integration
**Selected:** CLI-first with TUI consuming projections (Recommended)

**Rationale:**
- CLI commands as primary interface
- TUI screens (S21-S24) consume data via EventStore projections
- Output formats: table (default), JSON, graphviz
- Progress events for scripting

**Decisions:**
- D-25: `m31a index [--incremental] [--progress]`
- D-26: `m31a impact <symbol> [--depth N] [--format table|json|graphviz]`
- D-27: `m31a arch check [--config arch.toml]` with exit code 1 on errors
- D-28: TUI screens S21-S24 consume projections; `IndexProgressMsg` for progress

---

## Agent's Discretion Items

- Exact debounce interval for fsnotify (current: 500ms)
- LSP idle timeout before shutdown (current: 5 min)
- Cache TTL for LSP semantic results (suggest: 10 min)
- Event batch size for graph event replay on startup
- Tarjan's algorithm vs Kahn's for cycle detection
- Graphviz output styling for `m31a impact --format graphviz`

---

## Deferred Ideas

None — discussion stayed within phase scope.

---

*Discussion log generated: 2026-08-24*