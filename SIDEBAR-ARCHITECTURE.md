# Sidebar Information Architecture — M31A v1.6.2 Milestone 2A

## 1. Sidebar Information Audit

### Complete Inventory

Every piece of information currently displayed or storable in the sidebar:

| # | Data Field | Current Location | Lines Used |
|---|-----------|-----------------|------------|
| 1 | Branch name | renderGitStatus | 1 |
| 2 | File status pills (●N +N -N ?N) | renderGitStatus | 0-1 |
| 3 | Version badge | renderHeader | 1 |
| 4 | Context bar [████░░░░] 42% | renderTokenUsage | 1 |
| 5 | Token count (42.3K tokens) | renderTokenUsage | 1 |
| 6 | Burn rate (120 tok/s) | renderTokenUsage | 0-1 |
| 7 | Cost ($0.05 ↑) | renderTokenUsage | 0-1 |
| 8 | Cost rate ($0.003/s) | renderTokenUsage | 0-1 |
| 9 | Model name | renderTokenUsage | 0-1 |
| 10 | Phase pipeline (✓I ✓D ●P ○E ○V ○R ○S) | renderPhasePipeline | 2-3 |
| 11 | Time in phase (45s in plan) | renderPhasePipeline | 0-1 |
| 12 | Tool call timeline (✓/●/✗ name duration) | renderToolTimeline | 0-5 |
| 13 | Speed metrics (1.2 tasks/min) | renderSpeedMetrics | 0-3 |
| 14 | Sub-agent badge (⬡ 2/3 agents) | renderSubAgentBadge | 0-1 |
| 15 | Pending permissions (! 2 pending) | renderPendingPermBadge | 0-1 |
| 16 | Progress bar [████░░] 66% | renderFileTreeOrTodo | 2 |
| 17 | Task summary (4/6 failed) | renderFileTreeOrTodo | 1 |
| 18 | Elapsed time (120s elapsed) | renderFileTreeOrTodo | 0-1 |
| 19 | Todo items (✓/●/✗ description) | renderFileTreeOrTodo | 0-N |
| 20 | File tree (file names + status) | renderFileTreeOrTodo | 0-N |
| 21 | Session ID | renderSession | 0-1 |
| 22 | Keyboard hints (ctrl+b sidebar) | renderHints | 0-1 |
| 23 | Current screen | renderHints | (implied) |
| 24 | Provider name | (stored, not rendered in sidebar) | 0 |
| 25 | Context pressure (0.0-1.0) | (stored, not rendered) | 0 |
| 26 | Compaction events | (stored, not rendered) | 0 |
| 27 | Recently changed files | (stored, not rendered) | 0 |
| 28 | Phase history | (stored, used in pipeline) | 0 |
| 29 | Cost trend (↑/↓) | renderTokenUsage | 0-1 |

### Total Peak Line Count (Current Todo Mode)

Header(2) + GitStatus(2) + TokenUsage(5) + PhasePipeline(3) + ToolTimeline(6) + SpeedMetrics(3) + SubAgent(1) + PermBadge(1) + Progress(4) + Todo(10) + Session(1) + Hints(1) = **39 lines**

At 24-row terminal with 2 chrome lines = 22 content rows. The sidebar overflows by 17 lines.

---

## 2. Classification

### Always Visible

Shown in both Idle and Active modes without expansion.

| Item | Justification |
|------|--------------|
| **Branch name** | "Where am I?" — primary orientation question. Never changes mid-session. |
| **Current phase** | "What is M31A doing?" — answers the second orientation question. |
| **Pending approvals** | "What should I pay attention to?" — requires immediate user action. |

### Visible While Active

Shown only when M31A is working (workflow phase != idle).

| Item | Justification |
|------|--------------|
| **File status pills** | Shows scope of changes. Only relevant during active work. |
| **In-progress tasks** | What M31A is currently executing. Maximum 2 shown. |
| **Cost** | Useful to know but not orientation. Active only. |
| **Sub-agent count** | Awareness of parallel work. Only when agents are running. |

### Expandable (Progressive Disclosure)

Collapsed by default. Expands on explicit user action (ctrl+g cycle) or when threshold exceeded.

| Item | Expand Trigger | Collapse Trigger |
|------|---------------|-----------------|
| **Context bar** | Context > 70% | Context < 50% |
| **Tool timeline** | Manual expand | After 30s idle |
| **Speed metrics** | Manual expand | After 30s idle |
| **Progress bar** | Workflow starts | Workflow completes |
| **Task list** | Workflow starts | Workflow completes |
| **File tree** | Manual expand | Manual collapse |

### Hidden

Never shown in the sidebar. Available elsewhere or too granular.

| Item | Why Hidden | Where Else |
|------|-----------|------------|
| **Version badge** | Debug info, not orientation | Header already has brand |
| **Session ID** | Debug info, not orientation | Available in session metadata |
| **Keyboard hints** | Already in footer | Footer right zone |
| **Model name** | Already in header | Header right zone |
| **Provider name** | Already in header | Header right zone |
| **Token count** | Diagnostic, not orientation | Context bar conveys pressure |
| **Burn rate** | Diagnostic, not orientation | Context bar + cost trend |
| **Cost rate** | Diagnostic, not orientation | Cost trend arrow |
| **Cost trend arrows** | Noise — cost speaks for itself | Cost number is sufficient |
| **Phase pipeline** | Redundant with current phase | Single phase indicator is clearer |
| **Phase elapsed time** | Diagnostic, not orientation | Available in workflow state |
| **Task elapsed time** | Diagnostic, not orientation | Available in workflow state |
| **Task summary line** | Redundant with progress bar | Progress bar + percentage |
| **Context pressure** | Raw metric, not meaningful | Context bar conveys it |
| **Compaction events** | Technical detail | Available in log |
| **Recently changed files** | File tree shows this | File tree is the source |
| **Provider diagnostics** | Debug info | Log files |

---

## 3. Idle Mode Wireframe

When the user is reading or thinking. Sidebar is calm.

Width: 28-32 chars (standard sidebar width)

```
┌──────────────────────────┐
│ ⎇ main                   │  ← branch (always visible)
│                          │
│ ctrl+b close  ctrl+g ↑↓  │  ← footer hints (from footer)
└──────────────────────────┘
```

**Total: 3 lines of content**

When cost is available:

```
┌──────────────────────────┐
│ ⎇ main                   │
│ $0.05                    │  ← cost (visible while active)
│                          │
└──────────────────────────┘
```

**Total: 2-3 lines of content**

### Idle Mode Rules

1. No section dividers
2. No progress bars
3. No tool calls
4. No file status
5. No token metrics
6. Maximum 3 lines
7. Visual weight: zero — the sidebar is invisible unless looked at

---

## 4. Active Mode Wireframe

When M31A is executing. Sidebar shows orientation + action required.

```
┌──────────────────────────┐
│ ⎇ feature/auth           │  ← branch
│ ●3 +2 -1 ?4              │  ← file status pills
│                          │
│ ▸ execute                │  ← current phase (highlighted)
│   Read config.go         │  ← in-progress task 1
│   Write auth.go          │  ← in-progress task 2
│                          │
│ $0.12                    │  ← cost
│ ! 1 pending              │  ← pending approval (warning color)
└──────────────────────────┘
```

**Total: 7-8 lines**

### Active Mode Rules

1. No section dividers (cleaner, less noise)
2. Phase uses brand color for emphasis
3. In-progress tasks use muted text (they're temporary)
4. Pending approvals use warning color (attention required)
5. Maximum 8 lines

### When Tasks Are Running (No In-Progress Items)

```
┌──────────────────────────┐
│ ⎇ main                   │
│ ●1 +3                    │
│                          │
│ ▸ plan                   │
│                          │
│ $0.03                    │
└──────────────────────────┘
```

**Total: 5 lines**

### When Approvals Are Pending

```
┌──────────────────────────┐
│ ⎇ main                   │
│                          │
│ ▸ execute                │
│   Reading file.go        │
│                          │
│ ! 2 pending              │  ← warning: needs attention
└──────────────────────────┘
```

**Total: 5 lines**

---

## 5. Information Priority Matrix

Ranked by visual weight and position. No two items share equal priority.

| Priority | Item | Visual Weight | Position | Condition |
|----------|------|--------------|----------|-----------|
| **P1** | Pending approvals | Warning color, bold | Bottom of content (last = most seen) | Count > 0 |
| **P2** | Current phase | Brand color, bold | Center of content | Phase != idle |
| **P3** | Branch name | Secondary text | Top of content (always first) | Always |
| **P4** | In-progress tasks | Muted text | Below phase | Max 2, when active |
| **P5** | Cost | Muted text | Below tasks | When active, cost > 0 |
| **P6** | File status pills | Colored counts | Below branch | When active, changes > 0 |
| **P7** | Sub-agent count | Muted text | Below cost | When agents > 0 |

### Priority Justification

- **P1 (Approvals)** is highest because it blocks the user. Without action, M31A cannot proceed.
- **P2 (Phase)** is second because it answers "what is happening?" — the core active-mode question.
- **P3 (Branch)** is third because it answers "where am I?" — always needed but never urgent.
- **P4-P7** are progressively less important and only shown when relevant.

### Attention Budget

The sidebar may have at most:

| Element Type | Maximum | Current |
|-------------|---------|---------|
| Highlighted sections | 1 | 1 (phase) |
| Progress indicators | 0 | 0 (removed from sidebar) |
| Warnings | 1 | 1 (pending approvals) |
| Action-required items | 1 | 1 (pending approvals) |

Everything else uses muted/secondary text. No bold, no warning colors, no animation.

---

## 6. Progressive Disclosure

### Automatic Collapse Rules

| Section | Auto-Expand | Auto-Collapse | Rationale |
|---------|------------|---------------|-----------|
| Context bar | Context > 70% | Context < 50% | Warning: approaching limit |
| Tool timeline | Never auto-expand | 30s after last tool call | Diagnostic, not orientation |
| Speed metrics | Never auto-expand | 30s after last metric update | Diagnostic, not orientation |
| Progress bar | Workflow phase != idle | Workflow phase == idle | Active only |
| Task list | Workflow phase != idle | Workflow phase == idle | Active only |
| File tree | Never auto-expand | Manual collapse | User-initiated exploration |

### Manual Expansion

User cycles through modes with ctrl+g:

```
Idle → Active → Files → Todo → Idle
```

Each mode is a distinct view. No partial expansion within a mode.

### Collapse Behavior

When a section auto-collapses:
1. No animation or transition
2. Content simply disappears
3. A subtle "…" indicator may appear if content was previously visible
4. Re-expansion shows fresh data (not cached)

---

## 7. Width Budget

### Sidebar Width

| Terminal Width | Sidebar Width | Content Width | Behavior |
|---------------|--------------|---------------|----------|
| < 80 | 0 (overlay) | 0 | Sidebar renders as overlay via ctrl+b |
| 80-119 | 28 | 27 | Standard sidebar, right border |
| 120-159 | 32 | 31 | Wider sidebar, more breathing room |
| 160+ | 36 | 35 | Maximum sidebar width |

### Content Width Calculations

With contentWidth = 28:

- Branch: `"⎇ feature/auth"` = 16 chars ✓
- File pills: `"●3 +2 -1 ?4"` = 12 chars ✓
- Phase: `"▸ execute"` = 9 chars ✓
- Task: `"  Reading config.go"` = 20 chars ✓ (truncated at 24)
- Cost: `"$0.12"` = 5 chars ✓
- Approval: `"! 1 pending"` = 11 chars ✓

### Truncation Rules

| Element | Max Length | Truncation |
|---------|-----------|-----------|
| Branch name | contentW - 2 | `branch-na…` |
| Task description | contentW - 4 | `  Readin…` |
| File path | contentW - 4 | `  src/co…` |
| Model name | contentW - 2 | `gpt-4-…` |

---

## 8. Responsive Behaviour

### Narrow (< 80 cols)

Sidebar is hidden. User presses ctrl+b to show as overlay.

```
┌─────────────────────────────────────────────────────┐
│ M31A  Chat                  gpt-4                    │ ← header
│                                                     │
│ ┌─────────────────────┐                              │
│ │ ⎇ main              │ ← overlay                   │
│ │ $0.05               │                              │
│ │                     │                              │
│ └─────────────────────┘                              │
│                                                     │
│ User: What does this function do?                   │
│                                                     │
│ ⌂ project ⎇ main  ·  ⠹ thinking  ·  ctrl+p         │ ← footer
└─────────────────────────────────────────────────────┘
```

Overlay width: min(contentW, terminalWidth - 10)

### Standard (80-119 cols)

Sidebar is inline. Content area shrinks by sidebar width.

```
┌──────────────────────────────────────────┬──────────────────────┐
│ M31A  Chat              gpt-4            │ ⎇ main               │
│                                          │                      │
│ What does this function do?              │ ▸ execute            │
│                                          │   Read config.go     │
│ Assistant: The function handles...       │   Write auth.go      │
│                                          │                      │
│                                          │ $0.05                │
│                                          │ ! 1 pending          │
│                                          │                      │
│ ⌂ project ⎇ main  ·  ⠹ thinking         │ ctrl+b close         │
└──────────────────────────────────────────┴──────────────────────┘
```

### Wide (120+ cols)

Sidebar is wider. More breathing room but same content.

```
┌────────────────────────────────────────────────────┬──────────────────────────┐
│ M31A  Chat                          gpt-4          │ ⎇ feature/auth           │
│                                                    │ ●3 +2 -1 ?4              │
│ What does this function do?                        │                          │
│                                                    │ ▸ execute                │
│ Assistant: The function handles authentication     │   Reading config.go      │
│ by validating the JWT token against the public     │   Writing auth.go        │
│ key and checking expiration claims.                │                          │
│                                                    │ $0.12                    │
│                                                    │ ! 1 pending              │
│                                                    │                          │
│ ⌂ project ⎇ main  ·  ⠹ thinking  ·  ctrl+p        │ ctrl+b close  ctrl+g ↑↓  │
└────────────────────────────────────────────────────┴──────────────────────────┘
```

---

## 9. Migration Plan for M2B

### What M2B Must Implement

| Task | Description | Files | Risk |
|------|------------|-------|------|
| 1 | Rewrite `renderIdle()` to match wireframe | sidebar_model.go | Low |
| 2 | Rewrite `renderActive()` to match wireframe | sidebar_model.go | Low |
| 3 | Remove `renderHints()` from sidebar | sidebar_model.go | Low |
| 4 | Remove `renderSession()` from sidebar | sidebar_model.go | Low |
| 5 | Remove `renderPhasePipeline()` from sidebar | sidebar_model.go | Low |
| 6 | Remove `renderSpeedMetrics()` from sidebar | sidebar_model.go | Low |
| 7 | Remove `renderTokenUsage()` from sidebar | sidebar_model.go | Medium |
| 8 | Remove `renderToolTimeline()` from sidebar | sidebar_model.go | Low |
| 9 | Remove token/cost fields from StatusBarInfo | repl_footer.go | Low |
| 10 | Update ctrl+g to cycle: Idle → Active → Files → Todo | app_key.go | Low |
| 11 | Auto-switch to Active mode when workflow starts | app_update.go | Medium |
| 12 | Auto-switch to Idle mode when workflow completes | app_update.go | Medium |
| 13 | Update sidebar width constants | sidebar_model.go | Low |
| 14 | Add overlay rendering for narrow terminals | layout/overlay.go | Medium |
| 15 | Write regression tests for all modes | sidebar_extra_test.go | Low |

### What M2B Must NOT Touch

- Conversation rendering
- Permission modal
- Header/footer rendering (M1)
- Workflow engine
- Tool dispatcher
- Provider layer

### Testing Strategy

| Test Category | Count | Coverage |
|--------------|-------|----------|
| Idle mode rendering | 3 | Empty, with branch, with cost |
| Active mode rendering | 5 | Empty, with tasks, with approvals, with sub-agents, full |
| Mode switching | 4 | Cycle through all 4 modes, auto-switch on workflow |
| Width responsiveness | 3 | Narrow (overlay), standard, wide |
| Truncation | 3 | Long branch, long task, long file path |
| Edge cases | 4 | No git, no workflow, zero cost, many approvals |
| Regression | 5 | Existing tests updated for new behavior |

**Total: 27 tests**

### Dependencies

- M1 (Layout Foundation) — complete
- No other milestones depend on M2A

### Rollback Strategy

If M2B implementation has issues:
1. Revert sidebar_model.go changes
2. Restore `renderHints()`, `renderSession()`, `renderPhasePipeline()`, `renderSpeedMetrics()`, `renderTokenUsage()`, `renderToolTimeline()`
3. Restore `visible: true` default
4. All M1 tests still pass (they test layout, not sidebar content)

---

## 10. Summary

### Design Principles Applied

1. **Sidebar is orientation, not dashboard** — removed 13 of 29 data fields
2. **Progressive disclosure** — 4 modes instead of one monolithic view
3. **Attention budget** — max 1 highlight, 1 warning, 1 action-required
4. **Calm idle mode** — 2-3 lines, no metrics, no noise
5. **Focused active mode** — 7-8 lines, only what matters right now
6. **No information duplication** — branch/phase/cost appear once, in sidebar only
7. **Width-aware** — overlay on narrow, inline on wide, max 36 cols

### Metrics

| Metric | Before (Todo Mode) | After (Active Mode) | After (Idle Mode) |
|--------|-------------------|--------------------|--------------------|
| Peak lines | 39 | 8 | 3 |
| Data fields shown | 15+ | 7 | 2-3 |
| Section dividers | 6 | 0 | 0 |
| Attention items | 0 | 1 (phase) | 0 |
| Warnings | 0 | 1 (approvals) | 0 |
| Action-required | 0 | 1 (approvals) | 0 |
