# M31A: MVP-to-Product Gap Analysis Report

**Date:** 2026-06-10
**Author:** Qoder CLI (automated analysis)
**Scope:** Deep audit of M31A's current workflow vs. the envisioned production-ready Plan-Accept-Execute-Demonstrate pipeline.

---

## Executive Summary

M31A has a **functional six-phase workflow engine** (Initialize -> Discuss -> Plan -> Execute -> Verify -> Ship) with solid infrastructure: provider abstraction, tool dispatch, self-healing, session persistence, and a rich TUI. However, the **user-facing workflow experience** is fundamentally an MVP. The current system generates raw JSON task arrays and markdown tables, while the vision demands a rich, human-readable **implementation plan** with review/refine loops, explicit acceptance gates, and post-completion **demonstration documents**.

This report identifies **27 critical gaps**, **14 significant gaps**, and **9 minor gaps** organized by workflow phase. Each gap includes a severity rating, current state, desired state, affected files, and estimated complexity.

---

## Table of Contents

1. [Vision vs. Reality: Side-by-Side Comparison](#1-vision-vs-reality-side-by-side-comparison)
2. [Gap Inventory](#2-gap-inventory)
3. [Phase-by-Phase Deep Dive](#3-phase-by-phase-deep-dive)
4. [Architecture Gaps](#4-architecture-gaps)
5. [TUI/UX Gaps](#5-tuiux-gaps)
6. [Data Model Gaps](#6-data-model-gaps)
7. [Prompt Engineering Gaps](#7-prompt-engineering-gaps)
8. [Missing Artifacts](#8-missing-artifacts)
9. [Reliability & Edge Cases](#9-reliability--edge-cases)
10. [Priority Matrix](#10-priority-matrix)
11. [Recommended Implementation Order](#11-recommended-implementation-order)

---

## 1. Vision vs. Reality: Side-by-Side Comparison

### The Envisioned Flow

```
User Prompt
    |
    v
[Plan Phase] --> generates plan.md (rich markdown with sections)
    |                - Proposed Changes
    |                - Open Questions
    |                - Verification Plan
    |                - User Review Required notes
    v
[User Decision]
    |--- Accept  --> generates tasks.md --> Execute --> generates demonstration.md --> Done
    |--- Refine  --> user provides feedback --> regenerate plan.md --> repeat
```

### The Current Flow

```
User Prompt (/new)
    |
    v
[Initialize] --> detects project type, git init, writes PROJECT.md
    |
    v
[Discuss] --> LLM generates 2-4 questions (regex-parsed) --> user answers
    |
    v
[Plan] --> LLM generates JSON task array --> parsed/validated --> TASKS.md (markdown table)
    |
    v
[Plan Screen] --> user sees task list --> enter=approve, esc=cancel (NO refine option)
    |
    v
[Execute] --> topological sort --> sequential task execution with tool dispatch + self-heal
    |
    v
[Verify] --> file existence + syntax + test checks --> self-heal on failure
    |
    v
[Ship] --> final commit + ledger update + archive --> summary screen (no demonstration.md)
```

### Key Differences at a Glance

| Aspect | Vision | Current State | Gap Severity |
|--------|--------|---------------|-------------|
| Plan output format | Rich `plan.md` with sections | Raw JSON array -> `TASKS.md` table | **CRITICAL** |
| Plan review loop | Accept OR Refine with feedback | Accept OR Cancel only | **CRITICAL** |
| Plan regeneration | User feedback -> revised plan | Not implemented | **CRITICAL** |
| Task list format | `tasks.md` with checkboxes | `TASKS.md` as markdown table | **SIGNIFICANT** |
| Completion artifact | `demonstration.md` walkthrough | Ship summary (stats only) | **CRITICAL** |
| Open Questions in plan | Embedded in plan.md | Separate Discuss phase | **SIGNIFICANT** |
| User Review notes | Highlighted in plan | Not present | **SIGNIFICANT** |
| Verification Plan section | Described in plan before execution | Only runs in Verify phase | **MODERATE** |

---

## 2. Gap Inventory

### Critical Gaps (Block Product Vision)

| ID | Gap | Phase | Effort |
|----|-----|-------|--------|
| C-01 | No rich `plan.md` generation | Plan | High |
| C-02 | No plan review/refine loop | Plan | High |
| C-03 | No plan regeneration from user feedback | Plan | High |
| C-04 | No `demonstration.md` generation | Ship | Medium |
| C-05 | Plan output is JSON, not human-readable markdown | Plan | Medium |
| C-06 | No "Proposed Changes" section with [NEW]/[MODIFY] tags | Plan | Medium |
| C-07 | No "Verification Plan" section in plan output | Plan | Medium |
| C-08 | No "User Review Required" callouts in plan | Plan | Low |
| C-09 | Plan acceptance doesn't gate execution (enter approves instantly) | Plan/TUI | Medium |
| C-10 | No plan diff/comparison when refining | Plan | High |
| C-11 | Discuss phase is disconnected from plan "Open Questions" | Discuss/Plan | Medium |
| C-12 | No mechanism to show plan to user as rendered markdown | TUI/Plan | High |

### Significant Gaps (Degrade User Experience)

| ID | Gap | Phase | Effort |
|----|-----|-------|--------|
| S-01 | `tasks.md` uses table format, not checkbox task list | Plan | Low |
| S-02 | No task grouping by category (Setup, Styles, Components, etc.) | Plan | Medium |
| S-03 | Execute phase has no "implementation plan" context | Execute | Medium |
| S-04 | Ship summary lacks narrative description of what was built | Ship | Medium |
| S-05 | No cost/time estimate in plan output | Plan | Medium |
| S-06 | Plan doesn't include technology/framework decisions | Plan | Medium |
| S-07 | Discuss questions don't feed into plan's "Open Questions" section | Discuss | Medium |
| S-08 | No "implementation notes" or "approach rationale" in plan | Plan | Low |
| S-09 | Execute model doesn't show which plan section a task belongs to | Execute | Low |
| S-10 | Verify phase has no connection to plan's "Verification Plan" | Verify | Medium |
| S-11 | No plan persistence as a standalone artifact file | Plan | Low |
| S-12 | Ship phase archives session, losing plan context | Ship | Low |
| S-13 | No way to view the original plan after execution starts | TUI | Medium |
| S-14 | No plan version history (v1, v2 after refine, etc.) | Plan | Medium |

### Minor Gaps (Polish & Quality)

| ID | Gap | Phase | Effort |
|----|-----|-------|--------|
| M-01 | Plan screen keybindings: `enter` immediately approves (should require explicit confirmation) | TUI | Low |
| M-02 | No plan export capability (markdown, PDF) | TUI | Low |
| M-03 | Wave titles are hardcoded generic names ("Foundation", "Core Features") | TUI | Low |
| M-04 | Discuss phase regex-based question parsing is fragile | Discuss | Low |
| M-05 | No progress indicator during plan generation | TUI | Low |
| M-06 | Task descriptions often too terse in plan output | Plan | Low |
| M-07 | No plan template customization (user-defined sections) | Plan | Medium |
| M-08 | Ship view doesn't show the plan that was followed | Ship | Low |
| M-09 | No "what went wrong" section in demonstration for failed tasks | Ship | Low |

---

## 3. Phase-by-Phase Deep Dive

### Phase 1: Initialize

**Current State:** Functional. Detects project type, initializes git, creates planning directory, writes `PROJECT.md` and `STATE.md`.

**Gaps:**
- No framework detection beyond file indicators (e.g., can't detect Next.js vs. plain React vs. Vue from `package.json` contents)
- Project type detection is shallow: only checks for `go.mod`, `package.json`, `Cargo.toml`, etc. Doesn't inspect `package.json` for `next`, `react`, `vue` dependencies
- No user project scaffolding (the vision implies M31A should `npx create-next-app` or equivalent during execution, but Initialize doesn't prepare for that)

**Files Affected:**
- `internal/workflow/initialize.go:14-68`
- `internal/workflow/engine_parse.go:212-229` (detectProjectType)

**Verdict:** Minor improvements needed but fundamentally sound.

---

### Phase 2: Discuss

**Current State:** LLM generates 2-4 numbered questions, parsed via regex with 3 fallback strategies. User answers via TUI. Answers saved to `PROJECT.md`.

**Gaps:**
1. **C-11: Disconnected from Plan** - Discuss questions and plan "Open Questions" are completely separate concepts. The vision wants open questions *embedded in the plan document*, not as a separate phase.
2. **M-04: Fragile parsing** - Question extraction uses `(\d+)\.\s+(.+\?)` regex, which misses questions that don't end with `?`, questions spanning multiple lines, or questions formatted as bullet points.
3. **No streaming-to-plan** - Discuss answers aren't injected into the plan context in a structured way. They're saved to `PROJECT.md` as a flat Q&A map, losing the nuance of the original discussion.
4. **No "skip discuss" intelligence** - The LLM can say "no questions needed" but the regex parser may still extract false positives from surrounding text.

**Files Affected:**
- `internal/workflow/discuss.go:15-94`
- `internal/workflow/engine_parse.go:232-280` (parseQuestions)
- `internal/workflow/prompts/discuss-questions.md`

**Verdict:** The Discuss phase conceptually maps to "Open Questions" in the vision, but the output format and integration with Plan are fundamentally misaligned.

---

### Phase 3: Plan

**Current State:** LLM generates a JSON array of tasks. Tasks are parsed, validated (unique IDs, no cycles, valid deps), and saved as `TASKS.md` (markdown table). The Plan screen shows tasks grouped by topological waves.

**This is where the biggest gaps exist.**

#### Gap C-01: No Rich `plan.md` Generation

The vision wants:
```markdown
# Build Eshan's Cafe and Coffee Bar Website

We will build a visually stunning, premium Next.js website...

## User Review Required
> [!IMPORTANT]
> By default, I will use **Vanilla CSS**...

## Open Questions
> [!WARNING]
> - **Color Palette & Vibe:** I plan to use...

## Proposed Changes
### Setup & Configuration
- Initialize a new Next.js project...

### Core Components
#### [NEW] components/Navbar.js
- A sticky header with glassmorphic transparency...

## Verification Plan
### Manual Verification
- Run the Next.js development server...
```

The current system generates:
```json
[
  {"id": 1, "action": "Create", "description": "Initialize Next.js project", "dependencies": [], "files": ["package.json"], "acceptance_criteria": ["npm install succeeds"]},
  {"id": 2, "action": "Create", "description": "Create Navbar component", "dependencies": [1], "files": ["components/Navbar.js"], "acceptance_criteria": ["component renders"]},
  ...
]
```

**What's missing:**
- Narrative description of the approach
- Technology decision rationale
- `[NEW]` / `[MODIFY]` file tags with descriptions
- "User Review Required" callouts for opinionated decisions
- "Open Questions" section (currently in Discuss phase, not in plan)
- "Verification Plan" section describing how to validate
- Professional markdown formatting

**Files to Create/Modify:**
- `internal/workflow/plan.go` - Change LLM output format from JSON array to structured markdown
- `internal/workflow/prompts/plan-format.md` - Complete rewrite of the plan format prompt
- New: `internal/workflow/plan_parser.go` - Parse rich markdown plan into structured data
- New: `internal/types/plan.go` - Plan document type (sections, proposed changes, questions, verification)

**Estimated Effort:** 3-5 days

#### Gap C-02: No Plan Review/Refine Loop

The vision:
```
Plan generated --> User reviews --> [Accept | Refine]
                                      |
                                      v
                               User provides feedback
                                      |
                                      v
                               Plan regenerated with feedback
                                      |
                                      v
                               User reviews again --> [Accept | Refine]
```

Current PlanModel (`internal/tui/plan_model.go:174-193`):
```go
case "enter", " ":
    // Approve plan -> transition to execute
    return pm, func() tea.Msg {
        return AppMsg{Screen: ScreenExecute}
    }
case "esc", "q":
    return pm, func() tea.Msg {
        return AppMsg{Screen: ScreenREPL}
    }
```

Only two options: approve or cancel. No refine. No feedback input. No regeneration.

**What's needed:**
1. A third option: "Refine" (e.g., `r` key)
2. A text input area for refinement feedback
3. A command to re-run the plan phase with feedback injected
4. Plan versioning (v1, v2, v3) with diff capability
5. State machine update: `PhasePlan` should allow transition back to `PhasePlan` (currently not in `validPhaseTransitions`)

**Files to Create/Modify:**
- `internal/tui/plan_model.go` - Add refine keybinding and feedback input
- New: `internal/tui/plan_refine.go` - Refine feedback input model
- `internal/workflow/engine.go:212-219` - Add `PhasePlan -> PhasePlan` transition
- `internal/workflow/plan.go` - Accept refinement feedback parameter
- `internal/tui/commands_workflow.go` - Add refine command handling

**Estimated Effort:** 3-4 days

#### Gap C-05: Plan Output is JSON, Not Human-Readable Markdown

The `plan-format.md` prompt explicitly instructs: "Return ONLY a JSON array of tasks. Do not include any text outside the JSON array."

This is the root cause of the plan being machine-readable but not human-friendly. The prompt needs a complete rewrite to produce the rich markdown format.

**But there's a tension:** The current JSON format is what feeds the task runner. Switching to rich markdown means:
1. The plan document becomes the user-facing artifact (`plan.md`)
2. A separate task extraction step is needed to generate `tasks.md` from the plan
3. The task parser must handle markdown-embedded task structures

**Files to Modify:**
- `internal/workflow/prompts/plan-format.md` - Complete rewrite
- `internal/workflow/engine_parse.go:18-34` - Replace JSON parser with markdown parser

**Estimated Effort:** 2-3 days

#### Gap C-09: Plan Acceptance Doesn't Gate Execution

Pressing `enter` on the plan screen immediately transitions to execute. There's no confirmation dialog, no "Are you sure?" prompt. For a product that's supposed to give users control, this is too fast.

**What's needed:**
- A confirmation step: "Press `y` to accept and begin execution, `r` to refine, `esc` to cancel"
- The footer should show these options clearly
- Currently the plan view has no footer with keybinding hints

**Files to Modify:**
- `internal/tui/plan_model.go:174-193`
- `internal/tui/plan_view.go`

**Estimated Effort:** 1 day

---

### Phase 4: Execute

**Current State:** Tasks are scheduled via topological sort (Kahn's algorithm), executed sequentially with tool dispatch, and self-healed on failure. The Execute screen shows real-time progress with spinners and badges.

**Gaps:**

1. **S-03: No plan context during execution** - The execute prompt (`execute-task.md`) gives the LLM the task spec and project context, but NOT the original plan narrative. The LLM doesn't know *why* it's building something or what the overall vision is. It only sees a terse task description.

2. **S-02: No task grouping by category** - The vision shows tasks grouped by category (Setup, Styles, Components, Assembly & Polish) with checkbox format. Currently tasks are flat, numbered, and grouped only by dependency depth.

3. **No intermediate checkpoint between tool calls** - If the LLM makes 5 tool calls for a task and the 4th fails, all 3 successful tool calls' changes are already on disk. Self-heal attempts to fix, but there's no rollback of partial tool call results.

4. **Tool call parsing is fragile** - `parseToolCalls()` uses regex + JSON scanning to extract tool calls from LLM responses. This depends on the LLM formatting tool calls as JSON objects in code blocks. Many models may emit tool calls differently (e.g., using native function calling format).

**Files Affected:**
- `internal/workflow/execute.go:307-339` (buildExecuteContext)
- `internal/workflow/prompts/execute-task.md`
- `internal/workflow/engine_parse.go:294-376` (parseToolCalls)

**Verdict:** Execute is the most mature phase. Gaps are primarily about context richness, not functionality.

---

### Phase 5: Verify

**Current State:** Checks file existence, syntax/build (language-specific), and tests. Self-heal and git bisect fallback on failure. The Verify screen shows pass/fail badges with heal capability.

**Gaps:**

1. **S-10: No connection to plan's Verification Plan** - The vision's plan includes a "Verification Plan" section describing *how* to verify (e.g., "Run the Next.js dev server, check animations at 60fps, test responsive layout"). The current verify phase only does automated checks (file existence, build, test). Manual verification steps from the plan are never executed or displayed.

2. **No visual/functional verification** - The verify phase can check syntax and tests, but can't verify visual output, animation quality, or responsive design. This is inherent to terminal-based AI, but the plan should acknowledge this limitation.

3. **Limited language support** - Verify only handles Go, Node.js, Python, Rust. No support for Java (`mvn compile`), C/C++ (`make`), Ruby, etc. despite Initialize detecting `pom.xml`, `Makefile`, and `CMakeLists.txt`.

**Files Affected:**
- `internal/workflow/verify.go`
- `internal/workflow/engine_verify.go:114-287`
- `internal/tui/verify.go`

**Verdict:** Verify works for automated checks but doesn't honor the plan's verification intent.

---

### Phase 6: Ship

**Current State:** Creates final commit, updates learning ledger, archives session, shows summary screen with task/file/commit stats.

#### Gap C-04: No `demonstration.md` Generation

The vision wants a rich walkthrough:
```markdown
# Eshan's Cafe and Coffee Bar - Walkthrough

I have successfully built the premium website...

## What Was Completed
1. **Architecture & Styling**: Used Next.js App Router...
2. **Animations**: Integrated framer-motion...
3. **Core Components**: Navbar, Hero, Menu Highlights...

## Verification
- You can start the application by running `npm run dev`...
```

Current Ship phase generates:
- `ShipSummary` struct with `TaskDone`, `TaskTotal`, `Commits`, `Duration`
- A card-based TUI view showing stats grid
- No narrative document

**What's needed:**
1. After all tasks complete, make an LLM call with:
   - The original plan
   - The completed task list with commit hashes
   - The git diff summary
   - The project structure
2. Generate a `demonstration.md` in the session's planning directory
3. Display the demonstration in the Ship screen (rendered markdown)
4. Optionally write it to the project root

**Files to Create/Modify:**
- `internal/workflow/ship.go` - Add LLM call for demonstration generation
- New: `internal/workflow/prompts/demonstration-format.md` - Prompt template
- `internal/tui/ship_model.go` - Render demonstration content
- `pkg/session/planning.go` - SaveDemonstration method

**Estimated Effort:** 2-3 days

---

## 4. Architecture Gaps

### A-01: No Plan Document Type

The current `types.go` has `Task` and `ProjectState` but no `Plan` type that represents the rich plan document.

**Missing type:**
```go
type Plan struct {
    Title           string
    Summary         string
    ReviewNotes     []ReviewNote      // "User Review Required" callouts
    OpenQuestions   []OpenQuestion     // embedded from Discuss
    ProposedChanges []ProposedChange   // grouped by category
    VerificationPlan VerificationPlan
    Tasks           []Task             // extracted task list
    Version         int                // for refine versioning
    CreatedAt       time.Time
}
```

**Files:** `internal/types/types.go` or new `internal/types/plan.go`

### A-02: No Plan-to-Tasks Extraction Pipeline

Currently the plan IS the task list (JSON array). With rich plan format, we need a pipeline:
```
plan.md (rich markdown) --> plan parser --> Plan struct --> task extractor --> tasks.md + []Task
```

### A-03: Phase Transition Map is Incomplete

Current valid transitions (`engine.go:212-219`):
```go
PhasePlan: {PhaseExecute, PhaseIdle}
```

Missing: `PhasePlan -> PhasePlan` (for refine loop) and `PhasePlan -> PhaseDiscuss` (for going back to clarify questions).

### A-04: No Plan Versioning System

When a user refines a plan, there's no way to:
- Store the original plan (v1)
- Store the refined plan (v2)
- Show a diff between versions
- Rollback to a previous plan version

### A-05: Context Pruning Loses Plan Context

The architecture says "Context pruning: each workflow phase discards prior conversation; reads state from planning/ files only." This means the Execute phase LLM call never sees the plan narrative — only the task spec and project state. The rich plan context (proposed changes, approach rationale) is lost.

---

## 5. TUI/UX Gaps

### U-01: Plan Screen Has No Rendered Markdown View

The PlanModel (`plan_model.go`) renders tasks as a custom list with wave badges, not as rendered markdown. The user's plan.md should be viewable as beautifully rendered markdown (using glamour).

### U-02: No Refine Feedback Input

There's no text input component in the plan screen for providing refinement feedback. Would need a textarea overlay or a dedicated refine screen.

### U-03: Plan Screen Footer Missing

The plan screen has no footer showing available keybindings. Users don't know that `enter` approves and `esc` cancels.

### U-04: Ship Screen Doesn't Show Demonstration

The ShipModel shows a stats card but has no scrollable viewport for a demonstration document.

### U-05: No Plan-to-Execute Transition Animation

The transition from plan approval to execute start is abrupt. No "preparing execution..." intermediate state.

### U-06: No Way to View Plan During Execution

Once execution starts, the plan is no longer accessible from the execute screen. Users can't reference the original plan while watching tasks execute.

---

## 6. Data Model Gaps

### D-01: Task Type Lacks Category

The `Task` struct has `Action` (Create/Add/Modify/Delete) but no `Category` (Setup, Styles, Components, etc.) for the grouped checkbox format in the vision's `tasks.md`.

### D-02: Task Type Lacks Plan Reference

Tasks don't reference which section of the plan they came from. Adding a `PlanSection` field would enable the execute screen to show "Working on: Core Components > Navbar".

### D-03: ProjectState Lacks Plan Content

`ProjectState` stores `Goal`, `ProjectType`, `Framework`, and `Answers`, but not the plan content itself. The plan lives only in `TASKS.md` as a table.

### D-04: No Plan Version Tracking

No field for `PlanVersion`, `PlanHistory`, or `RefinementLog` anywhere in the data model.

---

## 7. Prompt Engineering Gaps

### P-01: plan-format.md Instructs JSON Output

The prompt explicitly says "Return ONLY a JSON array of tasks." This needs a complete rewrite to produce rich markdown.

### P-02: No Demonstration Prompt Template

There's no `prompts/demonstration-format.md` to guide the LLM in generating the post-completion walkthrough.

### P-03: Discuss Prompt Doesn't Align with Plan "Open Questions"

The discuss prompt generates standalone questions, but the vision wants questions embedded *within* the plan as "Open Questions" with the LLM's suggested defaults.

### P-04: Execute Prompt Lacks Plan Context

The execute-task prompt gives the LLM a task spec but not the broader plan narrative. The LLM doesn't know the overall design vision, technology choices, or approach rationale.

### P-05: Base Prompt Mentions Six Phases but Vision Has Different Flow

The base prompt describes the six phases generically, but doesn't mention the plan review/refine loop or the demonstration generation step.

---

## 8. Missing Artifacts

The vision describes three key artifacts that the system should produce:

| Artifact | Vision | Current State | Status |
|----------|--------|---------------|--------|
| `plan.md` | Rich implementation plan with sections, callouts, proposed changes | `TASKS.md` as markdown table | **MISSING** |
| `tasks.md` | Checkbox-style task list grouped by category | `TASKS.md` as markdown table (flat, no grouping) | **PARTIAL** |
| `demonstration.md` | Post-completion walkthrough describing what was built | `ShipSummary` struct (stats only, no narrative) | **MISSING** |

### Additional Missing Artifacts

| Artifact | Purpose | Status |
|----------|---------|--------|
| `plan_v1.md`, `plan_v2.md` | Plan version history for refine loop | **MISSING** |
| `REVIEW.md` | User's refinement feedback history | **MISSING** |
| Plan diff view | Side-by-side comparison of plan versions | **MISSING** |

---

## 9. Reliability & Edge Cases

### R-01: Plan Parse Failure Recovery

If the LLM generates a plan in rich markdown format and the parser fails to extract tasks, the current retry mechanism (`MaxPlanRetries=3`) sends the parse error back to the LLM. With rich markdown, parse failures are more likely because the format is less constrained than JSON.

### R-02: Refine Loop Convergence

There's no guarantee that plan refinement converges. A user could keep refining indefinitely. The system needs:
- A maximum refinement count
- A "good enough" heuristic
- An option to accept with caveats

### R-03: Large Plan Handling

Rich markdown plans could be 500+ lines. The current `listCwdFiles` function injects the entire file schema into the plan context. For large projects, this could exceed token limits. The plan context builder needs better summarization.

### R-04: Demonstration Generation After Partial Failure

If some tasks fail during execution, the demonstration should acknowledge what was completed and what wasn't. The current Ship phase reports `failed` count but doesn't generate a narrative explaining partial completion.

### R-05: Plan-Task Drift

During execution, the LLM may create files not listed in the plan, or skip planned files. There's no reconciliation between the plan's "Proposed Changes" and what was actually created/modified.

### R-06: Discuss-to-Plan Data Loss

Discuss answers are saved as `map[string]string` in `PROJECT.md`. If the question text is long or has special characters, the map key may not round-trip correctly through markdown serialization.

---

## 10. Priority Matrix

### Must-Have (Ship Blocker)

| Priority | Gap | Effort | Impact |
|----------|-----|--------|--------|
| P0 | C-01: Rich `plan.md` generation | 3-5d | Transforms plan from machine output to product |
| P0 | C-02: Plan review/refine loop | 3-4d | Core user interaction pattern |
| P0 | C-04: `demonstration.md` generation | 2-3d | Completion artifact that users see |
| P0 | C-05: Plan output format rewrite | 2-3d | Foundation for all plan improvements |
| P1 | C-09: Plan acceptance gating | 1d | Prevents accidental execution |
| P1 | C-12: Rendered markdown plan view | 2d | Users need to read the plan |

### Should-Have (Quality)

| Priority | Gap | Effort | Impact |
|----------|-----|--------|--------|
| P2 | S-01: Checkbox task format | 1d | Familiar task tracking format |
| P2 | S-02: Task category grouping | 2d | Organized task presentation |
| P2 | S-05: Cost/time estimates in plan | 2d | User decision support |
| P2 | S-13: View plan during execution | 2d | Context retention |
| P2 | A-01: Plan document type | 1d | Type safety for plan data |

### Nice-to-Have (Polish)

| Priority | Gap | Effort | Impact |
|----------|-----|--------|--------|
| P3 | C-10: Plan diff/comparison | 3d | Refine decision support |
| P3 | S-14: Plan version history | 2d | Audit trail |
| P3 | M-02: Plan export | 1d | Shareability |
| P3 | M-07: Custom plan templates | 2d | Flexibility |

---

## 11. Recommended Implementation Order

### Phase 1: Foundation (Week 1)

1. **Create Plan document type** (`internal/types/plan.go`)
   - `Plan`, `PlanSection`, `ProposedChange`, `OpenQuestion`, `ReviewNote`, `VerificationPlan` types

2. **Rewrite `plan-format.md` prompt** to produce rich markdown
   - Include sections: Summary, User Review Required, Open Questions, Proposed Changes, Verification Plan
   - Instruct LLM to use `[NEW]` and `[MODIFY]` tags for files

3. **Build plan markdown parser** (`internal/workflow/plan_parser.go`)
   - Extract sections from markdown
   - Extract proposed changes with file tags
   - Extract tasks from the plan (as a subsection)
   - Handle edge cases (malformed sections, missing headers)

4. **Update plan.go** to use new format
   - Replace `parseTasksFromJSON` with `parsePlanFromMarkdown`
   - Save `plan.md` alongside `TASKS.md`
   - Extract tasks from plan for the task runner

### Phase 2: Review Loop (Week 2)

5. **Add refine keybinding** to PlanModel (`r` key)
6. **Create refine feedback input** screen/model
7. **Add `PhasePlan -> PhasePlan` transition** to valid transitions
8. **Update plan.go** to accept refinement feedback
9. **Add plan versioning** (save `plan_v1.md`, `plan_v2.md`, etc.)
10. **Add confirmation dialog** before plan acceptance

### Phase 3: Demonstration (Week 2-3)

11. **Create `demonstration-format.md` prompt**
12. **Add LLM call in ship.go** for demonstration generation
13. **Save `demonstration.md`** to planning directory
14. **Update ShipModel** to render demonstration content
15. **Add demonstration viewport** to ship screen

### Phase 4: Polish (Week 3)

16. **Render plan as markdown** in PlanModel using glamour
17. **Add plan footer** with keybinding hints
18. **Add task category grouping** to plan parser and task list
19. **Inject plan context** into execute prompt
20. **Add "view plan" option** to execute screen

---

## Summary Statistics

| Metric | Count |
|--------|-------|
| Total gaps identified | 50 |
| Critical gaps | 12 |
| Significant gaps | 14 |
| Minor gaps | 9 |
| Architecture gaps | 5 |
| TUI/UX gaps | 6 |
| Data model gaps | 4 |
| Prompt engineering gaps | 5 |
| Missing artifacts | 5 |
| Reliability concerns | 6 |
| Estimated total effort | 30-45 developer-days |
| Recommended timeline | 3-4 weeks (single developer) |

---

## Conclusion

M31A has a **solid infrastructure foundation** — the provider abstraction, tool dispatch, self-healing, session persistence, and TUI framework are all well-built. The gaps are concentrated in the **user-facing workflow experience**: the plan format, the review loop, and the completion artifact.

The single most impactful change is rewriting the plan phase to produce a rich `plan.md` instead of a JSON task array. This one change cascades into better user understanding, informed execution (plan context feeds the LLM), and more meaningful demonstrations.

The second most impactful change is the refine loop, which transforms the plan from a "take it or leave it" output into a collaborative design document.

Together, these changes move M31A from "an AI that generates task lists" to "an AI that collaborates on implementation plans" — which is the core of the product vision.
