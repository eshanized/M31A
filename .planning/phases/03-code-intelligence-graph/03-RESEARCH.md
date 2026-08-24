# Phase 03: Code Intelligence Graph - Research

**Researched:** 2026-08-24
**Domain:** Language-agnostic symbol graph (Tree-sitter + LSP), incremental indexing, impact analysis, architecture violation detection
**Confidence:** HIGH

## Summary

Phase 3 extends the existing `internal/integrations/codeintel/` package (Tree-sitter parsing, import graph, symbol index, caching) into a full Code Intelligence Graph with EventStore persistence, LSP semantic analysis, incremental file watching, impact analysis, and architecture violation detection. The existing codebase provides a solid foundation: `gotreesitter` v0.20.5 (CGO-free) handles 12 languages, `fsnotify` v1.10.1 is already in go.mod, and the EventStore pattern (SQLite append-only events) is established in Phase 1. The primary new work is: (1) bridging codeintel events into EventStore, (2) adding LSP client management for 4 core languages, (3) building impact analysis with call graph traversal, (4) implementing architecture violation detection with Tarjan's SCC algorithm, (5) multi-repo workspace support, and (6) CLI commands (`m31a index`, `m31a impact`, `m31a arch check`).

**Primary recommendation:** Extend existing codeintel package with EventStore event emission; build LSP client as separate `internal/integrations/lsp/` package; implement impact analysis and arch violation as projections from the EventStore-backed graph.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Hybrid approach -- Tree-sitter for syntax/structure, LSP on-demand for semantic analysis (4 core languages)
- **D-02:** Persistent LSP connections per project per language; 5-min idle timeout; restart on crash
- **D-03:** LSP called on-demand for semantic queries; results cached with TTL; Tree-sitter graph remains primary
- **D-04:** gotreesitter (pure Go, CGO-free) for 12 languages; regex fallback for Python/Rust
- **D-05:** 10 languages with Tree-sitter parsing: Go, TypeScript, JavaScript, Python, Rust, Java, C/C++, C#, Ruby, PHP
- **D-06:** 4 languages with LSP semantic analysis: Go (gopls), TypeScript (typescript-language-server), Python (pyright), Rust (rust-analyzer)
- **D-07:** Config files (JSON, YAML, TOML) and Markdown/Shell via Tree-sitter only; no LSP
- **D-08:** fsnotify for file watching with 500ms debounce; change queue processed by background indexer goroutine
- **D-09:** Cache invalidation via file hash (xxhash) + modtime; cache persists to `.m31a/codeintel/cache.bin`
- **D-10:** Progress reporting via TUI event system: `IndexProgressMsg{Phase, FilesDone, TotalFiles, CurrentFile}`
- **D-11:** Manual trigger: `m31a index --incremental` forces incremental rebuild; `m31a index` does full rebuild
- **D-12:** EventStore (SQLite) as authoritative graph store; append-only events; projections build in-memory adjacency
- **D-13:** Event types: `FileIndexed`, `SymbolDefined`, `ImportResolved`, `CallEdgeAdded`, `InheritanceEdgeAdded`, `TypeHierarchyEdgeAdded`, `FileDeleted`, `SymbolRemoved`
- **D-14:** In-memory adjacency lists rebuilt from events on startup; incremental updates apply delta events
- **D-15:** Query API: `Define`, `References`, `Callers`, `Callees`, `Upstream`, `Downstream`, `Neighbors`, `Impact`
- **D-16:** Per-project LSP processes; lazy start on first semantic query; 5-min idle shutdown
- **D-17:** go-lsp for gopls; typescript-language-server, pyright, rust-analyzer via stdio transport; health check; auto-restart
- **D-18:** LSP capabilities negotiated at startup; graceful degradation if unsupported
- **D-19:** Config-based layer definitions in `config.toml` `[arch.layers]`
- **D-20:** Forbidden imports, circular dependencies (Tarjan's algorithm), public API changes detected
- **D-21:** Violation severity: `error` (forbidden import, circular dep), `warning` (API change), `info` (deprecated)
- **D-22:** Per-repo indexes in `.m31a/codeintel/`; cross-repo imports tracked via import path resolution
- **D-23:** Workspace root detection: `.m31a/`, `go.work`, `package.json` (workspaces), `Cargo.toml` (workspace)
- **D-24:** Unified query API merges results from all repo indexes
- **D-25:** `m31a index [--incremental] [--progress]` CLI command
- **D-26:** `m31a impact <symbol> [--depth N] [--format table|json|graphviz]` CLI command
- **D-27:** `m31a arch check [--config arch.toml]` CLI command; exit code 1 on errors
- **D-28:** TUI screens S21-S24 consume graph data via EventStore projections

### the agent's Discretion
- Exact debounce interval (current: 500ms)
- LSP idle timeout (current: 5 min)
- Cache TTL for LSP semantic results (suggest: 10 min)
- Event batch size for graph event replay on startup
- Tarjan's algorithm vs Kahn's for cycle detection (recommend: Tarjan -- single-pass, finds SCCs directly)
- Graphviz output styling for `m31a impact --format graphviz`

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CODE-01 | Language-agnostic symbol graph via Tree-sitter parsers (10 languages) | gotreesitter v0.20.5 already in go.mod; `grammars.DetectLanguage()` handles language routing; existing `TreeSitterParser` covers 12 languages with AST walkers |
| CODE-02 | LSP client integration for semantic analysis (go to definition, references, call hierarchy, type hierarchy) | go-lsp (sourcegraph) or custom JSON-RPC over stdio; per-project subprocess management; capability negotiation at init |
| CODE-03 | Graph includes files, symbols, types, functions, classes, interfaces, imports, calls, inheritance, implementations, tests, APIs, DB entities, configs, build targets, package dependencies, Git history | Existing `FileInfo` covers files/symbols/imports/types/funcs; extend with call edges, inheritance edges, test markers, config entities |
| CODE-04 | Incremental indexing on file changes; full re-index on demand; progress reporting | fsnotify v1.10.1 already in go.mod; existing `CheckIncremental()` with hash+modtime; debounce pattern proven in `filewatcher.go` |
| CODE-05 | Impact analysis: direct callers (file:line), indirect dependents (transitive), affected tests, risk categories | BFS/DFS traversal on call+import graphs; existing `Upstream()`/`Downstream()` provide import traversal; extend with call graph |
| CODE-06 | Architecture violation detection: forbidden imports, layer boundary crossings, circular deps, public API changes | Tarjan's SCC for circular deps (looplab/tarjan or gonum); config-based layer definitions; exported symbol diff vs prior tag |
| CODE-07 | Multi-repo workspace: per-repo indexes, cross-repo import tracking, unified query API | Per-repo `.m31a/codeintel/` stores; cross-repo import resolution via module path matching; query merging across indexes |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Tree-sitter parsing | Intelligence | -- | Language-agnostic syntax analysis belongs in Intelligence plane |
| LSP client management | Intelligence | -- | Semantic analysis is read-only intelligence; LSP processes are Intelligence plane resources |
| Symbol graph storage | Memory (EventStore) | Intelligence | Graph events are durable state; in-memory projections are Intelligence plane |
| Incremental indexing | Intelligence | Execution (file watching) | Indexing logic is Intelligence; file watching touches Execution plane's filesystem |
| Impact analysis | Intelligence | Engineering | Read-only query over symbol graph; results feed into planning (Engineering) |
| Architecture violation detection | Intelligence | Assurance | Violation detection is read-only analysis; enforcement is Assurance plane |
| Multi-repo workspace | Intelligence | Memory | Cross-repo import resolution is Intelligence; per-repo persistence is Memory |
| CLI commands (index/impact/arch) | Interaction | Intelligence | CLI entry points are Interaction; they delegate to Intelligence plane services |
| TUI screens S21-S24 | Interaction | Intelligence | TUI is presentation; data comes from Intelligence plane projections |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| gotreesitter | v0.20.5 (in go.mod) | Tree-sitter parsing for 12 languages | Pure Go, CGO-free, supports all 10+ required languages, `grammars.DetectLanguage()` for routing |
| fsnotify | v1.10.1 (in go.mod) | File system watching for incremental indexing | Already in go.mod, proven in TUI filewatcher, cross-platform |
| looplab/tarjan | v0.3.0 | Tarjan's SCC algorithm for circular dependency detection | Pure Go, well-tested, simple API: `Connections(graph) [][]interface{}` |
| gonum/graph | latest | Graph algorithms (topological sort, SCC) | Pure Go, comprehensive graph library; alternative to looplab/tarjan if more graph operations needed |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| go-lsp (sourcegraph) | latest | LSP types and client for Go language servers | If building custom LSP client; provides LSP protocol types |
| custom JSON-RPC | stdlib | LSP communication over stdio | Preferred over go-lsp -- LSP is simple JSON-RPC; no need for full library |
| xxhash | v2 | Fast non-cash hash for cache invalidation | When D-09 switches from SHA-256 to xxhash for performance |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| gotreesitter | tree-sitter/go-tree-sitter (CGO) | CGO required -- violates static binary constraint |
| looplab/tarjan | gonum/graph/topo.TarjanSCC | gonum is heavier but more graph algorithms available |
| custom JSON-RPC | go-lsp library | go-lsp adds types but LSP is simple enough for custom impl |
| fsnotify polling | inotify (Linux only) | fsnotify is cross-platform; inotify is Linux-only |

**Installation:**
```bash
# Already in go.mod
# go get github.com/odvcencio/gotreesitter@v0.20.5
# go get github.com/fsnotify/fsnotify@v1.10.1

# New dependencies for Phase 3
go get github.com/looplab/tarjan@latest
# Optional: if more graph algorithms needed
# go get gonum.org/v1/gonum@latest
```

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| gotreesitter | npm/Go | 2+ yrs | active | github.com/odvcencio/gotreesitter | OK | Approved (already in go.mod) |
| fsnotify | npm/Go | 10+ yrs | 50M+/wk | github.com/fsnotify/fsnotify | OK | Approved (already in go.mod) |
| looplab/tarjan | Go | 8+ yrs | active | github.com/looplab/tarjan | OK | Approved |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

## Architecture Patterns

### System Architecture Diagram

```
                         CLI Commands
                    m31a index | impact | arch check
                              |
                    ┌─────────┴─────────┐
                    │   Interaction     │
                    │   Plane (CLI/TUI) │
                    └─────────┬─────────┘
                              │
                    ┌─────────┴─────────┐
                    │   Intelligence    │
                    │   Plane           │
                    │                   │
                    │  ┌──────────────┐ │
                    │  │ CodeIntel    │ │
                    │  │ Service      │ │
                    │  │              │ │
                    │  │ - Indexer    │ │
                    │  │ - LSP Pool   │ │
                    │  │ - Impact     │ │
                    │  │ - ArchCheck  │ │
                    │  └──────┬───────┘ │
                    │         │         │
                    │  ┌──────┴───────┐ │
                    │  │ Projections  │ │
                    │  │ (in-memory)  │ │
                    │  └──────────────┘ │
                    └─────────┬─────────┘
                              │
                    ┌─────────┴─────────┐
                    │   Memory Plane    │
                    │   (EventStore)    │
                    │                   │
                    │  ┌──────────────┐ │
                    │  │ SQLite       │ │
                    │  │ events.db    │ │
                    │  │              │ │
                    │  │ Graph Events │ │
                    │  │ (append-only)│ │
                    │  └──────────────┘ │
                    └───────────────────┘

Data Flow:
1. File change detected (fsnotify) or CLI trigger
2. Tree-sitter parses changed files -> FileInfo
3. LSP enriches on-demand (definitions, references, call hierarchy)
4. Events appended to EventStore (FileIndexed, SymbolDefined, etc.)
5. Projections rebuild in-memory adjacency graph
6. Queries (Impact, ArchCheck) read from in-memory graph
7. Results returned to CLI/TUI
```

### Recommended Project Structure
```
internal/
├── integrations/
│   ├── codeintel/           # EXISTING - extend with EventStore integration
│   │   ├── codeintel.go     # Indexer - add EventStore emission
│   │   ├── parser.go        # Tree-sitter parsers (keep as-is)
│   │   ├── graph.go         # ImportGraph - extend with call/inheritance edges
│   │   ├── index.go         # SymbolIndex - extend with LSP-enriched data
│   │   ├── cache.go         # FileCache - migrate to EventStore persistence
│   │   ├── relevance.go     # RelevanceScorer (keep as-is)
│   │   ├── trie.go          # SymbolTrie (keep as-is)
│   │   ├── events.go        # NEW: Event types for code intelligence
│   │   ├── projection.go    # NEW: In-memory graph projection from events
│   │   └── impact.go        # NEW: Impact analysis (callers, dependents, tests, risk)
│   ├── lsp/                 # NEW - LSP client management
│   │   ├── client.go        # LSP client (JSON-RPC over stdio)
│   │   ├── pool.go          # Per-project connection pool
│   │   ├── manager.go       # Language server lifecycle (start, health, restart)
│   │   ├── capabilities.go  # Capability negotiation
│   │   └── servers.go       # Server definitions (gopls, pyright, etc.)
│   └── archcheck/           # NEW - Architecture violation detection
│       ├── rules.go         # Layer definitions, allowed cross-layer imports
│       ├── detector.go      # Violation detection (forbidden imports, cycles, API changes)
│       └── tarjan.go        # SCC detection for circular dependencies
├── core/
│   └── types/
│       └── event.go         # EXTEND: Add code intelligence event types
└── cmd/m31a/
    ├── index.go             # NEW: `m31a index` command
    ├── impact.go            # NEW: `m31a impact` command
    └── arch.go              # NEW: `m31a arch check` command
```

### Pattern 1: EventStore-Backed Graph
**What:** All graph state changes (file indexed, symbol defined, edges added) are emitted as events to EventStore; in-memory graph is rebuilt from event replay.
**When:** Any durable graph state that must survive process crash and be queryable across sessions.
**Example:**
```go
// Event types for code intelligence (D-13)
const (
    EventFileIndexed           EventType = "FileIndexed"
    EventSymbolDefined         EventType = "SymbolDefined"
    EventImportResolved        EventType = "ImportResolved"
    EventCallEdgeAdded         EventType = "CallEdgeAdded"
    EventInheritanceEdgeAdded  EventType = "InheritanceEdgeAdded"
    EventTypeHierarchyEdgeAdded EventType = "TypeHierarchyEdgeAdded"
    EventFileDeleted           EventType = "FileDeleted"
    EventSymbolRemoved         EventType = "SymbolRemoved"
)

// Payload for FileIndexed event
type FileIndexedPayload struct {
    Path     string       `json:"path"`
    Language string       `json:"language"`
    Symbols  []SymbolInfo `json:"symbols"`
    Imports  []ImportInfo `json:"imports"`
    Hash     [32]byte     `json:"hash"`
    ModTime  time.Time    `json:"mod_time"`
}

// Emit event during indexing
func (idx *Indexer) emitFileIndexed(ctx context.Context, store EventStore, info *FileInfo) error {
    payload := FileIndexedPayload{
        Path:     info.Path,
        Language: info.Language,
        Symbols:  info.Exports,
        Imports:  info.Imports,
    }
    data, _ := json.Marshal(payload)
    evt := Event{
        Type:      EventFileIndexed,
        Payload:   data,
        Timestamp: time.Now(),
        Metadata:  EventMetadata{SchemaVersion: 1, Source: "codeintel"},
    }
    return store.Append(ctx, evt)
}
```

### Pattern 2: LSP Client Pool
**What:** Per-project, per-language LSP connections managed as a pool with lazy start, idle timeout, and crash recovery.
**When:** Multiple language servers needed for semantic analysis; each server is a separate OS process.
**Example:**
```go
// LSP client pool: map[projectRoot]map[language]*LSPClient
type LSPPool struct {
    mu       sync.RWMutex
    clients  map[string]map[string]*LSPClient  // projectRoot -> lang -> client
    idleTTL  time.Duration                      // default: 5 min
}

func (p *LSPPool) GetClient(projectRoot, language string) (*LSPClient, error) {
    p.mu.RLock()
    if clients, ok := p.clients[projectRoot]; ok {
        if client, ok := clients[language]; ok {
            client.Touch()  // reset idle timer
            p.mu.RUnlock()
            return client, nil
        }
    }
    p.mu.RUnlock()

    // Lazy start: spawn language server process
    return p.startClient(projectRoot, language)
}

func (p *LSPPool) startClient(projectRoot, language string) (*LSPClient, error) {
    cmd := languageServerCommand(language)
    client, err := NewLSPClient(cmd, projectRoot)
    if err != nil {
        return nil, fmt.Errorf("start LSP %s: %w", language, err)
    }

    p.mu.Lock()
    if p.clients[projectRoot] == nil {
        p.clients[projectRoot] = make(map[string]*LSPClient)
    }
    p.clients[projectRoot][language] = client
    p.mu.Unlock()

    return client, nil
}

// Language server commands (D-17)
func languageServerCommand(language string) []string {
    switch language {
    case "go":
        return []string{"gopls", "serve"}
    case "typescript", "javascript":
        return []string{"typescript-language-server", "--stdio"}
    case "python":
        return []string{"pyright-langserver", "--stdio"}
    case "rust":
        return []string{"rust-analyzer"}
    default:
        return nil
    }
}
```

### Pattern 3: Tarjan's SCC for Circular Dependencies
**What:** Detect strongly connected components in the import/call graph to find circular dependencies.
**When:** Architecture violation detection needs to identify cycles in dependency graph.
**Example:**
```go
// Using looplab/tarjan for cycle detection
import "github.com/looplab/tarjan"

func DetectCircularDeps(graph *ImportGraph) [][]string {
    // Build adjacency list for tarjan
    connections := make(map[interface{}][]interface{})
    for path, node := range graph.nodes {
        var edges []interface{}
        for _, imp := range node.Imports {
            edges = append(edges, imp)
        }
        connections[path] = edges
    }

    // Find strongly connected components
    sccs := tarjan.Connections(connections)

    // Filter: SCCs with size > 1 are circular dependencies
    var cycles [][]string
    for _, scc := range sccs {
        if len(scc) > 1 {
            cycle := make([]string, len(scc))
            for i, v := range scc {
                cycle[i] = v.(string)
            }
            cycles = append(cycles, cycle)
        }
    }
    return cycles
}
```

### Anti-Patterns to Avoid
- **Hand-rolling LSP JSON-RPC:** LSP is standard JSON-RPC 2.0 over stdio; don't build a custom protocol layer
- **Eager LSP indexing:** Don't call LSP for all files at build time; use on-demand (D-03) to avoid slow startup
- **Storing graph in flat files:** Use EventStore (SQLite) as authoritative store; flat files are projections only
- **Ignoring LSP crashes:** Language servers crash; implement auto-restart with exponential backoff (D-17)
- **Hardcoding language servers:** Use config for server paths; different environments have different installations

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON-RPC over stdio | Custom protocol layer | stdlib `encoding/json` + `os/exec` | LSP is standard JSON-RPC 2.0; simple request/response pattern |
| Tarjan's SCC algorithm | Custom cycle detection | `github.com/looplab/tarjan` | Well-tested, O(V+E), handles all edge cases |
| File watching | Custom inotify/polling | `github.com/fsnotify/fsnotify` (already in go.mod) | Cross-platform, event-based, already proven in codebase |
| Graph traversal (BFS/DFS) | Custom walk functions | Extend existing `Upstream()`/`Downstream()` | Already implemented in `graph.go`; just add call graph edges |

**Key insight:** The existing `ImportGraph` already has BFS traversal (`Upstream`/`Downstream`). Impact analysis extends this by adding call graph edges and risk categorization -- don't rebuild traversal from scratch.

## Common Pitfalls

### Pitfall 1: LSP Server Startup Latency
**What goes wrong:** First semantic query blocks for 5-30 seconds while LSP server starts and indexes the project.
**Why it happens:** Language servers (gopls, rust-analyzer) need to parse the entire project on startup before responding to queries.
**How to avoid:** Lazy-start LSP on first semantic query (D-16); show "indexing..." progress to user; cache results with TTL (D-03); fall back to Tree-sitter-only results if LSP times out.
**Warning signs:** User reports slow first `m31a impact` command; LSP health check fails on startup.

### Pitfall 2: EventStore Event Schema Evolution
**What goes wrong:** Code intelligence events have different schema than Phase 1 domain events; event type vocabulary grows unbounded.
**Why it happens:** Graph events (FileIndexed, SymbolDefined) are structurally different from workflow events (TaskCreated, RunCompleted).
**How to avoid:** Use same Event envelope (ID, Seq, Type, Payload, Metadata) but with `source: "codeintel"` tag; version payloads with `SchemaVersion`; define all event types in `internal/core/types/event.go` alongside existing types.
**Warning signs:** Event deserialization errors; projection rebuild failures.

### Pitfall 3: Call Graph Extraction Accuracy
**What goes wrong:** Tree-sitter extracts function definitions but not call sites; impact analysis misses indirect callers.
**Why it happens:** Tree-sitter AST walking for call expressions is language-specific and error-prone; different node types per language.
**How to avoid:** Use LSP `textDocument/references` and `textDocument/prepareCallHierarchy` for call graph (D-06); Tree-sitter provides structure, LSP provides semantics; cache LSP results with TTL.
**Warning signs:** Impact analysis reports fewer callers than expected; manual inspection finds missing edges.

### Pitfall 4: Multi-Repo Import Resolution
**What goes wrong:** Cross-repo imports (e.g., `github.com/org/other-repo/pkg`) don't resolve to local workspace repos.
**Why it happens:** Import resolution assumes single-module semantics; workspace repos have different module paths.
**How to avoid:** Detect workspace root (D-23) via `go.work`, `package.json` workspaces, `Cargo.toml` workspace; maintain workspace repo map; resolve imports against all workspace repos; add cross-repo edges to graph.
**Warning signs:** `m31a impact` on a symbol in repo A doesn't show dependents in repo B.

### Pitfall 5: Architecture Config Validation
**What goes wrong:** Config `[arch.layers]` references paths that don't exist; layer definitions are inconsistent.
**Why it happens:** Users write config without validation; typos in glob patterns; circular layer definitions.
**How to avoid:** Validate config on load: check path globs match at least one file; detect circular layer references; provide clear error messages; default to sensible layers for common project structures.
**Warning signs:** `m31a arch check` reports no violations when violations exist; config loading fails silently.

## Code Examples

### Existing Patterns to Follow

### File Watching with Debounce (from filewatcher.go)
```go
// Source: internal/ui/tui/filewatcher.go:133-143
// Existing debounce pattern -- reuse for incremental indexing
func (fw *FileWatcher) debounceEvent() {
    if fw.debounce != nil {
        fw.debounce.Stop()
    }
    fw.debounce = time.AfterFunc(300*time.Millisecond, func() {
        select {
        case fw.Events <- SidebarRefreshTickMsg{}:
        case <-fw.ctx.Done():
        }
    })
}
```

### Tree-sitter AST Walking (from parser.go)
```go
// Source: internal/integrations/codeintel/parser.go:132-184
// Existing AST walker -- extend with call expression extraction
func walkAST(node *gotreesitter.Node, content []byte, lang *gotreesitter.Language, langName string, info *FileInfo) {
    if node == nil {
        return
    }
    nodeType := node.Type(lang)
    switch nodeType {
    case "import_declaration", "import_statement":
        extractImport(node, content, lang, langName, info)
    case "function_declaration", "method_declaration", "function":
        extractFunction(node, content, lang, langName, info)
    // ... more node types
    }
    for i := 0; i < int(node.ChildCount()); i++ {
        child := node.Child(i)
        if child != nil && child.IsNamed() {
            walkAST(child, content, lang, langName, info)
        }
    }
}
```

### Import Graph Traversal (from graph.go)
```go
// Source: internal/integrations/codeintel/graph.go:126-168
// Existing BFS traversal -- extend with call graph edges
func (g *ImportGraph) traverse(start string, maxDepth int, next func(*Node) []string) []string {
    visited := make(map[string]bool)
    visited[start] = true
    type entry struct { path string; depth int }
    queue := []entry{{path: start, depth: 0}}
    var result []string
    for front := 0; front < len(queue); front++ {
        cur := queue[front]
        if maxDepth > 0 && cur.depth >= maxDepth { continue }
        node, ok := g.nodes[cur.path]
        if !ok { continue }
        for _, neighbor := range next(node) {
            if !visited[neighbor] {
                visited[neighbor] = true
                result = append(result, neighbor)
                queue = append(queue, entry{path: neighbor, depth: cur.depth + 1})
            }
        }
    }
    return result
}
```

### LSP Client (from reference implementation)
```go
// Source: docs.rs/codive-lsp (Rust reference, adapt to Go)
// LSP client pattern: spawn subprocess, JSON-RPC over stdin/stdout
type LSPClient struct {
    cmd    *exec.Cmd
    stdin  io.WriteCloser
    stdout *bufio.Reader
    nextID int64
}

func NewLSPClient(command []string, workDir string) (*LSPClient, error) {
    cmd := exec.Command(command[0], command[1:]...)
    cmd.Dir = workDir
    stdin, _ := cmd.StdinPipe()
    stdout, _ := cmd.StdoutPipe()
    cmd.Stderr = nil  // discard stderr

    if err := cmd.Start(); err != nil {
        return nil, err
    }

    client := &LSPClient{
        cmd:    cmd,
        stdin:  stdin,
        stdout: bufio.NewReader(stdout),
    }

    // Initialize: send initialize request
    initParams := map[string]interface{}{
        "rootUri": "file://" + workDir,
        "capabilities": map[string]interface{}{},
    }
    client.sendRequest("initialize", initParams)
    client.recvResponse()

    // Send initialized notification
    client.sendNotification("initialized", nil)

    return client, nil
}

func (c *LSPClient) Definition(file string, line, col int) ([]Location, error) {
    params := map[string]interface{}{
        "textDocument": map[string]interface{}{"uri": "file://" + file},
        "position":     map[string]interface{}{"line": line, "character": col},
    }
    return c.sendRequest("textDocument/definition", params)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Tree-sitter only (syntax) | Tree-sitter + LSP (semantic) | Phase 3 | Richer symbol graph; call hierarchy, type hierarchy available |
| Flat file cache (`.m31a/codeintel.cache`) | EventStore (SQLite) events | Phase 3 | Durable graph; survives crash; queryable across sessions |
| Import graph only | Import + call + inheritance graph | Phase 3 | Impact analysis needs call edges; arch check needs inheritance |
| Single-repo only | Multi-repo workspace | Phase 3 | Cross-repo import tracking; unified query API |
| Manual `m31a index` only | fsnotify auto-index + manual | Phase 3 | Near-instant incremental updates on file change |

**Deprecated/outdated:**
- Flat file cache (`codeintel.cache`): Replaced by EventStore events; cache becomes projection
- Import-only graph: Extended with call edges, inheritance edges, type hierarchy edges

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | go-lsp (sourcegraph) provides LSP types sufficient for custom client | Standard Stack | Low -- LSP is simple JSON-RPC; types are standard |
| A2 | looplab/tarjan v0.3.0 is current and maintained | Standard Stack | Low -- algorithm is stable; no updates needed |
| A3 | LSP servers (gopls, pyright, typescript-language-server, rust-analyzer) are available in user's PATH | Environment | Medium -- planner should add install check or graceful degradation |
| A4 | gotreesitter v0.20.5 supports all 10 required languages | Standard Stack | Low -- already proven in existing codebase |
| A5 | EventStore schema from Phase 1 supports code intelligence events | Architecture | Low -- Event envelope is generic; just add new event types |

## Open Questions

1. **LSP server availability in CI/headless environments**
   - What we know: LSP servers are external binaries (gopls, pyright, etc.) that must be installed
   - What's unclear: Whether CI environments will have these installed
   - Recommendation: Graceful degradation -- Tree-sitter-only mode when LSP unavailable; warn user

2. **Performance of EventStore replay for large codebases**
   - What we know: EventStore replays events on startup to rebuild in-memory graph
   - What's unclear: Replay time for 10K+ files with 100K+ events
   - Recommendation: Benchmark with real repos; consider snapshot + delta approach if replay too slow

3. **Cross-repo import resolution for non-Go languages**
   - What we know: Go has `go.work` for workspace detection; TS has `package.json` workspaces
   - What's unclear: How Python/Rust workspaces are detected (no standard mechanism)
   - Recommendation: Use config-based workspace root specification as fallback

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Go 1.26+ | Build | ✓ | 1.26.5 | -- |
| gopls | LSP for Go | ? | -- | Tree-sitter only |
| typescript-language-server | LSP for TS/JS | ? | -- | Tree-sitter only |
| pyright-langserver | LSP for Python | ? | -- | Tree-sitter only |
| rust-analyzer | LSP for Rust | ? | -- | Tree-sitter only |

**Missing dependencies with fallback:**
- LSP servers: Graceful degradation to Tree-sitter-only mode; warn user on first semantic query

**Missing dependencies with no fallback:**
- None -- all dependencies have Tree-sitter-only fallback

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Go standard testing + testify |
| Config file | go.mod (existing) |
| Quick run command | `make test-fast` |
| Full suite command | `make test` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CODE-01 | Tree-sitter parses 10 languages | unit | `go test ./internal/integrations/codeintel/... -run TestParser -x` | ✅ (parser_test.go) |
| CODE-02 | LSP client connects to language server | integration | `go test ./internal/integrations/lsp/... -run TestLSPClient -x` | ❌ Wave 0 |
| CODE-03 | Graph contains all required entity types | unit | `go test ./internal/integrations/codeintel/... -run TestGraphEntities -x` | ❌ Wave 0 |
| CODE-04 | Incremental indexing on file change | unit | `go test ./internal/integrations/codeintel/... -run TestIncremental -x` | ✅ (codeintel_test.go) |
| CODE-05 | Impact analysis returns callers/dependents | unit | `go test ./internal/integrations/codeintel/... -run TestImpact -x` | ❌ Wave 0 |
| CODE-06 | Architecture violation detection | unit | `go test ./internal/integrations/archcheck/... -run TestViolation -x` | ❌ Wave 0 |
| CODE-07 | Multi-repo workspace support | integration | `go test ./internal/integrations/codeintel/... -run TestMultiRepo -x` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `make test-fast`
- **Per wave merge:** `make test`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `internal/integrations/lsp/` -- new package, all tests needed
- [ ] `internal/integrations/archcheck/` -- new package, all tests needed
- [ ] `internal/integrations/codeintel/events.go` -- new file, event emission tests
- [ ] `internal/integrations/codeintel/projection.go` -- new file, projection rebuild tests
- [ ] `internal/integrations/codeintel/impact.go` -- new file, impact analysis tests
- [ ] `cmd/m31a/index.go` -- new CLI command, integration tests
- [ ] `cmd/m31a/impact.go` -- new CLI command, integration tests
- [ ] `cmd/m31a/arch.go` -- new CLI command, integration tests

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V5 Input Validation | yes | Validate all file paths through `ValidateTaskFiles()` before indexing; prevent path traversal |
| V6 Cryptography | no | File hashing uses SHA-256 for cache invalidation only; not security-critical |

### Known Threat Patterns for Code Intelligence Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Path traversal via malicious import paths | Tampering | Validate all paths through `ValidateTaskFiles()` with `EvalSymlinks` |
| LSP server process injection | Elevation of Privilege | Validate LSP server command from config; don't accept arbitrary commands |
| Symlink escape during file walking | Information Disclosure | Check `EvalSymlinks` against workspace root during `WalkDir` |

## Sources

### Primary (HIGH confidence)
- gotreesitter v0.20.5 API: `go doc` output from local installation -- confirmed Node, Parser, Language APIs
- fsnotify v1.10.1 API: Context7 docs -- confirmed event types, watcher creation, debounce pattern
- Existing codebase: `internal/integrations/codeintel/` -- 13 files, proven patterns
- EventStore pattern: `internal/core/types/event.go` -- 38 event types, generic envelope

### Secondary (MEDIUM confidence)
- looplab/tarjan: GitHub README + pkg.go.dev -- confirmed API: `Connections(graph) [][]interface{}`
- LSP client pattern: codive-lsp (Rust) and pathfinder (Rust) reference implementations -- adapted to Go
- Tarjan's algorithm: reintech.io tutorial + gonum implementation -- confirmed O(V+E) complexity

### Tertiary (LOW confidence)
- go-lsp (sourcegraph) -- not directly verified; may be stale or unmaintained
- LSP server availability in CI -- assumption based on typical developer environments

## Metadata

**Confidence breakdown:**
- Standard Stack: HIGH -- gotreesitter and fsverify already in go.mod and proven; tarjan is stable algorithm
- Architecture: HIGH -- EventStore pattern established in Phase 1; codeintel package already has 13 files
- Pitfalls: MEDIUM -- LSP startup latency and call graph accuracy are known challenges; mitigation strategies documented

**Research date:** 2026-08-24
**Valid until:** 2026-09-24 (30 days -- stable stack, well-established patterns)
