# Phase 2: User Experience - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-05
**Phase:** 02-user-experience
**Areas discussed:** Progress Display, Plan Presentation, Permission Prompts, First-Run Experience

---

## Progress Display

| Option | Description | Selected |
|--------|-------------|----------|
| Status bar at bottom | Persistent status line showing current phase, task, and spinner. Minimal footprint, always visible. | ✓ |
| Inline in chat | Progress messages appear in the conversation flow. Natural reading order but can clutter output. | |
| Sidebar panel | Dedicated sidebar showing progress tree. Detailed but takes screen space. | |

**User's choice:** Status bar at bottom
**Notes:** User prefers minimal footprint with persistent visibility.

| Option | Description | Selected |
|--------|-------------|----------|
| Phase + task only | Current phase name and active task. Simple and focused. | |
| Phase + task + time | Also show elapsed time for current phase. Helps users gauge progress. | ✓ |
| Phase + task + spinner | Animated spinner to show activity. Visual feedback that work is happening. | |

**User's choice:** Phase + task + time
**Notes:** User wants time tracking for progress awareness.

| Option | Description | Selected |
|--------|-------------|----------|
| No, only current | Keep it minimal — just what's happening now. | ✓ |
| Yes, as count | Show '3/7 tasks done' to give sense of progress. | |
| Yes, as list | Show recent completed tasks. More context but takes space. | |

**User's choice:** No, only current
**Notes:** User prefers minimal display without completed task clutter.

| Option | Description | Selected |
|--------|-------------|----------|
| Show primary only | Display the most important operation. Others are implied. | ✓ |
| Cycle through | Rotate between active operations every few seconds. | |
| Stack them | Show multiple lines for concurrent operations. | |

**User's choice:** Show primary only
**Notes:** User wants single-focus display for concurrent operations.

---

## Plan Presentation

| Option | Description | Selected |
|--------|-------------|----------|
| Step-by-step list | Numbered list of tasks with status indicators. Simple and scannable. | |
| Collapsible sections | Expandable sections for each task group. Detailed but more complex. | ✓ |
| Summary card | Compact card with key info. Minimal but may hide details. | |

**User's choice:** Collapsible sections
**Notes:** User wants detailed view with expand/collapse capability.

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, inline editing | Allow editing task descriptions directly in the plan view. | ✓ |
| Yes, via commands | Edit via slash commands like /plan edit, /plan remove. | |
| No, read-only | Plans are generated and executed as-is. Users can regenerate if needed. | |

**User's choice:** Yes, inline editing
**Notes:** User wants direct manipulation of plan tasks.

| Option | Description | Selected |
|--------|-------------|----------|
| Current task highlighted | Show full plan but highlight the active task. Context without clutter. | ✓ |
| Collapsed completed | Collapse completed tasks, expand current and future. Reduces visual noise. | |
| Full detail always | Show all task details at all times. Maximum context. | |

**User's choice:** Current task highlighted
**Notes:** User wants context preservation with active task emphasis.

| Option | Description | Selected |
|--------|-------------|----------|
| No estimates | Keep plans focused on what, not when. Estimates are often wrong. | |
| Yes, rough estimates | Show '~5 min' based on similar past tasks. Helps users plan. | ✓ |
| Yes, with confidence | Show estimate plus confidence level (high/medium/low). | |

**User's choice:** Yes, rough estimates
**Notes:** User wants time estimates for planning purposes.

---

## Permission Prompts

| Option | Description | Selected |
|--------|-------------|----------|
| Modal dialog | Center-screen modal that requires response. Clear but blocks flow. | ✓ |
| Inline in chat | Permission request appears in conversation. Natural but can be missed. | |
| Sidebar notification | Non-blocking sidebar alert. User can continue working while deciding. | |

**User's choice:** Modal dialog
**Notes:** User wants clear, attention-grabbing permission requests.

| Option | Description | Selected |
|--------|-------------|----------|
| Tool name + risk level | Simple: 'Run Bash (high risk)'. Quick to scan. | ✓ |
| Tool + command + risk | Show the actual command being executed. More context but longer. | |
| Full details | Tool, command, risk level, and affected files. Maximum transparency. | |

**User's choice:** Tool name + risk level
**Notes:** User wants quick scanning without verbosity.

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, batch approve | Approve all pending tool calls in one action. Efficient for known-safe operations. | ✓ |
| No, one at a time | Each tool call requires individual approval. Maximum control. | |
| Yes, with patterns | Approve by tool type (e.g., 'allow all FileRead'). Saves time for repetitive tasks. | |

**User's choice:** Yes, batch approve
**Notes:** User wants efficiency for batch operations.

| Option | Description | Selected |
|--------|-------------|----------|
| Auto-approve safe tools | Tools like FileRead, Glob, Grep run without prompt. Only risky tools ask. | ✓ |
| Always ask | Every tool call requires approval, regardless of risk. Maximum security. | |
| User-configurable | Let users set per-tool defaults in config. Flexible but more setup. | |

**User's choice:** Auto-approve safe tools
**Notes:** User wants frictionless experience for safe operations.

---

## First-Run Experience

| Option | Description | Selected |
|--------|-------------|----------|
| Quick config wizard | 3-5 step wizard to set API key, choose model, select theme. Fast and focused. | |
| Interactive tutorial | Guided tour through M31A features with examples. Comprehensive but longer. | ✓ |
| Quick-start guide | Static help text with key commands. Minimal but requires reading. | |

**User's choice:** Interactive tutorial
**Notes:** User wants comprehensive onboarding with examples.

| Option | Description | Selected |
|--------|-------------|----------|
| Core workflow only | How to start a project, what happens during execution, how to review results. | |
| Full feature tour | All features: workflow, tools, settings, keyboard shortcuts. Comprehensive. | ✓ |
| Customizable path | Let users choose what to learn about. Flexible but more complex. | |

**User's choice:** Full feature tour
**Notes:** User wants complete feature coverage in tutorial.

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, always skippable | Users can skip at any point. Resume later if needed. | ✓ |
| Yes, but required first time | Must complete once, then can skip on future runs. | |
| No, always show | Show tutorial on every first run until dismissed. | |

**User's choice:** Yes, always skippable
**Notes:** User wants user control over tutorial flow.

| Option | Description | Selected |
|--------|-------------|----------|
| Integrated setup | Tutorial includes API key configuration step. One flow for everything. | ✓ |
| Separate step | Tutorial pauses, directs user to config, then resumes. Clear separation. | |
| Skip if configured | If API key already exists, skip setup step. Smart detection. | |

**User's choice:** Integrated setup
**Notes:** User wants seamless onboarding without context switches.

---

## the agent's Discretion

- Status bar styling (colors, symbols, spacing)
- Collapsible section animation/transition
- Modal dialog positioning and animation
- Tutorial step ordering and content

## Deferred Ideas

None — discussion stayed within phase scope
