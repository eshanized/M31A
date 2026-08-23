# M31A — Core Problem & Architecture Correction

> **Status:** Authoritative problem diagnosis and architecture correction
> **Date:** 2026-08-23
> **Language:** Go (unchanged)
> **Scope:** Existing M31A repository, runtime architecture, workflow, context, persistence, execution, verification, recovery, and TUI
>
> **Important:** This document does **not** propose a language rewrite. M31A remains a Go project. The purpose is to identify why the existing Go implementation has become difficult to make reliable and how the existing codebase should be reorganized without losing its working capabilities.

---

# 1. Executive Diagnosis

M31A's core problem is **not that the project is written in Go**.

M31A's core problem is also **not simply that `workflow.Engine` is large**.

The deeper problem is:

> **M31A has multiple partially overlapping sources of truth for the same engineering run, and the workflow engine continuously translates between them through shared mutable state, filesystem persistence, prompt state, task state, and UI state.**

That creates a chain of secondary problems:

```text
No single authoritative run model
            ↓
Duplicated state
            ↓
Cross-component synchronization
            ↓
Mutexes / channels / file locks / caches
            ↓
State drift
            ↓
Context drift
            ↓
Workflow edge cases
            ↓
Recovery complexity
            ↓
Hard-to-test behavior
            ↓
Hard-to-predict agent behavior
```

This is the root issue to fix.

The oversized `workflow.Engine` is a **symptom** of this problem because it has become the place where those different state representations are coordinated.

Simply splitting `engine.go` into more files will improve readability but will not solve the architectural problem.

---

# 2. Current Repository Reality

The current Go repository already contains significant functionality.

The README describes a seven-phase system:

```text
Initialize
→ Discuss
→ Plan
→ Execute
→ Verify
→ Runtime
→ Ship
```

It includes model providers, streaming, task scheduling, tool calls, code intelligence, self-healing, context compaction, persistence, metrics, Git rollback, and a Bubble Tea TUI. citeturn0search1

The current workflow implementation is materially real, not merely scaffolding:

- `RunPhase` performs lifecycle setup and dispatches to concrete phase implementations.
- `runInitialize` performs project detection, optional deep analysis, preflight, Git initialization, planning-state creation, and checkpoints.
- `runDiscuss` constructs context, streams the model, parses questions, performs quality checks, and stores discussion state.
- `runPlan` performs research, planning, validation, plan checking, revision, coverage gates, and task persistence.
- `runExecute` schedules dependency groups, executes tasks with bounded parallelism, persists state, and performs self-healing.

Those behaviors should be preserved as capabilities while their ownership is corrected.

---

# 3. The Real Architectural Problem

## 3.1 One engineering run exists in too many representations

An M31A run currently has meaningful state in several places:

```text
Engine fields
WorkflowState
StateMachine
Session Manager
session.json
messages.json
planning files
Task files
WorkflowCache
ContextBuilder caches
Context Registry snapshot
Code-intel cache
Recovery state
Git state
TUI state
```

This is visible directly in the current implementation.

`Engine` stores session identity, working directory, provider/model, state machine, dispatcher, token estimator, session manager, prompt builder, workflow mode, per-phase models, code-intelligence state, ledger, compactor, context registry, metrics, phase coordinator, hooks, mutable workflow state, lifecycle channels, and pause/cancel channels. fileciteturn13file0

`WorkflowState` separately stores plan state, research output, cached prompts, conversation messages, intent results, dynamic context snapshots, decision logs, healing reports, checkpoint data, and the current goal, with multiple mutexes and an explicit lock-ordering contract. fileciteturn16file0

The session manager separately persists project-local state into `.m31a/` files such as `session.json` and `messages.json`, while also serving as the persistence API used by workflow code. fileciteturn18file0

This means the same conceptual run is reconstructed from several representations instead of being owned by one authoritative runtime model.

---

# 4. Why `workflow.Engine` Became So Large

The engine is not intrinsically too large because Go cannot handle large systems.

It is large because every subsystem needs access to some part of the current run.

The result is an object that acts as:

```text
workflow coordinator
+
state container
+
provider holder
+
model router
+
context owner
+
token tracker
+
code-intel owner
+
Git coordinator
+
tool dispatcher owner
+
session persistence adapter
+
recovery controller
+
pause controller
+
cancellation root
+
hook host
+
metrics host
+
TUI event source
```

That creates a **god object by dependency accumulation**, not by poor file organization.

The correct response is therefore not:

```text
engine.go → engine_a.go + engine_b.go + engine_c.go
```

The correct response is:

```text
Engine responsibilities
        ↓
explicit domain services
        ↓
explicit contracts
        ↓
explicit ownership
```

---

# 5. The Most Important Finding: State Is Split-Brained

This is the core diagnosis.

M31A has:

```text
Runtime state
```

and:

```text
Persistence state
```

and:

```text
Conversation/context state
```

and:

```text
UI state
```

and:

```text
Git state
```

and:

```text
derived cache state
```

but no single representation is the authoritative **engineering run**.

Instead, components continually synchronize with each other.

Example:

```text
Task status
   ↓
session.Manager
   ↓
taskrunner
   ↓
WorkflowState
   ↓
TODO.md
   ↓
TUI
   ↓
verification
```

Another:

```text
Conversation messages
   ↓
WorkflowState.Messages
   ↓
ContextBuilder
   ↓
Compactor
   ↓
Provider request
   ↓
TUI stream
   ↓
messages.json
```

Another:

```text
Current phase
   ↓
StateMachine
   ↓
Session JSON
   ↓
Recovery file
   ↓
TUI
   ↓
workflow callbacks
```

The architecture is therefore fundamentally synchronization-heavy.

---

# 6. Why This Produces So Many Bugs

When multiple components can represent the same state, bugs increasingly become **state convergence bugs** rather than local code bugs.

Examples of the failure class:

```text
TUI says executing
but persisted session says planning.

Task runner says done
but persisted task state says pending.

Git has a changed file
but code-intel cache still describes the old tree.

Context snapshot represents old source
but repository has changed.

Recovery says phase X
but StateMachine is already at phase Y.

Plan version in memory differs from plan artifact.
```

These bugs are harder to reproduce because the program can be locally correct while the combined state is inconsistent.

---

# 7. Context Is a Secondary Symptom of the Same Problem

The current context architecture is not merely a token-budget problem.

It currently has several layers:

```text
WorkflowState.Messages
ContextBuilder
ContextRegistry
ContextSnapshot map
cached dynamic context
PromptBuilder
Compactor
TokenEstimator
preflightContextCheck
```

`ContextBuilder` directly references workflow state, configuration, tokens, working directory, context registry, and a callback back into `Engine` for model selection. fileciteturn17file0

The current context guard is primarily a **conversation-size management mechanism**:

```text
estimate tokens
→ compact
→ truncate tool output
→ truncate old assistant messages
→ remove older messages
→ reject if still too large
```

That is useful, but it answers:

> "How do we stop the prompt from getting too big?"

rather than:

> "What does the model need for this particular engineering decision?"

Therefore the new Context Engine must be built on top of a canonical run/task model rather than being another layer of prompt manipulation.

---

# 8. The Core Correction

We do **not** need to rewrite M31A in another language.

We need to establish one authoritative runtime model for an engineering run.

The target is:

```text
                         EngineeringRun
                                │
          ┌─────────────────────┼────────────────────┐
          │                     │                    │
          ▼                     ▼                    ▼
        Plan                  Tasks               Context
          │                     │                    │
          │                     ▼                    │
          │                  Agents                  │
          │                     │                    │
          └─────────────────────┼────────────────────┘
                                │
                                ▼
                           Tool Calls
                                │
                                ▼
                         File / Git Changes
                                │
                                ▼
                           Verification
                                │
                         ┌──────┴──────┐
                         ▼             ▼
                      Recovery       Evidence
                                │
                                ▼
                           Run Outcome
```

Everything related to that run must reference the same IDs and authoritative records.

---

# 9. EngineeringRun Becomes the Runtime Boundary

Introduce a first-class run identity concept.

Conceptually:

```go
type EngineeringRun struct {
    ID         string
    ProjectID  string
    SessionID  string
    Goal       string
    Mode       WorkflowMode
    Status     RunStatus
    PlanID     string
    CreatedAt  time.Time
    UpdatedAt  time.Time
}
```

The exact struct can evolve.

The important property is that the run is the parent identity for everything else.

Tasks reference the run.

Agents reference the run and task.

Context snapshots reference the run and task.

Tool calls reference the run/task/agent.

Verification references the run/task.

Recovery references the run/task/attempt.

UI state derives from the run.

---

# 10. The State Authority Rule

M31A must establish a simple rule:

> **Every stateful subsystem gets one authoritative owner. Everything else is a projection, cache, or observation.**

For example:

```text
Run state
→ Runtime/Storage

Plan state
→ Planning/Storage

Task state
→ Task store

Agent state
→ Agent runtime/store

Context snapshot
→ Context store

Verification result
→ Verification store

Permission decision
→ Permission store

TUI
→ projection

Markdown
→ projection/artifact

Cache
→ disposable
```

This dramatically reduces synchronization complexity.

---

# 11. Persistence Should Be Reworked, Not Merely Replaced

The current session manager deliberately uses project-local flat files such as `session.json` and `messages.json`. It also maintains file locking and a coordinator for per-session concurrency. fileciteturn18file0

That design contains useful engineering lessons:

- atomic writes;
- file-size limits;
- corruption detection;
- session locking;
- concurrency coordination;
- resumable state.

The problem is that flat files are being used as the persistence boundary for a complex multi-entity runtime.

They become cumbersome because a run contains relationships between:

```text
run
plan
plan version
task
task dependency
agent
attempt
tool call
verification
evidence
recovery
checkpoint
message
context snapshot
```

These relationships are naturally relational.

Therefore the Go system should move authoritative run-state persistence into SQLite while retaining Markdown/JSON projections where useful.

This is a **persistence architecture correction**, not a language change.

---

# 12. Why SQLite Helps the Actual Problem

SQLite is appropriate because M31A is a local-first application.

It provides:

- atomic transactions;
- relational integrity;
- indexed queries;
- durable state;
- simple backup;
- predictable local operation;
- better recovery semantics than coordinating many flat files.

The design should not turn SQLite into a distributed event system.

Use it as the authoritative local runtime store.

Use explicit transactions for state changes such as:

```text
TaskStarted
TaskCompleted
VerificationPassed
RecoveryStarted
RunPaused
RunCompleted
```

The UI and artifacts then become projections.

---

# 13. Workflow Phases Should Become Orchestration, Not Storage

Keep the existing vocabulary:

```text
Initialize
Discuss
Plan
Execute
Verify
Runtime
Ship
```

It is useful and part of M31A's identity. fileciteturn6file0

But phases should call domain services.

Example:

```text
Execute
  ↓
TaskScheduler
  ↓
AgentRuntime
  ↓
ContextEngine
  ↓
ModelGateway
  ↓
ToolRuntime
```

The phase should not own every one of those systems.

---

# 14. What the Existing `WorkflowState` Is Telling Us

`WorkflowState` is effectively a second mini-runtime.

It contains:

```text
transition lock
plan state
research output
prompt caches
messages
intent result
context snapshots
cached dynamic context
decision logger
heal report
checkpoint data
current goal
```

and its own lock hierarchy. fileciteturn16file0

This is an important clue.

The team already recognized that `Engine` had too much mutable state and extracted `WorkflowState`.

But that extraction solved the wrong problem.

It reduced the number of fields on `Engine`.

It did not eliminate the fact that workflow state, conversation state, context state, checkpoint state, and decision state are all being modeled as fields on one shared mutable object.

The next step is therefore to split **state ownership**, not merely state location.

---

# 15. Current Context Architecture: What to Keep

Keep these ideas:

- token estimation;
- provider-specific context limits;
- automatic compaction;
- dynamic context sources;
- protected information;
- context thresholds;
- tool-output limits;
- model-specific prompt composition;
- repository intelligence.

These are good mechanisms.

But move them behind the new context contract.

---

# 16. New Context Architecture

The Context Engine should consume the authoritative run/task model.

```text
ContextRequest
   ↓
Run
   ↓
Task
   ↓
Plan slice
   ↓
Repository intelligence
   ↓
Evidence
   ↓
Memory
   ↓
Git state
   ↓
Policy / permissions
   ↓
Relevance
   ↓
Budget
   ↓
ContextSnapshot
```

The Context Engine should not need to inspect arbitrary fields on `WorkflowState`.

---

# 17. ContextSnapshot

Every model decision gets a snapshot.

Conceptually:

```go
type ContextSnapshot struct {
    ID          string
    RunID       string
    TaskID      string
    AgentID     string
    Decision    string
    Model       string
    TokenBudget int
    TokenUsed   int
    Sources     []ContextSource
    Hash        string
    CreatedAt   time.Time
}
```

The actual content can remain in a separate artifact if necessary.

The critical point is attribution.

M31A should be able to answer:

```text
What did the model know?
Why was it included?
What task was active?
What plan version was active?
Which repository state was observed?
What evidence was available?
```

---

# 18. Execution Must Consume a Task Graph

The current executor already uses dependency ordering and bounded parallelism. That behavior should remain.

The correction is to make the Task Graph authoritative.

```text
TaskGraph
├── Task
├── Dependency
├── State
├── Attempt
├── Acceptance
└── Verification
```

The scheduler produces runnable tasks.

The agent runtime executes a task.

The verification system decides whether it passed.

The scheduler never infers completion solely from model output.

---

# 19. Self-Healing Should Become Recovery

The existing self-heal feature is useful.

The problem is architectural coupling.

Today, the execute path can directly invoke healing, quality gates, verification, and commit logic.

The target is:

```text
Executor
  ↓
Failure
  ↓
RecoveryService
  ↓
Repair attempt
  ↓
Verifier
  ↓
Pass / Fail
```

The executor does not decide how recovery works.

The recovery system does not decide whether the final task is verified.

The verifier remains authoritative for verification.

---

# 20. The Same Principle Applies to Git

A Git commit is not proof of correctness.

A successful model response is not proof of correctness.

A successful tool call is not proof of correctness.

Only verification evidence can establish task completion.

The runtime should therefore track:

```text
Change
→ Verification
→ Evidence
→ Commit
```

not:

```text
Change
→ Commit
→ assume success
```

---

# 21. The TUI Is Also a Projection

The TUI should display runtime truth rather than maintain a second copy.

For example:

```text
SQLite / Runtime
      ↓
Event Bus
      ↓
Bubble Tea Model
      ↓
View
```

The TUI can optimistically render transient state, but durable workflow status must come from runtime state/events.

This reduces UI/runtime divergence.

---

# 22. The Current Seven-Phase System Should Not Be Deleted

The seven-phase workflow is one of M31A's product differentiators. The README explicitly presents it as the core product loop. fileciteturn6file0

Keep it.

Improve what each phase owns.

```text
Initialize
→ discover + prepare

Discuss
→ resolve ambiguity

Plan
→ produce validated task graph

Execute
→ perform bounded mutations

Verify
→ produce evidence

Runtime
→ optionally run deployed/local runtime checks

Ship
→ finalize verified changes
```

The architecture should support skipping phases for simple tasks, as the current modes already do.

---

# 23. `/planning` In the Correct Architecture

Example request:

```text
/planning
Add JWT authentication to the existing API while preserving current sessions for 30 days.
```

Planning should produce:

```text
Intent
Requirements
Constraints
Open Questions
Research Findings
Architecture Decision
Task Graph
Acceptance Criteria
Verification Strategy
```

Then persist the plan.

Example task graph:

```text
Task 1
Inspect current auth/session model.

Task 2
Add JWT verification infrastructure.
Depends on: 1

Task 3
Implement dual-auth compatibility layer.
Depends on: 2

Task 4
Migrate protected route handling.
Depends on: 3

Task 5
Add compatibility tests.
Depends on: 4

Task 6
Run security and regression verification.
Depends on: 5
```

This is the point where M31A should be substantially better than a generic coding agent.

---

# 24. Example End-to-End Run

User:

```text
Create a REST API for task management with Google OAuth,
PostgreSQL, and Docker deployment.
```

## Initialize

M31A discovers:

```text
Go
Gin
PostgreSQL
Docker
```

Creates:

```text
run
project
repository profile
```

## Discuss

M31A asks:

```text
Should tasks be private to each authenticated user?
```

User:

```text
Yes.
```

Decision becomes durable run state.

## Plan

M31A produces:

```text
Phase 1
Database foundation

Phase 2
OAuth/session

Phase 3
Task API

Phase 4
Tests + Docker verification
```

## Execute

Tasks run in dependency order.

Each task gets a ContextSnapshot.

## Verify

Run:

```text
go test ./...
go vet ./...
docker build ...
```

## Recovery

A test fails.

M31A creates:

```text
FailureRecord
RecoveryAttempt
new ContextSnapshot
```

It repairs and verifies again.

## Ship

Only after required evidence passes:

```text
review
commit
summary
ledger
```

This is the workflow we want, regardless of how many internal packages implement it.

---

# 25. Migration Plan Inside Go

Do this incrementally.

## Step 1 — Establish the run model

Introduce authoritative IDs for:

```text
Run
Plan
Task
Agent
Attempt
Verification
Evidence
```

Do not change the TUI yet.

## Step 2 — Introduce the storage abstraction

Hide persistence behind interfaces/services.

Keep the existing flat-file implementation temporarily if necessary.

Then add SQLite as the authoritative backend.

## Step 3 — Extract Context Engine

Move context construction away from `WorkflowState` and `Engine`.

## Step 4 — Extract Task/Execution Service

Move scheduler and agent lifecycle away from the monolithic engine.

## Step 5 — Extract Verification

Make verification produce durable evidence.

## Step 6 — Extract Recovery

Move self-healing policy out of execute code.

## Step 7 — Rework TUI as projection

Make Bubble Tea consume runtime events/state instead of owning lifecycle logic.

## Step 8 — Remove duplicate state

After all consumers move, delete old fields, caches, and compatibility paths.

---

# 26. What Not To Do

Do not:

```text
rewrite in Rust
rewrite every package at once
split engine.go without changing ownership
add more features before fixing state authority
add RAG before repository intelligence is correct
add multi-agent complexity before one-agent runs are reliable
add another cache to hide stale state
add another JSON artifact as a new source of truth
```

Do:

```text
fix state authority
fix ownership
fix lifecycle
fix context construction
fix persistence
fix evidence
then expand capabilities
```

---

# 27. The Core Problem in One Sentence

> **M31A is suffering from distributed state ownership inside a single-process application: the same engineering run is represented and mutated by the workflow engine, workflow state, session files, context state, task runner, recovery state, Git state, and TUI, which forces synchronization and creates drift.**

The oversized engine, lock complexity, context fragility, recovery complexity, and hard-to-test behavior are consequences of that problem.

---

# 28. The Correct Target

The target is not:

```text
smaller Engine
```

It is:

```text
one authoritative run model
        ↓
explicit domain owners
        ↓
projections everywhere else
```

Conceptually:

```text
                    EngineeringRun
                           │
          ┌────────────────┼────────────────┐
          ▼                ▼                ▼
       Planning         Execution        Verification
          │                │                │
          └────────────────┼────────────────┘
                           ▼
                      Context Engine
                           │
                           ▼
                      Model Gateway
                           │
                           ▼
                      Tool Runtime
                           │
                           ▼
                        Evidence
                           │
                           ▼
                       Recovery
                           │
                           ▼
                        Run State
                           │
                           ▼
                       SQLite
                           │
              ┌────────────┴────────────┐
              ▼                         ▼
          Bubble Tea               Markdown/JSON
          projection               projections
```

---

# 29. Architectural Invariants

### Invariant 1
There is one authoritative state for an EngineeringRun.

### Invariant 2
A projection cannot become authoritative merely because it is convenient.

### Invariant 3
Context is constructed for decisions, not merely for conversations.

### Invariant 4
Every model decision has attributable context.

### Invariant 5
Task state is not inferred from UI state.

### Invariant 6
Verification state is not inferred from model claims.

### Invariant 7
Recovery state is durable.

### Invariant 8
Every goroutine has an owner and cancellation path.

### Invariant 9
Every tool invocation passes through policy and capability checks.

### Invariant 10
Caches are disposable and never authoritative.

### Invariant 11
Current repository evidence outranks stale memory.

### Invariant 12
Autonomous mode uses the same reliable primitives as interactive mode.

---

# 30. Architectural Success Criteria

The rework is successful when:

- one run can be inspected as one coherent entity;
- state can be reconstructed after restart without guessing between files;
- context can be inspected and explained for every model decision;
- task state cannot silently diverge between scheduler, persistence, and UI;
- cancellation reaches all descendants;
- verification produces durable evidence;
- recovery is bounded and attributable;
- TUI state is derived from runtime state/events;
- new features do not require adding another responsibility to `workflow.Engine`;
- the existing seven-phase product experience remains intact;
- Go remains the implementation language.

---

# 31. Final Position

M31A does not have a **language problem**.

M31A does not primarily have a **feature problem**.

M31A does not primarily have an **LLM problem**.

M31A's core problem is **state authority and ownership**.

The current architecture has grown by adding capabilities to a central workflow object and surrounding it with additional caches, persistence mechanisms, locks, callbacks, and projections.

That growth has produced an application in which local components can be correct while the overall engineering run becomes inconsistent or difficult to reason about.

The correction is therefore:

```text
Current M31A

many representations
        ↓
many synchronization paths
        ↓
large central coordinator
        ↓
fragile emergent behavior

                    ↓ REWORK ↓

Target M31A

one authoritative run model
        ↓
explicit ownership
        ↓
domain services
        ↓
context snapshots
        ↓
evidence-backed transitions
        ↓
projections
        ↓
predictable autonomous engineering
```

**Keep Go. Keep the seven-phase workflow. Keep the good parts already built. Rework the ownership model underneath them.**
