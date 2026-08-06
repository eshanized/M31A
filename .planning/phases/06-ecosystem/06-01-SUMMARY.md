---
phase: 06-ecosystem
plan: 01
subsystem: extensions-core
tags: [pkg/extensions, json-rpc, subprocess, registry]
key-files:
  created:
    - pkg/extensions/types.go
    - pkg/extensions/protocol.go
    - pkg/extensions/subprocess.go
    - pkg/extensions/registry.go
    - pkg/extensions/config.go
    - pkg/extensions/adapter_tool.go
    - pkg/extensions/adapter_provider.go
    - pkg/extensions/adapter_hook.go
    - pkg/extensions/types_test.go
    - pkg/extensions/subprocess_test.go
  modified:
    - internal/core/config/types.go
    - internal/core/config/config_validate.go
tech-stack:
  added:
    - golang.org/x/perf/cmd/benchstat (dev dependency for benchmark CI)
  patterns:
    - Subprocess-based extension with JSON-RPC 2.0 over stdin/stdout
    - Adapter pattern: ExternalToolAdapter, ExternalProviderAdapter, PhaseHookAdapter
    - Registry pattern: ExtensionRegistry manages extension lifecycle
    - Config merge: 4-layer precedence (project > workspace > global > env)
key-decisions:
  - D-01: Config-based external commands (subprocess + JSON-RPC, no CGO/WASM)
  - D-02: Public pkg/extensions package with stable Go interfaces (costly reversibility)
  - D-03: Multi-file layered config with ExtensionsConfig section
  - Protocol version negotiation via handshake method
  - Subprocess lifecycle managed with sync.Once for Stop() idempotency
requirements-completed:
  - D-01
  - D-02
duration: 45 min
completed: "2026-08-06T11:30:00Z"
---

# Phase 06 Plan 01: pkg/extensions Foundation — Summary

**One-liner:** Created foundational pkg/extensions package with stable public interfaces, JSON-RPC 2.0 protocol, subprocess management, extension registry, and config integration.

## Accomplishments

### 1. Core Extension Types (pkg/extensions/types.go)
Defined public interfaces for external extensions:
- `ExternalTool` — Name, Description, RiskLevel, ParameterSchema, Execute
- `ExternalProvider` — Name, FetchModels, ChatCompletionStream, EstimateCost, HealthCheck
- `PhaseHookHandler` — PrePhase, PostPhase with PhaseHookPayload
- `WorkflowStateSnapshot` and `PhaseResult` for hook context

### 2. JSON-RPC 2.0 Protocol (pkg/extensions/protocol.go)
Complete protocol implementation with:
- Request/Response/Error/Notification types
- Method constants for tools, providers, hooks, and handshake
- Structured result types for each method (ToolExecuteResult, ProviderFetchModelsResult, etc.)
- HandshakeResult with protocol_version and supported_methods

### 3. Subprocess Manager (pkg/extensions/subprocess.go)
Safe subprocess lifecycle management:
- `Start()` — spawns process, opens pipes, starts reader goroutines, performs handshake
- `Call()` — async JSON-RPC request/response with channels and timeout
- `Stop()` — graceful shutdown with shutdown notification, context timeout, force kill on timeout
- `sync.Once` for idempotent Stop(), reader goroutines for stdout/stderr
- Protocol version verification (requires "1.0")

### 4. Extension Registry (pkg/extensions/registry.go)
Central registry managing all extensions:
- Loads from config (ExtensionsConfig), starts subprocesses, runs handshake
- Separate maps for tools, providers, pre-hooks, post-hooks
- Thread-safe with RWMutex
- `GetTool()`, `GetProvider()`, `GetHooks()` lookup methods
- `GetToolConfigs()`, `GetProviderConfigs()`, `GetHookConfigs()` for integrations

### 5. Extension Config (pkg/extensions/config.go)
Config structures matching RESEARCH.md Pattern 3:
- `ExtensionsConfig` with Tools, Providers, Hooks maps
- `ExternalToolConfig`, `ExternalProviderConfig`, `PhaseHookConfig` with Command, Args, Env, Timeout
- `PhaseHookConfig` includes Phases and HookTypes arrays
- `ParsedTimeout()` helpers with sensible defaults

### 6. Adapters (pkg/extensions/adapter_*.go)
Bridge external subprocesses to internal interfaces:
- **ExternalToolAdapter** — Implements types.Tool, fetches metadata on first access, executes via JSON-RPC
- **ExternalProviderAdapter** — Implements provider.LLMProvider, caches models, streaming via notifications
- **PhaseHookAdapter** — Implements PhaseHookHandler, invokes pre/post hooks via JSON-RPC

### 7. Config Integration (internal/core/config/)
- Added `Extensions ExtensionsConfig` field to Config struct with TOML/JSON tags
- Added `ExtensionsConfig`, `ExternalToolConfig`, `ExternalProviderConfig`, `PhaseHookConfig` types
- Added `validateExtensionsConfig()` with comprehensive validation:
  - Command required, timeout parsable, phases/hook_types valid
  - Added "extensions" to knownConfigKeys

### 8. Tests
- `types_test.go` — Interface contract tests
- `subprocess_test.go` — Tests SubprocessManager with echo script implementing JSON-RPC handshake + tool.name/execute

## Verification

All checks pass:
- `go build ./pkg/extensions/...` ✓
- `go test -race ./pkg/extensions/...` ✓ (3.4s)
- `go test -race ./internal/core/config/...` ✓ (6.4s)
- `go build ./...` ✓
- `go test ./...` — Core packages pass (pre-existing failures in website_build_test and one workflow test unrelated to this plan)

## Deviations from Plan

None — plan executed exactly as written.

## Impact

Establishes the extension platform foundation. All subsequent plans (02-06) build on this tracer slice:
- Plan 02: Config loader workspace/project layers
- Plan 03: Adapters → Dispatcher/Registry/Engine hooks
- Plan 04: Documentation + sample extensions
- Plan 05: CI/Release automation
- Plan 06: Contribution workflow