# M31A v1.6.2 — Engineering Execution Plan

**Document Status:** FINAL — Implementation Contract
**Version:** 1.6.2
**Date:** 2026-07-01
**Reference:** UX-ARCHITECTURE-SPEC.md (Design Freeze)

---

# Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Milestone Breakdown](#2-milestone-breakdown)
3. [Dependency Graph](#3-dependency-graph)
4. [File Impact Analysis](#4-file-impact-analysis)
5. [Reusable Components](#5-reusable-components)
6. [Technical Debt Report](#6-technical-debt-report)
7. [Regression Strategy](#7-regression-strategy)
8. [Rollback Strategy](#8-rollback-strategy)
9. [Risk Analysis](#9-risk-analysis)
10. [Implementation Schedule](#10-implementation-schedule)
11. [Definition of Done](#11-definition-of-done)

---

# 1. Executive Summary

## 1.1 Scope

This plan transforms the UX Architecture Specification into a shippable implementation across 10 milestones. Every change is backward-compatible, independently verifiable, and rollback-safe.

## 1.2 Codebase Baseline

| Metric | Value |
|--------|-------|
| TUI source files (non-test) | 167 |
| TUI test files | 39 |
| TUI total lines (non-test) | 39,402 |
| Component files | 41 (6,934 lines) |
| Layout files | 7 (1,495 lines) |
| Theme files | 14 (2,077 lines) |
| Screen model files | ~30 (varies) |
| App core files | ~15 (varies) |

## 1.3 Change Summary

| Category | Files Changed | Files Created | Files Deleted |
|----------|--------------|---------------|---------------|
| Chrome (header/footer/home) | 8 | 0 | 0 |
| Sidebar | 2 | 0 | 0 |
| Tool rendering | 4 | 1 | 0 |
| Permission modal | 3 | 1 | 0 |
| Workflow screens | 6 | 0 | 0 |
| Thinking display | 2 | 0 | 0 |
| Narrative engine | 0 | 1 | 0 |
| Empty states | 5 | 1 | 0 |
| Accessibility | 3 | 0 | 0 |
| Testing | 12 | 8 | 0 |
| **TOTAL** | **45** | **12** | **0** |

## 1.4 Effort Summary

| Metric | Estimate |
|--------|----------|
| Total implementation days | 28-32 |
| Review days | 8-10 |
| Testing days | 6-8 |
| **Total calendar days** | **42-50** |
| New LOC (estimated) | ~2,800 |
| Removed LOC (estimated) | ~1,200 |
| Net LOC change | +1,600 |

---

# 2. Milestone Breakdown

## M1: Layout Foundation

**Purpose:** Simplify header, footer, and home screen chrome. Establish the new visual baseline.

**Files affected:**
| File | Change Type | Lines Changed | Complexity |
|------|-------------|---------------|------------|
| `layout/page.go` | Moderate refactor | ~120 | Medium |
| `layout/responsive.go` | Minor change | ~5 | Low |
| `app_view.go` | Moderate refactor | ~80 | Medium |
| `home_view.go` | Moderate refactor | ~60 | Low |
| `home_model.go` | Minor change | ~15 | Low |
| `repl_footer.go` | Moderate refactor | ~90 | Medium |
| `repl_view.go` | Minor change | ~20 | Low |
| `helpers.go` | Minor change | ~10 | Low |

**Estimated LOC:** ~400 changed
**Complexity:** Medium
**Dependencies:** None (foundation milestone)
**Risk:** Low — visual changes only, no functional changes
**Migration strategy:** Feature flag `UX_V2_CHROME` in config. Old rendering preserved behind flag. New rendering enabled by default.
**Rollback strategy:** Revert the commit. All changes are in TUI layer only.
**Testing strategy:**
- Visual regression: screenshot at 80×24 and 120×40
- Functional: header renders, footer renders, home renders
- Responsive: 40, 60, 80, 120 columns
- Regression: `go test ./internal/tui/...`

**Acceptance criteria:**
- [ ] Header shows brand + breadcrumb + model only (no leader dots, no `│` separators, no provider badge, no context meter)
- [ ] Footer shows cwd+branch (left), operation (center), hints (right) (no `│` separators, no cost, no context ring)
- [ ] Home screen shows brand name (2-3 rows max), tagline, input (no glow, no glow line, no version)
- [ ] All responsive breakpoints work correctly
- [ ] All existing keyboard shortcuts work
- [ ] No visual regressions at any terminal size

---

## M2: Sidebar Redesign

**Purpose:** Transform sidebar from 11-section data firehose to 3-mode context-aware panel.

**Files affected:**
| File | Change Type | Lines Changed | Complexity |
|------|-------------|---------------|------------|
| `sidebar_model.go` | Large refactor | ~350 | High |
| `app_view.go` | Minor change | ~20 | Low |
| `app_handlers_misc.go` | Minor change | ~15 | Low |
| `constants.go` | Minor change | ~5 | Low |

**Estimated LOC:** ~390 changed
**Complexity:** High
**Dependencies:** M1 (header/footer chrome)
**Risk:** Medium — sidebar is complex, 1,429 lines
**Migration strategy:** Implement new modes alongside old rendering. Old rendering becomes `FocusedMode`. New modes (`Idle`, `Active`) are additions. No breaking changes to sidebar API.
**Rollback strategy:** Revert to pre-M2 commit. Sidebar reverts to 11-section layout.
**Testing strategy:**
- Mode switching: idle → active → focused → hidden
- Content rules: correct sections per mode
- Responsive: sidebar visible ≥80, overlay 60-79, hidden <60
- Focus mode: all sections visible
- Keyboard: ctrl+b toggle, ctrl+g mode switch
- Regression: `go test ./internal/tui/...`

**Acceptance criteria:**
- [ ] Idle mode shows brand + git branch + file counts (3 lines max)
- [ ] Active mode shows brand + activity label + file list + context (5-7 lines)
- [ ] Focused mode shows all current sections (backward compatible)
- [ ] ctrl+b toggles sidebar visibility
- [ ] ctrl+g switches between idle/active/focused when visible
- [ ] Sidebar renders correctly at all responsive breakpoints
- [ ] No section dividers (whitespace only)
- [ ] Git status shows `+3 -1 ?2` format (no colored pills)
- [ ] Context shows `42% context` (no gauge bar)
- [ ] Version, session ID, hints, burn rate, speed metrics, tool timeline removed from default views

---

## M3: Narrative Engine

**Purpose:** Build the intent classification and tool grouping system that powers Layer 2 (Narrative).

**Files affected:**
| File | Change Type | Lines Changed | Complexity |
|------|-------------|---------------|------------|
| `components/narrative.go` | **NEW FILE** | ~250 | High |
| `components/toolcard.go` | Moderate refactor | ~60 | Medium |
| `components/message.go` | Moderate refactor | ~80 | Medium |
| `repl_stream.go` | Moderate refactor | ~100 | Medium |
| `repl_model.go` | Minor change | ~20 | Low |

**Estimated LOC:** ~510 (250 new + 260 changed)
**Complexity:** High
**Dependencies:** None (can be built in parallel with M1, M2)
**Risk:** High — core rendering change, affects conversation display
**Migration strategy:** New `NarrativeRenderer` component alongside existing `ToolCard`. Conversation uses `NarrativeRenderer` for grouped tools, `ToolCard` for standalone tools (Bash, Edit, failures). Old rendering preserved for ToolDetail screen.
**Rollback strategy:** Revert narrative renderer. Conversation falls back to individual ToolCard display.
**Testing strategy:**
- Unit: intent classifier (18 tool patterns → labels)
- Unit: group buffer (flush rules, boundary detection)
- Unit: narrative renderer (group rendering, failure rendering)
- Integration: conversation with mixed tool calls
- Visual: narrative blocks in conversation at 80×24 and 120×40
- Regression: `go test ./internal/tui/components/...`

**Acceptance criteria:**
- [ ] Consecutive FileRead+Glob+Grep rendered as single intent block
- [ ] Intent labels are human-readable ("Reading source files", not "FileRead ×3")
- [ ] File lists show max 5 files with "and N more" overflow
- [ ] Failed tools render as standalone block with error context
- [ ] Bash commands always render as standalone (user-facing)
- [ ] Edit operations render as standalone (file modifications are important)
- [ ] Tool duration NOT shown in inline view
- [ ] Tool status icons NOT shown in grouped view
- [ ] Group buffer flushes on intent change, failure, LLM text, or 5-tool limit
- [ ] Narrative appears during thinking, streaming, and tool execution

---

## M4: Permission Modal Redesign

**Purpose:** Replace implementation-focused permission modal with plain English descriptions.

**Files affected:**
| File | Change Type | Lines Changed | Complexity |
|------|-------------|---------------|------------|
| `components/permission.go` | Moderate refactor | ~120 | Medium |
| `app_view.go` | Moderate refactor | ~100 | Medium |
| `components/permission_desc.go` | **NEW FILE** | ~180 | Medium |

**Estimated LOC:** ~400 (180 new + 220 changed)
**Complexity:** Medium
**Dependencies:** M1 (chrome layout)
**Risk:** Medium — permission is safety-critical
**Migration strategy:** New `PermissionDescription` generator produces plain English from tool+command. Old `PermissionModal` enhanced with description field. Fallback: if description generation fails, show original format.
**Rollback strategy:** Revert to old permission modal. No data loss.
**Testing strategy:**
- Unit: description generator (tool+command → English)
- Unit: consequence extractor (command → impact statement)
- Integration: permission modal for each tool type (Bash, Edit, FileWrite, FileDelete)
- Visual: modal at 80×24, 120×40, with long/short descriptions
- Safety: deny defaults to Escape, all approvals logged
- Regression: `go test ./internal/tui/components/...`

**Acceptance criteria:**
- [ ] Permission shows "M31A wants to [action]" as primary text
- [ ] Consequence explanation appears below description
- [ ] Command shown in code block below divider (secondary)
- [ ] Risk communicated via border color accent (not text label)
- [ ] Countdown only appears when <5 seconds remaining
- [ ] Phase/goal context woven into description
- [ ] Escape always means deny (safe default)
- [ ] "Allow All" available for batch operations
- [ ] Description generation has fallback for unknown tools

---

## M5: Workflow Screen Cleanup

**Purpose:** Simplify Execute, Plan, Discuss, Ship, and Verify screens.

**Files affected:**
| File | Change Type | Lines Changed | Complexity |
|------|-------------|---------------|------------|
| `execute_model.go` | Moderate refactor | ~100 | Medium |
| `plan_model.go` | Minor change | ~40 | Low |
| `plan_view.go` | Minor change | ~15 | Low |
| `discuss_model.go` | Moderate refactor | ~60 | Medium |
| `ship_model.go` | Moderate refactor | ~80 | Medium |
| `ship_view.go` | Minor change | ~20 | Low |
| `verify_model.go` | Minor change | ~30 | Low |

**Estimated LOC:** ~345 changed
**Complexity:** Medium
**Dependencies:** M1 (chrome), M3 (narrative for tool display)
**Risk:** Low — visual changes to existing screens
**Migration strategy:** Modify existing View() methods. No new components needed. Changes are localized to each screen.
**Rollback strategy: Per-screen revert. Each screen change is independent.
**Testing strategy:**
- Visual: each screen at 80×24 and 120×40
- Functional: keyboard shortcuts still work
- Workflow: full workflow execution (plan → execute → verify → ship)
- Regression: `go test ./internal/tui/...`

**Acceptance criteria:**

**Execute:**
- [ ] Current activity label replaces stats line ("Running tests…" not "3/5 tasks 60%")
- [ ] Progress bar is single, subtle line
- [ ] Completed tasks collapsed to single line with ✓ icon
- [ ] Failed task shows error inline (red, expanded)
- [ ] Pending tasks dimmed
- [ ] ETA/elapsed removed from primary view

**Plan:**
- [ ] Card border removed
- [ ] Content stands on its own with typography

**Discuss:**
- [ ] Card border removed
- [ ] Progress simplified to "2 of 3" (no dot indicators for ≤5 questions)
- [ ] Section divider removed
- [ ] Key hints removed (footer handles)

**Ship:**
- [ ] Card wrapper removed
- [ ] Title "Session Summary" removed
- [ ] Stats consolidated to 2 lines
- [ ] Commit log visible as secondary content

**Verify:**
- [ ] No card border
- [ ] Failed tasks highlighted with error context

---

## M6: Thinking Display

**Purpose:** Replace timer-based thinking with intent-based labels.

**Files affected:**
| File | Change Type | Lines Changed | Complexity |
|------|-------------|---------------|------------|
| `repl_footer.go` | Minor change | ~40 | Low |
| `app_view.go` | Minor change | ~15 | Low |
| `components/thinking.go` | Minor change | ~20 | Low |

**Estimated LOC:** ~75 changed
**Complexity:** Low
**Dependencies:** M1 (footer chrome)
**Risk:** Low — display-only change
**Migration strategy:** New `thinkingLabel` function maps phase+time to intent string. Old timer string preserved as tooltip/expandable.
**Rollback strategy: Revert to timer display.
**Testing strategy:**
- Unit: label classification (phase+time → label)
- Visual: thinking state at various phases
- Regression: `go test ./internal/tui/...`

**Acceptance criteria:**
- [ ] Thinking shows intent label ("Planning implementation", not "thinking · 3.2s")
- [ ] Labels change based on phase context
- [ ] Duration accessible via hover/focus or sidebar
- [ ] Labels are accurate for each phase (plan, execute, verify)

---

## M7: Empty States

**Purpose:** Design meaningful empty states for all screens.

**Files affected:**
| File | Change Type | Lines Changed | Complexity |
|------|-------------|---------------|------------|
| `components/empty_state.go` | Moderate refactor | ~80 | Medium |
| `repl_welcome.go` | Moderate refactor | ~100 | Medium |
| `helpers.go` | Minor change | ~30 | Low |
| Various screen models | Minor changes | ~15 each | Low |

**Estimated LOC:** ~250 changed + ~100 new
**Complexity:** Low-Medium
**Dependencies:** M1 (chrome), M3 (narrative)
**Risk:** Low — additive changes
**Migration strategy:** New `EmptyState` component with standard layout. Each screen implements its own empty state content. Old "loading" and "no data" messages replaced.
**Rollback strategy: Revert empty state changes. Screens fall back to previous empty behavior.
**Testing strategy:**
- Visual: each screen's empty state at 80×24
- Functional: empty states guide user to next action
- Regression: `go test ./internal/tui/...`

**Acceptance criteria:**
- [ ] Every screen has a designed empty state
- [ ] Empty states guide users toward the next action
- [ ] Empty states use consistent typography and spacing
- [ ] No "N/A" or blank screens
- [ ] Empty states are readable at all terminal sizes

---

## M8: Tool Detail & Progressive Disclosure

**Purpose:** Ensure tool details are inspectable but hidden by default.

**Files affected:**
| File | Change Type | Lines Changed | Complexity |
|------|-------------|---------------|------------|
| `tooldetail_model.go` | Moderate refactor | ~50 | Low |
| `components/toolcard.go` | Minor change | ~30 | Low |
| `components/message.go` | Minor change | ~20 | Low |
| `repl_view.go` | Minor change | ~15 | Low |

**Estimated LOC:** ~115 changed
**Complexity:** Low
**Dependencies:** M3 (narrative engine)
**Risk:** Low — enhancement of existing behavior
**Migration strategy:** ToolCard gets `collapsed` state by default. Click/Enter expands. Failed tools auto-expand. ToolDetail screen remains available for full inspection.
**Rollback strategy: Revert to always-expanded tool cards.
**Testing strategy:**
- Unit: collapse/expand state management
- Integration: click to expand, Escape to collapse
- Visual: collapsed vs expanded tool cards
- Regression: `go test ./internal/tui/...`

**Acceptance criteria:**
- [ ] Successful tools collapsed by default
- [ ] Failed tools auto-expand with error context
- [ ] Click/Enter on collapsed tool expands it
- [ ] Escape or click on expanded tool collapses it
- [ ] ToolDetail screen available for full inspection
- [ ] No information loss (all data accessible)

---

## M9: Accessibility Audit

**Purpose:** Verify and fix accessibility across all screens.

**Files affected:**
| File | Change Type | Lines Changed | Complexity |
|------|-------------|---------------|------------|
| `theme/tokens_semantic.go` | Minor change | ~20 | Low |
| `layout/page.go` | Minor change | ~10 | Low |
| `layout/responsive.go` | Minor change | ~5 | Low |
| Various screen models | Minor changes | ~5 each | Low |

**Estimated LOC:** ~80 changed
**Complexity:** Low
**Dependencies:** All previous milestones
**Risk:** Low — adjustments, not redesigns
**Migration strategy:** Contrast ratio fixes, spacing adjustments, keyboard navigation improvements. No structural changes.
**Rollback strategy: Revert individual fixes.
**Testing strategy:**
- Contrast: all text meets WCAG AA (4.5:1)
- Keyboard: all actions accessible without mouse
- Terminal: 40, 60, 80, 120, 160 columns
- tmux: verify in tmux session
- SSH: verify over SSH
- Screen reader: basic ARIA labels where applicable

**Acceptance criteria:**
- [ ] All text meets WCAG AA contrast ratio
- [ ] All interactive elements keyboard-accessible
- [ ] No information conveyed by color alone
- [ ] All screens usable at 40+ columns
- [ ] All screens usable in tmux
- [ ] All screens usable over SSH
- [ ] No focus traps

---

## M10: Regression Testing & Documentation

**Purpose:** Comprehensive regression testing and documentation updates.

**Files affected:**
| File | Change Type | Lines Changed | Complexity |
|------|-------------|---------------|------------|
| Various `_test.go` files | New + updated | ~800 new | Medium |
| `docs/UX.md` | **NEW FILE** | ~200 | Low |
| `docs/KEYBINDINGS.md` | Update | ~50 | Low |

**Estimated LOC:** ~1,050 new/changed
**Complexity:** Medium
**Dependencies:** All previous milestones
**Risk:** Low — testing and documentation only
**Migration strategy:** Tests written against new behavior. Documentation updated to reflect new UX.
**Rollback strategy: Tests and docs are additive, no rollback needed.
**Testing strategy:**
- Full regression suite
- Visual regression at all breakpoints
- Workflow execution end-to-end
- Permission workflow
- Sidebar interaction
- Streaming behavior
- Long conversation (500+ messages)
- Error handling

**Acceptance criteria:**
- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] `go test -race ./...` passes
- [ ] Visual regression baselines established
- [ ] UX documentation complete
- [ ] Keybinding documentation updated

---

# 3. Dependency Graph

## 3.1 Milestone Dependencies

```
M1: Layout Foundation ─────────────────────────────────────────┐
    │                                                          │
    ├── M2: Sidebar Redesign                                   │
    │       │                                                  │
    │       └── (no downstream)                                │
    │                                                          │
    ├── M4: Permission Modal                                   │
    │       │                                                  │
    │       └── (no downstream)                                │
    │                                                          │
    ├── M6: Thinking Display                                   │
    │       │                                                  │
    │       └── (no downstream)                                │
    │                                                          │
    └── M5: Workflow Screens ──────┐                           │
            │                      │                           │
            └──────────────────────┤                           │
                                   │                           │
M3: Narrative Engine ──────────────┤                           │
    │                              │                           │
    ├── M8: Progressive Disclosure │                           │
    │       │                      │                           │
    │       └──────────────────────┤                           │
    │                              │                           │
    └──────────────────────────────┤                           │
                                   │                           │
M7: Empty States ──────────────────┤                           │
    │                              │                           │
    └──────────────────────────────┤                           │
                                   │                           │
M9: Accessibility Audit ───────────┤                           │
    │                              │                           │
    └──────────────────────────────┤                           │
                                   │                           │
M10: Regression & Docs ────────────┘                           │
                                                               │
                                                               │
All milestones can be developed on separate branches.          │
M1 must be merged before M2, M4, M5, M6 can merge.           │
M3 must be merged before M5, M8 can merge.                    │
M9 depends on all feature milestones being merged.            │
M10 depends on M9.                                             │
```

## 3.2 Parallel Development Map

```
Week 1:  M1 (Layout Foundation) ──── M3 (Narrative Engine) [parallel]
Week 2:  M2 (Sidebar) ──── M4 (Permission) ──── M6 (Thinking) [parallel]
Week 3:  M5 (Workflow Screens) ──── M8 (Progressive Disclosure) [parallel]
Week 3:  M7 (Empty States) [parallel]
Week 4:  M9 (Accessibility)
Week 4-5: M10 (Regression & Docs)
```

**Maximum parallelism:** 3 engineers can work simultaneously in weeks 1-3.

## 3.3 Merge Order

```
1. M1 (Layout Foundation)     ← must merge first
2. M3 (Narrative Engine)      ← can merge in parallel with M2/M4/M6
3. M2 (Sidebar Redesign)      ← depends on M1
4. M4 (Permission Modal)      ← depends on M1
5. M6 (Thinking Display)      ← depends on M1
6. M5 (Workflow Screens)      ← depends on M1, M3
7. M8 (Progressive Disclosure) ← depends on M3
8. M7 (Empty States)          ← depends on M1, M3
9. M9 (Accessibility)         ← depends on all features
10. M10 (Regression & Docs)   ← depends on M9
```

---

# 4. File Impact Analysis

## 4.1 Complete File Classification

### Layout Package (`internal/tui/layout/`)

| File | Current LOC | Classification | Effort | Phase |
|------|-------------|----------------|--------|-------|
| `page.go` | 472 | Moderate refactor | 3h | M1 |
| `responsive.go` | 65 | Minor change | 0.5h | M1 |
| `solver.go` | 323 | No change | 0 | — |
| `stack.go` | 170 | No change | 0 | — |
| `box.go` | 190 | No change | 0 | — |
| `constraints.go` | 154 | No change | 0 | — |
| `minscreen.go` | 121 | No change | 0 | — |

### Theme Package (`internal/tui/theme/`)

| File | Current LOC | Classification | Effort | Phase |
|------|-------------|----------------|--------|-------|
| `tokens_semantic.go` | 771 | Minor change | 1h | M9 |
| `colors.go` | 276 | No change | 0 | — |
| `theme.go` | 226 | No change | 0 | — |
| `cache.go` | 227 | No change | 0 | — |
| `unicode.go` | 166 | No change | 0 | — |
| `tokens.go` | 104 | No change | 0 | — |
| `borders.go` | 61 | No change | 0 | — |
| `tokens_typography.go` | 67 | No change | 0 | — |
| `tokens_spacing.go` | 36 | No change | 0 | — |
| `shadow.go` | 32 | No change | 0 | — |
| `registry.go` | 38 | No change | 0 | — |
| `tokens_motion.go` | 23 | No change | 0 | — |
| `cache_gen.go` | 20 | No change | 0 | — |
| `tokens_elevation.go` | 19 | No change | 0 | — |
| `tabs.go` | 11 | No change | 0 | — |

### Components Package (`internal/tui/components/`)

| File | Current LOC | Classification | Effort | Phase |
|------|-------------|----------------|--------|-------|
| `message.go` | 654 | Moderate refactor | 4h | M3 |
| `toolcard.go` | 413 | Moderate refactor | 3h | M3, M8 |
| `progress.go` | 386 | No change | 0 | — |
| `thinking.go` | 276 | Minor change | 1h | M6 |
| `filetree.go` | 282 | No change | 0 | — |
| `permission.go` | 253 | Moderate refactor | 4h | M4 |
| `sparkline.go` | 255 | No change | 0 | — |
| `special_renderers.go` | 237 | No change | 0 | — |
| `badge.go` | 230 | No change | 0 | — |
| `card.go` | 139 | No change | 0 | — |
| `statrow.go` | 172 | No change | 0 | — |
| `virtual_viewport.go` | 165 | No change | 0 | — |
| `file_renderers.go` | 152 | No change | 0 | — |
| `notification_list.go` | 114 | No change | 0 | — |
| `scroll_state.go` | 145 | No change | 0 | — |
| `search.go` | 131 | No change | 0 | — |
| `bordered_input.go` | 112 | No change | 0 | — |
| `dropdown.go` | 88 | No change | 0 | — |
| `filterchips.go` | 128 | No change | 0 | — |
| `step_progress.go` | 146 | No change | 0 | — |
| `logo.go` | 128 | No change | 0 | — |
| `syntax.go` | 129 | No change | 0 | — |
| `question.go` | 186 | No change | 0 | — |
| `empty_state.go` | 119 | Moderate refactor | 2h | M7 |
| `glamour_cache.go` | 124 | No change | 0 | — |
| `spinner.go` | 101 | No change | 0 | — |
| `timeline.go` | 100 | No change | 0 | — |
| `toolrenderers.go` | 126 | No change | 0 | — |
| `splitpane.go` | 44 | No change | 0 | — |
| `starfield.go` | 159 | No change | 0 | — |
| `taskgraph.go` | 113 | No change | 0 | — |
| `tabbar.go` | 63 | No change | 0 | — |
| `truncate.go` | 73 | No change | 0 | — |
| `confirm.go` | 54 | No change | 0 | — |
| `bash_renderer.go` | 63 | No change | 0 | — |
| `error_banner.go` | 69 | No change | 0 | — |
| `focus.go` | 49 | No change | 0 | — |
| `workflow_phasebar.go` | 48 | No change | 0 | — |
| `hint_bar.go` | 61 | No change | 0 | — |
| `loading_indicator.go` | 58 | No change | 0 | — |
| `cursor_indicator.go` | 46 | No change | 0 | — |
| `screen_title.go` | 33 | No change | 0 | — |
| `shortcut_tip.go` | 83 | No change | 0 | — |
| `breadcrumb.go` | 37 | No change | 0 | — |
| `base.go` | 35 | No change | 0 | — |
| `metriccard.go` | 128 | No change | 0 | — |
| `divider.go` | 47 | No change | 0 | — |
| `codeblock.go` | 18 | No change | 0 | — |

**New files:**
| File | Estimated LOC | Phase |
|------|---------------|-------|
| `components/narrative.go` | ~250 | M3 |
| `components/permission_desc.go` | ~180 | M4 |
| `components/empty_state_templates.go` | ~100 | M7 |

### App Core (`internal/tui/`)

| File | Current LOC | Classification | Effort | Phase |
|------|-------------|----------------|--------|-------|
| `app_view.go` | 1,119 | Moderate refactor | 6h | M1, M4, M5 |
| `app_state.go` | 411 | No change | 0 | — |
| `app.go` | 757 | No change | 0 | — |
| `app_update.go` | 2,219 | No change | 0 | — |
| `app_update_commands.go` | 481 | No change | 0 | — |
| `app_update_phase.go` | 382 | No change | 0 | — |
| `app_routing.go` | 402 | No change | 0 | — |
| `app_channel.go` | 105 | No change | 0 | — |
| `app_handlers.go` | 175 | No change | 0 | — |
| `app_handlers_config.go` | 99 | No change | 0 | — |
| `app_handlers_misc.go` | 225 | Minor change | 1h | M2 |
| `app_handlers_provider.go` | 117 | No change | 0 | — |
| `app_handlers_tick.go` | 106 | No change | 0 | — |
| `app_handlers_workflow.go` | 464 | No change | 0 | — |
| `types.go` | 107 | No change | 0 | — |
| `constants.go` | 52 | Minor change | 0.5h | M2 |
| `streaming.go` | 35 | No change | 0 | — |

### REPL System (`internal/tui/`)

| File | Current LOC | Classification | Effort | Phase |
|------|-------------|----------------|--------|-------|
| `repl.go` | 523 | No change | 0 | — |
| `repl_model.go` | 192 | Minor change | 1h | M3 |
| `repl_state.go` | 541 | No change | 0 | — |
| `repl_view.go` | 381 | Minor change | 2h | M1, M8 |
| `repl_stream.go` | 315 | Moderate refactor | 4h | M3 |
| `repl_mouse.go` | 301 | No change | 0 | — |
| `repl_footer.go` | 291 | Moderate refactor | 3h | M1, M6 |
| `repl_scrollbar.go` | 104 | No change | 0 | — |
| `repl_search.go` | 165 | No change | 0 | — |
| `repl_clipboard.go` | 74 | No change | 0 | — |
| `repl_commands.go` | 43 | No change | 0 | — |
| `repl_quickactions.go` | 72 | No change | 0 | — |
| `repl_welcome.go` | 367 | Moderate refactor | 3h | M7 |
| `repl_thinking.go` | 44 | No change | 0 | — |

### Sidebar (`internal/tui/`)

| File | Current LOC | Classification | Effort | Phase |
|------|-------------|----------------|--------|-------|
| `sidebar_model.go` | 1,429 | Large refactor | 8h | M2 |

### Screen Models (`internal/tui/`)

| File | Current LOC | Classification | Effort | Phase |
|------|-------------|----------------|--------|-------|
| `home_model.go` | 216 | Minor change | 1h | M1 |
| `home_view.go` | 175 | Moderate refactor | 2h | M1 |
| `plan_model.go` | 385 | Minor change | 2h | M5 |
| `plan_view.go` | 39 | Minor change | 0.5h | M5 |
| `plan_refine.go` | 60 | No change | 0 | — |
| `execute_model.go` | 318 | Moderate refactor | 3h | M5 |
| `execute_view.go` | 12 | No change | 0 | — |
| `verify_model.go` | 337 | Minor change | 1.5h | M5 |
| `ship_model.go` | 232 | Moderate refactor | 3h | M5 |
| `ship_view.go` | 24 | Minor change | 0.5h | M5 |
| `discuss_model.go` | 245 | Moderate refactor | 2h | M5 |
| `runtime_model.go` | 168 | No change | 0 | — |
| `runtime_view.go` | 17 | No change | 0 | — |
| `settings_model.go` | 857 | No change | 0 | — |
| `settings_view.go` | 47 | No change | 0 | — |
| `settings_tabs.go` | 35 | No change | 0 | — |
| `config_model.go` | 1,134 | No change | 0 | — |
| `modelselector_model.go` | 293 | No change | 0 | — |
| `modelselector_view.go` | 138 | No change | 0 | — |
| `modelselector_list.go` | 182 | No change | 0 | — |
| `phasemodelpicker.go` | 298 | No change | 0 | — |
| `phasemodelpicker_view.go` | 158 | No change | 0 | — |
| `diff_model.go` | 119 | No change | 0 | — |
| `diff_view.go` | 130 | No change | 0 | — |
| `rollback_model.go` | 288 | No change | 0 | — |
| `ledger_model.go` | 156 | No change | 0 | — |
| `metrics_model.go` | 248 | No change | 0 | — |
| `bisect_model.go` | 247 | No change | 0 | — |
| `confirmquit_model.go` | 85 | No change | 0 | — |
| `goalinput_model.go` | 159 | No change | 0 | — |
| `resume_model.go` | 212 | No change | 0 | — |
| `resume_view.go` | 132 | No change | 0 | — |
| `notification_model.go` | 82 | No change | 0 | — |
| `dashboard_model.go` | 230 | No change | 0 | — |
| `sessiondetail_model.go` | 122 | No change | 0 | — |
| `fileexplorer_model.go` | 142 | No change | 0 | — |
| `tooldetail_model.go` | 92 | Minor change | 1h | M8 |
| `ghostpicker_model.go` | 193 | No change | 0 | — |
| `ghostoutput_model.go` | 178 | No change | 0 | — |
| `commandpalette_model.go` | 524 | No change | 0 | — |
| `chathistory_model.go` | 147 | No change | 0 | — |
| `chathistory_view.go` | 130 | No change | 0 | — |
| `subagents_model.go` | 317 | No change | 0 | — |

### Support Files (`internal/tui/`)

| File | Current LOC | Classification | Effort | Phase |
|------|-------------|----------------|--------|-------|
| `cmdpalette.go` | 473 | No change | 0 | — |
| `commands.go` | 24 | No change | 0 | — |
| `helpers.go` | 396 | Minor change | 1h | M1, M7 |
| `header.go` | 56 | No change | 0 | — |
| `keybindings.go` | 246 | No change | 0 | — |
| `keybindings_screens.go` | 38 | No change | 0 | — |
| `provider_registration.go` | 119 | No change | 0 | — |
| `mention.go` | 256 | No change | 0 | — |
| `mention_view.go` | 210 | No change | 0 | — |
| `transition.go` | 404 | No change | 0 | — |
| `toast.go` | 103 | No change | 0 | — |
| `truncate.go` | 27 | No change | 0 | — |
| `render_diff.go` | 78 | No change | 0 | — |
| `filewatcher.go` | 179 | No change | 0 | — |
| `subagent_bridge.go` | 36 | No change | 0 | — |

## 4.2 Summary

| Classification | Count | Total LOC |
|----------------|-------|-----------|
| No change | 115 | 29,800 |
| Minor change | 22 | 1,400 |
| Moderate refactor | 12 | 3,200 |
| Large refactor | 1 | 1,429 |
| Rewrite | 0 | 0 |
| **New files** | **3** | **~530** |
| **Total changed** | **38** | **~6,560** |

---

# 5. Reusable Components

## 5.1 New Reusable Components

### NarrativeRenderer (`components/narrative.go`)

**Purpose:** Renders tool groups as intent-labeled blocks.
**Interface:**
```go
type NarrativeRenderer struct {
    theme  theme.Theme
    width  int
}

type ToolGroup struct {
    Intent    string
    Files     []string
    Command   string
    Failed    bool
    Error     string
}

func (r *NarrativeRenderer) RenderGroup(group ToolGroup) string
func (r *NarrativeRenderer) RenderFailure(group ToolGroup) string
func (r *NarrativeRenderer) RenderIntentLabel(intent string) string
```

**Used by:** `components/message.go`, `repl_stream.go`
**Lines:** ~250

### PermissionDescription (`components/permission_desc.go`)

**Purpose:** Generates plain English descriptions for permission requests.
**Interface:**
```go
type PermissionDescription struct {
    ToolName    string
    Command     string
    RiskLevel   string
    Phase       string
    Goal        string
}

func (d PermissionDescription) Describe() string     // "M31A wants to..."
func (d PermissionDescription) Consequence() string   // "This will..."
func (d PermissionDescription) RenderCommand() string // code block
```

**Used by:** `components/permission.go`, `app_view.go`
**Lines:** ~180

### EmptyStateTemplates (`components/empty_state_templates.go`)

**Purpose:** Standard empty state layouts for all screens.
**Interface:**
```go
type EmptyStateTemplate struct {
    Title       string
    Description string
    Action      string
    Icon        string
}

func (t EmptyStateTemplate) Render(width int) string
```

**Used by:** All screen models
**Lines:** ~100

## 5.2 Existing Components to Generalize

### SectionDivider (`components/divider.go`)
- **Current:** Used in sidebar with section titles
- **Generalize:** Add option to suppress border line (whitespace-only mode)
- **Effort:** 0.5h

### Card (`components/card.go`)
- **Current:** Used for permission modal, discuss, plan, ship
- **Generalize:** Add `Borderless` variant for screens that want card structure without border
- **Effort:** 0.5h

### ProgressBar (`components/progress.go`)
- **Current:** Complex animated progress bar
- **Generalize:** Add `Compact` variant for single-line display (execute screen)
- **Effort:** 1h

### EmptyState (`components/empty_state.go`)
- **Current:** Basic empty state with action
- **Generalize:** Add icon, description, and multi-action support
- **Effort:** 1h

## 5.3 Components NOT to Generalize

- **ToolCard:** Keep as standalone tool renderer. NarrativeRenderer handles groups.
- **MessageRenderer:** Keep as conversation renderer. NarrativeRenderer is separate.
- **ThinkingBlock:** Keep as thinking renderer. Thinking labels are in footer, not block.
- **Sparkline:** Remove from header. Keep for future analytics features.

---

# 6. Technical Debt Report

## 6.1 Items to Remove After Migration

| Item | Location | Reason | Remove In |
|------|----------|--------|-----------|
| Leader dots rendering | `layout/page.go` L144 | Decorative, removed by spec | M1 |
| Provider badge rendering | `layout/page.go` L387-406 | Redundant with model name | M1 |
| Context meter in header | `layout/page.go` L167-211 | Duplicated in sidebar/footer | M1 |
| `│` separator rendering | `layout/page.go` L96, L224 | Visual noise, removed by spec | M1 |
| Context ring in footer | `repl_footer.go` L252-291 | Duplicated in sidebar | M1 |
| Cost in footer | `repl_footer.go` L127-138 | Single source: sidebar | M1 |
| Sidebar section dividers | `sidebar_model.go` (all render* methods) | Use whitespace | M2 |
| Sidebar version display | `sidebar_model.go` renderHeader | Available via /version | M2 |
| Sidebar session ID | `sidebar_model.go` renderSession | Available via /session | M2 |
| Sidebar keyboard hints | `sidebar_model.go` renderHints | Footer handles this | M2 |
| Sidebar burn rate | `sidebar_model.go` renderTokenUsage | Debug metric, hidden | M2 |
| Sidebar speed metrics | `sidebar_model.go` renderSpeedMetrics | Debug metric, hidden | M2 |
| Sidebar tool timeline | `sidebar_model.go` renderToolTimeline | Replaced by narrative | M2 |
| Sidebar cost trend arrows | `sidebar_model.go` renderTokenUsage | Noise | M2 |
| Sidebar gauge bar | `sidebar_model.go` renderTokenUsage L884-912 | Replace with text | M2 |
| Sidebar colored pills | `sidebar_model.go` renderGitStatus L849-865 | Replace with counts | M2 |
| Card borders on Plan screen | `plan_model.go` View() | Whitespace suffices | M5 |
| Card borders on Discuss screen | `discuss_model.go` View() | Whitespace suffices | M5 |
| Card wrapper on Ship screen | `ship_model.go` View() | Whitespace suffices | M5 |
| Dot progress in Discuss | `discuss_model.go` View() L154-176 | "2 of 3" is simpler | M5 |
| Section divider in Discuss | `discuss_model.go` View() L225-228 | Decorative | M5 |
| Explicit key hints in screens | discuss, plan, execute View() | Footer handles | M5 |
| Timer in thinking display | `repl_footer.go` L93-98 | Intent labels replace | M6 |
| Glow effect in home | `home_view.go` renderHome L24-35 | Decorative | M1 |
| Version in home content | `home_view.go` renderHome L55-60 | Available elsewhere | M1 |
| `renderGradientSeparator` | `repl_welcome.go` | Decorative | M7 |
| `renderLogoWithGlow` | `repl_welcome.go` | Replaced by simple logo | M7 |

## 6.2 Items to Deprecate (Keep but Don't Use)

| Item | Location | Reason |
|------|----------|--------|
| `ToolIcons` map | `components/toolcard.go` L38-47 | Used by ToolDetail only |
| `ToolStatusIcons` map | `components/toolcard.go` L50-54 | Used by standalone tools only |
| `Sparkline` component | `components/sparkline.go` | Remove from header, keep for analytics |
| `SectionDivider` border mode | `components/divider.go` | Keep for focused sidebar mode |
| `Card` bordered mode | `components/card.go` | Keep for modals |

## 6.3 Legacy Compatibility Layers

| Layer | Location | Keep Until |
|-------|----------|------------|
| Old ToolCard rendering | `components/toolcard.go` | M8 complete |
| Old sidebar full mode | `sidebar_model.go` | M2 complete |
| Old permission modal format | `components/permission.go` | M4 complete |

---

# 7. Regression Strategy

## 7.1 Per-Milestone Regression Checklist

Every completed milestone must pass:

### Functional Regression
- [ ] `go test ./internal/tui/...` — all unit tests pass
- [ ] `go test ./internal/tui/components/...` — component tests pass
- [ ] `go test ./internal/tui/layout/...` — layout tests pass
- [ ] `go test ./internal/tui/theme/...` — theme tests pass
- [ ] `go test -race ./...` — no race conditions

### Visual Regression
- [ ] Screenshot at 80×24 (standard terminal)
- [ ] Screenshot at 120×40 (large terminal)
- [ ] Screenshot at 60×24 (compact terminal)
- [ ] Screenshot at 40×15 (minimum terminal)
- [ ] Screenshot at 160×50 (ultra-wide)

### Responsive Regression
- [ ] Header renders correctly at all breakpoints
- [ ] Footer renders correctly at all breakpoints
- [ ] Sidebar renders/hidden correctly at all breakpoints
- [ ] Content area adjusts correctly at all breakpoints
- [ ] Modals render correctly at all breakpoints

### Interaction Regression
- [ ] All keyboard shortcuts work
- [ ] Mouse scroll works in viewports
- [ ] Click-to-expand works on collapsed elements
- [ ] Ctrl+P command palette opens/closes
- [ ] Ctrl+B sidebar toggles
- [ ] Escape navigates back
- [ ] Enter submits/activates
- [ ] Tab cycles in modals

### Workflow Regression
- [ ] Full workflow: goal → discuss → plan → execute → verify → ship
- [ ] Permission workflow: request → approve → execute
- [ ] Permission workflow: request → deny → adjust
- [ ] Streaming: response streams correctly
- [ ] Thinking: thinking state displays correctly
- [ ] Error: errors display correctly
- [ ] Self-healing: failed tasks trigger retry

### Terminal Environment Regression
- [ ] Works in tmux
- [ ] Works over SSH
- [ ] Works with 256-color terminal
- [ ] Works with truecolor terminal
- [ ] Works with no-color terminal (NO_COLOR=1)

### Long-Running Regression
- [ ] 100+ messages in conversation (no lag)
- [ ] 500+ messages in conversation (acceptable performance)
- [ ] 50+ tool calls in sequence (no crash)
- [ ] Streaming for 60+ seconds (no memory leak)

## 7.2 Visual Regression Process

1. Before starting M1, capture baseline screenshots at all breakpoints
2. After each milestone, capture new screenshots
3. Diff screenshots manually or with tool
4. Any unintended visual change must be fixed before merge
5. Intentional visual changes must match UX spec wireframes

## 7.3 Automated Regression

```bash
# Full test suite
go test ./...

# Race detection
go test -race ./...

# Coverage
go test -coverprofile=coverage.out ./...
go tool cover -html=coverage.out

# Benchmark (performance regression)
go test -bench=. ./internal/tui/...
```

---

# 8. Rollback Strategy

## 8.1 Per-Milestone Rollback

Every milestone is designed for clean rollback:

| Milestone | Rollback Scope | Data Loss | User Impact |
|-----------|---------------|-----------|-------------|
| M1 | Header/footer/home only | None | Visual only |
| M2 | Sidebar only | None | Sidebar reverts to old layout |
| M3 | Tool rendering only | None | Tools revert to individual cards |
| M4 | Permission modal only | None | Modal reverts to old format |
| M5 | Screen views only | None | Screens revert to old layout |
| M6 | Thinking display only | None | Timer reverts |
| M7 | Empty states only | None | Old empty messages return |
| M8 | Tool collapse only | None | Tools always expanded |
| M9 | Contrast/spacing fixes | None | Minor visual adjustments |
| M10 | Tests/docs only | None | N/A |

## 8.2 Rollback Commands

```bash
# Rollback specific milestone
git revert --no-commit <milestone-merge-commit>

# Rollback to pre-UX state
git checkout v1.6.1 -- internal/tui/

# Rollback specific file
git checkout HEAD~1 -- internal/tui/layout/page.go
```

## 8.3 Feature Flag Rollback

M1 introduces a feature flag `UX_V2_CHROME` in config:

```toml
[ui]
ux_v2_chrome = true  # Set to false to revert to old chrome
```

This allows runtime rollback without code changes. Flag is removed in M10 after stabilization.

## 8.4 Emergency Rollback

If a critical bug is found after release:

1. Revert the milestone merge commit
2. Tag as `v1.6.2-hotfix-N`
3. Push to release branch
4. Users can `git pull` to get the fix

---

# 9. Risk Analysis

## 9.1 Risk Matrix

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| M3 (Narrative) breaks conversation rendering | Medium | High | Feature flag, gradual rollout, fallback to ToolCard |
| M2 (Sidebar) introduces layout bugs | Medium | Medium | Extensive responsive testing, focused mode as fallback |
| M4 (Permission) affects safety | Low | Critical | Thorough testing of deny defaults, keep old format as fallback |
| M5 (Workflow) breaks workflow execution | Low | High | End-to-end workflow test before merge |
| Merge conflicts between milestones | Medium | Medium | Separate branches, clear file ownership |
| Performance regression from narrative grouping | Low | Medium | Benchmark before/after, group buffer limits |
| Accessibility regression | Low | Low | Per-milestone accessibility checks |

## 9.2 High-Risk Areas

### M3: Narrative Engine
**Risk:** The narrative engine is the most complex new component. It must correctly classify 18+ tool patterns, handle edge cases (mixed intents, failures, single tools), and integrate with the existing streaming pipeline.

**Mitigation:**
- Build with comprehensive unit tests (50+ test cases)
- Start with simple cases (FileRead, Glob, Grep)
- Add complexity incrementally (Bash, Edit, failures)
- Keep ToolCard as fallback
- Feature flag for gradual rollout

### M2: Sidebar Redesign
**Risk:** The sidebar has 1,429 lines and ~40 methods. Changing its rendering logic could break layout at various terminal sizes.

**Mitigation:**
- Add new modes alongside old rendering
- Old rendering becomes FocusedMode (backward compatible)
- Extensive responsive testing
- Separate branch for sidebar work

### M4: Permission Modal
**Risk:** Permission is safety-critical. Incorrect descriptions could lead to unintended destructive actions.

**Mitigation:**
- Description generator has fallback to old format
- "M31A wants to..." always checked against actual command
- Deny is always the default (Escape = deny)
- Thorough testing of each tool type

## 9.3 Low-Risk Areas

- M1 (chrome): Visual only, no functional changes
- M6 (thinking): Display only, timer preserved as fallback
- M7 (empty states): Additive, no existing behavior changed
- M8 (progressive disclosure): Enhancement of existing collapse behavior
- M9 (accessibility): Adjustments, not redesigns
- M10 (testing/docs): Additive

---

# 10. Implementation Schedule

## 10.1 Gantt Chart

```
Week 1:  ████████████████████████████████████
         M1: Layout Foundation (3 days)
         M3: Narrative Engine (4 days, parallel)

Week 2:  ████████████████████████████████████
         M2: Sidebar Redesign (3 days)
         M4: Permission Modal (2 days)
         M6: Thinking Display (1 day)

Week 3:  ████████████████████████████████████
         M5: Workflow Screens (3 days)
         M8: Progressive Disclosure (1 day)
         M7: Empty States (2 days)

Week 4:  ████████████████████████████████████
         M9: Accessibility Audit (3 days)
         M10: Regression & Docs (4 days)

Week 5:  ████████████████████████████████████
         M10: Regression & Docs (continued)
         Buffer for issues
```

## 10.2 Detailed Schedule

| Milestone | Start | End | Engineer | Branch |
|-----------|-------|-----|----------|--------|
| M1 | Day 1 | Day 3 | Engineer A | `ux/m1-chrome` |
| M3 | Day 1 | Day 4 | Engineer B | `ux/m3-narrative` |
| M2 | Day 5 | Day 7 | Engineer A | `ux/m2-sidebar` |
| M4 | Day 5 | Day 6 | Engineer C | `ux/m4-permission` |
| M6 | Day 5 | Day 5 | Engineer C | `ux/m6-thinking` |
| M5 | Day 8 | Day 10 | Engineer A | `ux/m5-workflow` |
| M8 | Day 8 | Day 8 | Engineer B | `ux/m8-disclosure` |
| M7 | Day 8 | Day 9 | Engineer C | `ux/m7-empty` |
| M9 | Day 11 | Day 13 | All | `ux/m9-accessibility` |
| M10 | Day 11 | Day 15 | All | `ux/m10-testing` |

## 10.3 Merge Windows

| Merge | After | Window | Risk Gate |
|-------|-------|--------|-----------|
| M1 | Tests pass | Day 3-4 | Visual regression |
| M3 | Tests pass | Day 4-5 | Narrative unit tests |
| M2 | M1 merged | Day 7-8 | Sidebar responsive |
| M4 | M1 merged | Day 6-7 | Permission safety |
| M6 | M1 merged | Day 5-6 | Thinking labels |
| M5 | M1+M3 merged | Day 10-11 | Workflow e2e |
| M8 | M3 merged | Day 9-10 | Collapse behavior |
| M7 | M1+M3 merged | Day 9-10 | Empty state visual |
| M9 | All merged | Day 13-14 | Accessibility |
| M10 | M9 merged | Day 15 | Full regression |

## 10.4 Buffer

5 days buffer (Days 16-20) for:
- Unexpected bugs
- Design review feedback
- Performance optimization
- Edge case handling

---

# 11. Definition of Done

## 11.1 Global Definition of Done

A milestone is DONE when:

1. **All acceptance criteria met** (per milestone section above)
2. **All tests pass** (`go test -race ./...`)
3. **Visual regression verified** (screenshots match spec)
4. **Responsive behavior verified** (40-160 columns)
5. **Code review approved** (at least 1 reviewer)
6. **No regressions** (existing functionality preserved)
7. **Documentation updated** (if applicable)
8. **Merged to main** (no open threads)

## 11.2 Per-Milestone Done Criteria

### M1: Layout Foundation — DONE when:
- [ ] Header shows brand + breadcrumb + model (no extras)
- [ ] Footer shows cwd+branch + operation + hints (no extras)
- [ ] Home shows brand + tagline + input (no decorations)
- [ ] All responsive breakpoints work
- [ ] Feature flag `UX_V2_CHROME` works (toggle old/new)
- [ ] All existing keyboard shortcuts work
- [ ] `go test -race ./internal/tui/...` passes
- [ ] Visual screenshots match spec wireframes

### M2: Sidebar Redesign — DONE when:
- [ ] Idle mode: brand + git + file counts (3 lines)
- [ ] Active mode: brand + activity + files + context (5-7 lines)
- [ ] Focused mode: all sections (backward compatible)
- [ ] ctrl+b toggles, ctrl+g switches modes
- [ ] Sidebar works at all responsive breakpoints
- [ ] No section dividers (whitespace only)
- [ ] Git shows `+3 -1 ?2` format
- [ ] Context shows `42% context` text
- [ ] All sidebar tests pass
- [ ] Sidebar responsive tests pass

### M3: Narrative Engine — DONE when:
- [ ] FileRead+Glob+Grep → single intent block
- [ ] Intent labels are human-readable
- [ ] File lists show max 5 + overflow
- [ ] Failed tools render standalone with context
- [ ] Bash always standalone
- [ ] Edit always standalone
- [ ] No tool duration in inline view
- [ ] Group buffer flush rules work
- [ ] All narrative unit tests pass (50+ cases)
- [ ] Conversation rendering works with narrative

### M4: Permission Modal — DONE when:
- [ ] "M31A wants to..." as primary text
- [ ] Consequence explanation present
- [ ] Command in code block below divider
- [ ] Risk via border color (not text label)
- [ ] Countdown only <5s
- [ ] Escape = deny always
- [ ] "Allow All" works for batch
- [ ] Fallback to old format works
- [ ] Permission tests pass

### M5: Workflow Screens — DONE when:
- [ ] Execute: activity label replaces stats
- [ ] Execute: progress bar is single line
- [ ] Execute: completed tasks collapsed
- [ ] Execute: failed task shows error
- [ ] Plan: no card border
- [ ] Discuss: no card border, "2 of 3" progress
- [ ] Ship: no card wrapper, stats on 2 lines
- [ ] All workflow screens at correct visual spec
- [ ] Full workflow execution passes

### M6: Thinking Display — DONE when:
- [ ] Intent labels replace timers
- [ ] Labels change by phase
- [ ] Duration accessible on demand
- [ ] Thinking tests pass

### M7: Empty States — DONE when:
- [ ] Every screen has designed empty state
- [ ] Empty states guide to next action
- [ ] Consistent typography/spacing
- [ ] Readable at all terminal sizes
- [ ] Empty state component tests pass

### M8: Progressive Disclosure — DONE when:
- [ ] Successful tools collapsed by default
- [ ] Failed tools auto-expand
- [ ] Click/Enter expands
- [ ] Escape collapses
- [ ] ToolDetail available for full inspection
- [ ] No information loss
- [ ] Collapse/expand tests pass

### M9: Accessibility — DONE when:
- [ ] All text WCAG AA contrast
- [ ] All actions keyboard-accessible
- [ ] No color-only information
- [ ] All screens at 40+ columns
- [ ] Works in tmux
- [ ] Works over SSH
- [ ] No focus traps

### M10: Regression & Docs — DONE when:
- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] `go test -race ./...` passes
- [ ] Visual baselines established
- [ ] UX documentation complete
- [ ] Keybinding docs updated
- [ ] No open issues

---

# Appendix A: File Change Summary

## New Files (3)

| File | Lines | Purpose | Phase |
|------|-------|---------|-------|
| `components/narrative.go` | ~250 | Tool grouping + intent labels | M3 |
| `components/permission_desc.go` | ~180 | Permission descriptions | M4 |
| `components/empty_state_templates.go` | ~100 | Empty state layouts | M7 |

## Modified Files (38)

| File | Classification | LOC Changed | Phase |
|------|----------------|-------------|-------|
| `layout/page.go` | Moderate refactor | ~120 | M1 |
| `layout/responsive.go` | Minor change | ~5 | M1 |
| `app_view.go` | Moderate refactor | ~200 | M1,M4,M5 |
| `home_view.go` | Moderate refactor | ~60 | M1 |
| `home_model.go` | Minor change | ~15 | M1 |
| `repl_footer.go` | Moderate refactor | ~130 | M1,M6 |
| `repl_view.go` | Minor change | ~35 | M1,M8 |
| `helpers.go` | Minor change | ~40 | M1,M7 |
| `sidebar_model.go` | Large refactor | ~350 | M2 |
| `app_handlers_misc.go` | Minor change | ~15 | M2 |
| `constants.go` | Minor change | ~5 | M2 |
| `components/message.go` | Moderate refactor | ~80 | M3 |
| `components/toolcard.go` | Moderate refactor | ~90 | M3,M8 |
| `repl_stream.go` | Moderate refactor | ~100 | M3 |
| `repl_model.go` | Minor change | ~20 | M3 |
| `components/permission.go` | Moderate refactor | ~120 | M4 |
| `components/thinking.go` | Minor change | ~20 | M6 |
| `execute_model.go` | Moderate refactor | ~100 | M5 |
| `plan_model.go` | Minor change | ~40 | M5 |
| `plan_view.go` | Minor change | ~15 | M5 |
| `discuss_model.go` | Moderate refactor | ~60 | M5 |
| `ship_model.go` | Moderate refactor | ~80 | M5 |
| `ship_view.go` | Minor change | ~20 | M5 |
| `verify_model.go` | Minor change | ~30 | M5 |
| `components/empty_state.go` | Moderate refactor | ~80 | M7 |
| `repl_welcome.go` | Moderate refactor | ~100 | M7 |
| `tooldetail_model.go` | Minor change | ~50 | M8 |
| `theme/tokens_semantic.go` | Minor change | ~20 | M9 |

**Total modified:** 38 files, ~2,325 LOC changed

## Unchanged Files (127)

All other TUI files remain unchanged. This represents 76% of the codebase requiring zero modification.

---

# Appendix B: Estimated Effort by Milestone

| Milestone | Implementation | Review | Testing | Total |
|-----------|---------------|--------|---------|-------|
| M1: Layout Foundation | 2.5 days | 0.5 day | 0.5 day | 3.5 days |
| M2: Sidebar Redesign | 3 days | 0.5 day | 1 day | 4.5 days |
| M3: Narrative Engine | 3.5 days | 1 day | 1 day | 5.5 days |
| M4: Permission Modal | 2 days | 0.5 day | 0.5 day | 3 days |
| M5: Workflow Screens | 2.5 days | 0.5 day | 0.5 day | 3.5 days |
| M6: Thinking Display | 0.5 day | 0.25 day | 0.25 day | 1 day |
| M7: Empty States | 1.5 days | 0.25 day | 0.25 day | 2 days |
| M8: Progressive Disclosure | 1 day | 0.25 day | 0.25 day | 1.5 days |
| M9: Accessibility | 2 days | 0.5 day | 1 day | 3.5 days |
| M10: Regression & Docs | 3 days | 1 day | 2 days | 6 days |
| **Buffer** | | | | **5 days** |
| **TOTAL** | **22 days** | **5.25 days** | **7.25 days** | **39.5 days** |

---

# Appendix C: Success Metrics

After M10 is complete, the following metrics should be measured:

| Metric | Baseline (v1.6.1) | Target (v1.6.2) |
|--------|-------------------|-----------------|
| Information layers visible by default | 4 | 2 (Conversation + Context) |
| Sidebar sections visible by default | 11 | 2-3 |
| Tool names shown inline | Every call | 0 (intent labels only) |
| Context displays | 3 (header+sidebar+footer) | 1 (footer) |
| Cost displays | 3 (header+sidebar+footer) | 1 (sidebar) |
| Card borders on content screens | 4 (Plan,Discuss,Ship,Verify) | 0 |
| Section dividers in sidebar | 7 | 0 |
| Decorative elements (glow, dots, etc.) | 4 | 0 |
| Empty states designed | ~5 of 33 | 33 of 33 |
| WCAG AA contrast compliance | ~80% | 100% |
| Keyboard-only accessible actions | ~90% | 100% |

---

*This document is the definitive engineering execution plan for M31A v1.6.2. All implementation must follow this plan. Amendments require review and approval.*
