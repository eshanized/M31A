# M31A v1.6.2 — UX Verification & Review Checklist

**Document Status:** FINAL — Review Contract
**Version:** 1.6.2
**Date:** 2026-07-01
**Reference:** UX-ARCHITECTURE-SPEC.md, ENGINEERING-EXECUTION-PLAN.md

---

# How to Use This Document

Every PR for v1.6.2 must be reviewed against the applicable checklists below.

1. **Before opening a PR:** The engineer completes the Definition of Ready checklist.
2. **During PR review:** The reviewer completes the PR Review Checklist and all applicable category checklists.
3. **Before merge:** All applicable items must be checked. Any unchecked item blocks merge.
4. **After merge:** Visual regression baselines are updated if intentional visual changes were made.

**Blocking rule:** Any item marked with ⛔ is a hard block. Any item marked with ⚠️ is a soft block (requires justification). Items marked with ℹ️ are advisory.

---

# 1. PR Review Checklist

Every PR must satisfy every applicable item.

## 1.1 Code Quality

- [ ] ⛔ `go vet ./...` passes
- [ ] ⛔ `gofmt -l .` shows no output
- [ ] ⛔ `go test -race ./...` passes
- [ ] ⛔ `go test ./internal/tui/...` passes
- [ ] ⛔ `go test ./internal/tui/components/...` passes
- [ ] ⛔ `go test ./internal/tui/layout/...` passes
- [ ] ⛔ No new lint warnings (`golangci-lint run`)
- [ ] ⛔ No new `errcheck` failures
- [ ] ⛔ No new `unused` failures
- [ ] ℹ️ Test coverage does not decrease

## 1.2 Spec Compliance

- [ ] ⛔ Changes match the UX Architecture Specification exactly
- [ ] ⛔ No UI elements added that are not in the spec
- [ ] ⛔ No UI elements modified beyond what the spec allows
- [ ] ⛔ No new features introduced (scope control)
- [ ] ⚠️ If spec ambiguity exists, document the interpretation in PR description

## 1.3 Backward Compatibility

- [ ] ⛔ All existing keyboard shortcuts work
- [ ] ⛔ All existing slash commands work
- [ ] ⛔ All existing config options work
- [ ] ⛔ Session files from v1.6.1 are loadable
- [ ] ⛔ No breaking changes to provider interface
- [ ] ⛔ Feature flag `UX_V2_CHROME` works (if applicable)

## 1.4 File Hygiene

- [ ] ⛔ No commented-out code left in
- [ ] ⛔ No debug `fmt.Println` or `log.Println` left in
- [ ] ⛔ No TODO/FIXME comments added without issue reference
- [ ] ⛔ No hardcoded values that should be constants
- [ ] ⛔ Imports are grouped correctly (stdlib / third-party / project)
- [ ] ℹ️ File names follow existing conventions

## 1.5 Documentation

- [ ] ⚠️ PR description explains what changed and why
- [ ] ⚠️ PR description includes screenshots for visual changes
- [ ] ℹ️ Code comments explain non-obvious decisions
- [ ] ℹ️ README updated if public API changed

---

# 2. UX Acceptance Checklist

Complete for every milestone PR.

## 2.1 Visual Hierarchy

- [ ] ⛔ Conversation is the dominant visual element
- [ ] ⛔ Narrative is always secondary to conversation
- [ ] ⛔ Context information is peripheral (footer, sidebar when active)
- [ ] ⛔ System information is hidden by default
- [ ] ⛔ Typography communicates hierarchy before color
- [ ] ⛔ Spacing is consistent across all screens
- [ ] ⛔ Nothing competes with the conversation for attention
- [ ] ⚠️ Font weights are used correctly (bold for headings, regular for body)
- [ ] ⚠️ Muted text is readable but clearly secondary
- [ ] ℹ️ Brand color is used sparingly (headers, active states only)

## 2.2 Information Architecture

- [ ] ⛔ No information is duplicated across components
- [ ] ⛔ Every data point has exactly one source of truth:
  - Context usage → footer only (not header, not sidebar)
  - Cost → sidebar only (not footer, not header)
  - Git branch → footer only (not sidebar in idle mode)
  - Model name → header only (not footer, not sidebar)
  - Version → header only (not sidebar, not home)
  - Session ID → hidden (accessible via /session)
- [ ] ⛔ Every screen has exactly one primary purpose
- [ ] ⛔ No unnecessary decorations exist (glow, leader dots, etc.)
- [ ] ⛔ No redundant badges or status indicators
- [ ] ⛔ No section dividers in sidebar (whitespace only)
- [ ] ⚠️ Screen labels in header match screen purpose

## 2.3 Conversation Rendering

- [ ] ⛔ User messages are visually distinct from assistant messages
- [ ] ⛔ Assistant messages support markdown rendering
- [ ] ⛔ Streaming text appears smoothly without flicker
- [ ] ⛔ Long conversations (100+ messages) render without lag
- [ ] ⛔ Tool groups render with intent labels
- [ ] ⛔ Individual tool names are NOT shown in grouped view
- [ ] ⛔ Tool durations are NOT shown in inline view
- [ ] ⛔ Failed tools render as standalone blocks with error context
- [ ] ⛔ Bash commands always render as standalone (user-facing)
- [ ] ⛔ File edits always render as standalone (important)
- [ ] ⚠️ Blank lines separate conversation elements correctly
- [ ] ℹ️ Code blocks are syntax-highlighted

## 2.4 Narrative Engine

- [ ] ⛔ Intent labels are human-readable (not implementation details)
- [ ] ⛔ File lists show max 5 files with overflow ("and N more")
- [ ] ⛔ Group buffer flushes correctly on intent change
- [ ] ⛔ Group buffer flushes correctly on failure
- [ ] ⛔ Group buffer flushes correctly on LLM text
- [ ] ⛔ Group buffer flushes at 5-tool limit
- [ ] ⚠️ Narrative appears during thinking, streaming, and tool execution
- [ ] ℹ️ Narrative labels are accurate for the current activity

## 2.5 Sidebar

- [ ] ⛔ Idle mode: brand + git + file counts only (3 lines max)
- [ ] ⛔ Active mode: brand + activity + files + context (5-7 lines)
- [ ] ⛔ Focused mode: all sections (backward compatible)
- [ ] ⛔ ctrl+b toggles sidebar visibility
- [ ] ⛔ ctrl+g switches between modes when visible
- [ ] ⛔ Sidebar hidden by default on screens <80 cols
- [ ] ⛔ Sidebar renders as overlay on screens 60-79 cols
- [ ] ⛔ No section dividers (whitespace separation only)
- [ ] ⛔ Git status shows `+3 -1 ?2` format (no colored pills)
- [ ] ⛔ Context shows `42% context` text (no gauge bar)
- [ ] ⛔ Version, session ID, hints, burn rate, speed metrics, tool timeline removed from default views
- [ ] ⚠️ Sidebar width is exactly 28 columns
- [ ] ℹ️ Sidebar content truncates with ellipsis if too long

## 2.6 Permission Modal

- [ ] ⛔ Primary text is "M31A wants to [action]" (plain English)
- [ ] ⛔ Consequence explanation is present
- [ ] ⛔ Command is shown in code block below divider (secondary)
- [ ] ⛔ Risk is communicated via border color accent (not text label)
- [ ] ⛔ Countdown only appears when <5 seconds remaining
- [ ] ⛔ Escape always means deny (safe default)
- [ ] ⛔ "Allow All" option is available for batch operations
- [ ] ⛔ Phase/goal context is woven into description
- [ ] ⚠️ Modal renders correctly at all terminal sizes
- [ ] ℹ️ Description generator has fallback for unknown tools

## 2.7 Workflow Screens

### Execute Screen
- [ ] ⛔ Current activity label replaces stats line
- [ ] ⛔ Progress bar is single, subtle line
- [ ] ⛔ Completed tasks collapsed to single line with ✓ icon
- [ ] ⛔ Failed task shows error inline (red, expanded)
- [ ] ⛔ Pending tasks are dimmed
- [ ] ⛔ ETA/elapsed removed from primary view

### Plan Screen
- [ ] ⛔ Card border removed
- [ ] ⛔ Content stands on its own with typography

### Discuss Screen
- [ ] ⛔ Card border removed
- [ ] ⛔ Progress simplified to "2 of 3" (no dot indicators for ≤5 questions)
- [ ] ⛔ Section divider removed
- [ ] ⛔ Key hints removed (footer handles)

### Ship Screen
- [ ] ⛔ Card wrapper removed
- [ ] ⛔ Title "Session Summary" removed
- [ ] ⛔ Stats consolidated to 2 lines
- [ ] ⛔ Commit log visible as secondary content

### Verify Screen
- [ ] ⛔ No card border
- [ ] ⛔ Failed tasks highlighted with error context

### Thinking Display
- [ ] ⛔ Intent labels replace timers ("Planning implementation" not "thinking · 3.2s")
- [ ] ⛔ Labels change based on phase context
- [ ] ⚠️ Duration accessible via hover/focus or sidebar

### Home Screen
- [ ] ⛔ Brand name only (2-3 rows max, no 6-row ASCII art)
- [ ] ⛔ One-line tagline present
- [ ] ⛔ Glow effect removed
- [ ] ⛔ Glow line removed
- [ ] ⛔ Version removed from content area

## 2.8 Empty States

- [ ] ⛔ Every screen has a designed empty state
- [ ] ⛔ Empty states guide users toward the next action
- [ ] ⛔ Empty states use consistent typography and spacing
- [ ] ⛔ No "N/A" or blank screens
- [ ] ⚠️ Empty states are readable at all terminal sizes

## 2.9 Progressive Disclosure

- [ ] ⛔ Successful tools collapsed by default
- [ ] ⛔ Failed tools auto-expand with error context
- [ ] ⛔ Click/Enter on collapsed tool expands it
- [ ] ⛔ Escape or click on expanded tool collapses it
- [ ] ⛔ ToolDetail screen available for full inspection
- [ ] ⛔ No information loss (all data accessible)

## 2.10 Navigation

- [ ] ⛔ Escape returns to previous screen on all screens
- [ ] ⛔ Escape on REPL is a no-op
- [ ] ⛔ Command palette (Ctrl+P) opens from any screen
- [ ] ⛔ Sidebar (Ctrl+B) toggles from any screen
- [ ] ⛔ Screen transitions are smooth (no jumps)
- [ ] ⚠️ Focus moves correctly on screen transitions
- [ ] ⚠️ Focus returns correctly on modal dismiss

---

# 3. Visual Regression Checklist

Complete for every milestone PR that changes rendering.

## 3.1 Screenshot Captures

Capture screenshots at every breakpoint for every changed screen:

- [ ] 40×15 (minimum terminal)
- [ ] 60×24 (compact terminal)
- [ ] 80×24 (standard terminal)
- [ ] 120×40 (large terminal)
- [ ] 160×50 (ultra-wide)

## 3.2 Per-Screen Verification

For each changed screen, verify:

### Header
- [ ] Brand name visible and correctly styled
- [ ] Breadcrumb shows correct screen name
- [ ] Model name visible (Full breakpoint only)
- [ ] No leader dots present
- [ ] No `│` separators present
- [ ] No provider badge present
- [ ] No context meter present

### Footer
- [ ] Cwd visible and correctly truncated
- [ ] Branch visible (Standard+ breakpoints)
- [ ] Operation text visible when active
- [ ] Hints visible (Full breakpoint only)
- [ ] No `│` separators present
- [ ] No cost displayed
- [ ] No context ring displayed

### Sidebar (when visible)
- [ ] Correct mode rendering (Idle/Active/Focused)
- [ ] No section dividers
- [ ] Git status in `+3 -1 ?2` format
- [ ] Context as `42% context` text
- [ ] Width exactly 28 columns
- [ ] Content truncates correctly

### Conversation
- [ ] Messages render correctly
- [ ] Tool groups render with intent labels
- [ ] No individual tool names in grouped view
- [ ] Failed tools render as standalone blocks
- [ ] Markdown renders correctly

### Modals
- [ ] Permission modal centered and readable
- [ ] Permission description in plain English
- [ ] Command in code block
- [ ] Buttons correctly styled
- [ ] Dimmed background visible

## 3.3 Layout Stability

- [ ] No layout jumps on screen transitions
- [ ] No content shifting when sidebar toggles
- [ ] No flicker on streaming
- [ ] No overflow at any terminal size
- [ ] No underflow (empty space) at any terminal size

## 3.4 Color Verification

- [ ] Brand color used correctly (headers, active states)
- [ ] Success color used for ✓ icons and positive states
- [ ] Warning color used for cost and caution states
- [ ] Error color used for ✗ icons and failures
- [ ] Muted color used for metadata and hints
- [ ] No color-only information conveyance

---

# 4. Accessibility Checklist

Complete for every milestone PR.

## 4.1 Terminal Sizes

- [ ] ⛔ All content readable at 40 columns
- [ ] ⛔ All content readable at 60 columns
- [ ] ⛔ All content readable at 80 columns
- [ ] ⛔ All content readable at 120 columns
- [ ] ⛔ All content readable at 160 columns
- [ ] ⛔ All interactive elements reachable at 40 columns
- [ ] ⚠️ Sidebar overlay works at 60-79 columns

## 4.2 Terminal Environments

- [ ] ⛔ Works in tmux (verify scrollback, colors, mouse)
- [ ] ⛔ Works over SSH (verify latency, rendering)
- [ ] ⛔ Works with NO_COLOR=1 (no color-dependent information)
- [ ] ⛔ Works with TERM=xterm-256color
- [ ] ⛔ Works with TERM=xterm
- [ ] ℹ️ Works with limited color terminals

## 4.3 Keyboard Accessibility

- [ ] ⛔ All actions have keyboard equivalents
- [ ] ⛔ No mouse-dependent information
- [ ] ⛔ Tab cycles within modals correctly
- [ ] ⛔ Focus is visible on all interactive elements
- [ ] ⛔ No focus traps (can always Escape)
- [ ] ⛔ Ctrl+C cancels operations correctly
- [ ] ⚠️ Keyboard shortcuts don't conflict with terminal defaults

## 4.4 Contrast

- [ ] ⛔ All text meets WCAG AA contrast ratio (4.5:1)
- [ ] ⛔ Muted text meets minimum contrast (3:1)
- [ ] ⛔ Interactive elements have visible focus indicators
- [ ] ⛔ Error/warning colors distinguishable from each other
- [ ] ℹ️ High contrast mode works if supported

## 4.5 Reduced Motion

- [ ] ⚠️ Animations can be disabled via config
- [ ] ⚠️ Spinner still conveys state without animation
- [ ] ℹ️ Screen transitions work without animation

## 4.6 Screen Reader (where applicable)

- [ ] ℹ️ Alt text for decorative elements (logo)
- [ ] ℹ️ Semantic structure (headings, lists)
- [ ] ℹ️ Status announcements for state changes

---

# 5. Performance Checklist

Complete for every milestone PR.

## 5.1 Rendering Performance

- [ ] ⛔ Frame render time <16ms (60fps) for normal operation
- [ ] ⛔ Frame render time <32ms (30fps) for heavy content
- [ ] ⛔ No new allocations in hot paths (Render methods)
- [ ] ⛔ No unnecessary string concatenation in render loops
- [ ] ⚠️ Glamour cache hit rate >80% for repeated content

## 5.2 Memory

- [ ] ⛔ No memory leaks in long-running sessions (500+ messages)
- [ ] ⛔ Tool group buffer properly flushed and GC'd
- [ ] ⛔ Narrative renderer doesn't accumulate state
- [ ] ⚠️ Memory usage comparable to v1.6.1 baseline

## 5.3 CPU

- [ ] ⛔ No busy-waiting or unnecessary goroutines
- [ ] ⛔ Tool group classification <1ms per tool call
- [ ] ⛔ Sidebar mode switch <50ms
- [ ] ⛔ Screen transition <100ms
- [ ] ⛔ Modal open/close <50ms
- [ ] ⚠️ Narrative flush <5ms
- [ ] ℹ️ Message render (cached) <1ms

## 5.4 Baseline Comparison

- [ ] ⛔ No performance regression against v1.6.1
- [ ] ⚠️ Benchmark before/after for changed render methods
- [ ] ℹ️ Profile hot paths if render time increases >10%

---

# 6. Product Identity Checklist

Every reviewer must answer these questions for every PR.

## 6.1 Calm

- [ ] ⛔ Does the interface feel calm?
- [ ] ⛔ Is there any visual noise that could be removed?
- [ ] ⛔ Are animations subtle and purposeful?
- [ ] ⛔ Does the system feel quiet when idle?
- [ ] ⚠️ Would a user feel relaxed using this interface?

## 6.2 Cognitive Load

- [ ] ⛔ Does this change reduce cognitive load?
- [ ] ⛔ Can a new user understand what's happening without documentation?
- [ ] ⛔ Is the most important information always most prominent?
- [ ] ⛔ Are implementation details hidden from non-technical users?
- [ ] ⚠️ Could any element be removed without losing essential information?

## 6.3 Intent Communication

- [ ] ⛔ Does the interface communicate what M31A is doing?
- [ ] ⛔ Does it communicate why M31A is doing it?
- [ ] ⛔ Does it communicate what happens next?
- [ ] ⛔ Are tool activities described in human-readable terms?
- [ ] ⚠️ Would a non-developer understand the progress display?

## 6.4 Premium Feel

- [ ] ⛔ Does this feel like a premium professional tool?
- [ ] ⛔ Would Apple, Linear, or Raycast ship this interaction?
- [ ] ⛔ Is every pixel earning its place?
- [ ] ⛔ Is the typography hierarchy clear and consistent?
- [ ] ⚠️ Is the spacing consistent and purposeful?

## 6.5 Predictability

- [ ] ⛔ Does the interface behave consistently across screens?
- [ ] ⛔ Are keyboard shortcuts consistent?
- [ ] ⛔ Are visual patterns consistent?
- [ ] ⛔ Would a user be surprised by any behavior?
- [ ] ⚠️ Could a user predict what happens next without documentation?

## 6.6 Trust

- [ ] ⛔ Does the interface communicate that M31A is trustworthy?
- [ ] ⛔ Are errors explained clearly?
- [ ] ⛔ Are permissions explained clearly?
- [ ] ⛔ Is the user always in control?
- [ ] ⚠️ Would a user feel safe letting M31A make changes?

---

# 7. Definition of Ready

A PR is READY for review when:

## 7.1 Implementation Ready

- [ ] ⛔ All code compiles (`go build ./...`)
- [ ] ⛔ All tests pass (`go test ./...`)
- [ ] ⛔ No race conditions (`go test -race ./...`)
- [ ] ⛔ No lint failures (`golangci-lint run`)
- [ ] ⛔ Feature flag works (if applicable)

## 7.2 Documentation Ready

- [ ] ⛔ PR description explains what changed and why
- [ ] ⛔ PR description references the UX spec section
- [ ] ⛔ PR description includes screenshots (for visual changes)
- [ ] ⛔ PR description lists affected screens
- [ ] ⚠️ PR description notes any spec ambiguities

## 7.3 Testing Ready

- [ ] ⛔ New code has unit tests
- [ ] ⛔ Changed code has updated tests
- [ ] ⛔ Integration tests cover new behavior
- [ ] ⛔ Visual regression screenshots captured
- [ ] ⚠️ Edge cases tested

## 7.4 Review Ready

- [ ] ⛔ PR is small enough to review (<500 LOC ideal, <1000 LOC max)
- [ ] ⛔ PR is focused on a single milestone
- [ ] ⛔ No unrelated changes included
- [ ] ⛔ Branch is up to date with main
- [ ] ℹ️ PR is assigned to appropriate reviewer

---

# 8. Definition of Done

A PR is DONE when:

## 8.1 Code Review

- [ ] ⛔ At least 1 reviewer approved
- [ ] ⛔ All review comments addressed
- [ ] ⛔ No unresolved discussions
- [ ] ⛔ CI passes (all checks green)

## 8.2 UX Review

- [ ] ⛔ UX Acceptance Checklist completed
- [ ] ⛔ Visual Regression Checklist completed
- [ ] ⛔ All blocking items addressed
- [ ] ⚠️ All warning items addressed or justified

## 8.3 Accessibility Review

- [ ] ⛔ Accessibility Checklist completed
- [ ] ⛔ All terminal sizes verified
- [ ] ⛔ Keyboard-only usage verified
- [ ] ⚠️ Contrast ratios verified

## 8.4 Performance Review

- [ ] ⛔ Performance Checklist completed
- [ ] ⛔ No regressions against baseline
- [ ] ⚠️ Benchmarks recorded

## 8.5 Product Identity Review

- [ ] ⛔ Product Identity Checklist completed
- [ ] ⛔ All identity questions answered positively
- [ ] ⚠️ Any "no" answers documented with justification

## 8.6 Merge

- [ ] ⛔ All above items checked
- [ ] ⛔ Branch is up to date with main
- [ ] ⛔ Squash merge (clean commit history)
- [ ] ⛔ Commit message follows convention (`feat:`, `fix:`, `refactor:`)
- [ ] ⛔ Visual regression baselines updated (if intentional changes)

---

# Appendix: Milestone-Specific Checklist Additions

## M1: Layout Foundation

Additional items:
- [ ] ⛔ Feature flag `UX_V2_CHROME` works (toggle old/new)
- [ ] ⛔ Old rendering preserved behind flag
- [ ] ⛔ No changes to functionality (visual only)

## M2: Sidebar Redesign

Additional items:
- [ ] ⛔ All 3 modes render correctly
- [ ] ⛔ Mode switching works via keyboard
- [ ] ⛔ Focused mode is backward compatible
- [ ] ⛔ Sidebar overlay works on narrow screens

## M3: Narrative Engine

Additional items:
- [ ] ⛔ Intent classifier handles all 18 tool patterns
- [ ] ⛔ Group buffer flush rules work correctly
- [ ] ⛔ Fallback to ToolCard works if narrative fails
- [ ] ⛔ 50+ unit test cases pass

## M4: Permission Modal

Additional items:
- [ ] ⛔ Description generator tested for each tool type
- [ ] ⛔ Fallback to old format works
- [ ] ⛔ Deny default verified (Escape = deny)
- [ ] ⛔ "Allow All" batch approval works

## M5: Workflow Screens

Additional items:
- [ ] ⛔ Full workflow execution passes (goal → ship)
- [ ] ⛔ Each screen's specific acceptance criteria met
- [ ] ⛔ No workflow regressions

## M6: Thinking Display

Additional items:
- [ ] ⛔ Labels are accurate for each phase
- [ ] ⛔ Timer preserved as fallback
- [ ] ⛔ Duration accessible on demand

## M7: Empty States

Additional items:
- [ ] ⛔ Every screen has designed empty state
- [ ] ⛔ Empty states guide to next action
- [ ] ⛔ Consistent across all screens

## M8: Progressive Disclosure

Additional items:
- [ ] ⛔ Collapse/expand behavior works correctly
- [ ] ⛔ No information loss
- [ ] ⛔ ToolDetail accessible for full inspection

## M9: Accessibility

Additional items:
- [ ] ⛔ All contrast ratios verified
- [ ] ⛔ All terminal environments verified
- [ ] ⛔ No focus traps

## M10: Regression & Docs

Additional items:
- [ ] ⛔ Full regression suite passes
- [ ] ⛔ Visual baselines established
- [ ] ⛔ Documentation complete

---

# Appendix: Reviewer Assignment

| Milestone | Primary Reviewer | Secondary Reviewer |
|-----------|-----------------|-------------------|
| M1 | Layout team | Design lead |
| M2 | Sidebar team | Design lead |
| M3 | Components team | Design lead |
| M4 | Components team | Security review |
| M5 | Screen teams | Design lead |
| M6 | REPL team | Design lead |
| M7 | Components team | Design lead |
| M8 | Components team | Design lead |
| M9 | All teams | Accessibility lead |
| M10 | All teams | QA lead |

---

# Appendix: Reject Criteria

A PR MUST be rejected if:

1. ⛔ It adds UI elements not in the spec
2. ⛔ It removes UI elements that the spec requires
3. ⛔ It breaks existing keyboard shortcuts
4. ⛔ It breaks existing slash commands
5. ⛔ It introduces performance regressions
6. ⛔ It fails accessibility checks
7. ⛔ It has unaddressed review comments
8. ⛔ It contains unrelated changes
9. ⛔ It has uncommented-out code
10. ⛔ It has debug print statements
11. ⛔ It fails any ⛔ item in any checklist
12. ⛔ The Product Identity questions are answered "no" without justification
13. ⛔ Visual changes don't match the spec wireframes
14. ⛔ Information is duplicated across components
15. ⛔ Implementation details are exposed to users

---

*This document is the definitive review contract for M31A v1.6.2. Every PR must satisfy every applicable item before merging.*
