# M31A Architecture Reset

> **Status:** Authoritative architecture correction
> **Date:** 2026-08-23
> **Language:** Go
> **Scope:** Product architecture, runtime, workflow, context, planning, execution, verification, persistence, security, TUI, extensions, and migration strategy
>
> **Purpose:** Replace the previous architecture direction with a Go-native redesign. M31A is **not** being rewritten in Rust. The existing Go codebase remains the implementation base and behavioral reference.

---

## 1. Executive Decision

M31A will remain a **Go application**.

We will rework the architecture around the existing Go implementation rather than replacing the language or mechanically porting the current `workflow.Engine`.

The goal is not:

```text
Go → Rust
```

The goal is:

```text
Current Go architecture
        ↓
Explicit domain boundaries
        ↓
Independent runtime services
        ↓
Durable state + events
        ↓
First-class context system
        ↓
Reliable planning/execution/verification
        ↓
Autonomous engineering runtime
```

The current repository already contains substantial working behavior: workflow orchestration, model providers, streaming, tools, task scheduling, self-healing, Git integration, code intelligence, context management, session persistence, metrics, and a Bubble Tea TUI.

The architectural problem is therefore **coupling**, not lack of implementation.

We should preserve working behavior while changing ownership boundaries.

---

# 2. What We Are Building

M31A is an **autonomous terminal software engineer**.

Given a user request and a repository, M31A should be able to:

```text
Understand
   ↓
Inspect
   ↓
Research when necessary
   ↓
Discuss ambiguity when necessary
   ↓
Plan
   ↓
Decompose
   ↓
Execute
   ↓
Verify
   ↓
Recover when necessary
   ↓
Review
   ↓
Ship
```

The important distinction is that these are **capabilities of the runtime**, not seven giant states owned by one object.

M31A must be able to choose the appropriate workflow for the request.

A trivial request should not require the full planning pipeline.

A dangerous architectural migration should receive substantially more research, planning, verification, and human confirmation.

---

# 3. What We Learned From GSD Core

GSD Core is valuable primarily as a **workflow design reference**, not as an implementation template.

Its current command surface demonstrates a mature planning/execution lifecycle: project initialization, onboarding, discussion, research and planning, execution in waves, verification, progress/resume/pause, autonomous execution, configuration, and project management. citeturn0search0turn0search5

Its documented core loop is:

```text
Discuss
   ↓
Plan
   ↓
Execute
   ↓
Verify
   ↓
Ship
```

and the planning workflow explicitly performs research, decomposition, and plan verification before execution. citeturn0search5turn0search4

GSD's project tutorial also demonstrates the useful artifact lifecycle:

```text
RESEARCH.md
PLAN.md
SUMMARY.md
VERIFICATION.md
```

with independent plans executed in waves and verification performed against phase goals. citeturn0search2

M31A should adopt the **principles** behind this system while improving the runtime architecture around them.

We should not copy GSD's command files, agent prompts, or implementation structure line-for-line.

### Principles M31A adopts

1. Planning is a first-class operation.
2. Research happens before planning when uncertainty warrants it.
3. Plans must be validated before execution.
4. Independent work can execute concurrently.
5. Execution receives focused context rather than an unbounded transcript.
6. Verification is a separate step from implementation.
7. Work must be resumable.
8. Progress must be inspectable.
9. Autonomous mode should compose the same reliable primitives rather than bypass them.
10. Artifacts are useful evidence and handoffs, not merely logs.

---

# 4. What We Are Correcting in M31A

## 4.1 Do not keep growing `workflow.Engine`

The existing `workflow.Engine` has become the implicit owner of too many concerns.

The previous instinct was:

```text
engine.go too large
        ↓
split engine.go into more files
```

That is insufficient.

The new rule is:

```text
large object
    ↓
identify domain ownership
    ↓
extract services/components
    ↓
reduce shared mutable state
```

Splitting one object across ten files is not architectural decomposition.

---

## 4.2 Workflow phase is not application state

M31A can still expose:

```text
Initialize
Discuss
Plan
Execute
Verify
Runtime
Ship
```

as user-facing workflow stages.

But internal state must distinguish:

```text
RunState
TaskState
AgentState
PermissionState
VerificationState
RecoveryState
SessionState
RepositoryState
```

A task can be executing while another task is verifying.

A recovery attempt can occur after a verification failure without resetting the whole application state.

The architecture must represent that explicitly.

---

## 4.3 Context is not conversation history

The current context implementation has useful token estimation and compaction behavior, but the new design must treat context as a dedicated system.

The question is not:

> "How much conversation can fit?"

The question is:

> "What information is required for this decision?"

M31A should build task-specific context from repository facts, plan state, task requirements, evidence, memory, relevant files, Git state, tool output, and prior decisions.

---

## 4.4 Persistence must have a clear source of truth

Markdown and JSON remain valuable.

They are not forbidden.

But the runtime must have a clear authoritative representation of recoverable state.

For M31A, that means **SQLite-backed state in Go**.

Markdown artifacts become human-readable projections and planning artifacts.

---

## 4.5 Execution must not claim success

The model saying:

```text
"Done."
```

is not verification.

M31A must independently establish whether acceptance criteria were satisfied.

---

## 4.6 Security must be capability based

Command-string blocklists remain useful defense-in-depth.

They must not be the primary security boundary.

Tools need explicit capabilities, policy checks, permission decisions, resource limits, and cancellation semantics.

---

# 5. Target Go Architecture

```text
                         ┌─────────────────────┐
                         │     Bubble Tea      │
                         │        TUI          │
                         └──────────┬──────────┘
                                    │ commands/events
                                    ▼
                         ┌─────────────────────┐
                         │ Application Layer   │
                         │ CLI + orchestration │
                         └──────────┬──────────┘
                                    │
                                    ▼
                         ┌─────────────────────┐
                         │       Runtime       │
                         │ lifecycle + events  │
                         └──────┬──────┬───────┘
                                │      │
                ┌───────────────┘      └────────────────┐
                ▼                                        ▼
        ┌────────────────┐                      ┌────────────────┐
        │    Planning    │                      │   Execution    │
        │ research/plan  │                      │ tasks/agents   │
        └───────┬────────┘                      └───────┬────────┘
                │                                       │
                └────────────────┬──────────────────────┘
                                 ▼
                       ┌─────────────────────┐
                       │   Context Engine    │
                       │ retrieve/rank/build │
                       └──────────┬──────────┘
                                  │
                                  ▼
                       ┌─────────────────────┐
                       │    Model Gateway    │
                       │ providers + routing │
                       └──────────┬──────────┘
                                  │
                                  ▼
                       ┌─────────────────────┐
                       │    Tool Runtime     │
                       │ capability/policy   │
                       └──────────┬──────────┘
                                  │
                                  ▼
                       ┌─────────────────────┐
                       │ Verification Engine  │
                       │ checks + evidence    │
                       └──────────┬──────────┘
                                  │
                         ┌────────┴────────┐
                         ▼                 ▼
                    Recovery             Git
                         │                 │
                         └────────┬────────┘
                                  ▼
                       ┌─────────────────────┐
                       │ Persistence / Events│
                       │      SQLite         │
                       └─────────────────────┘
```

This architecture is deliberately implementable in Go without introducing unnecessary distributed-system machinery.

---

# 6. Recommended Go Package Structure

The exact package names can evolve, but ownership should look approximately like:

```text
cmd/
  m31a/

internal/
  app/
  runtime/
  workflow/
  planning/
  execution/
  tasks/
  agents/
  context/
  model/
  providers/
  tools/
  permissions/
  policy/
  verification/
  recovery/
  repository/
  git/
  storage/
  session/
  memory/
  events/
  metrics/
  extensions/
  config/
  tui/
```

The current `workflow` package should eventually become an orchestration layer rather than the home of every subsystem.

A package may depend on another package's public contracts, but it must not reach into unrelated internal mutable state.

---

# 7. Runtime

The Runtime is the coordinator.

It should own:

- application lifecycle;
- run lifecycle;
- cancellation roots;
- event publication;
- service wiring;
- workflow selection;
- top-level recovery;
- graceful shutdown.

It should **not** own every domain's state.

Conceptually:

```go
type Runtime struct {
    store        *storage.Store
    events       *events.Bus
    planner      *planning.Service
    executor     *execution.Service
    context      *context.Engine
    models       *model.Gateway
    tools        *tools.Runtime
    verifier     *verification.Service
    recovery     *recovery.Service
    repository   *repository.Service
    git          *git.Service
    sessions     *session.Service
}
```

The actual implementation should use interfaces where they provide real testing or substitution value. Do not create interfaces mechanically for every struct.

---

# 8. EngineeringRun

The top-level runtime concept is an `EngineeringRun`.

```text
EngineeringRun
├── Project
├── Intent
├── Workflow
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

Important distinctions:

```text
Run            ≠ Phase
Task           ≠ Model Call
Agent          ≠ Task
Context        ≠ Conversation
Event          ≠ Log Line
Evidence       ≠ Model Claim
Checkpoint     ≠ Git Commit
```

These distinctions should appear in the types and storage schema.

---

# 9. Event Model

Use explicit runtime events.

Examples:

```text
RunCreated
IntentClassified
RepositoryInspected
ResearchStarted
ResearchCompleted
DiscussionStarted
DecisionRecorded
PlanCreated
PlanValidated
PlanRevised
TaskScheduled
TaskStarted
TaskCompleted
AgentSpawned
AgentCompleted
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
CheckpointCreated
RunPaused
RunResumed
RunCancelled
RunCompleted
RunFailed
```

Events are durable facts about the runtime.

Logs remain diagnostic output.

A log line may disappear without invalidating state.

A state transition that must survive restart should be represented in durable state/event records.

---

# 10. SQLite State Model

Use SQLite as the authoritative local runtime database.

Likely tables include:

```text
projects
runs
run_events
sessions
plans
plan_versions
tasks
task_dependencies
agents
agent_attempts
tool_calls
permission_requests
verification_runs
evidence
recovery_attempts
context_snapshots
model_calls
usage_records
checkpoints
file_changes
workspaces
```

SQLite should be accessed through a dedicated storage package.

No workflow package should construct SQL directly.

Transactions should cover state changes that must be atomic.

Use WAL mode and explicit connection/pooling rules appropriate for a local concurrent application. SQLite's WAL model supports concurrent readers with a writer, but long-lived readers and checkpoint behavior still need to be managed intentionally.

### Authoritative rule

```text
SQLite
   ↓
authoritative runtime state

Markdown / JSON
   ↓
projection / human artifact / compatibility artifact
```

If generated `STATE.md` disagrees with SQLite, regenerate it.

---

# 11. Planning System

Planning should be a domain service.

```text
User Intent
    ↓
Intent Assessment
    ↓
Repository Understanding
    ↓
Research (when warranted)
    ↓
Discussion (when ambiguity exists)
    ↓
Plan Draft
    ↓
Plan Checker
    ↓
Coverage / Risk / Security Review
    ↓
Plan Revision
    ↓
Task Graph
```

## Planning outputs

```text
PROJECT.md
REQUIREMENTS.md
ROADMAP.md
STATE.md
RESEARCH.md
CONTEXT.md
PLAN.md
TASKS.md
```

Not every request requires every artifact.

The runtime should create the minimum artifact set required for reliable work.

---

# 12. Example: What `/planning` Does in M31A

M31A should expose a human-friendly planning command such as:

```text
/planning
```

or a CLI equivalent.

The command is an **entry point into the Planning Service**, not a magic prompt.

Suppose the user says:

```text
/planning Add OAuth2 login with Google and GitHub.
Users should be able to sign in, link accounts, and revoke sessions.
```

The runtime should do approximately:

```text
1. Parse intent
2. Inspect repository
3. Determine project architecture
4. Identify authentication-related code
5. Identify database/storage layer
6. Identify existing session model
7. Determine test/build commands
8. Determine security-sensitive areas
9. Decide whether external research is needed
10. Gather research if required
11. Produce implementation requirements
12. Decompose into phases/tasks
13. Validate dependencies
14. Validate acceptance criteria
15. Validate security coverage
16. Persist plan
17. Present plan to user
```

Example generated artifacts:

```text
.m31a/
├── m31a.db
├── PROJECT.md
├── REQUIREMENTS.md
├── ROADMAP.md
├── STATE.md
└── phases/
    └── 01-authentication/
        ├── CONTEXT.md
        ├── RESEARCH.md
        ├── PLAN.md
        ├── TASKS.md
        └── ACCEPTANCE.md
```

Example plan:

```text
Phase 1 — Authentication Foundation

Task 1
Create OAuth provider abstraction.
Depends on: none
Verify: unit tests + interface compilation

Task 2
Implement Google OAuth provider.
Depends on: Task 1
Verify: provider contract tests

Task 3
Implement GitHub OAuth provider.
Depends on: Task 1
Verify: provider contract tests

Task 4
Implement account linking.
Depends on: Task 1
Verify: integration tests

Task 5
Implement session revocation.
Depends on: Task 4
Verify: security/integration tests

Task 6
Add end-to-end authentication tests.
Depends on: Tasks 2–5
Verify: complete test suite
```

The planner should **not** modify production source code.

It creates an executable plan.

---

# 13. Research

Research is an input to planning, not a mandatory ritual.

Use research when:

- the technology is unfamiliar;
- external APIs/specifications matter;
- security implications require authoritative information;
- the repository contains uncertainty that local inspection cannot resolve;
- a design decision has significant consequences.

Skip or reduce research when:

- the task is obvious;
- the repository already contains authoritative information;
- the change is a small local modification.

Research should produce durable findings rather than merely adding text to the model context.

```text
Research Finding
├── source
├── claim
├── relevance
├── confidence
├── timestamp
└── applicability
```

---

# 14. Discussion

Discussion exists to resolve uncertainty that planning cannot safely infer.

For example:

```text
User:
"Add notifications."

M31A:
"Which channels should be supported?
1. Email
2. In-app
3. Web push
4. All three"
```

After the user answers:

```text
DecisionRecorded
```

The answer becomes part of the durable project/run context.

Discussion should not become an endless conversational loop.

The runtime should stop asking when the information required for a safe plan is sufficient.

---

# 15. Context Engine

The Context Engine is one of the most important architectural changes.

```text
ContextRequest
      ↓
Source Discovery
      ↓
Retrieval
      ↓
Authority Resolution
      ↓
Freshness Check
      ↓
Relevance Ranking
      ↓
Token Budgeting
      ↓
Compression
      ↓
Assembly
      ↓
ContextSnapshot
```

Possible sources:

```text
System Rules
User Intent
Project Profile
Requirements
Plan
Task
Repository Structure
Symbols
Dependency Graph
Relevant Files
Git Diff
Previous Decisions
Memory
Tool Results
Verification Failures
Prior Evidence
Research
```

The Context Engine should answer:

```text
What does this agent need to know?
Why does it need to know it?
Where did the information come from?
How authoritative is it?
How fresh is it?
How much context budget should it consume?
```

### Context snapshot

Each important model call should be attributable to:

```text
ContextSnapshot
├── run_id
├── task_id
├── agent_id
├── sources
├── source versions/hashes
├── selected content
├── token budget
├── compression metadata
└── context hash
```

This makes debugging and evaluation possible.

---

# 16. Execution

Execution consumes a validated task graph.

```text
Task Graph
    ↓
Dependency Scheduler
    ↓
Runnable Tasks
    ↓
Agent Runtime
    ↓
Task Context
    ↓
Model
    ↓
Tool Runtime
    ↓
Filesystem/Git
    ↓
Evidence
```

Independent tasks can run in parallel.

Concurrency must be explicitly bounded.

Each task should have:

```text
Task
├── ID
├── description
├── dependencies
├── acceptance criteria
├── context requirements
├── allowed capabilities
├── timeout
├── retry policy
├── verification strategy
└── state
```

---

# 17. Agent Runtime

An agent is an execution actor with a bounded responsibility.

```text
Agent
├── identity
├── task
├── context
├── model
├── capabilities
├── cancellation
├── attempt
└── evidence
```

An agent should not have unrestricted access to the global Runtime object.

Give it explicit dependencies:

```go
type AgentDeps struct {
    Context  *context.Engine
    Model    model.Client
    Tools    tools.Executor
    Store    storage.TaskStore
    Events   events.Publisher
}
```

This prevents hidden coupling and makes agent behavior testable.

---

# 18. Tool Runtime

Tools are capabilities.

```text
ToolRequest
    ↓
Capability Check
    ↓
Policy Check
    ↓
Permission
    ↓
Resource Limits
    ↓
Execution
    ↓
Evidence
```

Examples:

```text
filesystem.read
filesystem.write
process.execute
process.spawn
network.request
git.read
git.write
browser.open
mcp.invoke
```

Risk should be explicit.

```text
SAFE
LOW
MEDIUM
HIGH
CRITICAL
```

The permission system decides whether the capability can execute under the current policy.

---

# 19. Cancellation and Concurrency

This is a major area of the Go rework.

Every long-running operation must accept a `context.Context`.

```go
func (s *Service) Execute(ctx context.Context, req Request) error
```

No subsystem should create an untracked goroutine.

Every goroutine must have a clear owner and termination condition.

Use:

- `errgroup` for structured concurrent work;
- context cancellation for lifecycle propagation;
- bounded worker pools where appropriate;
- channels for event/data flow, not as a substitute for state ownership;
- mutexes only around clearly owned mutable state;
- documented lock ordering where multiple locks are unavoidable.

Avoid:

```text
shared global state
unbounded goroutine spawning
anonymous background goroutines
channel ownership ambiguity
mutex chains
sleep-based synchronization
```

### Cancellation contract

Cancelling a run must propagate to:

```text
Run
 ↓
Agent
 ↓
Model stream
 ↓
Tool execution
 ↓
Child process
 ↓
Retries
 ↓
Subagents
```

Then persist a recoverable terminal/intermediate state.

---

# 20. Verification

Verification is independent from execution.

```text
Task Completed
      ↓
Verification Strategy
      ↓
Build / Test / Static Analysis / Targeted Checks
      ↓
Evidence
      ↓
Verification Result
```

A verification result should contain:

```text
VerificationResult
├── status
├── checks
├── evidence IDs
├── failures
├── timestamp
└── verifier version
```

Verification should be capable of discovering that an apparently successful implementation is wrong.

---

# 21. Recovery

Recovery should classify failures rather than blindly retry.

```text
Failure
  ↓
Classify
  ├── transient
  ├── environment
  ├── tool
  ├── model
  ├── implementation
  ├── plan
  └── permission
  ↓
Choose strategy
  ├── retry
  ├── repair
  ├── re-contextualize
  ├── re-plan
  ├── rollback
  └── ask user
```

Every recovery attempt must be bounded.

A repeated identical failure should trigger strategy escalation rather than infinite retry.

---

# 22. Git and Checkpoints

Git is a source-control integration.

It is not the runtime database.

A checkpoint is a recovery boundary.

A commit is a Git history object.

They may coincide, but they are not conceptually identical.

M31A should be able to answer:

```text
Which task changed this file?
Which agent made the change?
Which run produced it?
Was it verified?
Which checkpoint contains it?
Which Git commit contains it?
Can it be reverted safely?
```

---

# 23. TUI

Keep Bubble Tea for the Go implementation unless there is a demonstrated reason to replace it.

The TUI must become a **client of application state**, not the owner of runtime state.

Target model:

```text
Runtime Event Bus
       ↓
Application/UI Adapter
       ↓
Bubble Tea Model
       ↓
View
```

User input should produce commands sent to the application layer.

The UI must not directly mutate planner, executor, provider, or storage internals.

The TUI should expose:

```text
Current Run
Current Phase
Current Task
Task Graph
Active Agents
Tool Calls
Permission Requests
Verification
Recovery
Context Usage
Model Usage
Git Changes
Errors
```

---

# 24. Example: Complete M31A Project Lifecycle

Consider a new repository:

```text
acme-api/
```

The user starts:

```text
m31a
```

and asks:

```text
Build a Go REST API for a SaaS billing system.
Users need organizations, plans, subscriptions, invoices,
and Stripe webhook handling.
Use PostgreSQL and expose OpenAPI documentation.
```

## Step 1 — Initialize

M31A inspects:

```text
Go version
module
existing source
existing tests
Git state
configuration
project structure
```

It creates runtime state and project artifacts.

```text
.m31a/
├── m31a.db
├── PROJECT.md
├── REQUIREMENTS.md
├── ROADMAP.md
└── STATE.md
```

## Step 2 — Understand

Repository analysis discovers:

```text
cmd/api
internal/http
internal/db
internal/auth
```

Suppose the repository is nearly empty.

M31A records that fact rather than inventing architecture from assumptions.

## Step 3 — Discuss

M31A asks only decisions that materially affect implementation:

```text
Which Stripe integration model?
1. Checkout + Customer Portal
2. Direct subscription API
3. Both
```

User selects `3`.

The decision is persisted.

## Step 4 — Research

M31A researches current Stripe webhook requirements and relevant Go/PostgreSQL implementation concerns.

Research findings are stored with sources and applicability.

## Step 5 — Plan

M31A creates:

```text
Phase 1 — Service foundation
Phase 2 — Database/domain model
Phase 3 — Authentication/organizations
Phase 4 — Billing/subscriptions
Phase 5 — Stripe webhooks
Phase 6 — API/OpenAPI
Phase 7 — Integration/verification
```

Each phase contains tasks with dependencies and acceptance criteria.

## Step 6 — Execute

Independent tasks run concurrently where safe.

For example:

```text
Wave 1
├── project structure
├── configuration
└── database foundation

Wave 2
├── organization model
├── billing model
└── API scaffolding

Wave 3
├── subscription service
├── Stripe integration
└── OpenAPI generation
```

Each agent receives only the context required for its task.

## Step 7 — Verify

M31A runs:

```text
go test ./...
go vet ./...
OpenAPI generation check
migration checks
integration tests
Stripe webhook signature tests
```

The result becomes evidence.

## Step 8 — Recover

Suppose webhook integration tests fail because the generated event parser expects an outdated field.

M31A does not blindly retry.

It classifies the failure, updates context, inspects the relevant API contract, repairs the implementation, and reruns verification.

## Step 9 — Ship

After acceptance criteria pass:

```text
Git diff review
verification summary
commit
final project state
```

M31A reports:

```text
7 phases
42 tasks
39 completed automatically
3 required recovery attempts
0 unresolved verification failures
```

The complete run remains recoverable from SQLite.

---

# 25. Command Architecture

Commands should be thin entry points into services.

Example command surface:

```text
m31a
m31a init
m31a planning
m31a discuss
m31a plan
m31a execute
m31a verify
m31a status
m31a resume
m31a pause
m31a recover
m31a ship
m31a inspect
m31a context
m31a doctor
```

Slash commands may exist inside the TUI:

```text
/planning
/execute
/verify
/status
/resume
/pause
```

But command spelling must not determine domain architecture.

For example:

```text
/planning
      ↓
planning.Service
```

not:

```text
/planning
      ↓
TUI handler
      ↓
Engine mutation
      ↓
random helper functions
```

---

# 26. Autonomous Mode

Autonomous execution should compose normal services.

```text
AutonomousRun
    ↓
Determine next action
    ↓
Discuss if required
    ↓
Plan if required
    ↓
Execute
    ↓
Verify
    ↓
Recover if required
    ↓
Advance
```

Autonomous mode must not bypass verification, permission, cancellation, or persistence rules.

This mirrors the useful GSD concept of chaining workflow operations while preserving each operation's safety gates. GSD's current `/gsd-autonomous` and `/gsd-progress --next --auto` are examples of this composition model. citeturn0search0

---

# 27. Context and Artifact Lifecycle

M31A should distinguish four kinds of information:

### 27.1 Authoritative state

Stored in SQLite.

### 27.2 Planning artifacts

Human-readable artifacts such as:

```text
PROJECT.md
REQUIREMENTS.md
ROADMAP.md
CONTEXT.md
RESEARCH.md
PLAN.md
TASKS.md
STATE.md
```

### 27.3 Runtime evidence

Structured verification/tool/recovery records.

### 27.4 Diagnostic logs

Operational information useful for debugging but not required to reconstruct the run.

Do not mix these categories.

---

# 28. Model Gateway

Providers should be hidden behind a stable model contract.

```go
type Client interface {
    Complete(ctx context.Context, req Request) (Response, error)
    Stream(ctx context.Context, req Request) (Stream, error)
}
```

Provider-specific features should be represented explicitly rather than leaking provider types into workflow packages.

The gateway owns:

- provider selection;
- model selection;
- fallback;
- retry classification;
- usage accounting;
- streaming normalization;
- capability discovery.

It does not own task state.

---

# 29. Configuration

Configuration should be resolved once at the application boundary.

Suggested precedence:

```text
built-in defaults
    ↓
user configuration
    ↓
workspace configuration
    ↓
project configuration
    ↓
run configuration
    ↓
explicit CLI flags
```

Resolved configuration should be immutable for a run unless a feature explicitly supports live changes.

This prevents configuration reads from becoming hidden dependencies throughout the codebase.

---

# 30. Memory

Memory should be separated into:

```text
Project Memory
Run Memory
Task Memory
Agent Findings
Decision Records
Research Findings
```

Memory is not an unbounded transcript.

Every memory item should have provenance and applicability.

A stale memory item should not automatically outrank current repository evidence.

---

# 31. Repository Intelligence

The existing code-intelligence work should be preserved and moved behind a dedicated repository service.

The repository service can provide:

```text
ProjectProfile
FileTree
Symbols
References
DependencyGraph
Tests
BuildSystem
LanguageProfile
GitStatus
RelevantFiles
```

The Context Engine consumes these facts.

Planning and execution should not each independently rediscover repository structure.

---

# 32. Extension Architecture

Extensions should be introduced only after core ownership is stable.

Potential extension points:

```text
Provider
Tool
Repository Analyzer
Verifier
Workflow Strategy
Hook
MCP Integration
Notification
```

An extension must interact through explicit contracts.

Do not expose internal mutable structs as plugin APIs.

The current roadmap already identifies providers, tools, workflows, integrations, and hooks as extension areas. fileciteturn3file0

---

# 33. Testing Strategy

The goal is behavioral confidence, not line coverage.

## Unit tests

Use for:

- parsers;
- state transitions;
- dependency resolution;
- context ranking;
- token budgeting;
- policy evaluation;
- plan validation;
- retry classification.

## Integration tests

Use for:

- SQLite persistence;
- provider adapters;
- tool execution;
- Git operations;
- repository analysis;
- planner/executor integration.

## End-to-end tests

Test complete workflows:

```text
request
→ planning
→ execution
→ verification
→ persistence
```

## Failure tests

Explicitly test:

- cancellation;
- process death;
- provider failure;
- malformed tool call;
- tool timeout;
- permission denial;
- database interruption;
- verification failure;
- repeated recovery failure;
- concurrent tasks;
- resumed sessions.

---

# 34. Observability

Every run should expose:

```text
run duration
task duration
model latency
first-token latency
tool latency
verification duration
retry count
recovery count
context size
model tokens
estimated cost
parallelism
cancellation reason
completion status
```

Metrics should answer engineering questions rather than merely produce dashboards.

---

# 35. Migration Strategy From Current Go Code

This is an **in-place architectural rework**, not a greenfield language rewrite.

## Stage 1 — Freeze architectural expansion

Do not add major subsystems to the current giant engine boundary.

Bug fixes are allowed.

Critical reliability/security fixes take priority.

## Stage 2 — Define contracts

Create stable contracts for:

```text
ModelClient
ToolExecutor
ContextProvider
Planner
Executor
Verifier
RecoveryService
RepositoryService
Storage
EventPublisher
```

## Stage 3 — Introduce SQLite

Move authoritative session/run/task state into the storage layer.

Keep existing Markdown/JSON files as projections during migration.

## Stage 4 — Extract Context Engine

Move context construction, retrieval, ranking, budgeting, and compaction out of workflow code.

## Stage 5 — Extract Planning Service

Move research, planning, validation, and task graph construction out of `Engine`.

## Stage 6 — Extract Execution Service

Move scheduling, agent execution, tool loops, and task state out of `Engine`.

## Stage 7 — Extract Verification and Recovery

Make them independent services with explicit contracts.

## Stage 8 — Rework TUI boundary

Bubble Tea becomes a consumer of runtime events and a producer of application commands.

## Stage 9 — Delete obsolete engine state

Only after all callers migrate should old shared state and compatibility helpers be removed.

---

# 36. What We Keep From the Existing Go Project

Preserve the proven behavior and ideas from the current implementation:

```text
provider abstraction
streaming
native tool calls
permission model
task dependencies
bounded parallel execution
plan validation
research
self-healing concepts
Git integration
code intelligence
context compaction concepts
metrics
decision logging
session/resume concepts
Bubble Tea UI
```

These are valuable implementation knowledge.

---

# 37. What We Do Not Preserve

Do not preserve these as architectural constraints merely because they already exist:

```text
Engine as universal owner
workflow package owning unrelated state
TUI owning runtime state
workflow code constructing all context
file artifacts acting as hidden database
unbounded shared mutable state
implicit goroutine ownership
provider-specific types leaking everywhere
shell blocklists as primary security
execution deciding its own verification result
blind retry loops
```

Compatibility is not a reason to retain bad ownership.

---

# 38. Non-Goals

This architecture does **not** require:

- rewriting M31A in Rust;
- microservices;
- remote/distributed orchestration;
- a cloud control plane;
- Kubernetes;
- a custom database server;
- replacing Bubble Tea without evidence;
- rewriting every existing subsystem immediately;
- copying GSD Core's implementation.

M31A remains a local-first Go application.

---

# 39. Architectural Rules

These rules are mandatory for new code.

### Rule 1
No new feature should add another responsibility to `workflow.Engine` unless that responsibility is genuinely workflow orchestration.

### Rule 2
Every long-running operation accepts `context.Context`.

### Rule 3
Every goroutine has an owner and termination path.

### Rule 4
SQLite is authoritative for recoverable runtime state.

### Rule 5
Markdown/JSON are artifacts or projections unless explicitly designated otherwise.

### Rule 6
The model cannot declare implementation success without independent verification.

### Rule 7
Tools execute capabilities under explicit policy.

### Rule 8
The Context Engine owns model-context assembly.

### Rule 9
The TUI never becomes the source of truth for runtime state.

### Rule 10
Autonomous mode uses the same planning, execution, verification, permission, cancellation, and recovery primitives as interactive mode.

### Rule 11
Prefer composition over giant interfaces and giant state objects.

### Rule 12
Do not introduce an abstraction unless it solves an actual ownership, substitution, testing, or lifecycle problem.

---

# 40. Definition of Architectural Success

M31A is considered architecturally reworked when:

- `workflow.Engine` is an orchestration boundary rather than a universal state container;
- runtime state can be reconstructed from SQLite;
- a run can be paused and resumed reliably;
- cancellation propagates through every child operation;
- planning is independently testable;
- execution is independently testable;
- verification is independent from model claims;
- recovery is independently testable;
- context construction is independently testable;
- tools are governed by explicit capabilities and policies;
- TUI and runtime can evolve independently;
- provider implementations can change without rewriting workflow code;
- concurrent task execution has explicit ownership and limits;
- end-to-end workflows have deterministic state transitions;
- important runtime decisions are observable and auditable.

---

# 41. Final Architectural Position

M31A does not need a new language.

It needs a new **ownership model**.

Go is capable of implementing the architecture we want. The failure mode to avoid is not Go itself; it is allowing one mutable workflow object to become the application's operating system.

The new M31A should therefore be:

```text
Go
│
├── Runtime
├── Planning
├── Execution
├── Context
├── Model Gateway
├── Tool Runtime
├── Verification
├── Recovery
├── Repository Intelligence
├── Git
├── SQLite Storage
├── Events
└── Bubble Tea UI
```

with clear contracts between them.

The central design principle is:

> **M31A is a Go-native autonomous engineering runtime composed of explicit services, durable state, focused context, verifiable execution, and recoverable workflows.**

That is the architecture we should implement from this point forward.
