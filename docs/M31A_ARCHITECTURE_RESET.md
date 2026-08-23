# M31A Architecture Reset

> **Status:** Authoritative architecture correction
> **Date:** 2026-08-23
> **Scope:** M31A product, runtime, persistence, workflow, context, execution, verification, security, TUI, and migration strategy
> **Supersedes:** Earlier architecture/roadmap assumptions that treat the existing Go `workflow.Engine` as the long-term system boundary

---

## 1. Purpose

M31A has accumulated a substantial amount of working functionality in Go: workflow orchestration, provider integrations, streaming, tool execution, task scheduling, self-healing, Git integration, code intelligence, context compaction, session persistence, metrics, and a Bubble Tea TUI.

The problem is no longer simply missing features or isolated bugs. The central problem is architectural coupling.

The existing implementation concentrates too much responsibility in `workflow.Engine` and spreads runtime state across mutable in-memory structures, channels, mutexes, session files, and workflow-specific helpers. Continued feature development on that foundation increases complexity faster than capability.

This document establishes the correction:

> **M31A will evolve into a Rust/Tokio autonomous engineering runtime with explicit domain ownership, event-driven orchestration, SQLite-backed authoritative state, a first-class Context Engine, capability-based tool execution, explicit verification/evidence, and a TUI that is a client of the runtime rather than the runtime itself.**

The current Go implementation is retained as a behavioral and feature reference during migration. It is not the architectural template for the new core.

---

## 2. Current-State Reality

The current repository is materially more complete than earlier audit snapshots suggested.

Implemented behavior includes, among other things:

- seven workflow phases and guarded phase transitions;
- streamed LLM responses and native tool-call accumulation;
- plan generation, validation, revision, research, and coverage gates;
- dependency-aware task execution with bounded parallelism;
- task self-healing and acceptance-quality checks;
- Git initialization, commits, checkpoints, and recovery concepts;
- code intelligence and repository analysis;
- context estimation, compaction, and truncation;
- permission and tool-dispatch infrastructure;
- metrics and decision logging;
- Bubble Tea terminal UI and workflow progress reporting.

Therefore this reset is **not** a declaration that the Go project is a failed prototype. It is an architectural decision based on the fact that a large amount of real behavior now exists behind an increasingly coupled runtime boundary.

Historical audit documents remain useful as evidence of previously discovered problems, but they must not be treated as a current product-status snapshot without checking the current `master` implementation.

---

## 3. Problems This Reset Corrects

### 3.1 The `workflow.Engine` is overloaded

The current engine owns or coordinates provider/model state, workflow state, task execution, session persistence, code intelligence, context construction, compaction, metrics, hooks, permissions, recovery, cancellation, pause/resume, and tool dispatch.

Splitting `engine.go` into multiple files reduces file size but does not solve ownership. The correction is to split the **domain**, not merely the source file.

### 3.2 Workflow phases are being used as the primary application state model

`Initialize → Discuss → Plan → Execute → Verify → Runtime → Ship` is useful as a user-facing workflow vocabulary, but it is not sufficient as the complete runtime state model.

M31A needs independent state dimensions for runs, tasks, agents, permissions, verification, recovery, and sessions. Workflow phases should orchestrate these domains rather than own them.

### 3.3 Context construction is too coupled to workflow code

The current context path combines prompts, memory, project information, file listings, code intelligence, intent, conversation history, token estimation, compaction, and truncation inside workflow code.

This is not a reusable Context Engine. It is workflow-specific prompt assembly plus safety truncation.

### 3.4 File projections have become persistence boundaries

Markdown and JSON artifacts are valuable user-facing projections, but they should not be the authoritative runtime database.

The new runtime uses SQLite as the authoritative state store. Markdown/JSON files remain generated artifacts where they are useful to humans, Git, or compatibility workflows.

### 3.5 Execution, verification, recovery, and shipping are too entangled

An executor should execute. A verifier should verify. Recovery should recover. Shipping should ship.

These concerns must communicate through explicit state and evidence rather than calling each other through a giant engine object.

### 3.6 Security cannot depend primarily on command-string filtering

Shell command blocklists are useful defense-in-depth but are not a sufficient security boundary. Tool execution must be modeled as capabilities subject to policy, permission, resource limits, and sandboxing.

---

## 4. Target Architecture

```text
                         ┌─────────────────────┐
                         │       Ratatui       │
                         │        TUI          │
                         └──────────┬──────────┘
                                    │ events/views
                                    ▼
                         ┌─────────────────────┐
                         │ Application Control │
                         └──────────┬──────────┘
                                    │
                                    ▼
                         ┌─────────────────────┐
                         │       Runtime       │
                         │   orchestration     │
                         └──────┬──────┬───────┘
                                │      │
              ┌─────────────────┘      └─────────────────┐
              ▼                                          ▼
      ┌───────────────┐                          ┌────────────────┐
      │   Planning    │                          │   Execution    │
      │ research/plan │                          │ tasks/agents   │
      └───────┬───────┘                          └───────┬────────┘
              │                                          │
              └────────────────┬─────────────────────────┘
                               ▼
                     ┌─────────────────────┐
                     │   Context Engine    │
                     │ retrieval/ranking/  │
                     │ budget/compression  │
                     └──────────┬──────────┘
                                │
                                ▼
                     ┌─────────────────────┐
                     │     Model Client    │
                     │ providers + routing │
                     └──────────┬──────────┘
                                │
                                ▼
                     ┌─────────────────────┐
                     │    Tool Runtime     │
                     │ capabilities/policy │
                     │ permission/sandbox   │
                     └──────────┬──────────┘
                                │
                                ▼
                     ┌─────────────────────┐
                     │ Verification/Evidence│
                     └──────────┬──────────┘
                                │
                       ┌────────┴────────┐
                       ▼                 ▼
                 Recovery             Git/Ship
                       │                 │
                       └────────┬────────┘
                                ▼
                     ┌─────────────────────┐
                     │ Event + State Store │
                     │       SQLite        │
                     └─────────────────────┘
```

### Core rule

No UI package may become the owner of workflow state. No provider package may become the owner of task state. No tool may directly mutate unrelated runtime state. No Markdown file may be the only source of truth for recoverable runtime state.

---

## 5. Rust Workspace

The new implementation should be a Cargo workspace with independently testable crates/modules.

```text
m31a/
├── apps/
│   └── m31a/
├── crates/
│   ├── contracts/
│   ├── core/
│   ├── runtime/
│   ├── storage/
│   ├── model/
│   ├── providers/
│   ├── context/
│   ├── repository/
│   ├── planning/
│   ├── tasks/
│   ├── agents/
│   ├── tools/
│   ├── permissions/
│   ├── policies/
│   ├── sandbox/
│   ├── verification/
│   ├── recovery/
│   ├── git/
│   ├── memory/
│   ├── session/
│   ├── hooks/
│   ├── mcp/
│   ├── extensions/
│   ├── evaluation/
│   └── tui/
├── fixtures/
├── evals/
└── docs/
```

The exact crate count may change during implementation. The architectural boundaries must not.

---

## 6. Runtime Model

The top-level runtime identity is an `EngineeringRun`.

```text
EngineeringRun
├── Project
├── Intent
├── Plan
├── Tasks
├── Agents
├── ContextSnapshots
├── ToolCalls
├── FileChanges
├── VerificationRuns
├── RecoveryAttempts
├── PermissionRequests
├── Events
└── Evidence
```

A run is not equivalent to a workflow phase.

A task is not equivalent to an LLM call.

An agent is not equivalent to a task.

A context snapshot is not equivalent to conversation history.

An event is not equivalent to a log line.

These distinctions are architectural contracts.

---

## 7. State and Events

The runtime should use an explicit event model.

Examples:

```text
RunCreated
IntentClassified
PlanCreated
PlanRevised
TaskScheduled
TaskStarted
ToolRequested
PermissionRequested
ToolStarted
ToolCompleted
FileChanged
VerificationStarted
VerificationPassed
VerificationFailed
RecoveryStarted
RecoveryCompleted
AgentSpawned
AgentCompleted
CheckpointCreated
RunPaused
RunResumed
RunCancelled
RunCompleted
RunFailed
```

Events are durable facts. Logs are diagnostic output.

The runtime may maintain derived state for efficient queries, but important transitions must be represented durably enough to support recovery, inspection, and evaluation.

---

## 8. SQLite as Authoritative State

SQLite becomes the runtime source of truth for:

- runs and sessions;
- workflow state;
- plans and plan versions;
- tasks and dependencies;
- agents and attempts;
- tool calls;
- permissions;
- verification results;
- evidence;
- recovery attempts;
- context snapshots and metadata;
- model/usage accounting;
- durable runtime events.

Use transactions for state transitions that must be atomic.

WAL mode is appropriate for the local application because SQLite documents that WAL permits readers and writers to proceed concurrently, while still requiring attention to checkpointing and long-lived readers. The implementation must also use a SQLite version containing current WAL fixes rather than assuming the database engine is an invisible dependency. citeturn0search0turn0search6

The database is local and authoritative. It is not a distributed coordination system.

### File projections

The following may remain generated projections:

```text
.m31a/
├── m31a.db
├── plan.md
├── tasks.md
├── PROJECT.md
├── STATE.md
├── MEMORY.md
└── exports/
```

If a projection conflicts with SQLite state, SQLite wins and the projection is regenerated.

---

## 9. Context Engine

The Context Engine is a first-class subsystem.

```text
ContextRequest
    ↓
Source Discovery
    ↓
Retrieval
    ↓
Authority Resolution
    ↓
Freshness Evaluation
    ↓
Relevance Ranking
    ↓
Budget Allocation
    ↓
Compression
    ↓
Assembly
    ↓
ContextSnapshot
```

### Sources

Potential sources include:

- system instructions;
- user intent;
- project profile;
- repository structure;
- symbols and dependency graph;
- relevant files;
- task specification;
- plan and acceptance criteria;
- prior evidence;
- verification failures;
- memory;
- Git diff/history;
- tool outputs;
- previous agent findings.

### Important rule

Conversation history is only one context source.

The model should receive the smallest high-value context that allows a correct decision, not an ever-growing transcript.

### Context snapshots

Every important model interaction should be attributable to a `ContextSnapshot` containing at least:

- run ID;
- task/agent ID when applicable;
- source identifiers;
- source versions/hashes where available;
- ranking/relevance metadata;
- token budget;
- selected content hashes;
- compaction/compression metadata.

This enables reproducibility and evaluation.

---

## 10. Planning Architecture

Planning becomes a domain service rather than an `Engine` method family.

```text
Intent
  ↓
Assessment
  ↓
Research (when warranted)
  ↓
Plan Draft
  ↓
Plan Validation
  ↓
Coverage/Security/Gaps
  ↓
Plan Revision
  ↓
Task Graph
```

Existing Go behavior such as research, chunked planning, validation, plan checking, revision loops, coverage gates, and task merging should be treated as migration requirements rather than discarded functionality.

Planning must produce explicit outputs:

```text
Plan
TaskGraph
AcceptanceCriteria
RiskModel
VerificationStrategy
```

---

## 11. Execution Architecture

Execution is task-graph driven.

```text
TaskGraph
   ↓
Scheduler
   ↓
Runnable Tasks
   ↓
Agent Runtime
   ↓
ContextSnapshot
   ↓
Model
   ↓
Tool Runtime
   ↓
Changes + Evidence
```

Independent tasks may execute concurrently, but all concurrency must be bounded by explicit resource policies.

Cancellation must propagate through:

```text
Run
 → Agent
 → Model stream
 → Tool call
 → Child process
 → Network operation
 → Retry/backoff
```

No subsystem may create an untracked long-lived task.

---

## 12. Tool Runtime and Security

Tools are capabilities, not arbitrary functions.

A tool invocation passes through:

```text
ToolRequest
    ↓
Capability Check
    ↓
Policy Evaluation
    ↓
Permission Decision
    ↓
Resource Limits
    ↓
Sandbox / Process Isolation
    ↓
Execution
    ↓
ToolResult + Evidence
```

### Security principles

- deny by default for dangerous capabilities;
- classify risk explicitly;
- never treat string blocklists as the primary boundary;
- constrain working directory;
- constrain environment variables;
- constrain network access where possible;
- enforce CPU/time/memory/output limits;
- terminate child processes on cancellation;
- record permission decisions;
- record tool inputs/outputs with appropriate secret redaction;
- prevent tools from silently escalating authority.

The tool API should make capabilities explicit enough that security review can happen without reading every caller.

---

## 13. Verification and Evidence

Verification is a separate domain.

```text
Execution
   ↓
Changed State
   ↓
Verification Plan
   ↓
Checks
   ├── syntax/build
   ├── tests
   ├── lint/static analysis
   ├── targeted behavior
   └── runtime smoke checks
   ↓
Evidence
   ↓
Verification Result
```

A task is not successful because the model says it is successful.

A task is successful only when its acceptance criteria have sufficient evidence.

Evidence should be structured and persisted:

```text
Evidence
├── command
├── exit status
├── duration
├── stdout/stderr references
├── files examined
├── checks performed
└── timestamp
```

---

## 14. Recovery

Recovery becomes a dedicated subsystem.

```text
Failure
  ↓
Classify
  ↓
Determine Recoverability
  ↓
Capture Evidence
  ↓
Create Recovery Attempt
  ↓
Repair / Retry / Re-plan / Rollback
  ↓
Verify
  ↓
Continue or Escalate
```

Recovery must never blindly retry the same operation without changing the relevant state, context, strategy, or input.

Recovery attempts must have explicit limits and durable records.

---

## 15. Git and Workspace Safety

Git is a repository integration, not the workflow database.

The runtime must distinguish:

```text
Runtime State
Git State
Filesystem State
```

The system must be able to answer:

- what changed;
- which task caused it;
- which agent caused it;
- whether the change was verified;
- whether it was committed;
- which commit contains it;
- what recovery operation can revert it.

Subagents may use isolated worktrees where appropriate, but worktree lifecycle belongs to the agent/runtime layer rather than the TUI or task parser.

---

## 16. TUI Architecture

The new TUI uses Ratatui + Crossterm and remains a client of the runtime.

Ratatui deliberately leaves event handling architecture to the application, and its documentation describes centralized event loops as well as asynchronous/event-driven approaches. M31A should use an explicit event channel between runtime and TUI rather than coupling runtime ownership to the rendering loop. citeturn0search5turn0search10

Target shape:

```text
Runtime Event Bus
       │
       ▼
TUI Event Adapter
       │
       ▼
UI State / View Model
       │
       ▼
Ratatui Renderer
```

The TUI may request commands such as:

```text
PauseRun
ResumeRun
CancelRun
ApprovePermission
RejectPermission
RetryTask
SkipTask
OpenPlan
InspectEvidence
```

It must not mutate runtime state directly.

---

## 17. Provider Architecture

Providers implement a stable model-client contract.

The workflow runtime should not know provider-specific protocol details.

```text
ModelRequest
ModelResponse
StreamEvent
ToolCall
Usage
ModelCapabilities
```

Provider selection belongs to a routing policy that can consider:

- task type;
- context size;
- required capabilities;
- latency;
- cost;
- reliability;
- configured user policy.

Provider failures must be classified and handled by retry/fallback policy rather than ad-hoc workflow branches.

---

## 18. Concurrency Contract

Rust/Tokio does not eliminate concurrency bugs. It makes ownership explicit but still permits incorrect async design.

M31A must therefore define these rules:

1. Prefer ownership transfer over shared mutable state.
2. Prefer message passing over shared locks.
3. Every spawned task has an owner and cancellation path.
4. Every bounded resource has an explicit semaphore/queue policy.
5. Never hold a mutex across an `.await` unless the lifetime and contention are deliberately justified.
6. Runtime shutdown is hierarchical.
7. Child processes are registered with their owner and terminated during cancellation.
8. Backpressure is explicit; unbounded event channels are prohibited for high-volume streams.

---

## 19. Observability

Metrics must answer product questions, not merely infrastructure questions.

Track:

- run completion rate;
- task completion rate;
- verification pass rate;
- recovery success rate;
- retry rate;
- cancellation rate;
- tool failure rate;
- model latency;
- first-token latency;
- context size and compression ratio;
- token/cost usage;
- time spent per workflow stage;
- false-success rate;
- permission interruption rate.

Every important metric should be attributable to a run/task/agent where privacy and storage constraints permit.

---

## 20. Migration Strategy

Do not attempt a mechanical rewrite of every Go file.

### Stage 0 — Freeze architecture expansion

- No new major subsystems in `workflow.Engine`.
- Bug fixes are allowed when needed for safety or migration fixtures.
- New features must have an explicit migration justification.

### Stage 1 — Behavioral inventory

Extract contracts from the Go implementation:

- provider behavior;
- tool schemas;
- permission semantics;
- plan/task semantics;
- state transitions;
- Git behavior;
- recovery behavior;
- context requirements;
- TUI-visible events.

### Stage 2 — Golden fixtures

Create deterministic fixtures for:

- initialization;
- planning;
- execution;
- verification;
- failure/recovery;
- cancellation;
- permission flows;
- provider failures;
- context overflow;
- parallel task scheduling.

The new implementation must be judged against behavior, not line-by-line source similarity.

### Stage 3 — Rust foundation

Build:

```text
contracts
core
storage
runtime
model
providers
```

before migrating high-level workflow behavior.

### Stage 4 — Context Engine

Implement the new context model before migrating complex agent execution. Context is a dependency of planning, execution, recovery, and verification.

### Stage 5 — Planning + Task Graph

Port the proven planning semantics into independent Rust domains.

### Stage 6 — Tool Runtime + Security

Implement capabilities, permissions, policies, process control, and sandbox boundaries.

### Stage 7 — Execution + Verification + Recovery

Build the runtime around durable state and evidence.

### Stage 8 — TUI

Build Ratatui against the runtime event model.

### Stage 9 — Compatibility and projection

Generate familiar Markdown/JSON artifacts from SQLite state where compatibility is useful.

### Stage 10 — Go retirement

Remove Go components only after behavioral parity and production validation have been demonstrated.

---

## 21. What to Preserve From Go

Preserve semantics and proven algorithms where they are valuable:

- provider abstraction concepts;
- streaming behavior;
- tool schemas;
- permission categories;
- rate limiting concepts;
- task dependency scheduling;
- plan validation;
- plan checking/revision;
- repository intelligence requirements;
- Git semantics;
- self-healing strategy;
- decision logging semantics;
- metrics vocabulary.

Do **not** mechanically preserve:

- `workflow.Engine` ownership;
- the current shared mutable engine state;
- Bubble Tea's internal runtime architecture;
- file-based persistence as authoritative state;
- workflow-specific prompt assembly;
- engine-wide mutex topology;
- shell blocklists as the primary security boundary.

---

## 22. Explicit Non-Goals

The rewrite is not intended to:

- reproduce every current internal type;
- preserve every historical compatibility quirk;
- make the new code structurally similar to Go;
- add features simply because the old implementation contains them;
- treat line-count reduction as architectural success;
- optimize prematurely before runtime behavior is measurable.

The objective is a smaller number of stronger boundaries, not fewer files for their own sake.

---

## 23. Definition of Architectural Success

The reset is successful when all of the following are true:

- `workflow.Engine` is no longer the architectural center of M31A;
- runtime state has one authoritative persistence model;
- workflow phases are orchestration concepts rather than universal state;
- context construction is independently testable;
- tools cannot bypass capability/policy enforcement;
- execution, verification, recovery, and shipping have explicit ownership;
- cancellation is hierarchical and testable;
- every spawned async task has a lifecycle owner;
- important runtime transitions are durable and inspectable;
- TUI can be replaced without rewriting the runtime;
- providers can be replaced without rewriting workflow logic;
- planning can be evaluated without running the TUI;
- recovery can resume from durable state;
- behavioral fixtures demonstrate parity for required legacy behavior;
- new features can be added without enlarging a central engine object.

---

## 24. Architecture Decision Record

### Decision

Rebuild the M31A core around Rust/Tokio, explicit domain boundaries, SQLite-backed state, a first-class Context Engine, capability-based tools, evidence-driven verification, and a runtime-owned event model.

### Why

The current Go implementation contains substantial working behavior but has accumulated excessive central coupling around `workflow.Engine`. Continuing to extend that boundary creates increasing maintenance and concurrency cost.

### Consequences

Positive:

- explicit ownership;
- easier cancellation and recovery;
- clearer testing boundaries;
- durable runtime state;
- replaceable UI/provider layers;
- stronger security model;
- better long-term extensibility.

Negative:

- substantial rewrite cost;
- temporary feature parity burden;
- two implementations during migration;
- new Rust operational/tooling complexity;
- migration fixtures and evaluation infrastructure become mandatory.

### Rejected alternative

**Continue incrementally splitting the existing Go `Engine` without changing the state/persistence/context model.**

Reason for rejection: this reduces local complexity but preserves the central ownership model that caused the architectural problem.

---

## 25. Immediate Next Steps

1. Treat this document as the architecture authority for the rewrite.
2. Stop adding major capabilities to `workflow.Engine`.
3. Create the Rust workspace skeleton.
4. Define shared contracts and IDs.
5. Design the SQLite schema and event model.
6. Build the behavioral fixture suite from the current Go implementation.
7. Implement the Context Engine independently.
8. Implement planning and task-graph domains.
9. Implement capability/policy/tool execution.
10. Build execution, verification, recovery, and Git integration.
11. Build Ratatui as a runtime client.
12. Migrate features based on verified behavior rather than source structure.

---

## Appendix A — Current Repository Evidence

The current repository demonstrates real implementation of the major workflow areas. In particular:

- `workflow.Engine` currently aggregates provider, state-machine, dispatcher, token estimator, session manager, context registry, compactor, metrics, hooks, recovery, and pause/cancellation state.
- The state machine validates transitions and guards `Plan ↔ Discuss` cycles.
- Initialize performs project detection, optional deep analysis/preflight, Git setup, planning directory creation, project/state persistence, and checkpointing.
- Discuss performs streaming, question parsing, quality checking, retry, completeness handling, and context assembly.
- Plan performs research, chunked or standard generation, validation, plan checking, revision, coverage gates, persistence, and task synchronization.
- Execute schedules dependency groups, executes tools, performs self-healing/quality checks, invalidates repository intelligence after mutations, persists state, checkpoints, and synchronizes task projections.
- Streaming includes response-size limits, native tool-call accumulation, token calibration, retries, and context preflight.
- Context management includes compaction and progressive truncation.

These are migration inputs. They are not reasons to preserve the current engine boundary.

---

## Appendix B — External Design References

The target design deliberately aligns with established behavior of the technologies selected for the rewrite:

- SQLite transactions provide atomicity and durability suitable for authoritative local runtime state. citeturn0search1turn0search6
- SQLite WAL permits concurrent readers and writers but requires explicit awareness of checkpointing, long-lived readers, and current SQLite fixes. citeturn0search0
- Ratatui supports multiple event-handling architectures, including centralized and asynchronous event loops, which allows the UI to remain a client of an independent runtime. citeturn0search5turn0search10

The final implementation must still validate library versions, APIs, and platform behavior at implementation time.

---

## Final Principle

> **M31A should be a runtime with a UI, not a UI application with a giant engine behind it.**
>
> **The runtime owns truth. Context owns model input. Tools own capabilities. Verification owns evidence. Recovery owns failure. SQLite owns durable state. The TUI observes and commands the system; it does not become the system.**
