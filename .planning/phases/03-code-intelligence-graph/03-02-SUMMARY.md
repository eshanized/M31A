---
phase: 03-code-intelligence-graph
plan: 02
subsystem: lsp
tags: [lsp, json-rpc, semantic-analysis, connection-pool, lifecycle]
dependencies:
  requires: [03-01]
  provides: [lsp-client, lsp-pool, lsp-manager, lsp-capabilities, lsp-servers]
  affects: [code-intel, impact-analysis, arch-check]
tech_stack:
  added: []
  patterns: [json-rpc-2.0, connection-pooling, lazy-initialization, graceful-degradation]
key_files:
  created:
    - internal/integrations/lsp/client.go
    - internal/integrations/lsp/capabilities.go
    - internal/integrations/lsp/servers.go
    - internal/integrations/lsp/pool.go
    - internal/integrations/lsp/manager.go
    - internal/integrations/lsp/client_test.go
    - internal/integrations/lsp/pool_test.go
  modified: []
decisions:
  - "LSP communication uses JSON-RPC 2.0 over stdio (not HTTP) per LSP spec"
  - "Connection pool uses lazy initialization — servers start on first semantic query"
  - "5-minute idle timeout before shutting down unused connections"
  - "Crash recovery with single retry on broken pipe/process exit errors"
  - "Graceful degradation when language server binary not found — returns error, no panic"
  - "Server commands validated from config via ServerConfigForLanguage() — no arbitrary commands"
metrics:
  duration: "2h"
  completed: "2026-08-24T18:00:00Z"
status: complete
actuals:
  tokens: 52000
  tasks: 3
  commits: 1
---

# Phase 03 Plan 02: LSP Client Infrastructure Summary

## One-Liner

LSP client infrastructure with JSON-RPC 2.0 over stdio, connection pooling, and lifecycle management for Go, TypeScript, Python, and Rust language servers.

## What Was Built

This plan delivers the complete LSP client infrastructure for on-demand semantic analysis across 4 core languages. The implementation follows the hybrid approach (D-01) where Tree-sitter handles syntax parsing and LSP provides semantic analysis (definitions, references, call hierarchy, type hierarchy, hover).

### Files Created

| File | Purpose |
|------|---------|
| `client.go` | LSP client with JSON-RPC 2.0 over stdio, initialize handshake, and semantic query methods |
| `capabilities.go` | Capability negotiation parsing server capabilities from initialize response |
| `servers.go` | Default server configurations for 4 languages with binary validation |
| `pool.go` | Per-project per-language connection pool with 5-min idle timeout |
| `manager.go` | Lifecycle management with crash recovery, idle reaper, and graceful degradation |
| `client_test.go` | Unit tests for client initialization, capability negotiation, JSON-RPC format |
| `pool_test.go` | Unit tests for pool lazy start, reuse, idle shutdown, binary not found |

## Key Features Implemented

### LSP Client (`client.go`)
- **JSON-RPC 2.0 over stdio**: Proper request/response/notification handling with auto-incrementing IDs
- **Initialize handshake**: Sends `initialize` with processId, rootUri, and textDocument capabilities; parses server response; sends `initialized` notification
- **Semantic query methods**:
  - `Definition(file, line, col)` → `textDocument/definition`
  - `References(file, line, col, includeDeclaration)` → `textDocument/references`
  - `CallHierarchy(file, line, col)` → `textDocument/prepareCallHierarchy` + `callHierarchy/incomingCalls`
  - `TypeHierarchy(file, line, col)` → `textDocument/prepareTypeHierarchy` + `typeHierarchy/supertypes`
  - `Hover(file, line, col)` → `textDocument/hover`
- **Graceful shutdown**: Sends `shutdown` request, `exit` notification, kills process with timeout
- **Error handling**: Returns error (not panic) when binary not found

### Capability Negotiation (`capabilities.go`)
- `LSPCapabilities` struct with 6 boolean fields: Definition, References, CallHierarchy, TypeHierarchy, Hover, Completion
- `NegotiateCapabilities()` parses server capabilities JSON and sets fields based on presence of providers
- Handles boolean `true`, empty objects `{}`, and `null` values per LSP spec

### Server Configurations (`servers.go`)
- 4 language servers defined: Go (gopls), TypeScript (typescript-language-server), Python (pyright-langserver), Rust (rust-analyzer)
- `ServerConfigForLanguage()` returns validated config from default map
- `FindLanguageForFile()` maps extensions to languages
- `BinaryExists()` validates binary presence in PATH

### Connection Pool (`pool.go`)
- `LSPPool` with `map[projectRoot]map[language]*poolEntry` structure
- Lazy start: client created on first `GetClient()` call
- Reuse: same client returned for subsequent calls to same project/language
- `Touch()` updates last-used timestamp
- `ShutdownIdle()` closes clients idle > 5 minutes
- `ShutdownAll()` closes all clients
- `Stats()` returns connection counts per language/project

### Lifecycle Manager (`manager.go`)
- `NewLSPManager()` creates pool with 5-min idle TTL and loads server configs
- `EnsureStarted()` checks language support and binary existence, returns nil for unknown/missing (graceful degradation)
- `SemanticQuery()` executes callback with client, handles crash recovery:
  - Detects crash errors (broken pipe, connection reset, process exited, EOF)
  - Closes crashed client, removes from pool
  - Creates new client and retries once
- `StartIdleReaper()` runs `ShutdownIdle()` every minute in background goroutine
- `Shutdown()` closes all clients
- `LanguageForFile()` delegates to `FindLanguageForFile()`

## Verification Results

All automated verification checks pass:

```
go build ./internal/integrations/lsp/...           ✓ exits 0
go test ./internal/integrations/lsp/... -run TestLSPClient  ✓ passes
go test ./internal/integrations/lsp/... -run TestNegotiate  ✓ passes
go test ./internal/integrations/lsp/... -run TestPool       ✓ passes
go vet ./internal/integrations/lsp/...             ✓ exits 0
grep -r "exec.Command" internal/integrations/lsp/ | grep -v "_test.go" | wc -l  ✓ equals 1
```

### Test Coverage
- `TestNewLSPClient_CommandNotFound`: Nonexistent binary returns error, not panic
- `TestNegotiateCapabilities`: 5 test cases covering all/partial/empty/false/null capabilities
- `TestSendRequestJSON`: Verifies JSON-RPC 2.0 envelope format (jsonrpc, method, params, id)
- Serialization tests for Location, HoverInfo, CallHierarchyItem
- `TestPoolGetClient_LazyStart`: Client created on first call for missing binary
- `TestPoolGetClient_Reuse`: Same client returned for same project/language
- `TestPoolShutdownIdle`: Idle clients closed after TTL
- `TestPoolShutdownAll`: All clients closed
- `TestPoolBinaryNotFound`: Error message contains "not found in PATH"
- `TestSupportedLanguages/Extensions/FindLanguageForFile/ServerConfigForLanguage/BinaryExists`: Config validation

## Deviations from Plan

None — plan executed exactly as written. All must-haves satisfied:
- ✅ LSPClient communicates via JSON-RPC 2.0 over stdio
- ✅ Initialize handshake with rootUri and capabilities
- ✅ LSPPool manages per-project per-language connections with lazy start
- ✅ LSPPool shuts down idle connections after 5 minutes
- ✅ LSPManager auto-restarts crashed servers with retry
- ✅ Capability negotiation parses all 6 capability types
- ✅ 4 language server commands defined
- ✅ Graceful degradation when binary not found (returns error, no panic)
- ✅ No CGO required (pure Go stdlib implementation)
- ✅ Server commands validated from config map only

## Security Considerations

Per threat model:
- **T-03-03 (Elevation of Privilege)**: Mitigated — server commands only from `defaultServers` map via `ServerConfigForLanguage()`, no arbitrary command input
- **T-03-04 (DoS)**: Mitigated — crash recovery with single retry (exponential backoff could be added in future)
- **T-03-05 (Info Disclosure)**: Accepted — stderr captured in buffer, not exposed to user

## Integration Points

The LSP infrastructure integrates with the code intelligence graph:
- `LSPManager.SemanticQuery()` called by Indexer for on-demand semantic enrichment
- `LSPClient.Definition/References/CallHierarchy/TypeHierarchy/Hover` enrich Tree-sitter graph
- Results cached with TTL per D-03
- Pool's `GetClient()` provides lazy start per project per language per D-02/D-16
- Idle reaper implements 5-min shutdown per D-02/D-16

## Next Steps

Plan 03-03 will extend the codeintel package with:
- EventStore integration (FileIndexed, SymbolDefined, CallEdgeAdded events)
- Projection rebuild from events
- Impact analysis with call graph traversal
- Architecture violation detection (forbidden imports, circular deps, API changes)

## Self-Check: PASSED