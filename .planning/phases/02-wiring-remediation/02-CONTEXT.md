# Phase 2: Wiring Remediation - Context

**Gathered:** 2026-07-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Fix all 129 wiring issues identified in Phase 1 by severity order (Critical → High → Medium → Low) and add missing test coverage to meet targets (75% overall, 90% for pkg/taskrunner, pkg/bisect, pkg/rollback). This is a remediation phase — no new features, no scope expansion.

</domain>

<decisions>
## Implementation Decisions

### Critical Fix Strategy (PS-01, PS-04)
- **D-01:** Session save errors must fail-fast. In `manager.go:SaveSession`, return error to caller. In `app.Shutdown`, if session save fails, log error and exit. Prevents silent data loss.
- **D-02:** Rollback must use save-then-restore pattern. Before git restore: save session to disk via existing `SaveSession`. After restore: reload session from disk via existing `LoadSession`. Session state preserved through rollback.

### Dead Code & Duplication (PK-02, DC-07, DC-08, DC-04)
- **D-03:** Consolidate keychain to `pkg/keychain` as canonical. Remove `internal/keychain`. Update all imports to use `pkg/keychain`. Single source of truth for keychain interface and implementations.
- **D-04:** Move `pkg/skills` and `pkg/arbitrage` to `internal/` rather than deleting. Preserve code but correct the layering — these are internal implementation details, not public API.
- **D-05:** Remove `CodeComplexity` tool registration from `defaults.go`. Tool is dead — Execute() never called, no consumer. Clean removal.

### Test Coverage Strategy (REMED-05)
- **D-06:** Use Go's standard `testing` package with `testify` for assertions. Already in go.mod, standard in Go ecosystem.
- **D-07:** Hybrid mocking approach: `httptest` for provider HTTP tests (mock HTTP servers), hand-written fakes/implementations for internal interfaces. Best of both worlds — providers get realistic HTTP mocking, internal interfaces get simple fakes.
- **D-08:** Enforce coverage targets in CI via `-coverprofile`. Fail builds below 75% overall or 90% for pkg/taskrunner, pkg/bisect, pkg/rollback. Catches regressions.

### Lifecycle & Cleanup (SW-04, SW-05, TL-06)
- **D-09:** Background workers tracked with both `context.Context` (cancellation signal) and `sync.WaitGroup` (completion tracking). Context cancelled on shutdown, WaitGroup waits with timeout for graceful exit.
- **D-10:** Config watcher stopped by storing its cancel function in AppState. Call cancel in `Shutdown()`. Clean and explicit lifecycle management.
- **D-11:** Agent tool changed from background to child (`main.go:393` false → true). Prevents recursive background spawning. Simplest fix for TL-06.

### the agent's Discretion
- Medium severity issues (34 items): Agent decides fix approach per issue — lifecycle patterns, config consolidation, permission defaults, oscillation guard behavior
- Low severity issues (85 items): Agent decides fix approach per issue — doc updates, dead field removal, View() purity, debounce timers
- Specific file-level implementation details for each fix

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Audit Findings
- `.planning/phases/01-wiring-audit/WIRING-AUDIT-REPORT.md` — Complete 129-issue audit with severity matrix, exact file locations, root cause analysis, and recommended fixes
- `.planning/phases/01-wiring-audit/01-01-SUMMARY.md` — Package wiring and startup trace findings
- `.planning/phases/01-wiring-audit/01-02-SUMMARY.md` — Runtime systems wiring findings
- `.planning/phases/01-wiring-audit/01-03-SUMMARY.md` — Tool/provider/workflow wiring findings

### Architecture & Conventions
- `.planning/codebase/ARCHITECTURE.md` — System architecture (note: claims 6 phases, code has 7 — update needed)
- `.planning/codebase/CONVENTIONS.md` — Code conventions and patterns
- `.planning/codebase/STACK.md` — Tech stack (note: claims Go 1.23, actual is 1.25.0 — update needed)
- `.planning/codebase/INTEGRATIONS.md` — Integration points (note: claims Anthropic provider, actual is OpenRouter/Zen/Nvidia — update needed)
- `.planning/codebase/CONCERNS.md` — Technical debt and risks

### Requirements & State
- `.planning/REQUIREMENTS.md` — REMED-01 through REMED-05 requirements
- `.planning/PROJECT.md` — Project context and constraints
- `.planning/STATE.md` — Current project state
- `AGENTS.md` — Build/test/lint commands, code style, architecture rules

### Project Rules
- `Makefile` — Build targets (make test, make lint, make check)
- `go.mod` — Go 1.25.0, CGO_ENABLED=0 constraint

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `pkg/keychain/` — Canonical keychain implementation to keep and expand
- `internal/session/manager.go` — SaveSession/LoadSession methods for D-02 rollback pattern
- `internal/tools/defaults.go` — Tool registration hub (remove CodeComplexity from here)
- `internal/tui/app.go` — AppState struct (add WaitGroup, context, cancel func here)
- `internal/tools/dispatcher.go` — Dispatcher with Stop() method (fix ticker leak)

### Established Patterns
- Bubble Tea Elm architecture: all state mutations through Update() only
- Provider pattern: 3 providers implement types.LLMProvider interface
- Tool pattern: 18 tools implement types.Tool interface, registered in defaults.go
- Lifecycle: components created in main.go, injected into AppState, cleaned up in Shutdown()

### Integration Points
- `cmd/m31a/main.go` — Entry point where all wiring happens (TL-06 fix location)
- `internal/tui/app.go:Shutdown()` — Central cleanup point (SW-04, SW-05 fixes)
- `internal/workflow/engine.go` — Engine lifecycle (WF-02, WF-06 fixes)
- `pkg/session/manager.go` — Session persistence (PS-01 fix location)

</code_context>

<specifics>
## Specific Ideas

- Follow severity order: Critical first, then High, then Medium, then Low
- Each fix should be atomic and independently verifiable
- Test coverage additions should happen alongside or immediately after each fix
- Documentation updates (doc drift) can be batched at the end

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 2-Wiring Remediation*
*Context gathered: 2026-07-10*
