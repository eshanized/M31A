# Phase 3: Engineering Excellence - Context

**Gathered:** 2026-08-05
**Status:** Ready for planning

<domain>
## Phase Boundary

Reduce maintenance cost while improving extensibility. This phase strengthens module boundaries, standardizes internal APIs, eliminates technical debt, documents architecture, and improves developer experience. Exit criteria: new contributors can understand and modify the codebase confidently.

</domain>

<decisions>
## Implementation Decisions

### Refactoring Priority
- **D-01:** Start with engine.go split (1832 lines → focused structs) before documentation/DX work — **Reversibility:** costly — extracting state into new structs changes how all engine code accesses state; multiple call sites affected
- **D-02:** Focus on engine.go first because it unlocks safer concurrent changes later — **Reversibility:** reversible — can reorder priorities later

### Documentation Strategy
- **D-03:** Put architecture docs in `.planning/codebase/` (already has ARCHITECTURE.md, CONCERNS.md) — **Reversibility:** reversible — can move to in-code comments later
- **D-04:** Keep code clean, docs versioned with plans — **Reversibility:** reversible — can add doc comments later

### Module Boundary Rules
- **D-05:** Use convention + interface boundaries — Go module system for hard boundaries, interfaces for soft boundaries — **Reversibility:** reversible — can add stricter enforcement later
- **D-06:** Low overhead approach — no runtime DI container, no compile-time tooling — **Reversibility:** reversible — can add enforcement later

### Developer Experience Scope
- **D-07:** Implement all three DX improvements: debug logging, profiling setup, and release automation — **Reversibility:** reversible — can remove tools later
- **D-08:** Debug logging is most immediately useful for contributor productivity — **Reversibility:** reversible — can change format later

### the agent's Discretion
- Agent may choose specific file boundaries when splitting engine.go
- Agent may select documentation format within .planning/codebase/
- Agent may design debug logging format and profiling integration
- Agent may choose release automation tooling (goreleaser, Makefile targets, etc.)

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

### Standards
- `AGENTS.md` — Build commands, code style, conventional commits, lint config

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `.planning/codebase/ARCHITECTURE.md` — Already documents component responsibilities and patterns
- `.planning/codebase/CONCERNS.md` — Already identifies tech debt and fragile areas
- `internal/core/types/` — Shared vocabulary that breaks circular dependencies
- `internal/infrastructure/retry/policy.go` — Existing retry policies; can be extended

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
- Debug logging should use structured logging (slog) with consistent format across all modules
- Profiling setup should include pprof endpoints for CPU and memory profiling
- Release automation should use goreleaser for cross-platform builds

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 3-Engineering Excellence*
*Context gathered: 2026-08-05*
