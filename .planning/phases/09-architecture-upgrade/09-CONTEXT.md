# Phase 9: Architecture Upgrade & Directory Restructuring - Context

**Gathered:** 2026-07-16
**Status:** Ready for planning

<domain>
## Phase Boundary

Reorganize the M31A codebase: split large packages into sub-packages, remove the `internal/types/` alias layer, move `pkg/` contents into `internal/`, and establish interface-driven boundaries between packages. This phase restructures existing code — it does not add new features or change behavior.

</domain>

<decisions>
## Implementation Decisions

### Package Splitting Strategy
- **D-01:** Conservative splitting — only split where there's a clear domain boundary. Do not restructure healthy code.
- **D-02:** `internal/tui/` → split screen-specific code into `internal/tui/screens/<name>/` (e.g., `screens/home/`, `screens/repl/`, `screens/settings/`). Keep shared concerns (routing, theme, layout) in `tui/` root.
- **D-03:** `internal/workflow/` → extract shared concerns into sub-packages: `workflow/engine/` for core orchestration, `workflow/phases/` for phase runners, `workflow/streaming/` for LLM streaming. Keep phase files named as-is.
- **D-04:** `internal/tools/` → group tools by domain: `tools/fileops/` (read/write/edit/delete/move), `tools/exec/` (bash/devserver), `tools/search/` (glob/grep/webfetch/websearch), `tools/ai/` (agent/question). Keep dispatcher at `tools/` root.

### Type Layering (internal vs pkg)
- **D-05:** Delete `internal/types/` entirely. All internal packages import `pkg/types/` directly. This is a breaking change for all import paths but removes unnecessary indirection.
- **D-06:** Audit `pkg/` for types only used internally and move them into `internal/`. Keep `pkg/` lean — only truly shared types stay.
- **D-07:** Domain-specific types live near their domain: TUI messages in `tui/tuitypes/`, workflow messages stay with workflow, tool types stay with tools. Do not centralize all message types into one package.

### pkg/ vs internal/ Boundary
- **D-08:** Move all `pkg/` contents into `internal/`. M31A has no external importers — the `pkg/` layer adds unnecessary indirection.
- **D-09:** Migration strategy: move `pkg/` as a subtree (e.g., `pkg/session/` → `internal/session/`). Keep package names the same to minimize import churn. Each package migrates to its logical home.
- **D-10:** Keep each package as a separate unit even if small (e.g., `coordinator/`, `retry/`). Clear boundaries over consolidation.

### Decomposition Approach
- **D-11:** Extract a clean `WorkflowEngine` interface that TUI imports. Implementation behind the interface. Reduces coupling — TUI depends on interface, not concrete type.
- **D-12:** Group TUI handler files by domain: `handlers/workflow.go` (phase transitions), `handlers/config.go` (settings), `handlers/navigation.go` (screen switching). Currently scattered across `app_handlers*.go` files.
- **D-13:** Define clear interfaces at package boundaries (e.g., `WorkflowRunner`, `ToolExecutor`, `ProviderRegistry`). Internal packages depend on interfaces, not concrete types.
- **D-14:** Use constructor injection for wiring — pass all dependencies through constructors. No singletons or package-level state. Combine with interface-driven boundaries.

### the agent's Discretion
- File naming conventions within new sub-packages
- Import ordering in reorganized files
- Whether to do the migration in one commit or incremental atomic commits
- Exact placement of borderline files (e.g., where does `narrative_emitter.go` belong after restructuring?)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Architecture & Structure
- `.planning/codebase/ARCHITECTURE.md` — current system design, layers, data flows, anti-patterns
- `.planning/codebase/STRUCTURE.md` — full directory layout, naming conventions, where to add new code
- `.planning/codebase/CONVENTIONS.md` — code style rules
- `AGENTS.md` — build/test/lint commands, architecture rules, dependency constraint (`pkg/` must NOT import `internal/`)

### Requirements
- `.planning/REQUIREMENTS.md` — NFR-4 (Maintainability): 75% coverage, gofmt-clean, golangci-lint, conventional commits

### Current Type System
- `pkg/types/types.go` — canonical type definitions (368 lines)
- `internal/types/types.go` — aliases to be removed (147 lines)
- `internal/tui/tuitypes/types.go` — TUI-specific types (to remain in place)

### Key Large Files to Decompose
- `internal/workflow/engine.go` — 1707 lines, core orchestration
- `internal/tui/app.go` — 777 lines, AppState definition
- `internal/tools/dispatcher.go` — ~14K lines, execution pipeline

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **Go module system**: Already enforces `pkg/` cannot import `internal/`. This constraint remains valid after restructuring.
- **Build tags**: Platform-specific files (`*_linux.go`, `*_darwin.go`, `*_windows.go`) already use build tags. New sub-packages should follow the same pattern.
- **`go:embed`**: Workflow engine embeds templates and prompts. After split, embed directives must stay with the package that reads them.
- **Existing sub-packages**: `tui/components/`, `tui/commands/`, `tui/layout/`, `tui/streaming/`, `tui/theme/`, `tui/tuitypes/` already exist — new screen sub-packages follow this pattern.

### Established Patterns
- **Screen routing**: `app_routing.go` maps Screen constants to model constructors. After split, routing logic stays in `tui/` root, models move to `screens/<name>/`.
- **Message passing**: TUI messages flow through `tea.Msg` channel → `Update()`. This pattern is不变 — only file locations change.
- **Tool registration**: `defaults.go` registers all tools in `DefaultDispatcher()`. After domain grouping, registration stays centralized but tool implementations move to sub-packages.
- **Provider embedding**: Each provider embeds `BaseClient` for shared HTTP/caching/SSE. Provider structure unchanged.

### Integration Points
- **`cmd/m31a/main.go`**: Entry point imports `internal/config`, `internal/provider`, `internal/tui`, `internal/tools`, `internal/workflow`, `pkg/session`, `pkg/keychain`. Import paths change after pkg/ → internal/ migration.
- **`internal/tui/app.go`**: Imports `internal/workflow`, `internal/tools`, `internal/types`, `pkg/session`, `pkg/metrics`, `pkg/arbitrage`. Import paths change.
- **`internal/workflow/engine.go`**: Imports `internal/provider`, `internal/tools`, `internal/tokens`, `internal/errors`, `internal/decision`, `internal/context`, `internal/codeintel`, `pkg/compaction`, `pkg/ledger`, `pkg/metrics`, `pkg/retry`, `pkg/session`. Import paths change.

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard Go restructuring approaches. The user wants cleaner code organization, not a rewrite.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 9-Architecture Upgrade & Directory Restructuring*
*Context gathered: 2026-07-16*
