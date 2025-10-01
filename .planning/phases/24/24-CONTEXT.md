# Phase 24: TUI Redesign — Context & Decisions

**Gathered:** 2026-06-05
**Status:** Ready for planning
**Source:** TUI Redesign Proposal (`rush/tui_redesign_proposal.md`)

---

<domain>
## Phase Boundary

This phase implements a **complete visual and interaction redesign** of all M31A TUI screens based on the TUI Redesign Proposal. The redesign covers:

- **10 existing screens**: FirstRun, REPL, ModelSelector, Settings, Resume, Plan, Execute, Verify, Ship, Diff
- **6 new proposed screens**: Additional screens from the proposal
- **Shared chrome components**: Header, StatusBar, Sidebar, CommandPalette, PermissionModal
- **Theme system**: Enhanced with new visual language, box drawing strategy, and color tokens

The redesign maintains architectural constraints:
- Bubble Tea single-threaded model
- `lipgloss` styling only (no CSS animations)
- charmbracelet component ecosystem (bubbles, glamour)
- CGO_ENABLED=0 static binary
</domain>

---

<decisions>
## Implementation Decisions

### Visual Language (Locked)
- **Brand Color**: #7C3AED (violet) — primary accent
- **Accent**: #06B6D4 (cyan)
- **Success**: #10B981 (emerald)
- **Warning**: #F59E0B (amber)
- **Error**: #EF4444 (red)
- **Surface**: #1E1E2E (deep navy)
- **Box Drawing**: Rounded borders (`╭─╮│╰─╯`) for panels, double borders (`╔═╗║╚═╝`) for modals, half-blocks (`▀▄`) for progress, braille (`⣀⣠⣤⣶⣾⣿`) for sparklines, block elements (`█▓▒░`) for density

### Screen Redesigns (Locked - from proposal)

1. **ScreenFirstRun → "Launchpad"**: Full-terminal galaxy metaphor with star field, 2×2 feature cards (30+ cols each), CTA box with SurfaceElevated background, inline keyboard shortcuts

2. **ScreenREPL → "Mission Control"**: Chat bubbles with role-colored gutters, timestamp bars (`┤ HH:MM ├`), double-border tool cards (`╔═╗`), inline git status strip, redesigned input frame with model context line, scroll indicator (▼)

3. **ScreenModelSelector → "Model Observatory"**: Split-pane with cost sparklines (braille/block), capability badges (Thinking/Vision/Function Calling), horizontal bar charts for cost comparison, detail pane on Tab, ★ favorites

4. **ScreenSettings → "Control Tower"**: Icon tabs (⚙ 🔑 🤖 🛡), two-column layout (fields + description pane), inline dropdowns for enums, unsaved indicator (● N unsaved), active tab underline (━━━)

5. **ScreenResume → "Session Vault"**: Timeline view with date groupings (TODAY/YESTERDAY), session cards with rounded borders, phase badge with progress indicator, ● active dot, rich preview pane on wide terminals

6. **ScreenPlan → "Blueprint"**: Kanban-style with timeline, selected task detail box (double-border), file impact section with line estimates, action badges (✦ NEW, ~ MOD, ✗ DEL), horizontal dependency graph (Tab), arbitrage panel (O key)

7. **ScreenExecute → "Mission Live"**: Live metrics bar (elapsed/tasks/tool calls/cost/ctx), running task panel with streaming tool cards, compact task list with elapsed time and tool call count (12✦), block indicators

8. **ScreenVerify → "QA Gate"**: Per-task result panels with diff preview, self-heal status with attempts remaining, expandable error details, H/S keys for heal/skip

9. **ScreenShip**: Commit review with summary (follows same pattern)

10. **ScreenDiff**: Enhanced diff rendering with syntax highlighting

### Shared Chrome (Locked)
- **Header**: FNV cache retained, enhanced with connection status, context bar, model badge
- **StatusBar**: Enhanced content density
- **Sidebar**: Configurable width (default 120, was 42 fixed)
- **CommandPalette**: Standard pattern
- **PermissionModal**: Enhanced with risk badge, timeout countdown, syntax highlighting

### Theme System (Locked)
- Existing 76+ color tokens in `theme/theme.go` and `theme/colors.go` are well-structured
- Both dark and light themes exist
- New visual language adds semantic color roles
- Auto mode detects terminal background via `termenv`

### Architecture Constraints (Locked)
- Bubble Tea single-threaded: ALL state mutations through Update() only
- No direct goroutine mutations of AppState — use tea.Cmd/tea.Msg
- 16ms frame budget for View()
- Streaming via tea.Cmd with StreamChunkMsg
- Health checks via background goroutine emitting HealthUpdateMsg

### the agent's Discretion
- Exact component file structure (new files vs. refactoring existing)
- Animation frame rates for spinners (proposal suggests 10fps)
- Specific braille/block character choices for sparklines
- Migration strategy: incremental per-screen vs. big bang
- Backward compatibility with existing theme config
- Performance optimization for 80-220 column range
- Accessibility considerations (color blindness, screen readers)
</decisions>

---

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Core Architecture
- `/home/snigdha/Desktop/Helix/M31A/docs/ARCHITECTURE.md` — Package dependency graph, data flow, phase lifecycle
- `/home/snigdha/Desktop/Helix/M31A/docs/INTERFACES.md` — All Go interface and type definitions
- `/home/snigdha/Desktop/Helix/M31A/docs/TYPES.md` — Environment variables, constants, sentinel errors, enum types
- `/home/snigdha/Desktop/Helix/M31A/AGENTS.md` — Project overview, build/test commands, architecture rules, package layout

### TUI Codebase (Existing)
- `/home/snigdha/Desktop/Helix/M31A/internal/tui/` — All TUI screens and components
- `/home/snigdha/Desktop/Helix/M31A/internal/tui/theme/` — Theme system (theme.go, colors.go)
- `/home/snigdha/Desktop/Helix/M31A/internal/tui/screens/` — Screen implementations
- `/home/snigdha/Desktop/Helix/M31A/internal/tui/components/` — Reusable components

### Design Proposal (Source of Truth)
- `/home/snigdha/Desktop/Helix/M31A/rush/tui_redesign_proposal.md` — Complete redesign specification with mockups

### GSD Workflow References
- `/home/snigdha/.config/opencode/get-shit-done/references/ui-brand.md` — Visual patterns for GSD output
- `/home/snigdha/.config/opencode/get-shit-done/workflows/plan-phase.md` — This workflow
</canonical_refs>

---

<specifics>
## Specific Ideas from Proposal

### Key Visual Patterns to Implement
1. **Message gutter** — Left border line (`│`) with role label forming "speech margin"
2. **Timestamp bars** — `┤ HH:MM ├───────` separators between conversation turns
3. **Double-border tool cards** — `╔═╗` distinct from panel borders (`╭─╮`)
4. **Sparklines** — Braille patterns (`⣀⣠⣤⣶⣾⣿`) for usage frequency
5. **Cost bars** — Horizontal block charts (`████░░░░`) for input/output cost
6. **Phase badges** — Progress-ish indicators (EXECUTE = ██████░░ ~66%)
7. **Date groupings** — TODAY/YESTERDAY/JUN 03 section headers
8. **Action badges** — `✦ NEW`, `~ MOD`, `✗ DEL` with color coding
9. **Horizontal dependency graph** — Left-to-right flow with `●───` `└──●` edges
10. **Live metrics bar** — Dense single-line: `Elapsed: 2m 14s │ 3/8 tasks │ 47 tool calls │ $0.08 │ 89K ctx`

### Keyboard Shortcuts (Discoverable)
- All screens: Key hints always visible in footer/status
- REPL: `T` = toggle thinking, `Enter` = send, `Ctrl+C` = cancel
- ModelSelector: `Tab` = detail, `F` = favorite, `P` = filter, `/` = search
- Plan: `A` = accept, `E` = edit, `R` = retry, `D` = diff, `Tab` = graph, `O` = optimize
- Execute: `P` = pause, `S` = skip, `↑↓` = navigate
- Verify: `H` = heal, `S` = skip, `Esc` = back
- Settings: `Ctrl+S` = save, `Tab`/`Shift+Tab` = tabs, `Esc` = discard
- Resume: `Enter` = open, `N` = new, `D` = delete, `/` = search, `Esc` = back
</specifics>

---

<deferred>
## Deferred Ideas (Out of Scope for This Phase)

- **Vision support** — No terminal UX for image input/output (CapFlags.Vision exists but unused)
- **Voice interaction** — Terminal not suited for audio
- **Multi-modal outputs** — Terminal cannot render images/video
- **Plugin system** — Adds complexity to core
- **Team collaboration** — Requires server component
- **Ghost mode, PiP, Concurrent subagents** — V1.1 features (Phase 9+)
- **CSS-style animations** — Prohibited by architecture (Bubble Tea uses Unicode spinners + frame redraws)
- **Sub-pixel rendering** — Not applicable to terminal
</deferred>

---

*Phase: 24-tui-redesign*
*Context gathered: 2026-06-05 via TUI Redesign Proposal*