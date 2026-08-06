# Phase 4: Performance - Context

**Gathered:** 2026-08-06
**Status:** Ready for planning

<domain>
## Phase Boundary

Make M31A feel instantaneous. This phase covers startup optimization (sub-500ms target), memory efficiency (sync.Pool for hot paths), parallel execution of indexing/analysis/verification, and comprehensive performance measurement with CI regression detection. Exit criteria: performance bottlenecks are measurable, documented, and continuously monitored.

</domain>

<decisions>
## Implementation Decisions

### Startup Optimization
- **D-01:** Target under 500ms from binary invocation to first interactive prompt — **Reversibility:** reversible — can relax target later
- **D-02:** Pure lazy loading — defer provider registration, config validation, and keychain lookup until first LLM call — **Reversibility:** reversible — can move init back to startup
- **D-03:** Config validation errors surface at first use, not at startup — **Reversibility:** reversible — can add eager validation
- **D-04:** No background warm-up goroutines — nothing loaded until needed — **Reversibility:** reversible — can add pre-fetching later

### Memory Strategy
- **D-05:** sync.Pool for hot paths (SSE parsing, tool dispatch, LLM streaming) — **Reversibility:** reversible — can remove pools
- **D-06:** Pooled SSE buffers via sync.Pool instead of 1MB per connection — **Reversibility:** reversible — can revert to per-connection allocation
- **D-07:** Shared buffer pool for dev servers with total memory limit — **Reversibility:** reversible — can revert to per-server buffers
- **D-08:** Add -memprofile to test suite first, identify top allocation sites, then optimize — **Reversibility:** reversible — can reorder to optimize first

### Parallelism Scope
- **D-09:** All operations (indexing, analysis, verification, independent tasks) get concurrent execution — **Reversibility:** reversible — can disable parallelism per operation
- **D-10:** Semaphore-based concurrency control, consistent with existing dispatcher.go pattern — **Reversibility:** reversible — can change to worker pool
- **D-11:** Reuse existing task runner (internal/engine/taskrunner/runner.go) for new parallel operations — **Reversibility:** reversible — can build separate concurrency
- **D-12:** Collect all errors from parallel operations, report together (not fail-fast) — **Reversibility:** reversible — can switch to fail-fast per operation

### Performance Measurement
- **D-13:** Both CI benchmarks for regression detection + pprof endpoints for deep investigation — **Reversibility:** reversible — can remove either
- **D-14:** Comprehensive benchmark coverage — all major operations: startup, streaming, dispatch, indexing, compaction, session resume — **Reversibility:** reversible — can reduce scope
- **D-15:** 50% regression threshold for CI benchmark failures (lenient, tolerates CI noise) — **Reversibility:** reversible — can tighten threshold
- **D-16:** pprof exposed via --debug flag (or M31A_DEBUG=true), zero overhead in production — **Reversibility:** reversible — can expose differently

### the agent's Discretion
- Agent may choose specific sync.Pool sizing and reset strategies per hot path
- Agent may select which operations to parallelize first within the task runner
- Agent may design benchmark function naming and organization
- Agent may choose pprof HTTP server port and shutdown behavior

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Architecture & Codebase
- `.planning/codebase/ARCHITECTURE.md` — System overview, component responsibilities, engine file organization, data flow
- `.planning/codebase/CONCERNS.md` — Known performance bottlenecks (CodeIntel 30s build, SSE 1MB buffer, rate limiter overhead, dev server log buffer)
- `.planning/codebase/STACK.md` — Go 1.25+, CGO_ENABLED=0, Bubble Tea, key dependencies

### Key Source Files
- `internal/integrations/provider/sse.go` — SSE parser with 1MB buffer allocation per connection
- `internal/tools/dispatcher.go` — Tool dispatcher with channel-based rate limiter and concurrency semaphore
- `internal/engine/taskrunner/runner.go` — Parallel task execution with dependency-aware scheduling
- `internal/tools/exec/devserver.go` — Dev server with 256KB ring buffer per server
- `internal/engine/workflow/engine.go` — Core workflow engine (post-decomposition)
- `internal/engine/workflow/engine_streaming.go` — LLM streaming with retry and token calibration
- `internal/engine/workflow/engine_checkpoint.go` — Checkpoint save/load, recovery, rollback
- `internal/integrations/codeintel/codeintel.go` — Code intelligence with 30s build timeout

### Standards
- `AGENTS.md` — Build commands, code style, conventional commits, lint config

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `internal/engine/taskrunner/runner.go` — Existing parallel task execution with dependency-aware scheduling; extend for new parallel operations
- `internal/tools/dispatcher.go` — Existing semaphore-based concurrency control pattern; replicate for new parallel operations
- `internal/infrastructure/retry/policy.go` — Existing retry policies; can be extended for performance-aware retries
- `internal/engine/workflow/engine_streaming.go` — Existing streaming with retry and token calibration; optimize with sync.Pool

### Established Patterns
- Bubble Tea Elm architecture: all state mutations through Update(), goroutines communicate via tea.Cmd/tea.Msg
- Semaphore-based concurrency control in dispatcher.go
- Channel-based message passing between workflow goroutine and TUI
- Sync.RWMutex for concurrent state access with read-heavy workloads

### Integration Points
- `cmd/m31a/main.go` — Entry point; startup optimization target (lazy-load providers, config, keychain)
- `internal/engine/workflow/engine.go:RunPhase()` — Phase execution; parallel operations hook in here
- `internal/tools/dispatcher.go:CallTool()` — Tool execution; sync.Pool optimization target
- `internal/integrations/provider/sse.go` — SSE parsing; pooled buffer optimization target
- `internal/tools/exec/devserver.go` — Dev server; shared buffer pool target

</code_context>

<specifics>
## Specific Ideas

- Startup optimization should be measurable with a benchmark that times from binary start to first TUI render
- sync.Pool objects should be properly reset (zeroed) before returning to pool to prevent stale data
- Task runner extension should preserve existing dependency-aware scheduling behavior
- pprof server should use a fixed port (e.g., 6060) with clear logging when started
- CI benchmarks should use `go test -bench=. -benchmem` with benchstat for comparison

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 4-Performance*
*Context gathered: 2026-08-06*
