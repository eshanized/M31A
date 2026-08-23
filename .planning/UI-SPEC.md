# M31A — Terminal UI Specification

> Canonical v1 TUI blueprint for `gitlab.com/eshanized/M31A`.
>
> Primary implementation: Go + Bubble Tea.
> Governing runtime contract: `CONTEXT_M31A.md`.

This document defines the screen system, information architecture, component library, interaction model, responsive behavior, state handling, safety presentation, and testing contract for the M31A terminal experience. The TUI is a projection of the runtime; it is never the owner of orchestration, LLM, filesystem, Git, policy, or verification logic.

## 1. Product UI doctrine

M31A must feel like a serious engineering workstation, not a terminal log viewer. The central visual loop is:

```text
intent → understanding → impact → plan → execution → verification → result
```

The UI must always make these questions answerable: what is happening, why is it happening, what can I safely do, and what evidence exists?

Non-negotiables:
- structured runtime state takes visual precedence over model prose
- verification is evidence-backed, never self-declared
- risky actions show scope and policy before mutation
- long output is summarized and lazily expanded
- keyboard interaction is complete
- the UI remains useful in narrow, SSH, no-color, and degraded-provider environments
- the TUI cannot become the sole API to the runtime

## 2. Architecture boundary

```text
Bubble Tea TUI
      │
      ▼
Application Command / Query API
      │
      ▼
M31A runtime
 ├─ sessions
 ├─ runs
 ├─ planning
 ├─ agents
 ├─ tools
 ├─ policy
 ├─ Git/workspaces
 ├─ intelligence
 ├─ verification
 └─ persistence
      │
      ▼
Events + Query Projections
```

The TUI may own transient presentation state: focus, selected rows, open overlays, scroll offsets, local draft input, terminal dimensions, and cached view projections. It must not own domain truth.

## 3. Visual language

Use restrained terminal-native styling. Semantic tokens are preferred over hard-coded colors:

```text
primary secondary muted info success warning error critical
pending running selected border background surface surface_alt
```

State must never be communicated by color alone. Use text labels and symbols with ASCII fallbacks:

```text
✓ → [ok]    ✗ → [x]    ● → [*]    ○ → [ ]
⚠ → [!]     ◇ → [gate]  ↻ → [retry]
```

Use a small spacing scale and consistent section/border hierarchy. Do not make the terminal visually noisy merely to appear feature-rich.

## 4. Global shell

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ M31A · project · branch · RUN 84F2                               ● RUNNING │
├───────────────┬───────────────────────────────────────────────┬──────────────┤
│ NAV            │ PRIMARY VIEW                                  │ CONTEXT      │
│ Chat           │                                               │ Task         │
│ Plan           │                                               │ Agent        │
│ Run            │                                               │ Risk         │
│ Verify         │                                               │ Git          │
│ Intelligence   │                                               │ Files        │
│ Git            │                                               │              │
│ Runs           │                                               │              │
│ Project        │                                               │              │
│ Settings       │                                               │              │
├───────────────┴───────────────────────────────────────────────┴──────────────┤
│ > Ask M31A anything...                                          ^K commands │
└──────────────────────────────────────────────────────────────────────────────┘
```

Shell regions:
- `TopBar`: product/project/branch/run/mode/provider state
- `NavigationRail`: major destinations
- `MainViewport`: current screen
- `ContextPanel`: context-sensitive inspector
- `InputBar`: command/natural-language entry
- `NotificationLayer`: transient feedback
- `ModalLayer`: blocking decisions and dialogs

## 5. Responsive layout contract

| Width | Layout |
|---|---|
| `<80` | compact single-column; hide secondary metadata |
| `80–119` | primary content + compact status region |
| `120–159` | navigation + main + contextual panel |
| `160+` | full three-region layout with richer telemetry |

Never assume fixed terminal height. Recompute layout on resize. Preserve focus and relevant scroll positions. Modal content must switch to full-screen compact mode if it no longer fits.

## 6. Navigation model

Major routes:

```text
chat
plan
run
verify
intelligence
git
history
project
settings
diagnostics
```

Deep routes:

```text
plan/task
plan/review
run/task
run/agent
run/tool
run/checkpoint
verify/check
git/diff
git/branches
git/worktrees
intel/symbol
intel/impact
intel/dependencies
history/run
project/requirements
project/decisions
project/research
```

Back semantics are universal: close modal → leave detail → leave subview → leave major surface. `Esc` must not unexpectedly terminate M31A.

## 7. Global keyboard contract

```text
Enter       open / submit / confirm
Esc         back / cancel / close
Tab         next focus
Shift+Tab   previous focus
Ctrl+K      command palette
Ctrl+L      redraw
Ctrl+C      cancel current operation; second press may exit
?           contextual help
/           local search where applicable
↑↓          selection
j/k         list navigation where conventional
```

Every primary action must have a keyboard path. Mouse support is additive and must never be required.

## 8. Runtime event model exposed to the TUI

The UI should subscribe to normalized application events such as:

```text
session.created / session.updated
run.created / run.started / run.paused / run.resumed / run.failed / run.completed
agent.started / agent.completed
task.created / task.started / task.completed / task.failed
tool.started / tool.completed / tool.failed
checkpoint.required / checkpoint.resolved
verification.started / verification.check.completed / verification.completed
git.status.changed / git.commit.created
workspace.changed
provider.status.changed
```

Events should carry monotonic ordering or equivalent freshness metadata. Stale events must never move the rendered state backwards.

## 9. Screen state contract

Every screen supports at least:

```text
initializing · loading · ready · empty · running · paused
waiting · success · warning · error · recoverable_error
offline_or_unavailable · terminal_resized
```

A screen must never render undefined data as if it were valid. A provider with unknown state is displayed as `Provider: unavailable`, not as an empty field.

## 10. Primary screens

The following 36 screens form the v1 navigation contract.

### S01 — Welcome / Project Discovery

**Purpose:** No project or first launch. Show current path, Git detection, M31A state, provider health, and two clear actions: onboard existing repository or initialize a new project.

**Primary actions:** Onboard repository, Start new project, Quit

**Blueprint:**

```text
M31A

Persistent engineering in your terminal.

Current directory
  /home/eshan/projects/example

Repository
  ✓ Git repository detected

Project state
  ○ Not initialized

[O] Onboard   [N] New project   [Q] Quit
```

### S02 — Main Chat

**Purpose:** Default working surface. Combine conversation with structured plan, impact, run, checkpoint, and verification cards.

**Primary actions:** Submit, Open plan, Inspect impact, Inspect run, Create task, Research claim

**Blueprint:**

```text
You
> Add organization-level RBAC with admin, manager and member roles.

M31A
I found the authorization boundary and identified 22 affected files.

IMPACT
  HIGH · 22 files · 8 tests · 2 public interfaces

PLAN READY
  5 tasks · 2 waves · MEDIUM

[View plan] [Run] [Edit]
```

### S03 — Context Explorer

**Purpose:** Show what M31A currently knows about the project and current run.

**Primary actions:** Inspect source, Exclude source for next run, Refresh, Compare context epoch

**Blueprint:**

```text
Context
────────────
Project map                 ✓
Requirements                ✓
Git state                   ✓
Architecture                ✓
Symbol graph                ✓

Active context
  project constraints
  current requirements
  relevant symbols
  active plan
  current diff
```

### S04 — Project Overview

**Purpose:** Durable project state: milestone, phase, requirements, active runs, decisions, recent verification.

**Primary actions:** Open roadmap, Open requirements, Open decisions, Open state

**Blueprint:**

```text
PROJECT · M31A

Current milestone  M1 · Runtime foundation
Current phase      4 · Session and Run Runtime
Requirements       42 total · 31 complete
Open decisions     4
Active runs        1
Health             GOOD
```

### S05 — Onboarding / Repository Mapping

**Purpose:** Progressive repository archaeology: discovery, language/build detection, dependencies, architecture, symbols, Git history, conventions.

**Primary actions:** Pause, Resume, Restart, Discard partial map

**Blueprint:**

```text
ONBOARDING

Repository discovery       ✓
Languages / build          ✓
Dependencies               ✓
Architecture map           ●
Symbol index               ○
Git history                ○
Conventions                ○

Files 1,482 / 1,931
Symbols 8,404
```

### S06 — Plan Overview

**Purpose:** Canonical executable plan: objective, impact, waves, requirements, risks, checkpoints, acceptance criteria.

**Primary actions:** Run, Edit plan, Review, Back

**Blueprint:**

```text
PLAN · RBAC

Objective
  Introduce organization-level role-based access control.

Risk   MEDIUM
Impact 18–27 files · 8 tests · 2 interfaces

Wave 1
  ✓ 1 Role model
  ✓ 2 Authorization service
Wave 2
  ○ 3 Route guards
  ○ 4 Persistence
  ○ 5 Verification

◇ migration decision checkpoint
```

### S07 — Plan Detail

**Purpose:** Full contract for one TaskGraph node: dependencies, files, actions, acceptance, verification.

**Primary actions:** Run task, Edit, Open files

**Blueprint:**

```text
TASK 3 · ROUTE GUARDS

Status READY    Wave 2    Depends on 1,2

Read first
  middleware/auth.go
  services/authorization.go

Acceptance
  ✓ unauthorized roles receive 403
  ✓ public routes remain accessible
  ✓ membership is checked

Verification
  go test ./...
```

### S08 — Plan Review

**Purpose:** Pre-execution gate. Show requirement coverage, dependencies, pattern consistency, risk, verification coverage, unresolved choices.

**Primary actions:** Approve, Revise, Inspect evidence, Cancel

**Blueprint:**

```text
PLAN REVIEW

Requirement coverage      8 / 8      ✓
Dependency consistency    complete   ✓
Pattern consistency       6 / 6      ✓
Risk analysis             2 medium  !
One-way-door decisions    1          ◇
Verification coverage     5 / 5      ✓

Decision required: migration strategy
```

### S09 — Run Dashboard

**Purpose:** Operational center: task graph, active task, agent, tool, files, elapsed time, controls, and verification state.

**Primary actions:** Pause, Cancel, Diff, Verify, Inspect

**Blueprint:**

```text
RUN 84F2 · IMPLEMENT RBAC

State RUNNING · 02:14 · Implementer

✓ 1 Role model
✓ 2 Authorization service
● 3 Route guards
○ 4 Persistence
○ 5 Verification

Current action  edit_file
Files touched   3

[P] Pause  [C] Cancel  [D] Diff  [V] Verify
```

### S10 — Task Detail

**Purpose:** One active task with its execution state, current step, files, tools, retries, and controls.

**Primary actions:** Pause task, Cancel task, Open diff, Open tool output

**Blueprint:**

```text
TASK 3/5 · ROUTE GUARDS

State RUNNING
Agent Implementer

✓ read router
✓ inspect middleware
● edit router
○ run tests
○ verify invariants

Tool calls 7 · Files changed 2 · Retries 0
```

### S11 — Agent Activity

**Purpose:** Show specialist agents and their current contract/capabilities.

**Primary actions:** Inspect agent, Inspect capabilities, Inspect handoff

**Blueprint:**

```text
AGENTS

Supervisor          ● coordinating
Planner             ✓
Repository Analyst  ✓
Implementer         ●
Verifier            ○
Reviewer            ○

Selected: Implementer
Capabilities: filesystem.write, shell.test, git.read
Forbidden: git.push, destructive.reset
```

### S12 — Tool Activity

**Purpose:** Structured tool-call timeline. Raw output collapsed by default.

**Primary actions:** Open output, Inspect policy, Copy command, Safe rerun

**Blueprint:**

```text
TOOL ACTIVITY

read_file   ✓ 18ms
grep        ✓ 42ms
edit_file   ✓ 91ms
shell       ● running

Selected shell
Command  go test ./internal/authz/...
Policy   allowed
Scope    repository
```

### S13 — Checkpoint

**Purpose:** Blocking human decision for one-way-door, destructive, risky, or policy-defined actions.

**Primary actions:** Choose option, Inspect evidence, Cancel run

**Blueprint:**

```text
CHECKPOINT REQUIRED

Decision
  Choose database migration strategy

A · in-place
B · dual-write
C · defer

Risk HIGH · Reversible NO

[A] [B] [C] [I] Inspect [Q] Cancel
```

### S14 — Verification Overview

**Purpose:** Show truths, artifacts, key links, automated checks, evidence count, and final verdict.

**Primary actions:** Inspect check, Open diff, Review, Complete

**Blueprint:**

```text
VERIFICATION · RUN 84F2

Overall VERIFIED

✓ admins can manage members
✓ managers can modify allowed resources
✓ members cannot mutate protected resources

Typecheck          ✓
Unit tests         ✓
Integration        ✓
Static analysis    ✓
Regression scan    ✓
```

### S15 — Verification Detail

**Purpose:** One verification criterion with expectation, observed result, evidence, command, source, and provenance.

**Primary actions:** Open evidence, Open file, Open related task

**Blueprint:**

```text
CHECK · manager access

Status VERIFIED
Expectation   manager → 200
Observed      200
Evidence      TestManagerAuthorization
File          internal/authz/routes_test.go:184
Command       go test ./internal/authz/... -run TestManagerAuthorization
```

### S16 — Failure / Recovery

**Purpose:** Causal failure surface. Distinguish retry, repair, replan, inspect, and stop.

**Primary actions:** Retry, Repair, Replan, Inspect, Stop

**Blueprint:**

```text
RUN FAILED

Task 4 · Persistence integration
Failure   integration test failed
Cause     expected organization_id column missing in fixture
Impact    2 tests failing

Recovery
[1] Repair fixture and rerun
[2] Replan persistence task
[3] Inspect failure
[4] Stop run

Retry budget 1 / 3
```

### S17 — Diff Review

**Purpose:** Semantic diff surface with file list, change groups, symbol changes, and lazy raw diff.

**Primary actions:** Next/previous file, Open hunk, Stage, Unstage, Revert with checkpoint

**Blueprint:**

```text
DIFF · RUN 84F2

18 files   +482 -117

Logical changes
  Auth model           +1
  Authorization       +1
  Route guards        +3
  Tests               +6

Selected internal/authz/policy.go
```

### S18 — Git Overview

**Purpose:** Repository/branch/upstream/dirty-state/commit distance and safety policy.

**Primary actions:** Open diff, Branches, Worktrees, Commit, Push with policy

**Blueprint:**

```text
GIT

Branch          feature/rbac
Upstream        origin/feature/rbac
Working tree    18 modified · 3 added
Ahead           3
Behind          0

Safety
  force push       blocked
  reset --hard     checkpoint required
```

### S19 — Commit Composer

**Purpose:** Generate logical commits from task boundaries and semantic change groups.

**Primary actions:** Create commit, Edit message, Split, Cancel

**Blueprint:**

```text
COMMIT COMPOSER

1 feat(auth): add organization roles
2 feat(auth): enforce role guards
3 test(auth): add authorization matrix

✓ cohesive
✓ tests grouped with behavior
✓ linked to verified tasks
```

### S20 — Branch / Worktree Manager

**Purpose:** Manage isolated workspaces, active runs, branches and lifecycle safely.

**Primary actions:** New worktree, Inspect, Resume, Remove with checkpoint, Merge

**Blueprint:**

```text
WORKSPACES

● feature/rbac   active   RUN 84F2
○ feature/pay    suspended RUN 71B1
○ main           clean

Removing non-empty worktree requires checkpoint.
```

### S21 — Code Intelligence

**Purpose:** Repository graph overview: packages, files, symbols, imports, tests, hotspots, architecture issues.

**Primary actions:** Open hotspot, Open architecture, Search

**Blueprint:**

```text
CODE INTELLIGENCE

Packages 28 · Files 1,931 · Symbols 14,282
Imports 19,227 · Tests 612

Hotspots
  auth/service.go       HIGH
  router/router.go      HIGH
  storage/repository.go MEDIUM
```

### S22 — Symbol Explorer

**Purpose:** Search and browse symbols using the intelligence graph.

**Primary actions:** Open symbol, References, Callers, History, Impact

**Blueprint:**

```text
SYMBOLS
Search: AuthorizationService

1 AuthorizationService          interface
2 DefaultAuthorizationService   struct
3 NewAuthorizationService       function

References: callers 17 · tests 8 · imports 14
```

### S23 — Impact Analysis

**Purpose:** Signature M31A view. Show direct/indirect dependents, risk categories, and recommended TaskGraph.

**Primary actions:** Generate plan, Inspect graph, Open dependents

**Blueprint:**

```text
IMPACT ANALYSIS

Target AuthorizationService
Requested change: generic provider

Direct   17 callers · 8 tests · 2 interfaces
Indirect 4 HTTP handlers · 1 CLI · 3 fixtures

Risk: API HIGH · tests MEDIUM · runtime HIGH
Suggested plan: 3 tasks · 2 waves
```

### S24 — Dependency Explorer

**Purpose:** Package inventory and legitimacy/risk evidence.

**Primary actions:** Inspect package, Compare alternatives, Approve/reject dependency

**Blueprint:**

```text
DEPENDENCIES

Package            Version   Risk
stdlib             go1.24    LOW
chi                v5.2      LOW
sqlite              v1.37     MEDIUM
external-auth       v0.9      HIGH

Verdict: SUSPECT / HUMAN REVIEW
```

### S25 — Git History / Regression

**Purpose:** Commit candidates, bisect status, hypothesis and evidence.

**Primary actions:** Verify hypothesis, Create repair task, Continue bisect, Compare commit

**Blueprint:**

```text
REGRESSION

Symptom: auth request returns 500

Candidate commits
  a91d3c middleware refactor HIGH
  7f8a10 repository update    MEDIUM

Bisect narrowed culprit to a91d3c

Evidence ✓ reproduction · ✓ introducing diff
```

### S26 — Search Results

**Purpose:** Combined literal/symbol/semantic search with filters.

**Primary actions:** Open result, Filter, Refine query

**Blueprint:**

```text
SEARCH
Query: authorization middleware

Literal 14 · Symbol 9 · Semantic 31

1 internal/authz/middleware.go
2 internal/router/router.go
3 tests/authz/integration_test.go
```

### S27 — Run History

**Purpose:** Paged history of runs with status, objective, duration, branch, risk filters.

**Primary actions:** Open run, Filter, Search

**Blueprint:**

```text
RUN HISTORY

84F2 Add organization RBAC    VERIFIED 04:31
7B1C Fix auth regression       FAILED   01:19
2A91 Explain payment flow      COMPLETE 00:41
991E Refactor repository       CANCELLED 03:22
```

### S28 — Run Detail

**Purpose:** Complete durable run lifecycle, tasks, tools, verification, Git, artifacts.

**Primary actions:** Open task, Open artifacts, Open verification, Resume if paused

**Blueprint:**

```text
RUN 84F2

Created 19:31 · Planned 19:31 · Executing 19:32
Verifying 19:35 · Completed 19:35

Tasks 5/5 · Tool calls 73 · Retries 2
Evidence 14 · Commits 3 · Files 21
```

### S29 — Project State

**Purpose:** Current phase, objective, blockers, open work, decisions, learnings and continuation state.

**Primary actions:** Open blocker, Open decisions, Open handoff

**Blueprint:**

```text
PROJECT STATE

Current phase   Runtime foundation
Objective       Persistent session/run runtime
Blocked by      none
Open work       7 tasks
Recent decisions D-12, D-13
Continuation    no pending handoff
```

### S30 — Decisions

**Purpose:** Durable architecture/product decisions with rationale and status.

**Primary actions:** Inspect decision, Create decision, Supersede decision

**Blueprint:**

```text
DECISIONS

D-12 SQLite for durable runtime state  LOW
D-13 worktree-first execution          MEDIUM
D-14 destructive Git checkpoint        HIGH

Selected D-14: preserve user work and prevent irreversible automation.
```

### S31 — Research

**Purpose:** Research questions, sources, findings, risks, recommendations, unresolved items and evidence class.

**Primary actions:** Inspect source, Compare options, Resolve gap

**Blueprint:**

```text
RESEARCH
Question: How should RBAC persistence work?

Sources: repository ✓ ADRs ✓ docs ✓ registry ✓ web ✓
Findings 7 · Risks 2 · Recommendations 3 · Unresolved 1

Evidence classes: VERIFIED · INFERRED · ASSUMED · UNRESOLVED
```

### S32 — Configuration

**Purpose:** Provider/model/execution/safety/Git/context/TUI/plugin/MCP settings.

**Primary actions:** Edit setting, Reset value, Inspect source

**Blueprint:**

```text
CONFIGURATION

Provider  NVIDIA Build       connected
Model     nemotron-3-ultra   configured
Execution autonomous        enabled
Safety    secret access      deny
Git       force push         deny
TUI       density            compact
```

### S33 — Command Palette

**Purpose:** Fuzzy expert command launcher with category, availability, and context relevance.

**Primary actions:** Search, Select, Close

**Blueprint:**

```text
COMMAND PALETTE
> verify

Verify current run              Run
Verify current task             Run
Open verification               Navigation
Show verification failures     Navigation

↑↓ navigate · Enter select · Esc close
```

### S34 — Help / Keyboard Reference

**Purpose:** Contextual help plus global keyboard reference.

**Primary actions:** Open category, Search command, Close

**Blueprint:**

```text
KEYBOARD

Global  ^K palette · ^C cancel/quit · ? help · Esc back
Run     p pause · c cancel · r retry · v verify · d diff
Git     d diff · c commit · b branches · w worktrees
```

### S35 — Logs / Diagnostics

**Purpose:** Advanced diagnostics: component health, structured events, stack traces, process tree, storage, provider.

**Primary actions:** Inspect event, Open source, Copy diagnostics

**Blueprint:**

```text
DIAGNOSTICS
runtime.orchestrator  healthy
llm.nvidia             healthy
storage.sqlite         warning
provider.stream        healthy

Selected event: task.retry · RUN 84F2 · Task 4
```

### S36 — Error Boundary

**Purpose:** Recoverable fatal UI/runtime boundary. Always preserve run/state context where possible.

**Primary actions:** Resume, Diagnostics, Exit safely

**Blueprint:**

```text
M31A ERROR

The runtime lost the active session lease.
Session S-19AF · Run 84F2
State preserved.

[Resume] [Inspect diagnostics] [Exit]
ERR-84F2-LEASE
```

## 11. Mandatory overlays

Required overlays:

```text
CommandPalette
ConfirmDialog
ChoiceDialog
CheckpointDialog
TextInputDialog
ErrorDialog
DiffPreview
SearchOverlay
FilePicker
RunSelector
HelpDialog
```

Overlay rules:
- save previous focus before opening
- block background interaction while modal
- Esc returns focus to previous region
- at most one blocking decision surface at a time
- destructive actions require proportionate confirmation
- checkpoint state is persisted before display

## 12. Component library

The component system should be composable, typed, deterministic, and independent of runtime domain types. Components consume UI view models/projections.

### `TopBar`

Project identity, branch, run, mode, provider status.

Example:
```text
M31A · project · feature/rbac · RUN 84F2 · ● running
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `NavigationRail`

Major route selection; collapsible below 120 columns.

Example:
```text
Chat / Plan / Run / Verify / Intel / Git / Runs / Project / Settings
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `ContextPanel`

Screen-aware runtime inspector.

Example:
```text
Run: task/agent/tool; Plan: tasks/risk; Verify: evidence; Git: branch/tree
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `InputBar`

Natural-language and explicit command entry.

Example:
```text
> implement RBAC                                          ^K commands
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `StatusBadge`

Semantic state label independent of color.

Example:
```text
[RUNNING] [VERIFIED] [FAILED] [HIGH]
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `ProgressMeter`

Known progress only. Never fabricate percent for unknown duration.

Example:
```text
██████████░░  76%
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `TaskRow`

Compact task status and dependency metadata.

Example:
```text
● 3 Route guards   MEDIUM   Implementer   3 files
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `Timeline`

Chronological runtime events; virtualized for long runs.

Example:
```text
19:32 ● start · 19:33 ✓ tests · 19:34 ◇ checkpoint
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `ToolRow`

Tool identity, status, latency, summary and provenance.

Example:
```text
shell ● 2.31s · Task 3 · 114 tests passed
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `AgentBadge`

Engineering role identity.

Example:
```text
[Planner] [Implementer] [Verifier] [Reviewer]
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `EvidenceRow`

Verification evidence with provenance.

Example:
```text
✓ go test ./...   Test suite
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `RiskIndicator`

LOW/MEDIUM/HIGH/CRITICAL + reason.

Example:
```text
HIGH · one-way-door migration
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `KeyHintBar`

Contextual shortcuts only.

Example:
```text
↑↓ select · Enter open · / search · ^K commands
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `EmptyState`

Explain why empty and offer next useful action.

Example:
```text
No verification yet. [Verify now]
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `ErrorState`

Explain failure, likely cause, and recovery action.

Example:
```text
SQLite locked. [Retry] [Diagnostics]
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `Citation`

Repository/Git/test/research reference.

Example:
```text
[repo] internal/authz/policy.go:71
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `PlanCard`

Compact plan summary in chat.

Example:
```text
PLAN READY · 5 tasks · 2 waves · [Open plan] [Approve]
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `RunCard`

Compact active/completed run projection.

Example:
```text
RUNNING · 3/5 · [Inspect run]
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `VerificationCard`

Compact evidence-backed result.

Example:
```text
VERIFIED · 5/5 checks · 14 evidence items
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `CheckpointCard`

Non-modal checkpoint teaser in chat.

Example:
```text
◇ CHECKPOINT REQUIRED · migration strategy · [Choose]
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `DiffStats`

Change summary before raw diff.

Example:
```text
18 files · +482 -117 · 4 logical groups
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `TaskGraph`

Dependency graph with list fallback on narrow widths.

Example:
```text
1 → 2 → 3 → 5; 1 → 4 → 5
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `VerificationMatrix`

Truth/artifact/check status matrix.

Example:
```text
Role model ✓ · Migration ! · Concurrency ?
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `RequirementTrace`

Requirement → task → file → test → verification chain.

Example:
```text
REQ-AUTH-01 → TASK-001 → roles.go → TEST-01 → VERIFIED
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `PolicyInspector`

Capability decision and scope.

Example:
```text
filesystem.write · workspace scope · allow
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `ModelStatus`

Provider/model availability.

Example:
```text
NVIDIA Build · Nemotron 3 Ultra · ● connected
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `ContextMeter`

Provider-aware but model-neutral context budget meter.

Example:
```text
Context 42k / provider-reported capacity
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `SearchList`

Paged search with source/type filters.

Example:
```text
literal 14 · symbol 9 · semantic 31
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `SymbolView`

Read-only symbol metadata and navigation.

Example:
```text
Authorize() · 17 callers · 8 tests
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `ImpactSummary`

Blast radius with direct/indirect classification.

Example:
```text
HIGH · 17 callers · 8 tests · 6 indirect
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `DependencyRow`

Package/version/risk/evidence.

Example:
```text
external-auth · v0.9 · HIGH · SUSPECT
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `GitStatus`

Branch/tree/upstream/safety.

Example:
```text
feature/rbac · ahead 3 · dirty 21
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `CommitProposal`

Semantic commit grouping tied to tasks.

Example:
```text
feat(auth): add organization roles
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `WorktreeRow`

Branch/path/run/status.

Example:
```text
● feature/rbac · active · RUN 84F2
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `UATCase`

Manual verification step with pass/fail/defer.

Example:
```text
Case 1 · Admin can manage members · [Pass] [Fail]
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

### `Notification`

Transient semantic status message.

Example:
```text
✓ Run 84F2 verified
```

Required contract:
- typed input model
- deterministic render
- focus behavior where applicable
- empty/loading/error treatment
- keyboard behavior
- truncation rule
- accessibility label

## 13. Conversation and agent UX

### Message types

```text
user
assistant
status
plan
execution
verification
checkpoint
warning
error
context_event
```

Low-level read/tool events should be grouped by default. Streaming assistant text is incremental. Provider-private hidden reasoning must not be rendered as if it were normal diagnostics. User-visible progress language should be explicit: `Analyzing repository…`, `Evaluating impact…`, `Planning tasks…`, `Verifying changes…`.

### Fresh-context handoff

When a specialist starts:

```text
IMPLEMENTER STARTED
Fresh context
Loaded: plan + 6 files + 3 constraints
Capabilities: filesystem.write, shell.test, git.read
```

When it completes:

```text
IMPLEMENTER COMPLETE
Changed 7 files · 21 tests passed · no deviations
Handoff → verifier
```

The UI must never imply that a fresh-context specialist retained arbitrary prior conversation history.

## 14. Execution UX contract

Every active run exposes:

```text
run id
objective
state
elapsed time
agent
model
current task
task progress
current tool
files touched
retry count
verification state
policy/approval state
```

Run state machine displayed to users:

```text
QUEUED → RUNNING → VERIFYING → COMPLETED
            │             │
            ▼             ▼
          PAUSED        FAILED
            │             │
            └→ RESUME    ├→ RETRY
                          └→ REPLAN
```

Do not auto-report `COMPLETED` before verification requirements are satisfied.

## 15. Checkpoint UX contract

Checkpoints are first-class runtime states. Persist before presentation. Examples:

```text
- destructive Git
- one-way-door architecture choice
- production-impacting mutation
- suspect dependency installation
- unresolved security risk
- policy-defined human review
```

Checkpoint must show:

```text
what decision is required
why it matters
risk level
reversibility
options
related evidence
what happens after each option
```

If the terminal closes, the checkpoint remains durable as `WAITING_CHECKPOINT`.

## 16. Verification UX contract

Verification must distinguish:

```text
VERIFIED
FAILED
BLOCKED
PRESENT_BEHAVIOR_UNVERIFIED
HUMAN_NEEDED
```

Never collapse `UNVERIFIED` to success. Every check exposes provenance:

```text
expectation
observed
evidence source
command/test
file or artifact
related task
agent provenance
```

If files change during verification, the UI must invalidate the old evidence and request a fresh verification run.

## 17. Git safety UX

Git is a first-class engineering surface. Always expose:

```text
repository
branch
upstream
working tree state
pre-existing changes
agent changes
safety policy
```

Destructive operations require stronger friction. Example:

```text
CRITICAL: force push requested
Reason: history rewrite
This can destroy remote history.
[Type FORCE-PUSH to continue] [Cancel]
```

If a dirty worktree overlaps with planned changes, show the overlap and prefer an isolated worktree. Never silently overwrite ambiguous user changes.

## 18. Code intelligence UX

Core intelligence surfaces:

```text
repository overview
symbol explorer
code viewer
references/callers
dependency explorer
impact analysis
architecture violations
Git history/regression
semantic search
```

The signature M31A workflow is:

```text
requested change
→ direct dependents
→ indirect dependents
→ affected tests
→ architecture/risk analysis
→ suggested TaskGraph
```

Direct and inferred/indirect impact must be visually distinct.

## 19. Research and uncertainty UX

Use evidence classes:

```text
VERIFIED
STRONGLY INFERRED
INFERRED
ASSUMED
UNRESOLVED
```

Avoid arbitrary model confidence percentages. When M31A is uncertain, provide an `Investigate` action that gathers more evidence. Explanation answers what is currently known; investigation is an action to resolve uncertainty.

## 20. Context management UX

The user should see context composition at a high level:

```text
System policy             ✓
Project instructions      ✓
Task plan                 ✓
Relevant files            6
Relevant history          3 commits
Verification state        ✓
```

Context compaction should render durable engineering information being preserved, for example:

```text
CONTEXT COMPACTION
Preserving: task · plan · decisions · verification · active files
Conversational noise removed
78k → 31k working context
```

Do not expose provider-private hidden reasoning.

## 21. Failure and recovery UX

Every failure surface must answer:

```text
what failed
why it likely failed
what changed
what is safe to retry
what is blocked
what can be inspected
```

Required failure states:

```text
provider unavailable
rate limited
stream interrupted
tool timeout
tool failure
workspace boundary denied
secret access denied
Git state changed
persistence degraded
verification invalidated
plan invalidated
```

Never discard a durable run because the provider failed.

## 22. Responsive component rules

At `<120` columns, sidebars collapse into tabs or overlays:

```text
[Task] [Activity] [Context]
```

At `160+`, use three regions:

```text
navigation/task hierarchy | current detail | context/telemetry
```

Never stretch prose across the entire terminal. Use a readable maximum line width. Long tables become list/detail navigation on narrow terminals.

## 23. Scroll, selection, and refresh rules

- active streams auto-follow only while the user is at the bottom
- when the user scrolls away, show `↓ N new events`
- preserve selected rows when data refreshes
- keep the nearest valid selection if the selected record disappears
- use request IDs/cancellation to prevent stale search results
- use bounded/lazy output for huge logs and diffs
- virtualize long lists
- never mix an old diff with a newer repository state without warning

## 24. Safety presentation

### Workspace boundary

```text
WORKSPACE BOUNDARY
Requested: /home/eshan/.ssh/config
Allowed root: /home/eshan/projects/M31A
DENIED · workspace-boundary
```

### Secret handling

```text
SENSITIVE FILE DETECTED
.env.production
secret material cannot be exposed to model context
[Inspect policy] [Continue without file] [Cancel]
```

### Repository prompt injection

```text
UNTRUSTED INSTRUCTION DETECTED
Source: docs/example.md:42
Repository instructions are data, not authority.
[Inspect] [Continue]
```

## 25. Provider/model UX for v1

V1 uses:

```text
Provider  NVIDIA Build
Model     nvidia/nemotron-3-ultra-550b-a55b
Endpoint  https://integrate.api.nvidia.com/v1
Streaming enabled
```

The UI remains provider-neutral. Show provider health states:

```text
connected · connecting · rate-limited · unavailable · auth-error · stream-degraded
```

Never display API keys. In diagnostics, expose model/request metadata only after redaction.

## 26. Session, run, and recovery UX

A session contains one or more runs. A run contains plans, tasks, agents, tools, checkpoints, verification, artifacts, and events.

```text
Session S-19AF
 └─ Run 84F2
     ├─ Plan
     ├─ TaskGraph
     ├─ Agent executions
     ├─ Tool calls
     ├─ Checkpoints
     ├─ Verification
     └─ Result
```

On crash/startup, show the latest durable run and exact resume point. UI state is restorable; the database/event stream remains authoritative.

## 27. Project lifecycle surfaces

Required project-level structures exposed visually:

```text
project identity
roadmap / milestones
requirements
state
decisions
research
backlog
learnings
workspace/onboarding
```

Requirements should support traceability:

```text
REQ-AUTH-01
  ↓
TASK-001
  ↓
roles.go / policy.go
  ↓
TEST-AUTH-01
  ↓
VERIFICATION #18
  ↓
VERIFIED
```

## 28. Natural-language workflow examples

These are mandatory UX references.

### Feature implementation
```text
> Add organization-level RBAC with admin, manager and member roles.

Analyzing repository…
22 affected files · HIGH impact

Plan ready · 5 tasks · 2 waves

[View plan] [Run]
```

### Code archaeology
```text
> Why does AuthorizationService exist?

Introduced in commit a13f9b.
Original problem: cross-organization access.
Current callers: 17.
Original rationale: still relevant.

[Open history] [Open callers]
```

### Regression
```text
> Find the regression introduced after commit a91d3c.

8 candidate commits → culprit narrowed to a91d3c.
Evidence ✓ reproduction · ✓ diff · ✓ call graph.

[Verify hypothesis] [Create repair task]
```

### Git semantic commits
```text
> Split my current changes into logical commits.

Detected 4 groups.
1 feat(auth): add organization roles
2 feat(auth): enforce role guards
3 test(auth): add authorization matrix
4 docs(auth): document role semantics

[Accept] [Customize]
```

## 29. Special mode surfaces

### Plan mode
```text
PLAN MODE · mutation disabled
Available: research · impact · planning · review
```

### Review mode
```text
REVIEW MODE · no mutations
Available: plan · diff · verification · evidence · Git
```

### Verify-only
```text
VERIFY · working tree
[Run all] [Select checks]
```

### Read-only degraded mode
```text
M31A · READ ONLY
Reason: workspace mounted read-only
```

## 30. UAT and human verification

Manual verification is a legitimate terminal surface, not an error path.

```text
USER ACCEPTANCE TEST
Case 1 · Admin can manage members
1. Sign in as admin
2. Open organization members
3. Invite member
Expected: invite succeeds

[Pass] [Fail] [Skip]
```

UAT sessions are durable and resumable.

## 31. Diagnostics and observability

Normal screens show semantic summaries. Diagnostics show raw structured state.

Required diagnostic views:

```text
component health
event stream
state transitions
process tree
SQLite/storage
provider request metadata
tool latency
agent performance
```

Never make raw debug logs the normal workflow.

## 32. TUI package blueprint

Recommended Go structure:

```text
internal/tui/
├── app/
├── router/
├── layout/
├── theme/
├── keys/
├── viewport/
├── screen/
│   ├── chat/
│   ├── project/
│   ├── onboarding/
│   ├── plan/
│   ├── run/
│   ├── verify/
│   ├── git/
│   ├── intelligence/
│   ├── history/
│   ├── research/
│   ├── decisions/
│   ├── settings/
│   └── diagnostics/
├── component/
│   ├── primitive/
│   ├── conversation/
│   ├── plan/
│   ├── execution/
│   ├── verification/
│   ├── git/
│   └── intelligence/
└── overlay/
```

Foundational libraries: Bubble Tea, Lip Gloss, Bubbles. Every additional UI dependency requires justification.

## 33. View-model rules

Complex screens should follow:

```text
runtime domain state
      ↓
application query / event projection
      ↓
UI view model
      ↓
Bubble Tea model
      ↓
render
```

Components must not receive SQL handles, Git clients, LLM clients, or policy engines. They receive narrow view models and emit intent-oriented events such as `OnOpenTask(taskID)` or `OnApprovePlan(planID)`.

## 34. Rendering and performance rules

Required:
- no domain logic in `View()`
- no direct DB calls from components
- no full-screen rebuild for every token when avoidable
- no unbounded output kept in active render state
- lazy raw diff/log loading
- virtualized long lists
- deterministic map ordering for snapshots
- safe Unicode display-width calculations
- safe terminal state restoration on all exits
- no panic on resize, malformed event, missing Git, unavailable provider, or partial project initialization

## 35. Accessibility and terminal compatibility

Support:

```text
NO_COLOR
monochrome theme
reduced motion
ASCII fallback
SSH-friendly input
keyboard-only operation
```

Never rely on color or animation for essential meaning. Native terminal mouse selection should not be broken by mandatory mouse capture.

## 36. Testing contract

The TUI requires:

```text
unit tests
component tests
Bubble Tea update/model tests
snapshot/golden tests
keyboard interaction tests
resize tests
integration tests
failure-injection tests
```

Required golden fixtures:

```text
chat_ready
chat_streaming
plan_ready
plan_blocked
run_running
run_paused
checkpoint_required
verification_passed
verification_failed
provider_unavailable
git_clean
git_dirty
git_destructive_confirmation
impact_high
onboarding_running
onboarding_failed
```

Minimum snapshot sizes:
```text
80x24
100x30
120x40
160x50
```

## 37. Failure-injection journeys

Manually and automatically exercise:

```text
provider disconnect
rate limit
stream corruption
SQLite lock
terminal resize storm
tool hang
tool timeout
run cancellation
checkpoint persistence failure
Git command failure
workspace disappearance
verification invalidation
pre-existing user changes
non-Git repository
multi-repository workspace
repository prompt injection
secret file access attempt
```

The TUI must preserve durable state and present a recovery action instead of panicking.

## 38. Mandatory user journeys

### Journey A — explain
`> explain the authentication flow`

Expected: targeted source references, architecture explanation, citations, no unnecessary plan.

### Journey B — implement
`> Add organization-level RBAC with admin, manager and member roles.`

Expected: intent → impact → plan → approval → execution → verification → result.

### Journey C — regression
`> Find the regression introduced after commit a91d3c.`

Expected: history → candidate commits → hypothesis → verification → repair task.

### Journey D — semantic Git
`> Split my current changes into logical commits.`

Expected: diff → logical groups → commit proposal → approval → commits.

### Journey E — destructive Git
`> Reset this branch to origin/main.`

Expected: preflight → risk → checkpoint → explicit confirmation → execution.

### Journey F — provider failure
Disconnect NVIDIA during execution. Expected: provider error → run preserved → retry/save/resume.

### Journey G — crash recovery
Kill M31A during a run. Expected: restart → recovery → resume from durable state.

### Journey H — workspace escape
Attempt access outside root. Expected: policy denied → safe explanation → no secret disclosure.

## 39. UI acceptance checklist

```text
[ ] stable startup and terminal restoration
[ ] project discovery
[ ] onboarding
[ ] chat
[ ] command palette
[ ] plan overview/detail/review
[ ] run dashboard/task/agent/tool
[ ] checkpoints
[ ] verification overview/detail/matrix/UAT
[ ] failure/recovery
[ ] diff review
[ ] Git overview/commit/worktree/branch
[ ] code intelligence/symbol/impact/dependency/regression
[ ] project state/requirements/decisions/research
[ ] run/session history
[ ] settings/diagnostics
[ ] responsive layouts
[ ] no-color/ASCII fallback
[ ] resize handling
[ ] cancellation/retry/replan UX
[ ] crash recovery
[ ] provider failure recovery
[ ] stale-event prevention
[ ] verification invalidation on source changes
[ ] no UI-owned domain logic
[ ] no policy bypass
[ ] no false verification claims
```

## 40. Implementation order

Build in this order:

```text
1 AppShell + routing + theme + focus
2 InputBar + CommandPalette
3 Chat + structured event cards
4 Project + onboarding
5 Plan surfaces
6 Run surfaces
7 Checkpoint
8 Verification
9 Git
10 Code intelligence
11 History
12 Settings
13 Diagnostics
```

Do not create polished mock screens against fake state and postpone runtime integration. Each slice should connect its query/event contracts to the real application layer as early as possible.

## 41. Definition of done for a screen

A screen is complete only when:

```text
runtime query/command contract exists
view-model projection exists
ready state exists
loading state exists
empty state exists
error state exists
responsive state exists
keyboard navigation exists
command palette actions exist
stale result handling exists where asynchronous
snapshot fixture exists
integration test exists
```

A happy-path mockup is not a finished M31A screen.

## 42. Canonical final experience

M31A should feel like this:

```text
M31A · RUN 84F2 · feature/rbac

PLAN
  5 tasks · 2 waves · MEDIUM

EXECUTION
  ✓ Role model
  ✓ Authorization service
  ● Route guards
  ○ Persistence
  ○ Verification

CURRENT
  Implementer · edit_file · 3 files · 00:42

VERIFICATION
  0/5 complete

> steer the run or inspect details...
```

Not:

```text
AI
> Thinking...
> running command...
> done!
```

The objective is engineering clarity, trust, control, recoverability, and speed.

## 43. Final doctrine

The TUI is the visual projection of the M31A engineering loop. It should reveal the system's actual state without turning the terminal into an information dump. M31A's power must come from the runtime—persistent state, task graphs, capability control, repository intelligence, safe execution, and evidence-backed verification—not from visual complexity.

> **Do not optimize M31A's TUI for novelty. Optimize it for engineering clarity, trust, control, recoverability, and speed.**
