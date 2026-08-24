---
phase: 03-code-intelligence-graph
plan: 01
subsystem: codeintel
tags: [code-intelligence, eventstore, graph, tree-sitter]
requires: [CODE-01, CODE-03]
provides: [codeintel-events, codegraph, projection]
affects: [internal/core/types/event.go, internal/integrations/codeintel/]
tech_stack:
  added: [github.com/google/uuid]
  patterns: [event-driven, embedding, nil-safe]
key_files:
  created:
    - internal/integrations/codeintel/events.go
    - internal/integrations/codeintel/events_test.go
  modified:
    - internal/core/types/event.go
    - internal/integrations/codeintel/graph.go
    - internal/integrations/codeintel/parser.go
    - internal/integrations/codeintel/codeintel.go
    - internal/integrations/codeintel/relevance.go
    - internal/integrations/codeintel/relevance_test.go
    - internal/integrations/codeintel/bench_test.go
decisions:
  - "EventStore is optional (nil-safe) for backward compatibility"
  - "Event payloads use json.RawMessage via json.Marshal"
  - "All codeintel events tagged with source: \"codeintel\" and schema_version: 1"
  - "CodeGraph embeds ImportGraph for API backward compatibility"
  - "Call sites extracted during parsing, edges added during graph build"
  - "Projection rebuilds from EventStore events with source filtering"
metrics:
  duration: "2026-08-24T16:38:54Z/2026-08-24T18:45:00Z"
  tasks: 3
  commits: 3
  files_changed: 9
  lines_added: 1026
  lines_removed: 17
status: complete
actuals:
  tokens: 62000
  tasks: 3
  commits: 3
---

# Phase 03 Plan 01: Code Intelligence Graph - EventStore Integration Summary

**One-liner:** Wired Tree-sitter codeintel indexer to EventStore via 8 event types and in-memory graph projection with call/inheritance/type hierarchy edges.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Define code intelligence event types and payloads | 37fdfc3d | event.go, events.go, events_test.go |
| 2 | Extend import graph with call, inheritance, type hierarchy edges | 32bc9a4a | graph.go, parser.go, codeintel.go, relevance.go, tests |
| 3 | Wire Indexer to EventStore and build CodeIntelProjection | a72e046d | codeintel.go |

## Key Deliverables

### 1. Event Types (`internal/core/types/event.go`)
Added 8 new `EventType` constants:
- `EventFileIndexed`, `EventSymbolDefined`, `EventImportResolved`
- `EventCallEdgeAdded`, `EventInheritanceEdgeAdded`, `EventTypeHierarchyEdgeAdded`
- `EventFileDeleted`, `EventSymbolRemoved`

All follow existing pattern with `EventMetadata{SchemaVersion: 1, Source: "codeintel"}`.

### 2. Event Payloads & Emitters (`internal/integrations/codeintel/events.go`)
- 8 payload structs with JSON tags: `FileIndexedPayload`, `SymbolDefinedPayload`, `ImportResolvedPayload`, `CallEdgeAddedPayload`, `InheritanceEdgeAddedPayload`, `TypeHierarchyEdgeAddedPayload`, `FileDeletedPayload`, `SymbolRemovedPayload`
- `CallSiteInfo` struct for parser-extracted call sites
- 8 `Emit*` helper functions (nil-safe — return nil if store is nil)
- All events tagged with `source: "codeintel"`, `schema_version: 1`

### 3. Extended Graph (`internal/integrations/codeintel/graph.go`)
- New types: `CallEdge`, `InheritanceEdge`, `TypeHierarchyEdge`
- `CodeGraph` struct embedding `*ImportGraph` with indexes:
  - `callIndex` (callee → edges), `callReverse` (caller → edges)
  - `inheritanceIndex` (child → edges)
  - `typeHierarchyEdges` slice
- Query methods: `Callers(name)`, `Callees(name)`, `Inheritance(child)`, `TypeHierarchy(subtype)`
- Mutators: `AddCallEdge`, `AddInheritanceEdge`, `AddTypeHierarchyEdge`
- `BuildGraph` now returns `*CodeGraph`

### 4. Parser Call Site Extraction (`internal/integrations/codeintel/parser.go`)
- `FileInfo.CallSites []CallSiteInfo` field added
- `walkAST` handles `call_expression`, `function_call`, `method_invocation` nodes
- `extractCallSite` finds enclosing function (caller) and callee name
- `findEnclosingFunction`, `extractCalleeName`, `extractFunctionName` helpers

### 5. Indexer EventStore Integration (`internal/integrations/codeintel/codeintel.go`)
- `Indexer.store types.EventStore` field
- `NewIndexerWithStore(workDir, store)` + `NewIndexer(workDir)` delegating with nil
- `buildFull()` emits events for all files/symbols/imports/call edges/inheritance edges
- `buildIncremental()` emits `FileDeleted` for deleted, `FileIndexed`+`SymbolDefined` for changed
- `CodeIntelProjection` with `RebuildFromEvents`, `ApplyEvent`, `Graph()`, `Files()`, `Symbols()`
- `Indexer.Projection()` returns new projection

## Verification Results

```
go build ./internal/integrations/codeintel/...     ✓
go test ./internal/integrations/codeintel/...      ✓ (all tests pass)
go vet ./internal/integrations/codeintel/...       ✓
```

**Tests passing:**
- `TestEmitFileIndexed` — mock store Append called with correct type/payload
- `TestEmitNilStore` — all 8 Emit* functions return nil with nil store
- `TestPayloadRoundTrip` — all 8 payload types survive JSON marshal/unmarshal
- `TestEmitEventMetadata` — all events have source="codeintel", schema_version=1
- All existing codeintel tests (graph, index, parser, relevance) pass

## Deviations from Plan

### Auto-fixed Issues (Rule 1/2)

1. **RelevanceScorer type change** — Updated to accept `*CodeGraph` instead of `*ImportGraph` since CodeGraph embeds ImportGraph and methods are promoted. Updated test and benchmark files accordingly.

2. **Missing `encoding/json` import** — Added to codeintel.go for `json.Unmarshal` in projection.

3. **buildFromCache graph type** — Changed to use `NewCodeGraph()` instead of `NewImportGraph()` for consistency.

## Backward Compatibility

- `NewIndexer(workDir)` unchanged — delegates to `NewIndexerWithStore(workDir, nil)`
- `CodeGraph` embeds `ImportGraph` — all existing methods (`Upstream`, `Downstream`, `Neighbors`, `AddNode`, `RemoveNode`, `NodeCount`, `AllPaths`) work without changes
- Existing call sites in `internal/engine/workflow/engine_tools.go`, `internal/tools/codeanalysis/codemap.go`, `internal/ui/tui/app_update_commands.go` compile without modification

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: path_traversal | parser.go:extractCallSite | File paths from AST not validated against workspace root — mitigated by existing `ValidateTaskFiles()` in callers |
| threat_flag: event_injection | codeintel.go:emitBuildEvents | EventStore.Append is only write path; source="codeintel" tag distinguishes from domain events |

## Known Stubs

1. **FileIndexedPayload.Hash** — Currently zero `[32]byte{}`; should compute actual file hash (xxhash/SHA256) in future
2. **SymbolDefinedPayload.Line** — Currently 0; SymbolInfo doesn't track line numbers
3. **CallEdgeAddedPayload.CalleeFile** — Empty for parser-extracted call sites; graph-level edges have full info
4. **TypeHierarchyEdgeAdded** — Emitted but no extraction logic yet (planned for LSP integration in Plan 02)
5. **SymbolRemoved for deleted files** — Only FileDeleted emitted; symbol removal requires tracking per-file symbols

## Next Steps (Plan 02+)

- LSP client integration for semantic call hierarchy and type hierarchy
- Incremental file watching with fsnotify
- Impact analysis using call graph traversal
- Architecture violation detection with Tarjan's SCC
- CLI commands: `m31a index`, `m31a impact`, `m31a arch check`