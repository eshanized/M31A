# Phase 3: Internal Package Organization - Context

**Gathered:** 2026-07-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Reorganize `internal/tools/` (66 root files + 5 subdirs) and `internal/ui/tui/` (173 root files + 8 subdirs) into a professional directory structure with clear package boundaries, logical groupings, and minimal root-level clutter.

**Scope boundaries:**
IN: File moves, package reorganization, import path updates, test consolidation
OUT: New features, new tools, behavioral changes, API changes

</domain>

<decisions>
## Implementation Decisions

### Tools Root File Grouping
- **D-01:** Move ALL tool implementations to subdirectories — root only keeps core infrastructure (interface.go, dispatcher.go, defaults.go, constants.go, toolcall.go, tooldefs.go, tools_reexport.go)
- **D-02:** Group related tools into shared subdirectories: git/ (git.go), todo/ (todo.go + todoread.go), codeanalysis/ (codecomplexity.go + codemap.go), network/ (dns_cache.go + httpcheck.go)
- **D-03:** Single-file tools get their own subdir or join a logical group based on dependency analysis

### TUI Root File Grouping
- **D-04:** Group by responsibility, not by file prefix — create subdirs: core/, handlers/, screens/, repl/, config/
- **D-05:** Split app_*.go files by function into: core/ (app.go, app_state.go, app_view.go), handlers/ (app_handlers*.go), input/ (app_input*.go), update/ (app_update*.go), routing/ (app_routing.go, app_nav.go, app_screens.go)
- **D-06:** Move all screen model/view files (bisect_model.go, chathistory_model.go, dashboard_model.go, etc.) into their respective screens/ subdirectories
- **D-07:** Move repl_*.go files to screens/repl/ or a dedicated repl/ directory

### Import Path Disruption
- **D-08:** Ideal structure — move everything to its proper home regardless of import churn
- **D-09:** Use sed/goimports to fix all imports in one pass after reorganization
- **D-10:** 55 files import internal/tools, 223 files import internal/ui/tui — all will be updated

### Test File Placement
- **D-11:** Consolidate test files into a separate tests/ directory structure (breaks standard Go _test.go convention)
- **D-12:** Mirror the source directory structure in tests/ (tests/tools/git/, tests/tui/core/, etc.)
- **D-13:** Unit tests that need unexported access stay with source; integration/e2e tests move to tests/

### the agent's Discretion
- Agent has flexibility in determining exact subgroupings based on dependency analysis
- Agent can decide which tests need unexported access vs can be consolidated
- Agent can adjust timeout values and migration strategy based on risk assessment

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project Structure
- `.planning/PROJECT.md` — Project context, module path, architecture constraints
- `.planning/ROADMAP.md` — Phase 3 goal and scope
- `.planning/codebase/STRUCTURE.md` — 257-line directory analysis, module boundaries, naming conventions
- `.planning/codebase/CONVENTIONS.md` — Code conventions including error handling, patterns, naming

### Architecture Rules
- `AGENTS.md` — Build/test/lint commands, CGO_ENABLED=0 constraint, package boundaries
- `go.mod` — Module path, Go version, dependencies

### Existing Reorganization Plans
- `.planning/phases/01-repo-reorganization/01-CONTEXT.md` — Phase 1 decisions (6-layer architecture approved)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `internal/tools/defaults.go` — Tool registration pattern (18 tools registered)
- `internal/tools/dispatcher.go` — Central executor with permissions, rate limiting, concurrency
- `internal/tools/interface.go` — Tool interface definition (must stay at root)
- `internal/ui/tui/app.go` — Core AppState with Init/Update/View
- `internal/ui/tui/screens/` — Existing screen directory structure (30+ screens)

### Established Patterns
- Package naming: lowercase, singular (tools, tui)
- Files: snake_case.go convention
- Go imports: stdlib / third-party / project grouping
- Error wrapping: fmt.Errorf("%w", err)
- Tests: table-driven with t.Parallel()

### Integration Points
- `cmd/m31a/main.go` — Entry point imports internal/tools and internal/ui/tui
- `internal/workflow/engine.go` — Workflow engine uses tools via dispatcher
- `internal/types/` — Shared vocabulary (leaf package, no internal imports)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard Go project organization approaches

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 3-Internal Package Organization*
*Context gathered: 2026-07-23*
