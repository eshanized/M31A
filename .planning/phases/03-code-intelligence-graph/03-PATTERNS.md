# Phase 03: Code Intelligence Graph - Pattern Map

**Mapped:** 2026-08-24
**Files analyzed:** 19 (14 new, 5 modified)
**Analogs found:** 12 / 19

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `internal/integrations/codeintel/events.go` | types | transform | `internal/core/types/event.go` | exact |
| `internal/integrations/codeintel/projection.go` | service | event-driven | `internal/integrations/codeintel/graph.go` | role-match |
| `internal/integrations/codeintel/impact.go` | service | transform | `internal/integrations/codeintel/graph.go` | exact |
| `internal/integrations/lsp/client.go` | service | request-response | `internal/integrations/provider/base_client.go` | role-match |
| `internal/integrations/lsp/pool.go` | service | event-driven | `internal/integrations/provider/cache.go` | role-match |
| `internal/integrations/lsp/manager.go` | service | event-driven | `internal/integrations/codeintel/codeintel.go` | role-match |
| `internal/integrations/lsp/capabilities.go` | types | transform | `internal/integrations/provider/capabilities.go` | exact |
| `internal/integrations/lsp/servers.go` | config | transform | `internal/integrations/codeintel/parser.go` | role-match |
| `internal/integrations/archcheck/rules.go` | config | transform | `internal/core/config/config.go` | role-match |
| `internal/integrations/archcheck/detector.go` | service | transform | `internal/integrations/codeintel/graph.go` | role-match |
| `internal/integrations/archcheck/tarjan.go` | utility | transform | N/A (no analog) | N/A |
| `cmd/m31a/index.go` | CLI | request-response | `cmd/m31a/main.go` | exact |
| `cmd/m31a/impact.go` | CLI | request-response | `cmd/m31a/main.go` | exact |
| `cmd/m31a/arch.go` | CLI | request-response | `cmd/m31a/main.go` | exact |
| `internal/integrations/codeintel/codeintel.go` | service | CRUD | same file | extend |
| `internal/integrations/codeintel/graph.go` | service | CRUD | same file | extend |
| `internal/integrations/codeintel/index.go` | service | CRUD | same file | extend |
| `internal/integrations/codeintel/cache.go` | service | file-I/O | same file | extend |
| `internal/core/types/event.go` | types | transform | same file | extend |

## Pattern Assignments

### `internal/integrations/codeintel/events.go` (types, transform)

**Analog:** `internal/core/types/event.go`

**Event type definition pattern** (lines 21-62):
```go
type EventType string

const (
    EventProjectInitialized    EventType = "ProjectInitialized"
    EventRepositoryIndexed     EventType = "RepositoryIndexed"
    EventSessionCreated        EventType = "SessionCreated"
    // ... more event types
)

type EventMetadata struct {
    SchemaVersion int      `json:"schema_version"`
    Tags          []string `json:"tags,omitempty"`
    Source        string   `json:"source,omitempty"`
}
```

**Code intelligence events to add:**
```go
// Phase 03 events (D-13)
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
```

**Event payload pattern** (from research):
```go
type FileIndexedPayload struct {
    Path     string       `json:"path"`
    Language string       `json:"language"`
    Symbols  []SymbolInfo `json:"symbols"`
    Imports  []ImportInfo `json:"imports"`
    Hash     [32]byte     `json:"hash"`
    ModTime  time.Time    `json:"mod_time"`
}
```

---

### `internal/integrations/codeintel/projection.go` (service, event-driven)

**Analog:** `internal/integrations/codeintel/graph.go` (import graph pattern)

**In-memory adjacency pattern** (lines 22-30):
```go
type ImportGraph struct {
    nodes map[string]*Node // relative path → node
}

func NewImportGraph() *ImportGraph {
    return &ImportGraph{nodes: make(map[string]*Node)}
}
```

**Projection rebuild from events pattern:**
```go
// CodeIntelProjection rebuilds in-memory graph from EventStore events
type CodeIntelProjection struct {
    mu       sync.RWMutex
    graph    *CodeGraph    // extended graph with call/inheritance edges
    files    map[string]*FileInfo
    symbols  map[string][]SymbolLocation
}

func (p *CodeIntelProjection) RebuildFromEvents(ctx context.Context, store EventStore) error {
    // Replay events from EventStore (D-14)
    // Build in-memory adjacency lists
    // Apply incremental delta events
}
```

**Key pattern:** Follow the same mutex-protected RLock/RLock pattern used in `graph.go` lines 136-168 for concurrent read access.

---

### `internal/integrations/codeintel/impact.go` (service, transform)

**Analog:** `internal/integrations/codeintel/graph.go`

**BFS traversal pattern** (lines 136-168):
```go
func (g *ImportGraph) traverse(start string, maxDepth int, next func(*Node) []string) []string {
    visited := make(map[string]bool)
    visited[start] = true
    type entry struct {
        path  string
        depth int
    }
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

**Impact analysis structure:**
```go
type ImpactResult struct {
    Symbol        string
    DirectCallers []CallerInfo     // file:line where symbol is called
    Indirect      []string         // transitive dependents
    AffectedTests []string         // test files that transitively depend
    Risk          RiskCategory     // API, runtime, test
}

type CallerInfo struct {
    File     string
    Line     int
    Symbol   string
    Category RiskCategory
}

func AnalyzeImpact(graph *CodeGraph, symbol string, depth int) *ImpactResult {
    // Use graph.Callers(symbol) for direct callers
    // Use graph.Callees(symbol) for what symbol calls
    // Use traverse() from graph.go pattern for transitive
    // Categorize risk: API (exported symbols), runtime (internal), test (test files)
}
```

---

### `internal/integrations/lsp/client.go` (service, request-response)

**Analog:** `internal/integrations/provider/base_client.go` (subprocess management pattern)

**LSP client pattern:**
```go
type LSPClient struct {
    cmd    *exec.Cmd
    stdin  io.WriteCloser
    stdout *bufio.Reader
    mu     sync.Mutex
    nextID int64
}

func NewLSPClient(command []string, workDir string) (*LSPClient, error) {
    cmd := exec.Command(command[0], command[1:]...)
    cmd.Dir = workDir
    stdin, _ := cmd.StdinPipe()
    stdout, _ := cmd.StdoutPipe()
    if err := cmd.Start(); err != nil {
        return nil, err
    }
    client := &LSPClient{
        cmd:    cmd,
        stdin:  stdin,
        stdout: bufio.NewReader(stdout),
    }
    // Initialize: send initialize request, wait for response
    // Send initialized notification
    return client, nil
}
```

**Key pattern:** JSON-RPC 2.0 over stdio (not HTTP). Use `encoding/json` for serialization, `os/exec` for subprocess management.

---

### `internal/integrations/lsp/pool.go` (service, event-driven)

**Analog:** `internal/integrations/provider/cache.go` (connection pooling with mutex)

**Connection pool pattern:**
```go
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
```

**Key pattern:** Same lazy initialization pattern used by provider registry (`provider.NewLazyRegistry` in main.go line 733).

---

### `internal/integrations/lsp/manager.go` (service, event-driven)

**Analog:** `internal/integrations/codeintel/codeintel.go` (lifecycle management)

**Lifecycle management pattern:**
```go
type LSPManager struct {
    pool     *LSPPool
    configs  map[string]ServerConfig  // language -> config
    logger   *slog.Logger
}

func (m *LSPManager) EnsureStarted(ctx context.Context, projectRoot, language string) error {
    // Check if language server binary exists
    // Start if not running
    // Health check
    // Auto-restart on crash with exponential backoff
}

func (m *LSPManager) ShutdownIdle() {
    // Close clients idle > 5 min
}
```

**Key pattern:** Same error wrapping pattern (`fmt.Errorf("start LSP %s: %w", language, err)`) used throughout codeintel package.

---

### `internal/integrations/lsp/capabilities.go` (types, transform)

**Analog:** `internal/integrations/provider/capabilities.go` (exact match)

**Capability negotiation pattern** (lines 99-108):
```go
type LSPCapabilities struct {
    SupportsDefinition     bool
    SupportsReferences     bool
    SupportsCallHierarchy  bool
    SupportsTypeHierarchy  bool
    SupportsHover          bool
    SupportsCompletion     bool
}

func NegotiateCapabilities(serverCaps interface{}) LSPCapabilities {
    // Parse server capabilities from initialize response
    // Return negotiated capabilities
    // Graceful degradation if feature not supported
}
```

---

### `internal/integrations/lsp/servers.go` (config, transform)

**Analog:** `internal/integrations/codeintel/parser.go` (language-specific configuration)

**Server definitions pattern:**
```go
type ServerConfig struct {
    Language    string
    Command     []string
    Extensions  []string
    Capabilities []string
}

var defaultServers = map[string]ServerConfig{
    "go": {
        Language:   "go",
        Command:    []string{"gopls", "serve"},
        Extensions: []string{".go"},
    },
    "typescript": {
        Language:   "typescript",
        Command:    []string{"typescript-language-server", "--stdio"},
        Extensions: []string{".ts", ".tsx", ".js", ".jsx"},
    },
    "python": {
        Language:   "python",
        Command:    []string{"pyright-langserver", "--stdio"},
        Extensions: []string{".py"},
    },
    "rust": {
        Language:   "rust",
        Command:    []string{"rust-analyzer"},
        Extensions: []string{".rs"},
    },
}
```

---

### `internal/integrations/archcheck/rules.go` (config, transform)

**Analog:** `internal/core/config/config.go` (config structure pattern)

**Architecture rules config:**
```go
type ArchRules struct {
    Layers              map[string][]string  `toml:"layers"`
    AllowedCrossLayer   []string            `toml:"allowed_cross_layer"`
    Severity            map[string]string   `toml:"severity"`
}

func LoadArchRules(configPath string) (*ArchRules, error) {
    // Load from TOML config
    // Validate: no circular layer references
    // Validate: path globs match at least one file
}
```

**Key pattern:** Follow the same `config.Load()` pattern used for main config (main.go line 701).

---

### `internal/integrations/archcheck/detector.go` (service, transform)

**Analog:** `internal/integrations/codeintel/graph.go` (graph traversal)

**Violation detection pattern:**
```go
type Violation struct {
    Type     string  // "forbidden_import", "circular_dep", "api_change"
    Severity string  // "error", "warning", "info"
    From     string  // source file
    To       string  // target file/layer
    Message  string
}

func DetectViolations(graph *CodeGraph, rules *ArchRules) []Violation {
    var violations []Violation
    // Check forbidden imports
    // Check layer boundary crossings
    // Check circular dependencies (via tarjan.go)
    // Check public API changes (vs prior release tag)
    return violations
}
```

---

### `internal/integrations/archcheck/tarjan.go` (utility, transform)

**No analog** -- new implementation using `looplab/tarjan` package.

**Pattern from research:**
```go
import "github.com/looplab/tarjan"

func DetectSCCs(graph *ImportGraph) [][]string {
    connections := make(map[interface{}][]interface{})
    for path, node := range graph.nodes {
        var edges []interface{}
        for _, imp := range node.Imports {
            edges = append(edges, imp)
        }
        connections[path] = edges
    }
    sccs := tarjan.Connections(connections)
    // Filter SCCs with size > 1 as circular dependencies
}
```

---

### `cmd/m31a/index.go` (CLI, request-response)

**Analog:** `cmd/m31a/main.go` (CLI command structure)

**CLI command pattern** (from main.go lines 346-424):
```go
func runIndex(args []string, workDir string, logger *slog.Logger) int {
    fs := flag.NewFlagSet("index", flag.ExitOnError)
    incremental := fs.Bool("incremental", false, "Incremental rebuild only")
    progress := fs.Bool("progress", false, "Emit JSON progress events")
    fs.Usage = func() {
        fmt.Fprintln(os.Stderr, "Usage: m31a index [--incremental] [--progress]")
        fmt.Fprintln(os.Stderr, "  --incremental  Only re-index changed files")
        fmt.Fprintln(os.Stderr, "  --progress     Emit JSON progress for scripting")
    }
    fs.Parse(args)

    // Create indexer, build with progress reporting
    // Return exit code
}
```

**Key pattern:** Same `flag.NewFlagSet` pattern used for `migrate` and `models list` commands. Same error printing to stderr.

---

### `cmd/m31a/impact.go` (CLI, request-response)

**Analog:** `cmd/m31a/main.go`

**Pattern:** Same as `index.go` above. Additional flags:
```go
fs := flag.NewFlagSet("impact", flag.ExitOnError)
depth := fs.Int("depth", 3, "Traversal depth")
format := fs.String("format", "table", "Output format: table, json, graphviz")
```

---

### `cmd/m31a/arch.go` (CLI, request-response)

**Analog:** `cmd/m31a/main.go`

**Pattern:** Same as `index.go` above. Additional flags:
```go
fs := flag.NewFlagSet("arch check", flag.ExitOnError)
configPath := fs.String("config", "arch.toml", "Architecture rules config file")
```

---

## Shared Patterns

### EventStore Integration
**Source:** `internal/core/types/event.go`
**Apply to:** `codeintel/events.go`, `codeintel/projection.go`
```go
// Event envelope (same as Phase 1)
type Event struct {
    ID        uuid.UUID       `json:"id"`
    Seq       int64           `json:"seq"`
    Type      EventType       `json:"type"`
    Timestamp time.Time       `json:"timestamp"`
    Payload   json.RawMessage `json:"payload"`
    Metadata  EventMetadata   `json:"metadata,omitempty"`
}
// Emit with source: "codeintel" in metadata
```

### Concurrency Pattern (Mutex + RLock/RLock)
**Source:** `internal/integrations/codeintel/codeintel.go` lines 22-28
**Apply to:** `lsp/pool.go`, `lsp/manager.go`, `codeintel/projection.go`
```go
mu      sync.RWMutex
graph   *ImportGraph
// RLock for reads, Lock for writes
idx.mu.RLock()
defer idx.mu.RUnlock()
```

### Error Wrapping
**Source:** `internal/integrations/codeintel/codeintel.go` line 69
**Apply to:** All new service files
```go
return fmt.Errorf("build graph: %w", err)
return fmt.Errorf("start LSP %s: %w", language, err)
```

### CLI Command Registration
**Source:** `cmd/m31a/main.go` lines 807-814
**Apply to:** `cmd/m31a/index.go`, `cmd/m31a/impact.go`, `cmd/m31a/arch.go`
```go
if flag.Arg(0) == "index" {
    return runIndex(flag.Args()[1:], workDir, logger)
}
```

### File Watching with Debounce
**Source:** `internal/ui/tui/filewatcher.go` lines 133-143
**Apply to:** `codeintel/watcher.go` (if file watching is in scope)
```go
func (fw *FileWatcher) debounceEvent() {
    if fw.debounce != nil {
        fw.debounce.Stop()
    }
    fw.debounce = time.AfterFunc(500*time.Millisecond, func() {
        select {
        case fw.Events <- IndexProgressMsg{}:
        case <-fw.ctx.Done():
        }
    })
}
```

### TUI Message Pattern
**Source:** `internal/ui/tui/tuitypes/tuitypes.go`
**Apply to:** `codeintel/progress.go` (IndexProgressMsg)
```go
type IndexProgressMsg struct {
    Phase      string  // "parsing", "indexing", "projecting"
    FilesDone  int
    TotalFiles int
    CurrentFile string
}
```

### Logging Pattern
**Source:** `internal/integrations/codeintel/graph.go` line 281
**Apply to:** All new files
```go
slog.Debug("BuildGraph: parse error, adding as isolated node",
    "file", job.relPath, "error", parseErr)
```

---

## No Analog Found

Files with no close match in the codebase (planner should use RESEARCH.md patterns instead):

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `internal/integrations/archcheck/tarjan.go` | utility | transform | No Tarjan/SCC algorithm exists in codebase |
| `internal/integrations/lsp/client.go` | service | request-response | No LSP client exists; closest is provider/base_client.go (HTTP, not stdio) |

---

## Metadata

**Analog search scope:** `internal/integrations/`, `internal/core/types/`, `cmd/m31a/`, `internal/ui/tui/`
**Files scanned:** 45+ Go files across 6 packages
**Pattern extraction date:** 2026-08-24
