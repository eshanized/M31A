# Phase 5: Codebase Maintainability — Split Large Files - Context

**Gathered:** 2026-07-15
**Status:** Ready for planning

<domain>
## Phase Boundary

Read all files in the codebase, identify files exceeding 200 lines, split them into smaller files by functionality, and verify the project builds and passes all tests after each split.

</domain>

<decisions>
## Implementation Decisions

### File Size Threshold
- **D-01:** Files exceeding 200 lines are considered "large" and should be split
- **D-02:** This threshold is moderate — catches files that are getting unwieldy but not yet critical

### Splitting Strategy
- **D-03:** Split files by functionality (separate concerns into focused files)
- **D-04:** This approach is most natural for Go packages (e.g., 'permissions.go', 'rate_limit.go' from dispatcher.go)

### Testing Approach
- **D-05:** Run full verification after each split: `make check` (fmt → tidy → vet → lint → test with race detector)
- **D-06:** This ensures nothing is broken and maintains code quality standards

### Priority Order
- **D-07:** Agent decides optimal order based on dependencies and risk
- **D-08:** This allows flexibility to prioritize files that are most critical or have the most dependencies

### Naming Conventions
- **D-09:** Use descriptive names based on content (e.g., 'tool_permission_handler.go')
- **D-10:** More explicit than snake_case, helps developers understand file purpose at a glance

### Commit Strategy
- **D-11:** One commit per file split
- **D-12:** Easy to review, easy to revert if something breaks

### Import Management
- **D-13:** Run goimports automatically after each split
- **D-14:** Standard Go tool, fixes imports and formatting in one step

### the agent's Discretion
- Agent has flexibility in deciding which files to split first based on dependencies and risk
- Agent can determine the optimal splitting approach per file based on code structure
- Agent decides commit granularity when batching related splits

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Codebase Structure
- `.planning/codebase/STRUCTURE.md` — Directory layout, file locations, naming conventions
- `.planning/codebase/ARCHITECTURE.md` — System overview, component responsibilities, architectural constraints

### Build and Test
- `Makefile` — Build, test, lint, cross-compile targets
- `.golangci.yml` — Linter configuration

### Code Quality
- `AGENTS.md` — Quick reference for agents, code style, conventional commits

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `make check` command: Full verification pipeline (fmt → tidy → vet → lint → test with race detector)
- `goimports` tool: Automatic import fixing and formatting
- Existing codebase maps: STRUCTURE.md, ARCHITECTURE.md, CONVENTIONS.md

### Established Patterns
- Snake_case.go file naming convention throughout codebase
- Co-located test files (*_test.go) with source files
- Go module boundary: pkg/ must NOT import internal/

### Integration Points
- `cmd/m31a/main.go`: CLI entry point — flag parsing, config load, provider registration
- `internal/tui/`: 164 files — likely candidates for splitting
- `internal/tools/`: 18 built-in tools — may have large dispatcher.go
- `internal/workflow/`: 7-phase engine — may have large engine.go

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard Go refactoring approaches.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 5-Codebase Maintainability — Split Large Files*
*Context gathered: 2026-07-15*
