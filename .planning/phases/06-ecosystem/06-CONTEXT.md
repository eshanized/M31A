# Phase 6: Ecosystem - Context

**Gathered:** 2026-08-06
**Status:** Ready for planning

<domain>
## Phase Boundary

Make M31A a platform rather than a standalone application. This phase covers: extension loading mechanism, stable plugin API surface, layered configuration profiles, workflow phase hooks, decentralized extension distribution, contribution workflow automation, comprehensive extension documentation, and CI/release/benchmark automation enhancements. Exit criteria: external contributors can extend M31A without modifying core components.
</domain>

<decisions>
## Implementation Decisions

### Extension Loading Mechanism
- **D-01:** Extensions are external commands registered via config file. M31A spawns them as subprocesses. No CGO, no WASM runtime, language-agnostic. — **Reversibility:** reversible — can add WASM support later if needed

### Plugin API Surface
- **D-02:** Create a new public `pkg/extensions` package with stable Go interfaces. External tools/providers implement these. Internal adapters bridge to existing Dispatcher/Registry. — **Reversibility:** costly — new public package establishes a published contract; undoing requires migration for any external adopters

### Configuration Profiles
- **D-03:** Multi-file layered config: global (`config.toml`), workspace/org (`.m31a/workspace.toml`), project (`m31a.json`), merged at load time with clear precedence (project > workspace > global > env vars). — **Reversibility:** reversible — can change merge order or add layers later

### Workflow Extensibility
- **D-04:** Core workflow phases (7) remain fixed. Extensions register handlers at pre/post phase hooks (e.g., post-plan, pre-execute). Hook points defined in engine, invoked via MsgEmitter. — **Reversibility:** reversible — can add more hook points or remove without breaking extensions

### Extension Distribution
- **D-05:** Decentralized — extensions are Go binaries/scripts distributed via GitHub Releases, Homebrew, Scoop, etc. M31A config points to local executable paths. No central registry. — **Reversibility:** reversible — can add registry later if ecosystem demands

### Contribution Workflow & Templates
- **D-06:** Full process docs + automation: issue triage labels, milestone planning, release checklist, semantic versioning guidelines, PR templates, automated checks (lint/test/build), required reviewers, branch protection. — **Reversibility:** reversible — can simplify workflow later

### Documentation & Sample Projects
- **D-07:** Comprehensive docs + tutorial + testing guide: API reference for pkg/extensions, extension authoring guide, 2-3 sample extensions (tool, provider, workflow hook), interactive tutorial, migration guide for internal-to-external patterns, extension testing patterns, FAQ. — **Reversibility:** reversible — can reduce scope based on feedback

### CI/Release/Benchmark Automation
- **D-08:** Full automation: GoReleaser enhancements, benchmark CI job with regression detection (benchstat), extension compatibility test matrix, automated changelog from conventional commits, release notes template, scheduled nightly builds, performance dashboards, automated dependency updates (Dependabot/Renovate), release candidate promotion workflow. — **Reversibility:** costly — infrastructure changes (nightly CI, dashboards) are significant to remove

### the agent's Discretion
- Agent may design the exact `pkg/extensions` interface signatures
- Agent may choose hook point names and payload structure
- Agent may select specific sample extensions to build
- Agent may design the multi-file config merge precedence rules
- Agent may choose benchmark suite and regression thresholds
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Architecture & Codebase
- `.planning/codebase/ARCHITECTURE.md` — System overview, component responsibilities, data flow, anti-patterns, engine file organization, entry points
- `.planning/codebase/STACK.md` — Technology stack, dependencies, platform requirements, Go 1.25, CGO_ENABLED=0
- `.planning/codebase/CONCERNS.md` — Tech debt, known bugs, test coverage gaps, fragile areas, security considerations, scaling limits
- `.planning/codebase/CONVENTIONS.md` — Coding conventions, file organization, concurrency, logging, interface boundaries, error handling, testing, commit messages
- `.planning/codebase/INTEGRATIONS.md` — LLM providers, web services, auth, monitoring, CI/CD, git integration, shell execution, code intelligence

### Key Source Files
- `internal/core/config/loader.go` — Config loading, validation, merge, hot-reload (extension point for multi-file config)
- `internal/tools/dispatcher.go` — Tool registration, permissions, rate limiting, concurrency control (extension point for tools)
- `internal/integrations/provider/registry.go` — LLM provider registration, active selection, fallback (extension point for providers)
- `internal/engine/workflow/engine.go` — Core workflow engine, RunPhase orchestration (extension point for phase hooks)
- `internal/engine/workflow/engine_concurrency.go` — Lock ordering hierarchy (must maintain for any new locks)
- `cmd/m31a/main.go` — CLI entry, flag parsing, config load, provider registration, TUI launch
- `.github/workflows/ci.yml` — CI pipeline: lint, test, security, build matrix, release
- `.goreleaser.yaml` — Release automation config (cross-compilation, packaging)

### Standards
- `AGENTS.md` — Build commands, code style, conventional commits, lint config

### Phase Context (Prior Decisions)
- `.planning/phases/01-reliability-first/01-CONTEXT.md` — Concurrency, cancellation, error handling, testing decisions
- `.planning/phases/02-user-experience/02-CONTEXT.md` — UI patterns, status bar, modals, first-run decisions
- `.planning/phases/03-engineering-excellence/03-CONTEXT.md` — Engine split, module boundaries, documentation, DX decisions
- `.planning/phases/04-performance/04-DISCUSSION-LOG.md` — Performance decisions (lazy loading, buffer pools, parallel indexing)
- `.planning/phases/05-05-intelligence/05-CONTEXT.md` — Plan generation, context management, model routing, verification decisions
</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `internal/core/config/loader.go` — Config loading with TOML parsing, env var override, hot-reload via fsnotify; extend for multi-file merge
- `internal/tools/dispatcher.go` — Tool registration via `RegisterTool()`, permission policies, rate limiting; adapter pattern for external tools
- `internal/integrations/provider/registry.go` — Provider registration via `Register()`, health checks, model caching; adapter pattern for external providers
- `internal/engine/workflow/` — Phase execution files (initialize.go, plan.go, execute.go, etc.); MsgEmitter for goroutine-to-TUI communication; add hook invocation points
- `.github/workflows/ci.yml` — Existing CI jobs (lint, test, security, build, release); extend with benchmark, nightly, compat matrix
- `.goreleaser.yaml` — Cross-platform builds (linux/darwin/windows x amd64/arm64), packaging (tar.gz, deb, rpm, apk, archlinux, scoop); extend for extension compatibility

### Established Patterns
- Go interfaces for extensibility: `types.Tool`, `LLMProvider`, `SchemaProvider` — new `pkg/extensions` follows same pattern
- TOML config with env var override and hot-reload — multi-file merge follows same loader
- GoReleaser for cross-platform releases — extension compat tests as additional matrix
- GitHub Actions for CI — new jobs for benchmarks, nightly, dependency updates
- Conventional commits (`feat:`, `fix:`, `docs:`, etc.) — automated changelog generation
- Bubble Tea Elm architecture — phase hooks invoked via MsgEmitter to maintain single-threaded UI contract
- Engine file splitting by concern — hook logic in new `engine_hooks.go` or similar

### Integration Points
- Config loader: add multi-file merge in `loader.go:Load()` before validation
- Dispatcher: add `RegisterExternalTool()` that wraps subprocess execution in `types.Tool` interface
- Provider registry: add `RegisterExternalProvider()` that wraps subprocess execution in `LLMProvider` interface
- Workflow engine: add `RegisterPhaseHook(phase, hookType, handler)` and invoke at phase transitions
- CI: add new workflow files for benchmarks, nightly, extension compat matrix
- GoReleaser: add extension test matrix to build verification
</code_context>

<specifics>
## Specific Ideas

- `pkg/extensions` interface design: `ExternalTool` with `Command`, `Args`, `Env`, `Schema()`; `ExternalProvider` with `Command`, `Args`, `Env`, `ChatCompletionStream()`
- Phase hook types: `PrePhaseHook`, `PostPhaseHook` with `PhaseName`, `WorkflowState` snapshot, `Context` for cancellation
- Config merge order: project (`m31a.json`) > workspace (`.m31a/workspace.toml`) > global (`~/.m31a/config.toml`) > env vars
- Sample extensions: 1) Custom linter tool, 2) Local LLM provider (Ollama), 3) Pre-commit hook for code style
- Benchmark suite: startup time, workflow execution time, tool dispatch latency, memory allocation
- Nightly CI: run full test suite + benchmarks + extension compat matrix against main branch
- Performance dashboard: GitHub Pages + benchstat JSON output for trend visualization
</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope
</deferred>

---

*Phase: 06-Ecosystem*
*Context gathered: 2026-08-06*