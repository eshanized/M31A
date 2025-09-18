---
phase: 25
plan: 25-03
subsystem: tools
tags: [error-wrapping, parameter-schema, tool-renderers, ssrf, architecture-docs, constants]

# Dependency graph
requires:
  - phase: 25-02
    provides: Provider hardening fixes and tool-layer groundwork
provides:
  - ErrToolExecution sentinel wrapping for all tool error returns
  - ParameterSchema methods on Edit, WebFetch, TodoWrite, AskUserQuestion
  - RendererForTool cases for WebFetch and AskUserQuestion
  - Architecture violation CR-09 documented
  - DNS double-resolution fix in WebFetch
  - MaxFileSize constant replacing hardcoded 5MB
affects: [26-01, 26-02]

# Tech tracking
tech-stack:
  added: []
  patterns: [error-sentinel-wrapping, parameter-schema-method, tool-renderer-interface]

key-files:
  created:
    - internal/tools/tools_test.go
    - internal/tools/webfetch_test.go
  modified:
    - internal/tools/fileread.go
    - internal/tools/filewrite.go
    - internal/tools/todo.go
    - internal/tools/question.go
    - internal/tools/edit.go
    - internal/tools/webfetch.go
    - internal/tui/components/toolrenderers.go
    - internal/tui/components/special_renderers.go
    - internal/tui/components/toolcard_test.go
    - docs/ARCHITECTURE.md

key-decisions:
  - "Used m31errors alias for internal/errors import to avoid collision with stdlib errors"
  - "Kept resolveAndCheck function for redirect SSRF checks, removed only the pre-request call"
  - "WebFetch and AskUserQuestion renderers follow BaseRenderer embedding pattern"

patterns-established:
  - "Error wrapping: all tool errors use fmt.Errorf(\"%w: %v\", m31errors.ErrToolExecution, err)"
  - "ParameterSchema: JSON Schema returned as string literal from tool method"
  - "Tool renderers: embed BaseRenderer, implement RenderInput/RenderOutput"

requirements-completed: [WIRE-03, WIRE-05]

# Metrics
duration: 11min
completed: 2026-06-06
---

# Phase 25 Plan 03: Tool/Permission Surface Fixes Summary

**ErrToolExecution sentinel wrapping across all tools, ParameterSchema methods on 4 tools, WebFetch/AskUserQuestion renderer coverage, architecture violation documentation, and DNS deduplication**

## Performance

- **Duration:** 11 min
- **Started:** 2026-06-06T02:40:05Z
- **Completed:** 2026-06-06T02:51:41Z
- **Tasks:** 7
- **Files modified:** 11

## Accomplishments
- Wrapped all error returns in FileRead, FileWrite, TodoWrite, and AskUserQuestion with ErrToolExecution sentinel
- Added ParameterSchema() methods to Edit, WebFetch, TodoWrite, and AskUserQuestion tools
- Added RendererForTool cases for WebFetch and AskUserQuestion with full renderer implementations
- Documented architecture violation CR-09 in docs/ARCHITECTURE.md
- Fixed DNS double-resolution in WebFetch by removing redundant resolveAndCheck call
- Replaced hardcoded 5MB with types.MaxFileSize constant in WebFetch

## Task Commits

Each task was committed atomically:

1. **Task W-20: FileRead/FileWrite Error Wrapping** - `5f59c9f` (fix)
2. **Task W-21: TodoWrite/AskUserQuestion Error Wrapping** - `f406b2b` (fix)
3. **Task W-22: Add ParameterSchema to 4 Tools** - `49830c2` (feat)
4. **Task W-23: Add RendererForTool Cases** - `85da580` (feat)
5. **Task W-25: Document Architecture Violation** - `1ee4f55` (docs)
6. **Task W-32: Fix WebFetch DNS Double-Resolution** - `42cabdf` (fix)
7. **Task W-33: Replace Hardcoded 5MB with MaxFileSize** - `383e6cd` (fix)

**Previously completed (W-13, W-16):** `3598b89`, `5d34a0e`

## Files Created/Modified
- `internal/tools/fileread.go` - Wrap errors with ErrToolExecution sentinel
- `internal/tools/filewrite.go` - Wrap errors with ErrToolExecution sentinel
- `internal/tools/todo.go` - Wrap errors with ErrToolExecution, add ParameterSchema
- `internal/tools/question.go` - Wrap errors with ErrToolExecution, add ParameterSchema
- `internal/tools/edit.go` - Add ParameterSchema method
- `internal/tools/webfetch.go` - Add ParameterSchema, remove DNS double-resolution, use MaxFileSize
- `internal/tools/tools_test.go` - New test: TestParameterSchema_AllTools
- `internal/tools/webfetch_test.go` - New tests: SSRF protection tests
- `internal/tui/components/toolrenderers.go` - Add WebFetch and AskUserQuestion cases
- `internal/tui/components/special_renderers.go` - Add WebFetchRenderer and AskUserQuestionRenderer
- `internal/tui/components/toolcard_test.go` - Add TestRendererForTool_AllTools
- `docs/ARCHITECTURE.md` - Document CR-09 architecture violation

## Decisions Made
- Used `m31errors` alias for internal/errors import to avoid collision with stdlib errors
- Kept resolveAndCheck function for redirect SSRF checks, removed only the pre-request call
- WebFetch and AskUserQuestion renderers follow BaseRenderer embedding pattern

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All tool error returns now wrapped with ErrToolExecution sentinel
- All tools have ParameterSchema methods for introspection
- WebFetch and AskUserQuestion have proper TUI renderers
- Architecture violation CR-09 documented for Phase 26+ fix
- Ready for Phase 26 implementation

---
*Phase: 25*
*Completed: 2026-06-06*

## Self-Check: PASSED
