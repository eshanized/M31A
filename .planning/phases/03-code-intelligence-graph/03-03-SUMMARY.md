---
phase: 03-code-intelligence-graph
plan: 03
subsystem: codeintel
tags: [impact-analysis, file-watching, incremental-indexing, code-graph]
dependency_graph:
  requires: [03-01]
  provides: [impact-analysis, file-watcher, incremental-rebuild]
  affects: [codeintel]
tech_stack:
  added:
    - github.com/fsnotify/fsnotify (already in go.mod)
  patterns:
    - event-driven incremental indexing
    - debounced file watching (500ms)
    - call graph traversal for impact analysis
    - risk categorization (API/runtime/test)
key_files:
  created:
    - internal/integrations/codeintel/impact.go
    - internal/integrations/codeintel/impact_test.go
    - internal/integrations/codeintel/watcher.go
    - internal/integrations/codeintel/watcher_test.go
  modified:
    - internal/integrations/codeintel/codeintel.go
decisions:
  - "Impact analysis uses call graph (CodeGraph.Callers/Callees) for direct/transitive callers, not just import graph"
  - "RiskCategory classifies symbols as API (exported), runtime (internal), test (test-only)"
  - "FileWatcher uses fsnotify with 500ms debounce, events delivered via channel"
  - "Indexer integrates FileWatcher for automatic incremental rebuild on file changes"
  - "NewIndexerWithWatcher constructor creates Indexer with integrated FileWatcher"
  - "BuildIncremental public method for manual incremental rebuilds (m31a index --incremental)"
metrics:
  duration: "~15 minutes"
  completed_date: "2026-08-24"
status: complete
actuals:
  tokens: 15421
  tasks: 3
  commits: 3
---

# Phase 03 Plan 03: Code Intelligence Graph - Impact Analysis & File Watching Summary

## One-Liner

Impact analysis with call graph traversal (direct callers, transitive dependents, affected tests, risk categories) and incremental file watching with 500ms fsnotify debounce, integrated with Indexer for automatic rebuild.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Build impact analysis with call graph traversal and risk categorization | eb061d4a | impact.go, impact_test.go |
| 2 | Build file watcher with fsnotify debounce for incremental indexing | 530142f5 | watcher.go, watcher_test.go |
| 3 | Integrate file watcher with Indexer for automatic incremental rebuild | 4d11b5d0 | codeintel.go, watcher.go |

## Verification Results

- `go build ./internal/integrations/codeintel/...` ✓
- `go test ./internal/integrations/codeintel/...` ✓ (all tests pass)
- `go vet ./internal/integrations/codeintel/...` ✓
- `grep -r "500.*Millisecond" internal/integrations/codeintel/watcher.go` → 1 match ✓

## Key Deliverables

### Impact Analysis (`impact.go`)

- **RiskCategory** type with constants: `RiskAPI`, `RiskRuntime`, `RiskTest`
- **CallerInfo** struct: File, Line, Symbol, Category
- **ImpactResult** struct: Symbol, DirectCallers[], Indirect[], AffectedTests[], Risk
- **AnalyzeImpact** function traverses CodeGraph.Callers() for direct callers, uses transitive call graph traversal for indirect dependents
- **CategorizeSymbolRisk** helper: API for exported symbols, Runtime for unexported, Test for test-only symbols
- Comprehensive unit tests covering all scenarios

### File Watcher (`watcher.go`)

- **WatcherEventType**: WatcherCreated, WatcherModified, WatcherDeleted
- **WatcherEvent** struct: Path, EventType, Timestamp
- **FileWatcher** struct with fsnotify watcher, debounce timer, events channel
- 500ms debounce using `time.AfterFunc`
- Events delivered via read-only channel for background consumption
- Recursive directory watching with ignored directory filtering
- Clean Start/Stop lifecycle with context control

### Indexer Integration (`codeintel.go`)

- Indexer struct extended with watcher, watchCtx, watchCancel fields
- **NewIndexerWithWatcher** creates Indexer with integrated FileWatcher
- **StartWatching(ctx)** launches background goroutine consuming watch events
- **handleWatchEvent** processes Created/Modified/Deleted events:
  - Modified/Created: parses file, updates graph/index, emits FileIndexed/SymbolDefined/ImportResolved/CallEdgeAdded events
  - Deleted: removes from graph/index, emits FileDeleted event
- **StopWatching** for clean shutdown
- **BuildIncremental** public method for manual incremental rebuilds
- Scorer and cache updated after each incremental change

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed indirect dependents logic in AnalyzeImpact**
- **Found during:** Task 1 testing
- **Issue:** Original implementation used import graph Downstream which returned wrong transitive dependents
- **Fix:** Changed to use call graph transitive callers (Callees traversal) for indirect dependents, plus import graph Downstream on symbol's definition file for import-based dependents
- **Files modified:** impact.go
- **Commit:** eb061d4a

**2. [Rule 3 - Blocking Issue] FileWatcher Start method missing**
- **Found during:** Task 3 integration
- **Issue:** FileWatcher auto-started in constructor but Indexer needed controlled Start/Stop lifecycle
- **Fix:** Added explicit Start(ctx) method, removed auto-start from NewFileWatcher, updated tests to call Start
- **Files modified:** watcher.go, watcher_test.go
- **Commit:** 530142f5, 4d11b5d0

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: symlink_escape | watcher.go | FileWatcher.AddDirectory should resolve symlinks and validate against workDir before adding to watch (mitigated by existing isIgnoredDir and path validation in walkAndAdd) |

## Known Stubs

None - all functionality implemented and tested.

## Test Coverage

- **TestAnalyzeImpact_DirectCallers**: Verifies direct callers with file:line and risk category
- **TestAnalyzeImpact_IndirectDependents**: Verifies transitive callers via call graph
- **TestAnalyzeImpact_AffectedTests**: Verifies test file detection (direct + transitive)
- **TestAnalyzeImpact_RiskCategories**: Verifies API/runtime/test classification
- **TestAnalyzeImpact_EmptySymbol**: Verifies unknown symbol handling
- **TestCategorizeSymbolRisk**: Verifies risk categorization helper
- **TestFileWatcher_DetectsCreate**: File creation triggers event
- **TestFileWatcher_DetectsModify**: File modification triggers event
- **TestFileWatcher_DetectsDelete**: File deletion triggers event
- **TestFileWatcher_Debounce**: Rapid changes produce debounced events
- **TestFileWatcher_Stop**: Clean shutdown without panic