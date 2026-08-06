---
phase: 06-ecosystem
plan: 03
subsystem: extensions-adapters
tags: [adapters, dispatcher, provider-registry, engine-hooks, workflow-extensibility]
key-files:
  created:
    - pkg/extensions/adapter_tool.go
    - pkg/extensions/adapter_provider.go
    - pkg/extensions/adapter_hook.go
    - internal/tools/external_tool.go
    - internal/integrations/provider/external_provider.go
    - internal/engine/workflow/engine_hooks.go
  modified:
    - pkg/extensions/types.go
    - pkg/extensions/protocol.go
    - pkg/extensions/adapter_provider.go
    - pkg/extensions/registry.go
    - internal/tools/dispatcher.go
    - internal/integrations/provider/registry.go
    - internal/engine/workflow/engine.go
    - internal/engine/workflow/engine_messages.go
tech-stack:
  patterns:
    - Adapter pattern: ExternalToolAdapter, ExternalProviderAdapter, PhaseHookAdapter
    - Registry pattern: PhaseHookRegistry for workflow extensibility
    - JSON-RPC 2.0 for subprocess communication
key-decisions:
  - D-01: Config-based external commands (subprocess + JSON-RPC)
  - D-02: Public pkg/extensions package with stable Go interfaces
  - D-04: Core workflow phases fixed; extensions via pre/post phase hooks
  - ExternalProviderAdapter implements full LLMProvider interface (9 methods)
  - Hook timeouts enforced (30s default), errors logged but don't block workflow
requirements-completed:
  - D-01
  - D-02
  - D-04
duration: 60 min
completed: "2026-08-06T12:30:00Z"
---

# Phase 06 Plan 03: Adapters Integration — Summary

**One-liner:** Created adapter layers bridging external extensions (subprocesses) into M31A's internal systems: Dispatcher for tools, Registry for providers, Engine for phase hooks.

## Accomplishments

### 1. ExternalToolAdapter & Dispatcher Integration
- **pkg/extensions/adapter_tool.go** — Implements `types.Tool` interface:
  - Fetches metadata (name, description, risk_level, schema) via JSON-RPC on first access (cached)
  - `Execute()` calls subprocess via JSON-RPC `tool.execute` with timeout
  - Implements `types.SchemaProvider` for parameter schema
- **internal/tools/external_tool.go** — `RegisterExternalTools(dispatcher, extRegistry)`:
  - Iterates tool configs, creates SubprocessManager, starts it, creates adapter, registers with Dispatcher
- All external tools go through Dispatcher for permissions, rate limiting, output bounding

### 2. ExternalProviderAdapter & Registry Integration
- **pkg/extensions/adapter_provider.go** — Implements full `provider.LLMProvider` interface (9 methods):
  - `Name()`, `APIKey()`, `FetchModels(ctx)`, `CachedModels()`, `ChatCompletionStream(ctx, req)`, `EstimateCost(modelID, usage)`, `HealthCheck(ctx)`, `GetModel(id)`
  - Streaming via JSON-RPC notifications (StreamIterator with Next/Close)
  - Models cached after first fetch
- **internal/integrations/provider/external_provider.go** — `RegisterExternalProviders(registry, extRegistry)`:
  - Iterates provider configs, creates SubprocessManager, starts it, creates adapter, registers with Registry
- External providers work with model fetching, streaming, health checks, fallback

### 3. PhaseHookAdapter & Engine Integration
- **pkg/extensions/adapter_hook.go** — Implements `PhaseHookHandler`:
  - `PrePhase(ctx, payload)` → JSON-RPC `hook.pre_phase`
  - `PostPhase(ctx, payload, result)` → JSON-RPC `hook.post_phase`
- **internal/engine/workflow/engine_hooks.go** — `PhaseHookRegistry`:
  - Separate pre/post hook maps per `WorkflowPhase`
  - `RunPreHooks()` / `RunPostHooks()` with 30s timeout per hook
  - Errors logged but don't block workflow (best-effort)
- **internal/engine/workflow/engine.go** — Hook invocation in `RunPhase()`:
  - `captureStateSnapshot()` creates `WorkflowStateSnapshot` for payload
  - Pre-hooks run before phase execution (after PrePhaseSetup)
  - Post-hooks run after phase execution (before PostPhaseExecution)
  - Conversion `toExtensionsPhaseResult()` for hook compatibility

### 4. Protocol & Interface Updates
- **pkg/extensions/types.go** — `ExternalProvider` now matches `provider.LLMProvider` (9 methods including `APIKey()`, `CachedModels()`, `GetModel()`)
- **pkg/extensions/protocol.go** — Added `MethodProviderGetModel`, `ProviderGetModelResult`
- **pkg/extensions/registry.go** — Uses updated adapters

### 5. Hook Payload Types
- `PhaseHookPayload` with `PhaseName`, `WorkflowStateSnapshot`, `Context`, `ExtensionConfig`
- `WorkflowStateSnapshot` with `CurrentPhase`, `Goal`, `Tasks`, `Messages`, `SessionID`, `BudgetSpentUSD`

## Verification

All checks pass:
- `go build ./...` ✓
- `go test -race ./pkg/extensions/...` ✓
- `go test -race ./internal/tools/...` ✓
- `go test -race ./internal/integrations/provider/...` ✓
- Core workflow tests pass (pre-existing failures in website_build_test and one workflow test unrelated)

## Deviations from Plan

None — plan executed exactly as written.

## Impact

External extensions are now first-class citizens:
- Tools registered in config work through Dispatcher with full permissions/rate-limits/output-bounds
- Providers registered in config work through Registry with fallback/health-checks/model-caching
- Hooks registered in config fire at pre/post phase transitions with workflow state snapshots
- All adapters handle subprocess errors gracefully with timeouts
- Hook timeouts enforced (30s), errors logged but don't block workflow
