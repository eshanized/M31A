# Phase 4: Audit and remove unused/irrelevant code from M31A codebase - Context

**Gathered:** 2026-07-27
**Status:** Ready for planning

<domain>
## Phase Boundary

Identify and remove dead code, unreferenced packages, deprecated features, and anything outside M31A's scope as an AI-powered CLI agent with TUI and workflow engine. Conservative approach: only remove code that is clearly dead with no references.

</domain>

<decisions>
## Implementation Decisions

### Scope of 'Irrelevant' Definition
- **D-01:** Remove code outside M31A's core purpose: AI-powered CLI agent with TUI, workflow engine, and multi-provider LLM support
- **D-02:** Remove experimental/abandoned features — code that was started but never completed, or experiments that didn't work out
- **D-03:** Remove provider-specific bloat — code specific to one provider (OpenRouter/Zen/Nvidia) that could be simplified or removed if not essential

### Cleanup Aggressiveness
- **D-04:** Conservative approach — only remove code that is clearly dead (no references, no tests, no imports)
- **D-05:** Keep anything that might be used, even if not currently active

### Test File Handling
- **D-06:** Remove tests together with dead code — if the code is dead, its tests are also dead
- **D-07:** Single commit for all removals to keep history clean

### Agent's Discretion
- Exact methodology for identifying dead code (grep for references, go vet, static analysis)
- Whether to run `go mod tidy` to clean unused dependencies
- Order of removal (packages first, then functions, then files)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project Configuration
- `AGENTS.md` — Build commands, code style, conventional commits, gotchas
- `Makefile` — Build/test/lint targets (make check, make test, make lint)

### Architecture Documentation
- `.planning/codebase/ARCHITECTURE.md` — System overview, component responsibilities, data flow
- `.planning/codebase/STACK.md` — Technology stack, dependencies, platform requirements
- `.planning/codebase/STRUCTURE.md` — Package layout, key file locations
- `.planning/codebase/CONVENTIONS.md` — Go coding conventions, error handling, concurrency rules

### Prior Phase Context
- `.planning/phases/03-fix-remaining-ci-issues/03-CONTEXT.md` — Phase 3 decisions (lint fixes, test fixes, security upgrades)
- `.planning/phases/02-fix-ci-regressions/02-CONTEXT.md` — Phase 2 decisions (workflow transitions, bash security, config merge)
- `.planning/phases/01-audit-fixes/01-CONTEXT.md` — Phase 1 decisions (one commit per fix, strict literal patterns)

### Codebase Maps
- `.planning/codebase/TESTING.md` — Test framework, patterns, mock conventions

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `tests/testutil/mocks/` — MockProvider, MockTool with configurable responses
- `tests/testutil/builders/` — NewTestDispatcher, NewTestDispatcherWithConfig with auto-cleanup
- `tests/testutil/envtest.go` — RequireAPIKey, LoadTestDotEnv helpers

### Established Patterns
- Table-driven tests with `t.Run` subtests and `t.Parallel()`
- `t.TempDir()` for filesystem isolation, `t.Cleanup()` for teardown
- One commit per logical fix (Phase 1 D-04)

### Integration Points
- `cmd/m31a/main.go` — Entry point, flag parsing, config load
- `internal/tools/defaults.go` — Tool registration (18 built-in tools)
- `internal/integrations/provider/registry.go` — Provider registration (OpenRouter, Zen, Nvidia)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — cleanup is driven by codebase analysis. Each removal must have clear evidence of being dead code (no references, no tests, no imports).

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 4-Audit and remove unused/irrelevant code from M31A codebase*
*Context gathered: 2026-07-27*
