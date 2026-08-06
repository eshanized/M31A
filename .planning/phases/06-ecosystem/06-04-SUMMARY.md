---
phase: 06-ecosystem
plan: 04
subsystem: extensions-documentation
tags: [documentation, samples, tutorials, api-reference]
key-files:
  created:
    - docs/extensions/api-reference.md
    - docs/extensions/authoring-guide.md
    - docs/extensions/tutorial.md
    - docs/extensions/testing-guide.md
    - docs/extensions/migration-guide.md
    - docs/extensions/faq.md
    - docs/examples/custom-linter/main.go
    - docs/examples/custom-linter/README.md
    - docs/examples/custom-linter/go.mod
    - docs/examples/ollama-provider/main.go
    - docs/examples/ollama-provider/README.md
    - docs/examples/ollama-provider/go.mod
    - docs/examples/pre-commit-hook/main.go
    - docs/examples/pre-commit-hook/README.md
    - docs/examples/pre-commit-hook/go.mod
tech-stack:
  patterns:
    - JSON-RPC 2.0 over stdin/stdout
    - Subprocess-based extension model
    - Language-agnostic (Go examples, but any language works)
key-decisions:
  - D-07: Comprehensive docs + tutorial + testing guide + 3 samples
  - All examples use stdlib only, CGO_ENABLED=0 compatible
  - Decentralized distribution via GitHub Releases/Homebrew/Scoop
requirements-completed:
  - D-07
duration: 90 min
completed: "2026-08-06T13:30:00Z"
---

# Phase 06 Plan 04: Documentation & Sample Extensions — Summary

**One-liner:** Created comprehensive documentation (6 files) and 3 working sample extensions demonstrating all extension types.

## Accomplishments

### 1. API Reference (docs/extensions/api-reference.md)
Complete reference for `pkg/extensions` public API:
- Interface definitions: `ExternalTool`, `ExternalProvider`, `PhaseHookHandler`
- Config structs: `ExtensionsConfig`, `ExternalToolConfig`, `ExternalProviderConfig`, `PhaseHookConfig`
- JSON-RPC 2.0 protocol: request/response types, method constants, handshake flow, error codes
- `SubprocessManager`: `Start()`, `Call()`, `Stop()` signatures
- `ExtensionRegistry`: constructor, `Start()`, `Stop()`, lookup methods
- Adapter constructors: `NewExternalToolAdapter`, `NewExternalProviderAdapter`, `NewPhaseHookAdapter`
- Config integration example for `m31a.json` / `config.toml`
- Version compatibility policy

### 2. Authoring Guide (docs/extensions/authoring-guide.md)
Step-by-step guide to build extensions:
- Prerequisites: Go 1.25+, JSON-RPC 2.0 understanding
- Extension structure: single executable, stdio communication
- Building a tool extension: implement 5 methods (name, description, risk_level, schema, execute)
- Building a provider extension: implement 6 methods (name, fetch_models, chat_completion_stream, estimate_cost, health_check)
- Building a hook extension: implement PrePhase/PostPhase
- JSON-RPC implementation patterns (request/response, notifications for streaming)
- Configuring in m31a.json / config.toml
- Testing locally: `m31a config show --extensions`, run extension manually
- Distribution: GitHub Releases, Homebrew formula, Scoop manifest
- Best practices: timeouts, error handling, logging to stderr, graceful shutdown

### 3. Interactive Tutorial (docs/extensions/tutorial.md)
End-to-end walkthrough (20 min):
- Step 1: Create "echo" tool extension (5 min) - basic JSON-RPC loop
- Step 2: Register in config, test with M31A (3 min)
- Step 3: Add parameters and schema (5 min) - repeat field
- Step 4: Build provider stub (5 min) - fake models
- Step 5: Build hook extension (5 min) - pre-execute style check
- Step 6: Package and distribute (2 min) - cross-platform builds, GitHub Releases
- Verification checklist and next steps

### 4. Testing Guide (docs/extensions/testing-guide.md)
Comprehensive testing patterns:
- Unit testing: mock `SubprocessManager`, test adapter logic
- Integration testing: spawn real extension subprocess, test JSON-RPC
- Test helpers: build extension binary for CI
- Contract testing: verify extension implements all required methods
- CI integration: GitHub Actions workflow template
- Debugging tips: run extension manually, inspect JSON-RPC traffic

### 5. Migration Guide (docs/extensions/migration-guide.md)
Converting internal to external:
- Tool migration: extract Execute() logic, wrap in JSON-RPC handler
- Provider migration: extract ChatCompletionStream, wrap in notifications
- Hook migration: identify phase transition points, extract logic
- Config changes: move from code registration to config declaration
- Testing parity: ensure external behaves identically to internal
- Rollback strategy: keep internal version during transition

### 6. FAQ (docs/extensions/faq.md)
Common questions answered:
- Why subprocess not WASM/Go plugins?
- Can extensions access M31A internals?
- How to handle streaming? (JSON-RPC notifications)
- Protocol versioning
- Performance overhead (~5-10ms per call)
- Can extensions call other extensions?
- How to debug?
- Config precedence (4 layers)
- Variable substitution
- Distribution (decentralized)
- Development best practices
- Security model

### 7. Sample Extensions (3 working examples)

#### docs/examples/custom-linter/
- Custom linter tool extension (Go)
- Implements `ExternalTool` interface via JSON-RPC
- Methods: `tool.name="custom-linter"`, `tool.description`, `tool.risk_level="safe"`, `tool.schema={path:string}`, `tool.execute=run golangci-lint`
- Config example for m31a.json
- Builds with `go build -o custom-linter .`

#### docs/examples/ollama-provider/
- Local LLM provider extension (Go)
- Implements `ExternalProvider` interface via JSON-RPC
- Methods: `provider.name="ollama"`, `provider.fetch_models` (calls Ollama /api/tags), `provider.chat_completion_stream` (streams from /api/chat via notifications), `provider.estimate_cost=0`, `provider.health_check=GET /api/version`
- Streaming via JSON-RPC notifications for chunks
- Config example with OLLAMA_HOST env var

#### docs/examples/pre-commit-hook/
- Workflow hook extension (Go)
- Implements `PhaseHookHandler` via JSON-RPC
- Methods: `hook.pre_phase` runs `gofmt -l` on staged Go files
- Registers for phases: `["execute", "verify"]`, hook_types: `["pre"]`
- Config example with 30s timeout

All examples:
- Use stdlib only (no external deps)
- Include proper error handling and logging to stderr
- Handle SIGTERM/SIGINT for graceful shutdown
- Build with `CGO_ENABLED=0` for consistency
- README includes build command, config snippet, manual test instructions

## Verification

- All 6 documentation files exist with substantial content (>50 lines each)
- All 3 sample extensions build successfully with `go build`
- Each responds to JSON-RPC handshake and method calls correctly
- Documentation enables new contributor to build first extension in <30 min

## Deviations from Plan

None — plan executed exactly as written.

## Impact

External contributors now have everything needed to build M31A extensions:
- Complete API reference
- Step-by-step authoring guide
- Interactive tutorial
- Testing patterns
- Migration path from internal
- FAQ for common issues
- 3 reference implementations (tool, provider, hook)