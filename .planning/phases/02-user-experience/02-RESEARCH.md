# Phase 2: User Experience - Research

**Researched:** 2026-08-05
**Domain:** Terminal UI / User Experience
**Confidence:** HIGH

## Summary

M31A already has a sophisticated TUI built on Bubble Tea with a rich component library, narrative engine, responsive layout system, and permission modal. Phase 2 builds on this foundation to add four UX capabilities: a persistent status bar for progress visibility, collapsible plan presentation with inline editing, improved permission prompts with batch approval, and an interactive first-run tutorial.

The key insight is that most of the infrastructure already exists. The footer already shows operation status, the narrative engine already classifies workflow events, the permission modal already handles tool risk levels, and the first-run model already has a multi-step wizard. Phase 2 extends and refines these existing patterns rather than building from scratch.

**Primary recommendation:** Compose new UX from existing components (ProgressBar, Spinner, Badge, PermissionModal, TourModel) and existing patterns (narrative sidebar, footer operation display, screen routing) — do not introduce new frameworks or layout primitives.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Use a persistent status bar at the bottom of the screen — Reversibility: reversible
- **D-02:** Status bar shows current phase name, active task, and elapsed time — Reversibility: reversible
- **D-03:** Status bar shows only current operation, not completed tasks — Reversibility: reversible
- **D-04:** For concurrent operations, show only the primary/most important one — Reversibility: reversible
- **D-05:** Display plans as collapsible sections with task groups — Reversibility: reversible
- **D-06:** Allow inline editing of plan tasks before execution — Reversibility: reversible
- **D-07:** Highlight current task during execution while showing full plan — Reversibility: reversible
- **D-08:** Show rough time estimates per task (e.g., "~5 min") — Reversibility: reversible
- **D-09:** Use modal dialog for permission requests (center-screen, requires response) — Reversibility: reversible
- **D-10:** Show tool name and risk level in permission prompt — Reversibility: reversible
- **D-11:** Allow batch approval of multiple pending tool calls — Reversibility: reversible
- **D-12:** Auto-approve safe tools (FileRead, Glob, Grep) without prompt — Reversibility: reversible
- **D-13:** Interactive tutorial with full feature tour — Reversibility: reversible
- **D-14:** Tutorial is always skippable at any point — Reversibility: reversible
- **D-15:** API key setup integrated into tutorial flow — Reversibility: reversible

### the agent's Discretion
- Agent may choose status bar styling (colors, symbols, spacing)
- Agent may design collapsible section animation/transition
- Agent may select modal dialog positioning and animation
- Agent may design tutorial step ordering and content

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| UX-01 | Persistent status bar showing phase, task, elapsed time | Existing footer `BuildFooter()` already shows operation; narrative engine provides phase/task data; extend footer or add status bar component |
| UX-02 | Collapsible plan sections with task groups | `PlanModel` already groups tasks by waves; add fold/unfold state per wave section |
| UX-03 | Inline plan task editing before execution | `PlanModel` already has refine mode; extend to per-task edit mode |
| UX-04 | Current task highlighting during execution | `ExecuteModel` already tracks `currentTask` index; add visual emphasis |
| UX-05 | Permission modal with tool name + risk level | `PermissionModal` already shows tool name, risk via border color, and risk-level styles; extend with risk text label |
| UX-06 | Batch permission approval | `Dispatcher` already has `ApproveBatch()` and `BatchApproval` struct; wire batch queue display into modal |
| UX-07 | Auto-approve safe tools (FileRead, Glob, Grep) | `Dispatcher.checkPermission()` already has auto-approve logic; verify safe tool list matches D-12 |
| UX-08 | Interactive first-run tutorial | `TourModel` component exists; `FirstRunModel` has multi-step wizard; compose tutorial as post-setup tour step |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Status bar display | UI Layer (TUI) | Engine (narrative) | Footer renders in `layout/page.go`; narrative engine feeds phase/task data |
| Plan collapsible sections | UI Layer (TUI) | — | Pure view concern in `PlanModel` |
| Plan inline editing | UI Layer (TUI) | Engine (workflow) | Edit happens in TUI; refinement submits to workflow via `PlanRefineMsg` |
| Permission prompts | UI Layer (TUI) | Tools (dispatcher) | Modal renders in TUI; permission logic in `tools/dispatcher.go` |
| Batch approval | Tools (dispatcher) | UI Layer (TUI) | `BatchApproval` in dispatcher; queue display in permission modal |
| Auto-approve safe tools | Tools (dispatcher) | — | `checkPermission()` already handles this |
| First-run tutorial | UI Layer (TUI) | — | `TourModel` component + `FirstRunModel` orchestration |
| Progress tracking | Engine (workflow) | UI Layer (TUI) | Workflow emits task/tool msgs; TUI renders via `ExecuteModel` |

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Bubble Tea | v1.3.0 | TUI framework | Already used; Elm architecture enforces single-threaded state |
| Lipgloss | v1.1.0 | Terminal styling | Already used; SemanticStyles system provides consistent design |
| Bubbles | v0.20.0 | TUI components | Already used; viewport, textinput, spinner components |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| Glamour | v0.6.0 | Markdown rendering | Render plan markdown content in viewport |
| Chroma | v0.10.0 | Syntax highlighting | Highlight commands in permission modal |

### Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Terminal layout | Custom grid system | `layout.Box` + `layout.RenderPage` | Existing flex-based layout handles responsive breakpoints |
| Progress animation | Frame-by-frame rendering | `components.AnimatedProgressBar` | Already implements ramp animation + flash effects |
| Spinner animation | Custom frame cycling | `components.Spinner` | Already has braille frame set + tick rate |
| Badge rendering | String formatting | `components.SimpleBadge` + `components.Badge` | Consistent badge system with semantic types |
| Permission modal | Inline prompt | `components.PermissionModal` | Already handles risk levels, command highlighting, keybindings |
| Tour/wizard | Custom step manager | `components.TourModel` | Already implements step progression, progress bar, skip |
| Narrative display | Custom event parser | `narrative.Engine` + `NarrativeState` | Already classifies and groups events |
| Responsive layout | Fixed dimensions | `layout.Detect()` breakpoint system | Already handles 4 width breakpoints |

**Key insight:** The existing codebase has a comprehensive design system. Phase 2 should compose from these primitives, not replace them.

## Common Pitfalls

### Pitfall 1: Status Bar vs Footer Overlap
**What goes wrong:** Adding a separate status bar that duplicates information already shown in the footer operation line.
**Why it happens:** The footer already shows "thinking...", streaming, and phase status in the center zone. A new status bar could conflict.
**How to avoid:** Decision D-01 says "persistent status bar at bottom of screen." The footer IS the bottom of the screen. Extend the existing footer's center zone to show phase name + active task + elapsed time rather than adding a new layout element. The `FooterInfo.Operation` field already carries this data; enrich it.
**Warning signs:** Two different status indicators at the bottom of the screen showing the same information.

### Pitfall 2: Collapsible Sections Breaking Viewport Scrolling
**What goes wrong:** Adding fold/unfold state to plan sections causes the viewport to jump or lose scroll position.
**Why it happens:** The `viewport.Model` tracks absolute line positions. Folding sections changes content height mid-scroll.
**How to avoid:** When toggling a section, recompute viewport content and use `viewport.GotoTop()` or track the section-relative offset. The existing `PlanModel.initViewport()` pattern shows how to reinitialize after content changes.
**Warning signs:** Viewport jumps to top or bottom when pressing fold/unfold key.

### Pitfall 3: Permission Modal Blocking Workflow Execution
**What goes wrong:** Modal blocks the entire TUI while waiting for permission, but the workflow engine goroutine is also blocked waiting for the response.
**Why it happens:** `Dispatcher.CallTool()` sends a permission request and blocks on `pendingResponses`. The TUI must process the `PermissionRequestMsg` and call `ApprovePermission()` to unblock.
**How to avoid:** This already works correctly in the existing code. The permission listener (`permListenerCmd`) reads from `d.RequestCh()`, emits `PermissionRequestMsg`, the modal handles key events, and calls `d.ApprovePermission()`. Do not change this flow — only enhance the modal's visual content.
**Warning signs:** "Thinking..." spinning indefinitely after permission prompt appears.

### Pitfall 4: Auto-Approve Bypassing Safety
**What goes wrong:** Auto-approving "safe" tools (D-12) accidentally auto-approves a tool that should require permission in certain contexts.
**Why it happens:** `FileRead` is safe in normal contexts but could read sensitive files. `Grep` could search `/etc/passwd`.
**How to avoid:** The existing `checkPermission()` in `permissions.go` already has risk-level-based auto-approve. Verify the safe tool list (FileRead, Glob, Grep) against the existing `types.RiskSafe` classification. The existing system is correct — do not change risk classifications.
**Warning signs:** Users surprised by unapproved file reads of sensitive paths.

### Pitfall 5: Tutorial Blocking First-Run Completion
**What goes wrong:** Tutorial (D-13) is placed before API key setup (D-15), preventing users from completing setup.
**Why it happens:** Misordering the flow steps.
**How to avoid:** D-15 says "API key setup integrated into tutorial flow." The existing `FirstRunModel` flow is: Welcome → Provider Select → API Key → Model Pick → Done. The tutorial should appear AFTER setup completes, as a post-setup guided tour. The `FirstRunCompleteMsg` handler already transitions to the REPL; insert the tour as a screen between completion and REPL.
**Warning signs:** Users unable to use M31A because they're stuck in tutorial without API keys.

## Code Examples

### Status Bar: Extending the Footer

The footer already shows operation status. Extend `FooterInfo.Operation` to include phase name + active task + elapsed time:

```go
// Source: internal/ui/tui/layout/page.go:BuildFooter
// The footer center zone already renders info.Operation with a spinner.
// Enhance the Operation field in buildFooterInfo() to show richer status.

// In app_view.go:buildFooterInfo()
if m.workflowPhase != types.PhaseIdle && m.workflowPhase != "" {
    // D-02: Show current phase name + active task + elapsed time
    phase := string(m.workflowPhase)
    task := m.currentActiveTask() // extract from executeModel or sidebar
    elapsed := time.Since(m.workflowStartTime).Truncate(time.Second)
    info.Operation = fmt.Sprintf("%s · %s · %s", phase, task, elapsed)
}
```

### Collapsible Plan Sections

Add fold/unfold state to `PlanModel` per wave group:

```go
// Source: internal/ui/tui/plan_model.go
// Add collapsed map to PlanModel
type PlanModel struct {
    // ... existing fields ...
    collapsed map[int]bool // wave index → collapsed
}

// Toggle collapse for the focused wave
func (pm *PlanModel) toggleCollapse(waveIdx int) {
    if pm.collapsed == nil {
        pm.collapsed = make(map[int]bool)
    }
    pm.collapsed[waveIdx] = !pm.collapsed[aveIdx]
    pm.refreshContent()
}

// In renderTasks(), skip tasks in collapsed waves
for waveIdx, wave := range pm.waves {
    if pm.collapsed[waveIdx] {
        // Render collapsed header only: "▸ Wave 1 — Foundation (3 tasks)"
        continue
    }
    // Render expanded wave as before
}
```

### Permission Modal: Risk Level Text Label

Extend `PermissionModal.Render()` to show explicit risk text (D-10):

```go
// Source: internal/ui/tui/components/permission.go:Render
// Add risk label next to the title
var riskLabel string
switch m.request.RiskLevel {
case types.RiskDangerous:
    riskLabel = s.PermRiskDanger.Render("DANGER")
case types.RiskDestructive:
    riskLabel = s.PermRiskDestruct.Render("DESTRUCTIVE")
case types.RiskMedium:
    riskLabel = s.PermRiskMedium.Render("CAUTION")
case types.RiskSafe:
    riskLabel = s.PermRiskSafe.Render("SAFE")
}
// Compose: "M31A wants to [action] [DANGER]"
```

### Batch Permission Queue Display

Show queued tool calls in the permission modal (D-11):

```go
// Source: internal/ui/tui/components/permission.go
// The request already has QueueDepth. Show queued items:
if m.request.QueueDepth > 0 {
    queueInfo = s.Caption.Render(
        fmt.Sprintf("  %d more tool(s) waiting for approval", m.request.QueueDepth),
    )
}
// For batch approval (D-11), add a "Approve all N" option:
keys = lipgloss.JoinVertical(lipgloss.Left,
    // ... existing keys ...
    lipgloss.JoinHorizontal(lipgloss.Top,
        s.PermKey.Render("[B]"),
        s.PermHint.Render(fmt.Sprintf(" Approve all %d", m.request.QueueDepth+1)),
    ),
)
```

### First-Run Tutorial: Post-Setup Tour

Insert the `TourModel` as a screen after `FirstRunCompleteMsg`:

```go
// Source: internal/ui/tui/app_state.go:handleFirstRunComplete
// After provider registration and config save, show tutorial:
func (m *AppState) handleFirstRunComplete(msg FirstRunCompleteMsg) tea.Cmd {
    // ... existing provider registration ...
    
    // D-13: Show interactive tutorial after setup
    m.firstRunModel = nil // clear wizard
    cw, ch := m.contentDimensions()
    m.tourModel = components.NewTourModel(m.themeManager.Current(), cw, ch)
    m.switchScreen(ScreenTour) // new screen for tutorial
    
    return tea.Batch(cmds...)
}
```

### Auto-Approve Safe Tools

Verify the existing auto-approve logic matches D-12:

```go
// Source: internal/tools/permissions.go:checkPermission
// Already checks RiskSafe level. Verify tool list:
// FileRead → RiskSafe ✓
// Glob → RiskSafe ✓  
// Grep → RiskSafe ✓
// The existing system classifies these as safe. No code change needed.
// Only verify that the risk classification hasn't changed.
```

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Go standard library `testing` |
| Config file | none — `go test ./...` |
| Quick run command | `make test-fast` |
| Full suite command | `make test` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| UX-01 | Footer shows phase + task + elapsed | unit | `go test ./internal/ui/tui/ -run TestFooter -count=1` | ❌ Wave 0 |
| UX-02 | Plan sections collapse/expand | unit | `go test ./internal/ui/tui/ -run TestPlanCollapse -count=1` | ❌ Wave 0 |
| UX-03 | Plan tasks editable before execution | unit | `go test ./internal/ui/tui/ -run TestPlanEdit -count=1` | ❌ Wave 0 |
| UX-04 | Current task highlighted during execution | unit | `go test ./internal/ui/tui/ -run TestExecuteHighlight -count=1` | ❌ Wave 0 |
| UX-05 | Permission modal shows risk level text | unit | `go test ./internal/ui/tui/components/ -run TestPermissionRiskLabel -count=1` | ❌ Wave 0 |
| UX-06 | Batch approval queue display | unit | `go test ./internal/ui/tui/components/ -run TestBatchApproval -count=1` | ❌ Wave 0 |
| UX-07 | Safe tools auto-approved | unit | `go test ./internal/tools/ -run TestAutoApproveSafe -count=1` | ❌ Wave 0 |
| UX-08 | Tutorial skippable at any point | unit | `go test ./internal/ui/tui/components/ -run TestTourSkip -count=1` | ✅ TourModel has Skip() |

### Sampling Rate
- **Per task commit:** `make test-fast`
- **Per wave merge:** `make test`
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `{internal/ui/tui/footer_status_test.go}` — covers UX-01 (footer status enrichment)
- [ ] `{internal/ui/tui/plan_collapse_test.go}` — covers UX-02 (collapsible sections)
- [ ] `{internal/ui/tui/plan_edit_test.go}` — covers UX-03 (inline editing)
- [ ] `{internal/ui/tui/execute_highlight_test.go}` — covers UX-04 (task highlighting)
- [ ] `{internal/ui/tui/components/permission_risk_test.go}` — covers UX-05 (risk label)
- [ ] `{internal/ui/tui/components/batch_approval_test.go}` — covers UX-06 (batch queue)
- [ ] `{internal/tools/auto_approve_test.go}` — covers UX-07 (safe tool auto-approve)

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V4 Access Control | yes | Permission modal enforces user approval for dangerous tools |
| V5 Input Validation | yes | Plan editing validates task structure before submission |

### Known Threat Patterns for Bubble Tea TUI Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Permission bypass via auto-approve | Elevation of Privilege | `checkPermission()` risk classification; safe tool list is conservative (read-only tools only) |
| Plan injection via inline edit | Tampering | Validate edited plan structure; reject malformed tasks |
| Tutorial skip during sensitive setup | Information Disclosure | Tutorial is post-setup only; API keys are already configured before tour begins |

## Sources

### Primary (HIGH confidence)
- Codebase analysis: `internal/ui/tui/` — all TUI components, models, layout
- Codebase analysis: `internal/tools/permissions.go` — permission system
- Codebase analysis: `internal/engine/narrative/` — narrative engine
- Codebase analysis: `internal/ui/tui/components/` — 51 reusable components

### Secondary (MEDIUM confidence)
- Bubble Tea documentation — Elm architecture patterns
- Charm ecosystem best practices — component composition

### Tertiary (LOW confidence)
- None — all findings are from direct codebase analysis

## Metadata

**Confidence breakdown:**
- Standard Stack: HIGH — all libraries are already in go.mod and actively used
- Architecture: HIGH — based on direct codebase analysis of existing patterns
- Pitfalls: HIGH — derived from understanding of Bubble Tea's single-threaded contract and existing permission flow

**Research date:** 2026-08-05
**Valid until:** 2026-09-05 (stable — Bubble Tea and Lipgloss are mature)

## Assumptions Log

> No claims tagged `[ASSUMED]` in this research. All findings were verified through direct codebase analysis.

## Open Questions

1. **Should the status bar be the existing footer or a new element?**
   - What we know: The footer already shows operation status in the center zone
   - What's unclear: Whether D-01 means extending the footer or adding a separate persistent bar
   - Recommendation: Extend the existing footer — it IS the bottom of the screen, and adding a second element would waste vertical space

2. **How should collapsible sections interact with the viewport?**
   - What we know: `viewport.Model` tracks absolute line positions
   - What's unclear: Whether to reinitialize viewport on toggle or track section-relative offsets
   - Recommendation: Reinitialize viewport content on toggle (existing pattern in `PlanModel.initViewport()`)

3. **Should batch approval apply across tasks or just within a task?**
   - What we know: `BatchApproval` struct has `ExpiresAt` for task-scoped expiry
   - What's unclear: Whether D-11 means per-task batch or cross-task batch
   - Recommendation: Per-task batch (existing behavior) — cross-task batch would reduce safety

4. **Where does the tutorial appear in the screen flow?**
   - What we know: `FirstRunCompleteMsg` transitions to REPL; `TourModel` component exists
   - What's unclear: Whether tutorial is a new screen or an overlay on the REPL
   - Recommendation: New screen (`ScreenTour`) that transitions to REPL on completion — keeps tutorial isolated and skippable

---

*Phase: 2-User Experience*
*Research completed: 2026-08-05*
