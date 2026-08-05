# Phase 2: User Experience - Context

**Gathered:** 2026-08-05
**Status:** Ready for planning

<domain>
## Phase Boundary

Make M31A feel effortless. Users should always understand what M31A is doing, why it is doing it, and what happens next. This phase covers progress visibility, plan presentation, permission prompts, and first-run onboarding.

</domain>

<decisions>
## Implementation Decisions

### Progress Display
- **D-01:** Use a persistent status bar at the bottom of the screen — **Reversibility:** reversible — can change layout later
- **D-02:** Status bar shows current phase name, active task, and elapsed time — **Reversibility:** reversible — can add/remove fields
- **D-03:** Status bar shows only current operation, not completed tasks — **Reversibility:** reversible — can add completed count later
- **D-04:** For concurrent operations, show only the primary/most important one — **Reversibility:** reversible — can add cycling or stacking later

### Plan Presentation
- **D-05:** Display plans as collapsible sections with task groups — **Reversibility:** reversible — can change layout
- **D-06:** Allow inline editing of plan tasks before execution — **Reversibility:** reversible — can make read-only
- **D-07:** Highlight current task during execution while showing full plan — **Reversibility:** reversible — can change display mode
- **D-08:** Show rough time estimates per task (e.g., "~5 min") — **Reversibility:** reversible — can remove estimates

### Permission Prompts
- **D-09:** Use modal dialog for permission requests (center-screen, requires response) — **Reversibility:** reversible — can change to inline/sidebar
- **D-10:** Show tool name and risk level in permission prompt — **Reversibility:** reversible — can add more details
- **D-11:** Allow batch approval of multiple pending tool calls — **Reversibility:** reversible — can remove batch feature
- **D-12:** Auto-approve safe tools (FileRead, Glob, Grep) without prompt — **Reversibility:** reversible — can require approval for all

### First-Run Experience
- **D-13:** Interactive tutorial with full feature tour — **Reversibility:** reversible — can simplify to quick-start
- **D-14:** Tutorial is always skippable at any point — **Reversibility:** reversible — can make mandatory
- **D-15:** API key setup integrated into tutorial flow — **Reversibility:** reversible — can separate into config step

### the agent's Discretion
- Agent may choose status bar styling (colors, symbols, spacing)
- Agent may design collapsible section animation/transition
- Agent may select modal dialog positioning and animation
- Agent may design tutorial step ordering and content

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Architecture & Codebase
- `.planning/codebase/ARCHITECTURE.md` — System overview, component responsibilities, TUI layer structure
- `.planning/codebase/CONCERNS.md` — Tech debt, known issues, UI-related concerns
- `.planning/codebase/STACK.md` — Technology stack, Bubble Tea version, dependencies

### Key Source Files
- `internal/ui/tui/app.go` — Top-level Bubble Tea model, Init/Update/View
- `internal/ui/tui/app_state.go` — AppState struct with all UI state
- `internal/ui/tui/app_view.go` — Main view rendering
- `internal/ui/tui/app_update.go` — Central message dispatch
- `internal/ui/tui/components/` — Reusable UI primitives (badge, card, progress, etc.)
- `internal/ui/tui/layout/` — Header/footer chrome, responsive constraints
- `internal/ui/tui/streaming/` — LLM streaming to TUI bridge
- `internal/ui/tui/commands/` — Slash command definitions
- `internal/ui/tui/firstrun_model.go` — Existing first-run model
- `internal/engine/narrative/engine.go` — Event classification for progress display

### Standards
- `AGENTS.md` — Build commands, code style, conventional commits, lint config

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `internal/ui/tui/components/` — Existing UI primitives (badge, card, progress bars) that can be composed for status bar and plan display
- `internal/ui/tui/layout/` — Responsive layout system with header/footer chrome
- `internal/ui/tui/firstrun_model.go` — Existing first-run model to extend for tutorial
- `internal/engine/narrative/engine.go` — Event classification that can feed progress status
- `internal/ui/tui/streaming/` — Streaming bridge that can provide real-time progress updates

### Established Patterns
- Bubble Tea Elm architecture: all state mutations through Update(), goroutines communicate via tea.Cmd/tea.Msg
- Component composition: UI built from reusable primitives in components/ directory
- Modal dialogs: existing permission modal pattern in app_handlers_modal.go
- Screen routing: per-phase screen models with dedicated handlers

### Integration Points
- `internal/ui/tui/app_update.go:Update()` — Central dispatch for all UI messages
- `internal/ui/tui/app_view.go:View()` — Main render loop
- `internal/tools/dispatcher.go:CallTool()` — Permission check integration point
- `internal/engine/workflow/engine.go:RunPhase()` — Phase execution for progress tracking

</code_context>

<specifics>
## Specific Ideas

- Status bar should use existing theme colors for consistency
- Collapsible sections should support keyboard navigation (expand/collapse with arrow keys)
- Permission modals should show tool risk level with color coding (green=safe, yellow=caution, red=danger)
- Tutorial should use existing screen routing to show interactive examples
- Time estimates should be based on task complexity heuristics, not historical data

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 2-User Experience*
*Context gathered: 2026-08-05*
