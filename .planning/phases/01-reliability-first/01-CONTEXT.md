# Phase 1: Reliability First - Context

**Gathered:** 2026-08-05
**Status:** Ready for planning

<domain>
## Phase Boundary

Build a foundation users can trust. Success is measured by correctness and predictability — not feature count. This phase covers workflow stability, concurrency safety, cancellation propagation, error handling, recovery, testing confidence, and architectural simplification.

</domain>

<decisions>
## Implementation Decisions

### Concurrency Architecture
- **D-01:** Restructure engine mutexes into focused structs with per-struct locking — **Reversibility:** costly — extracting state into new structs changes how all engine code accesses state; multiple call sites affected
- **D-02:** Maintain two-writer model (TUI Update() + workflow goroutine only) — **Reversibility:** reversible — adding more writers later is additive
- **D-03:** Document lock ordering hierarchy (transitionMu > planMu > messagesMu) and enforce via convention — **Reversibility:** reversible — can add runtime checks later
- **D-04:** All state reads happen inside Bubble Tea's Update(); no external query API — **Reversibility:** reversible — can add query API later if needed

### Cancellation Propagation
- **D-05:** Use context.Context as the primary cancellation mechanism, passed through all goroutines — **Reversibility:** reversible — stdlib pattern, easy to adopt
- **D-06:** Cancel LLM streaming immediately on user stop; discard partial content — **Reversibility:** reversible — can change to finish-chunk behavior later
- **D-07:** Kill process tree (SIGKILL to process group) for tool execution cancellation — **Reversibility:** reversible — can add graceful SIGTERM-first later
- **D-08:** Cancel all subagents when parent workflow is cancelled — **Reversibility:** reversible — can change to let-subagents-finish later

### Error Handling Standards
- **D-09:** Wrap all errors with fmt.Errorf("%w", err) and descriptive context — **Reversibility:** reversible — standard Go pattern
- **D-10:** Log retry attempts at debug level; user sees clean output — **Reversibility:** reversible — can change log level
- **D-11:** User-facing errors provide actionable guidance (what failed, why, what to do) — **Reversibility:** reversible — can refine messages
- **D-12:** Close errors (defer x.Close()) logged at debug level, not surfaced to users — **Reversibility:** reversible — can add stricter checking later

### Testing Confidence Strategy
- **D-13:** Use race detector + stress tests for concurrent code; high goroutine count tests in CI — **Reversibility:** reversible — can add more tests
- **D-14:** Integration-first testing; unit tests only for pure logic — **Reversibility:** reversible — can rebalance later
- **D-15:** Delete auto-generated coverage boost test files; replace with real integration tests — **Reversibility:** costly — may drop coverage % initially, requires writing replacement tests
- **D-16:** Prioritize filling known gaps (dispatcher edge cases, engine pause/resume, SSE parser) before new workflow tests — **Reversibility:** reversible — can reorder priorities

### the agent's Discretion
- Agent may choose appropriate file boundaries when splitting engine.go
- Agent may select specific stress test patterns and goroutine counts
- Agent may decide logging format/structure for debug-level retry logs

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Architecture & Codebase
- `.planning/codebase/ARCHITECTURE.md` — System overview, component responsibilities, data flow, anti-patterns
- `.planning/codebase/CONCERNS.md` — Tech debt, known bugs, test coverage gaps, fragile areas
- `.planning/codebase/STACK.md` — Technology stack, dependencies, platform requirements
- `.planning/codebase/TESTING.md` — Test framework, patterns, CI integration, coverage targets

### Key Source Files
- `internal/engine/workflow/engine.go` — Main workflow engine (1832 lines, 10+ mutexes — primary refactor target)
- `internal/engine/workflow/state_machine.go` — Phase transition state machine
- `internal/tools/dispatcher.go` — Tool execution dispatcher with permissions and rate limiting
- `internal/engine/taskrunner/runner.go` — Parallel task execution
- `internal/integrations/provider/sse.go` — SSE parser for LLM streaming
- `internal/tools/exec/devserver.go` — Dev server with port race concern

### Standards
- `AGENTS.md` — Build commands, code style, conventional commits, lint config

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `internal/infrastructure/retry/policy.go` — Existing retry policies; can be extended for cancellation-aware retries
- `internal/infrastructure/fileutil/atomic.go` — Atomic writes; useful for crash-safe state persistence
- `internal/ui/tui/app_channel.go` — MsgEmitter channel bridge; pattern for goroutine-to-TUI communication
- `tests/testutil/mocks/` — Mock implementations for tools, providers, dispatcher

### Established Patterns
- Bubble Tea Elm architecture: all state mutations through Update(), goroutines communicate via tea.Cmd/tea.Msg
- Sentinel errors + typed wrappers in `internal/core/errors/errors.go`
- Hand-written mocks (no mocking library) with t.Helper() and t.Cleanup()
- Table-driven tests with t.Parallel()

### Integration Points
- `cmd/m31a/main.go` — Entry point; flag parsing → config → provider registration → TUI
- `internal/ui/tui/app.go:Init()` — Session setup, screen routing
- `internal/tools/dispatcher.go:CallTool()` — Central tool execution hub
- `internal/engine/workflow/engine.go:RunPhase()` — Phase execution dispatch

</code_context>

<specifics>
## Specific Ideas

- Engine.go split should result in files no larger than ~500 lines each
- Lock ordering hierarchy should be documented in a single authoritative location (e.g., a doc comment at top of engine package)
- Stress tests should use at least 10 concurrent goroutines with 100+ iterations
- Integration tests should exercise real LLM provider calls (with mock providers, not real APIs)

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 1-Reliability First*
*Context gathered: 2026-08-05*
