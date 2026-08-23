# M31A — Complete Project Context

> Canonical engineering context for the M31A project.
>
> **Project:** M31A
>
> **Module:** `gitlab.com/eshanized/M31A`
>
> **Primary language:** Go
>
> **Primary execution surface:** Terminal CLI + interactive TUI
>
> **Primary development runtime:** OpenCode
>
> **v1 model:** `nvidia/nemotron-3-ultra-550b-a55b`
>
> **v1 model API provider:** NVIDIA Build / NVIDIA API Catalog
>
> **NVIDIA API base URL:** `https://integrate.api.nvidia.com/v1`
>
> **Primary API operation:** `POST /v1/chat/completions`
>
> **Project status:** greenfield, architecture-first
>
> **Document status:** canonical; implementation agents must treat this document as the top-level project contract unless a newer authoritative project decision explicitly supersedes a section.

---

## 0. Purpose of this document

This document is the source of truth for building M31A. It defines the product vision, engineering philosophy, architecture, domain model, runtime model, safety model, state model, agent model, tooling model, code-intelligence model, verification model, terminal UX, Git behavior, repository structure, development workflow, implementation constraints, failure modes, and v1 acceptance criteria.

M31A is **not** intended to be an OpenCode clone, a GSD prompt pack, a generic LLM wrapper, or a terminal chatbot with a shell command.

M31A is intended to become:

> **A persistent software-engineering agent runtime that turns natural-language intent into verified, resumable engineering work.**

The user should be able to express engineering intent in natural language and have M31A understand the repository, reason about impact, formulate an executable plan, perform work safely, verify the result, recover from failures, and preserve durable engineering knowledge.

The product is terminal-first. The terminal is not a fallback UI. It is the primary operating environment.

---

# 1. Product definition

## 1.1 What M31A is

M31A is an agentic coding environment that lives in a developer's terminal.

It must understand and operate across:

- source code
- project structure
- symbols
- dependencies
- tests
- build systems
- configuration
- documentation
- Git history
- branches
- worktrees
- repository conventions
- project requirements
- architecture decisions
- previous M31A runs
- verification evidence

M31A should support natural-language workflows such as:

```text
implement organization-level RBAC
explain the authentication flow
why does this abstraction exist?
what breaks if I change this interface?
find the regression introduced after commit X
review my current branch
prepare a release
split these changes into logical commits
make this test pass
investigate why this service is slow
add OAuth refresh-token rotation
```

The agent should translate these requests into explicit engineering work rather than treating every request as an unconstrained chat turn.

---

## 1.2 What M31A is not

M31A must not become:

- a web-first AI product
- a VS Code extension with a terminal attached
- a giant monolithic `Agent` class
- an LLM loop with unrestricted shell access
- a prompt-driven workflow whose state exists only inside context windows
- an automatic code generator that declares success without evidence
- an opaque database-only project memory system
- a collection of copied GSD workflows
- a direct OpenCode fork with a renamed UI

---

# 2. Competitive and architectural inspiration

M31A is informed by two important projects:

1. OpenCode: `https://github.com/anomalyco/opencode`
2. GSD Core: `https://github.com/open-gsd/gsd-core`

The goal is to learn from their architectural principles and failure modes, then implement M31A's own architecture.

## 2.1 OpenCode lessons

OpenCode has evolved into a serious agent runtime rather than a simple terminal assistant. Its current repository separates concerns into areas including core runtime, CLI, client, LLM handling, plugins, containers, code-mode, desktop/application surfaces, and supporting infrastructure.

Its current session architecture explicitly models durable session history, provider turns, context epochs, system-context sources, context snapshots, prompt promotion, compaction, tool output handling, and safe provider-turn boundaries.

Its tool layer has a registry/materialization architecture where tools are registered, permission-filtered, exposed as tool definitions, executed against a session/agent identity, bounded for output size, and persisted when needed.

M31A should learn from these principles:

- durable sessions
- provider abstraction
- typed tool registry
- permission-aware tool materialization
- bounded tool outputs
- explicit execution state
- context lifecycle management
- context compaction
- event streams
- client/runtime separation
- extensibility through plugins and MCP

M31A must not copy OpenCode's internal package layout or implementation merely for familiarity.

## 2.2 GSD lessons

GSD Core is fundamentally different. It is a meta-prompting and software-development workflow framework sitting between users and coding agents. It explicitly uses context engineering, fresh-context specialized agents, spec-driven development, persistent project state, planning artifacts, verification artifacts, and resumable workflows.

GSD's `.planning/` directory is particularly important conceptually. It stores project identity, requirements, roadmap, state, decisions, research, phase context, executable plans, execution summaries, verification, UAT state, and resume information.

GSD's core workflow is approximately:

```text
specification
    ↓
discussion / clarification
    ↓
research
    ↓
planning
    ↓
plan verification
    ↓
execution
    ↓
verification
    ↓
UAT / shipment
```

M31A should internalize the principles, but implement them as native runtime capabilities rather than Markdown prompt commands glued onto another agent runtime.

## 2.3 M31A's position

The intended positioning is:

| Product | Primary strength |
|---|---|
| OpenCode | Agent runtime |
| Claude Code | Coding agent + terminal UX |
| Codex CLI | Coding agent |
| Cline | IDE-centered agent |
| GSD | Engineering methodology |
| **M31A** | **Engineering operating system for autonomous coding** |

The differentiator is not raw tool count. The differentiator is the ability to take an engineering objective, build an explicit representation of the work, execute it under policy, verify it with evidence, and preserve the result.

---

# 3. Core product thesis

The basic agent architecture must not be:

```text
prompt → model → tools → response
```

Instead it must be:

```text
intent
  ↓
intent classification
  ↓
engineering state
  ↓
repository understanding
  ↓
impact analysis
  ↓
research
  ↓
plan
  ↓
execution graph
  ↓
tool actions
  ↓
verification
  ↓
durable result
```

The LLM is one reasoning component inside M31A.

The LLM must never be the only source of truth about project state.

---

# 4. Design principles

These are non-negotiable.

## 4.1 State survives context loss

A model context window is disposable.

Project state is durable.

Terminal restart, model replacement, network failure, process crash, context compaction, and interrupted execution must not destroy engineering state.

## 4.2 Planning before mutation for non-trivial work

M31A should analyze and plan before making broad changes.

Small, clearly safe edits can use a lightweight path.

High-impact work must use the full workflow:

```text
analyze → impact → plan → execute → verify
```

## 4.3 Verification is evidence, not confidence

The agent saying “done” is not verification.

A task is complete when acceptance criteria have corresponding evidence.

## 4.4 Least privilege by capability

Permissions should be based on capabilities, resource scope, risk, and action type rather than simple “allow shell / deny shell” toggles.

## 4.5 Fresh context for specialized reasoning

A planner, implementer, verifier, reviewer, and researcher should not automatically share one giant conversational context.

Specialized agents should receive focused context assembled from durable project state.

## 4.6 Thin orchestration

Orchestrators coordinate state, agents, dependencies, gates, and results.

They do not contain giant blocks of implementation-specific prompt logic.

## 4.7 Human control over irreversible actions

Reversible low-risk actions may be automated.

One-way-door decisions require a checkpoint unless an explicit policy overrides the checkpoint.

Examples of one-way or high-risk decisions:

- destructive database migrations
- force push
- deleting production infrastructure
- changing public APIs in compatibility-sensitive projects
- deleting large amounts of source code
- removing authentication controls
- changing security boundaries
- modifying secrets handling

## 4.8 Deterministic project state

Given the same durable state and event history, M31A must be able to reconstruct its current run state.

## 4.9 Human-readable artifacts

Important engineering knowledge must remain inspectable.

Machine state may live in SQLite/event storage, but significant project artifacts should remain human-readable Markdown/JSON inside `.m31a/`.

## 4.10 Provider-agnostic core

v1 uses NVIDIA Build and one NVIDIA model, but the runtime must be designed around a provider-neutral LLM interface.

The rest of M31A must never contain NVIDIA-specific business logic.

---

# 5. Language and platform decisions

## 5.1 Primary language: Go

M31A is a Go-native system.

Module:

```text
gitlab.com/eshanized/M31A
```

Use the currently supported stable Go toolchain available when implementation starts. Go 1.25 is a supported current baseline for this project, but implementation must use the exact toolchain pinned by the repository's `go.mod` and toolchain metadata rather than assuming whatever happens to be installed on the developer machine.

The Go core is chosen because M31A is simultaneously:

- a terminal application
- a process supervisor
- a filesystem engine
- a Git/worktree manager
- a concurrent agent runtime
- a network client
- a persistent state engine
- a plugin/MCP host
- a long-running execution service
- a cross-platform binary

Go provides an excellent complexity/performance/portability balance for this workload.

## 5.2 Rust boundary

Rust is allowed only for components where it gives a meaningful systems/security advantage, especially future sandboxing or isolated execution components.

The default is **not** a split-language system.

Preferred architecture:

```text
M31A Core
  └── Go

optional future:
  m31a-sandbox
    └── Rust
```

Do not introduce Rust merely because Rust is technically interesting.

## 5.3 TypeScript boundary

TypeScript is explicitly not the M31A core language.

It may later be used for:

- VS Code integration
- web UI
- desktop application surfaces
- external integration tools

Those clients must communicate with the Go runtime rather than duplicate the agent engine.

---

# 6. User experience

## 6.1 Primary interaction model

The primary interface is an interactive terminal session:

```bash
m31a
```

The user can then type natural language.

Example:

```text
> implement organization-level RBAC with admin, manager and member roles
```

M31A should understand that this is an implementation task without requiring the user to remember a complex command language.

## 6.2 Explicit commands

Natural language is primary, but deterministic commands must exist.

Initial command families should include:

```text
m31a
m31a ask
m31a explain
m31a investigate
m31a research
m31a plan
m31a implement
m31a fix
m31a review
m31a verify
m31a test
m31a inspect
m31a impact
m31a onboard
m31a status
m31a resume
m31a runs
m31a git
m31a ship
```

Inside interactive mode, slash-style shortcuts may also exist:

```text
/plan
/research
/verify
/review
/impact
/debug
/ship
```

Do not allow the command system to become the architecture. Commands are adapters into domain workflows.

---

# 7. Six-plane architecture

M31A should be organized conceptually around six planes.

```text
                    M31A
                     │
        ┌────────────┼────────────┐
        │            │            │
 interaction   intelligence   engineering
        │            │            │
        └────────────┼────────────┘
                     │
                 execution
                     │
                 assurance
                     │
                   memory
```

## 7.1 Interaction plane

Responsible for:

- CLI
- TUI
- terminal input
- terminal rendering
- command parsing
- progress display
- prompts/checkpoints
- cancellation
- interactive approval

## 7.2 Intelligence plane

Responsible for:

- LLM providers
- model routing
- context assembly
- repository intelligence
- symbol search
- code graph
- research
- retrieval
- impact analysis

## 7.3 Engineering plane

Responsible for:

- requirements
- plans
- task graphs
- project state
- architecture state
- decisions
- constraints
- work breakdown

## 7.4 Execution plane

Responsible for:

- agent execution
- tool execution
- shell processes
- filesystem mutations
- Git
- worktrees
- MCP
- plugins
- workspace isolation
- future sandbox

## 7.5 Assurance plane

Responsible for:

- tests
- static analysis
- build checks
- security checks
- impact validation
- verification
- adversarial review
- acceptance criteria
- UAT

## 7.6 Memory plane

Responsible for:

- session history
- runs
- event log
- durable state
- project artifacts
- decisions
- learnings
- verification evidence
- telemetry

---

# 8. Core domain model

M31A must be modeled around explicit domain objects.

At minimum:

```text
Project
Repository
Workspace
Session
Run
Intent
Requirement
Decision
ResearchArtifact
Plan
Task
TaskGraph
Agent
Tool
Capability
PermissionPolicy
Artifact
Verification
Checkpoint
Event
```

Do not create one mega-struct containing all project information.

## 8.1 Project

A Project represents the durable engineering identity and configuration for a project workspace.

Contains:

- project ID
- root path
- repository references
- project name
- detected stack
- configuration
- requirements
- project constraints
- architectural conventions
- current state
- active run

## 8.2 Session

A Session represents a user interaction history.

It must be durable enough for recovery, but it must not be the sole source of project state.

## 8.3 Run

A Run represents an engineering operation.

Example:

```text
RUN-2026-08-23-8F2A
```

A run owns:

- intent
- plan
- task graph
- agent executions
- tool events
- checkpoints
- verification
- final result

Example commands:

```bash
m31a run show 8F2A
m31a run resume 8F2A
m31a run verify 8F2A
m31a run rollback 8F2A
m31a run explain 8F2A
```

## 8.4 Task

A Task is a bounded unit of engineering work.

Example:

```text
Task T1
  objective: introduce AuthContext
  files:
    - src/auth/service.ts
    - src/auth/types.ts
  dependencies: []
  acceptance:
    - typecheck passes
    - existing API unchanged
    - auth tests pass
  verification:
    - unit tests
    - integration tests
```

## 8.5 TaskGraph

The TaskGraph is the executable representation of a plan.

It contains:

- objectives
- constraints
- requirements
- tasks
- dependencies
- preconditions
- acceptance criteria
- verification strategies
- risk classifications
- checkpoints
- execution waves

The LLM produces semantic intent; M31A compiles that into a task graph with explicit dependencies and lifecycle state.

---

# 9. The M31A planning model

M31A must have a strongly typed planning intermediate representation rather than treating Markdown as the executable plan.

Conceptual model:

```text
TaskGraph
 ├── Objective
 ├── Requirements[]
 ├── Constraints[]
 ├── Decisions[]
 ├── Tasks[]
 ├── Dependencies[]
 ├── Preconditions[]
 ├── AcceptanceCriteria[]
 ├── VerificationSteps[]
 ├── RiskLevel
 └── Checkpoints[]
```

Human-readable plan artifacts are generated from this IR.

The runtime executes the IR.

This prevents an important failure mode where a model writes a nice-looking plan that the executor interprets differently.

---

# 10. Execution state machine

Execution must be explicit.

Base states:

```text
PLANNED
  ↓
READY
  ↓
RUNNING
  ↓
WAITING
  ↓
VERIFYING
  ↓
COMPLETED
```

Failure states:

```text
RUNNING
  ↓
FAILED
  ├── retry
  ├── repair
  ├── replan
  └── escalate
```

Verification failure:

```text
VERIFYING
  ↓
FAILED
  ├── diagnose
  ├── patch
  └── rerun
```

The transition system must be explicit and validated. Do not allow arbitrary state changes from UI code or model-generated output.

---

# 11. Event model

M31A should be event-driven internally.

Minimum event vocabulary:

```text
ProjectInitialized
RepositoryIndexed
SessionCreated
RunCreated
IntentAccepted
ResearchStarted
ResearchCompleted
PlanCreated
PlanValidated
TaskCreated
TaskReady
AgentStarted
AgentCompleted
ToolCallRequested
ToolCallStarted
ToolCallCompleted
ToolCallFailed
FileChanged
GitChangeDetected
CheckpointRequested
CheckpointApproved
CheckpointDenied
TestStarted
TestPassed
TestFailed
VerificationStarted
VerificationPassed
VerificationFailed
TaskCompleted
TaskFailed
RunCompleted
RunFailed
RunPaused
RunResumed
```

Events must be append-oriented and durable.

Current state should be derivable from events where practical.

This gives M31A:

- recovery
- replay
- auditability
- debugging
- telemetry
- deterministic reconstruction

---

# 12. Project memory: `.m31a/`

M31A requires a project-local state directory.

Recommended structure:

```text
.m31a/
├── project.md
├── requirements.md
├── roadmap.md
├── state.json
├── config.toml
│
├── architecture/
│   ├── overview.md
│   ├── components.md
│   ├── dependencies.md
│   └── conventions.md
│
├── intelligence/
│   ├── symbols.db
│   ├── files.db
│   ├── imports.db
│   ├── calls.db
│   └── embeddings/
│
├── decisions/
│   ├── 0001-architecture.md
│   └── ...
│
├── research/
│
├── plans/
│
├── tasks/
│   ├── active/
│   ├── completed/
│   └── failed/
│
├── runs/
│   └── <run-id>/
│
├── verification/
│
├── handoffs/
│
└── events.db
```

This is inspired by GSD's persistent filesystem state model, but it is deliberately not a copy of `.planning/`.

## 12.1 Source-control policy

`.m31a/` must be designed with granular persistence rules.

Likely Git-tracked:

- `project.md`
- `requirements.md`
- `roadmap.md`
- architecture documents
- decisions
- important plans
- durable learnings
- selected verification reports

Likely ignored/local:

- local caches
- embeddings
- transient event indexes
- secrets
- provider credentials
- huge tool outputs
- temporary run artifacts

The exact default policy must be documented and configurable.

Never store API keys in `.m31a/`.

---

# 13. Repository intelligence

M31A must continuously understand the repository instead of rediscovering it on every prompt.

## 13.1 Code intelligence graph

Represent relationships such as:

```text
function A
 ├── calls → function B
 ├── reads → config X
 ├── tested-by → test Y
 ├── imported-by → module Z
 └── modified-by → commit C
```

Repository intelligence should include, when available:

- files
- directories
- symbols
- types
- functions
- classes
- interfaces
- imports
- calls
- inheritance
- implementations
- tests
- APIs
- database entities
- configurations
- build targets
- package dependencies
- Git history

## 13.2 Language support

M31A must be language-agnostic.

The runtime should orchestrate language intelligence through:

- Tree-sitter parsers where appropriate
- Language Server Protocol servers where appropriate
- language-native tooling
- static analysis tools
- compiler/type-checker outputs

Do not implement a separate complete parser for each language in M31A.

---

# 14. Repository onboarding

Command:

```bash
m31a onboard
```

For an existing repository, onboarding should perform:

```text
repository scan
    ↓
language detection
    ↓
build-system detection
    ↓
architecture detection
    ↓
dependency analysis
    ↓
test discovery
    ↓
configuration discovery
    ↓
Git analysis
    ↓
documentation analysis
    ↓
symbol indexing
    ↓
architecture summary
```

The result should populate `.m31a/` intelligence and project artifacts.

Onboarding must be safe and read-only by default.

---

# 15. Impact analysis

M31A should calculate the likely impact surface before high-risk mutations.

Example:

```text
USER
"Replace the UserRepository interface with a generic repository."

             ↓

       IMPACT ANALYSIS

       ┌───────────────┐
       │ target symbol │
       └───────┬───────┘
               │
        ┌──────┼───────────┐
        ▼      ▼           ▼
      callers tests     adapters
        │      │           │
        ▼      ▼           ▼
      APIs   fixtures    DI wiring
```

M31A should return something resembling:

```text
Impact: HIGH

Affected:
  14 source files
  8 tests
  2 public interfaces
  1 dependency injection module

Risk:
  API compatibility
  generic type inference
  test fixture breakage

Recommended execution:
  3 plans
  2 waves
```

Impact analysis is an input to planning, not merely a UI display.

---

# 16. Explain and archaeology capabilities

M31A must be able to answer why code exists, not merely what code says.

Example:

```bash
m31a explain "the authentication flow"
m31a investigate "why src/foo/bar.ts exists"
```

M31A should combine:

- current source
- dependency graph
- call graph
- tests
- Git blame
- commit history
- PR/issue information when available
- architecture decisions
- project documents

Example desired result:

```text
This abstraction was introduced in commit X
for reason Y.

Current consumers:
  A
  B
  C

Removing it directly would break:
  B
  C

Original rationale:
  ...

Current assessment:
  rationale still appears valid / likely obsolete.
```

The system must clearly distinguish evidence from inference.

---

# 17. Research engine

Research is a first-class M31A workflow.

Possible research sources:

```text
repository
local documentation
dependencies
Git history
ADRs
issue tracker
package registries
web
official vendor documentation
```

Example:

```bash
m31a research "replace redis client"
```

Expected result:

```text
Research question
Known architecture
Existing patterns
Available packages
Compatibility concerns
Security concerns
Migration concerns
Recommendation
```

External information must be cited and timestamped in research artifacts.

For current facts, M31A must prefer authoritative sources.

---

# 18. Dependency intelligence

When the agent proposes adding a dependency, M31A should investigate at minimum:

- registry existence
- project age
- recent releases
- maintenance activity
- source repository
- license
- known vulnerabilities
- dependency count / transitive impact
- API stability indicators
- package popularity where available

Package suggestions derived only from model memory must not be treated as verified facts.

For high-risk or ambiguous dependencies, a human checkpoint is required before installation.

---

# 19. Agent architecture

M31A uses specialized agents with explicit contracts.

Initial agents:

```text
Supervisor
Explorer
Researcher
Architect
Planner
Implementer
Tester
Debugger
Verifier
Reviewer
SecurityAuditor
GitSpecialist
ReleaseEngineer
```

Agents are roles with contract boundaries, not personalities.

## 19.1 Example planner contract

```text
PlannerAgent
requires:
  RepositoryGraph
  ProjectState
  Requirements
  ResearchArtifacts

produces:
  TaskGraph

must_not:
  mutate source files
  perform unrestricted shell actions
  silently change requirements
```

## 19.2 Example verifier contract

```text
VerifierAgent
requires:
  TaskGraph
  Diff
  AcceptanceCriteria
  TestResults

produces:
  VerificationReport

must_not:
  silently modify source code
  mark failed criteria as passed
  invent evidence
```

---

# 20. Context engineering

Context must be assembled dynamically.

Do not send the full repository to every agent.

Context should be composed from relevant sources:

```text
Global project context
      +
Repository context
      +
Current task context
      +
Relevant symbols
      +
Relevant history
      +
Relevant tests
      +
Relevant decisions
      +
Current plan
```

Planner context should prioritize:

- requirements
- constraints
- architecture
- existing patterns
- dependency relationships
- research

Executor context should prioritize:

- exact plan
- affected files
- relevant symbols
- acceptance criteria
- tests
- constraints

Verifier context should prioritize:

- requirements
- acceptance criteria
- task graph
- diff
- test output
- runtime behavior
- invariants

Reviewer context should prioritize:

- diff
- architectural dependencies
- tests
- historical rationale
- security boundaries

---

# 21. Context epochs and compaction

M31A must treat context as runtime state, not as an infinitely growing message list.

A conceptual context epoch is:

```text
Context Epoch
  ├── baseline system context
  ├── projected session history
  ├── relevant context sources
  └── tool outputs
```

When context becomes too large, M31A should:

```text
conversation
    ↓
extract durable engineering facts
    ↓
update project/run state
    ↓
create a compact context baseline
    ↓
discard conversational noise
```

The durable state should preserve facts such as:

```text
Decision D-12
Task T-43
Risk R-4
Current branch
Current plan
Completed validations
Open issues
Relevant files
```

Never rely on a prose summary alone for recovery-critical information.

---

# 22. Tool architecture

Tools must be registered through a typed registry.

A tool should declare:

```text
name
description
input schema
output schema
capabilities
risk class
resource scope
mutation class
side effects
idempotency properties
```

Example built-in tools:

```text
read_file
write_file
edit_file
search_text
glob_files
list_directory
run_command
run_tests
inspect_git
create_worktree
apply_patch
read_git_history
inspect_symbol
inspect_dependency_graph
request_checkpoint
```

Future tools:

```text
browser
http
database
container
MCP
GitHub
GitLab
CI
package registry
cloud providers
```

---

# 23. Capability-based permissions

Permissions must not be a simple global “yes/no tool” system.

Example:

```text
capability:
  filesystem.write

scope:
  repository

risk:
  medium

conditions:
  deny .env
  deny *.pem
  deny ~/.ssh/*
  deny provider credential files
```

Git example:

```text
capability:
  git.write

allowed:
  create branch
  stage files
  commit

requires approval:
  push
  force-push
  reset --hard
  rebase shared branch
```

Database example:

```text
capability:
  database.migrate

requires:
  checkpoint = migration
```

Permission evaluation must occur at execution time, not only when tools are presented to the model.

---

# 24. Security boundaries

M31A executes arbitrary developer-selected code and therefore must treat all repository content and model output as potentially adversarial.

Threats include:

- malicious repository files
- prompt injection in source/documents
- hidden instructions in README files
- malicious build scripts
- dependency confusion
- unsafe package suggestions
- command injection
- path traversal
- symlink attacks
- secret exfiltration
- accidental destructive commands
- model hallucination
- malicious tool/plugin behavior
- compromised MCP servers
- untrusted network resources

Important security rules:

1. Never treat repository text as trusted instructions.
2. Never execute commands solely because a README says to execute them.
3. Validate tool inputs before execution.
4. Resolve filesystem paths against trusted workspace roots.
5. Detect and constrain symlink escapes.
6. Keep secrets out of prompts where possible.
7. Never write provider credentials into project artifacts.
8. Redact secrets from logs and tool output.
9. Require explicit policy for dangerous Git operations.
10. Treat MCP/plugin tools as separate trust domains.

---

# 25. Workspace and transactional execution

M31A should prefer isolated workspaces for substantial code changes.

Preferred flow:

```text
user intent
    ↓
create worktree
    ↓
analyze
    ↓
plan
    ↓
execute
    ↓
test
    ↓
verify
    ↓
produce diff
    ↓
apply/merge after approval or policy decision
```

Example:

```bash
m31a implement "Add OAuth refresh-token rotation"
```

M31A may create:

```text
worktree/m31a/<run-id>
```

The user's main worktree should remain clean until policy allows integration.

Do not assume a repository is a single Git repository. Multi-repository workspaces must be a first-class case.

The system must also work when there is no Git repository at the current directory.

M31A must never fall back to the filesystem root as the workspace root merely because Git discovery failed.

This is a known class of failure in coding agents and can expose unrelated files from the machine. The workspace detector must prefer the resolved current directory when no Git root exists.

---

# 26. Git as a first-class domain

M31A must understand:

```text
repository state
branch state
working tree
diff
staging
commit graph
history
blame
tags
releases
remotes
worktrees
merge state
rebase state
```

Desired capabilities:

```text
m31a what changed?
m31a why was this changed?
m31a find the regression introduced after commit X
m31a review my current branch
m31a prepare a release
m31a split these changes into logical commits
m31a explain why this branch is diverged
```

M31A should understand semantic change boundaries when generating commits.

Example:

```text
commit 1
feat(auth): introduce token validator

commit 2
test(auth): add expiration coverage

commit 3
refactor(auth): isolate token parsing
```

Avoid automatic blind `git add . && git commit` behavior.

---

# 27. Regression intelligence

M31A should support causal debugging across Git history.

Example:

```bash
m31a investigate "this API started returning 500 after yesterday's changes"
```

Potential workflow:

```text
identify failing behavior
        ↓
inspect recent history
        ↓
bisect candidate commits
        ↓
inspect causal code changes
        ↓
construct hypothesis
        ↓
run verification
        ↓
report root cause
```

Expected output:

```text
Regression introduced by:
commit abc123

Likely mechanism:
...

Affected components:
...

Recommended fix:
...
```

M31A must distinguish “likely cause” from “verified cause.”

---

# 28. Verification engine

Verification is a core product capability.

For every acceptance criterion:

```text
criterion
  ↓
verification strategy
  ↓
command/test/analysis
  ↓
evidence
  ↓
verdict
```

Verification levels:

### Level 0 — Structural

```text
files exist
imports resolve
types compile
schemas validate
```

### Level 1 — Unit

```text
unit tests pass
```

### Level 2 — Integration

```text
API tests
integration tests
database tests
```

### Level 3 — Behavioral

```text
CLI behavior
HTTP behavior
runtime behavior
state transitions
```

### Level 4 — Architectural

```text
dependency constraints
layer boundaries
forbidden imports
public API invariants
```

### Level 5 — Human

```text
UAT
manual checkpoint
visual validation
```

The verifier must not silently mutate production code to make a failing verification pass.

If repair is required, the run transitions back into planning/execution explicitly.

---

# 29. Proof-carrying changes

Every completed task should produce a durable change record.

Conceptually:

```text
Task
 ├── intent
 ├── plan
 ├── changed files
 ├── diff
 ├── commands executed
 ├── tests
 ├── verification evidence
 ├── risks
 └── final verdict
```

Example:

```text
Run 84f2 completed.

Intent:
  Add OAuth refresh-token rotation.

Changed:
  7 files

Tests:
  23 passed

Verification:
  API integration       ✓
  expiry handling       ✓
  refresh rotation      ✓
  regression suite      ✓

Known risk:
  concurrent refresh requests are serialized but not load-tested.

Confidence:
  high, based on executed evidence.
```

Do not present numeric confidence as objective truth unless it is backed by a defined scoring model.

---

# 30. Adversarial review

After meaningful implementation work, M31A should be capable of independent review.

Adversarial review asks:

```text
What assumptions did the implementation make?
What requirements are only superficially satisfied?
What edge cases were missed?
What race conditions exist?
What behavior is untested?
What security boundary changed?
What existing functionality could regress?
```

Potential loop:

```text
Implementation
    ↓
Verification
    ↓
Adversarial Review
    ↓
Repair Plan
    ↓
Execution
    ↓
Verification
```

For high-risk work, use an independent model or an independent context whenever practical so the verifier is not simply reproducing the implementer's assumptions.

---

# 31. Decision intelligence

Important decisions must be classified.

Example:

```text
Decision:
Replace PostgreSQL with SQLite

reversibility: LOW
blast_radius: HIGH
data_migration: YES
public_contract: NO
operational_risk: HIGH

classification:
ONE_WAY_DOOR

action:
REQUIRE HUMAN APPROVAL
```

Another example:

```text
Decision:
Rename local variable

reversibility: HIGH
blast_radius: LOW

classification:
SAFE_AUTOMATION
```

Decision metadata must be durable.

---

# 32. Autonomy levels

M31A must support explicit autonomy policies.

## Assist

```text
M31A recommends actions.
User executes.
```

## Delegate

```text
M31A executes low-risk actions.
High-risk actions stop for approval.
```

## Autonomous

```text
M31A may execute the complete task graph,
including recovery and replanning,
subject to the configured policy.
```

Autonomy differences must be enforced by the policy engine, not by prompt wording.

---

# 33. Model routing

v1 has one mandated model:

```text
nvidia/nemotron-3-ultra-550b-a55b
```

However, the LLM interface must support future routing by task type.

Conceptual future mapping:

```text
simple edit       → fast/cheap model
code search       → fast reasoning model
architecture      → strongest reasoning model
debugging         → strong reasoning model
security review   → strongest available verifier
summarization     → low-cost model
verification      → independent model
```

Do not introduce artificial multi-model complexity into v1 when only one configured model exists.

Build the abstraction; keep the default implementation simple.

---

# 34. NVIDIA Build integration — v1

## 34.1 Provider

M31A v1 uses NVIDIA's Build / API Catalog endpoints.

Primary endpoint:

```text
https://integrate.api.nvidia.com/v1/chat/completions
```

API style:

```text
OpenAI-compatible Chat Completions
```

The provider adapter must use the exact configured model ID:

```text
nvidia/nemotron-3-ultra-550b-a55b
```

## 34.2 Authentication

Credential comes from an environment variable:

```text
NVIDIA_API_KEY
```

Never commit it.

Never write it to `.m31a/`.

Never display the full value in logs, errors, diagnostics, or tool output.

The CLI should provide an actionable error when the key is missing.

## 34.3 Current model characteristics

The NVIDIA Build catalog currently describes `nvidia/nemotron-3-ultra-550b-a55b` as an open hybrid Mamba-Transformer MoE model with approximately 550B total parameters and 55B active parameters, with a context length of up to 1M tokens.

The model is positioned for frontier reasoning, coding, planning, tool use, long-context analysis, and agentic workflows.

The current Build model page exposes a free API endpoint, partner endpoints, and downloadable/self-hosted availability. Availability and service behavior are provider-controlled and may change; M31A must not assume perpetual availability of the free endpoint.

## 34.4 Reasoning

The model supports configurable reasoning through chat-template settings. The NVIDIA examples show reasoning enabled using:

```json
{
  "chat_template_kwargs": {
    "enable_thinking": true
  }
}
```

The model also exposes reasoning content in streaming responses.

M31A should preserve provider reasoning metadata internally when available but must not expose hidden chain-of-thought as a user-facing transcript by default.

User-facing UI may expose compact status such as:

```text
reasoning: active
```

or a safe summary, not raw private reasoning traces.

## 34.5 Coding-agent request detail

NVIDIA's current documentation explicitly notes that coding agents should add:

```json
{
  "chat_template_kwargs": {
    "force_nonempty_content": true
  }
}
```

M31A must support provider-specific request options through a provider adapter and must enable the required coding-agent compatibility behavior for the selected model.

Do not scatter `chat_template_kwargs` throughout generic agent code.

The NVIDIA provider implementation owns these details.

## 34.6 Initial generation settings

Current NVIDIA Build examples use settings including:

```text
temperature = 1.0
top_p = 0.95
max_tokens = 16384
reasoning enabled
```

M31A should not blindly hardcode these values everywhere.

They must be configurable through the provider/model profile.

The initial default profile may use the documented Build defaults above, then be tuned through controlled evaluation.

## 34.7 Streaming

Streaming is required for the interactive terminal experience.

M31A must support incremental response chunks and expose a stable internal stream representation.

The provider adapter must tolerate:

- empty deltas
- reasoning-only deltas
- content-only deltas
- tool-call deltas
- mixed reasoning/content transitions
- stream termination
- malformed provider responses
- transient connection failure
- retry/recovery policy

Never assume every streamed chunk contains normal text content.

## 34.8 Tool calling

M31A uses tool calling as a first-class protocol capability.

The model supports tool usage, and the NVIDIA API is OpenAI-compatible.

Tool-call handling must be implemented as a generic protocol layer:

```text
assistant response
    ↓
parse tool calls
    ↓
validate tool name
    ↓
validate arguments
    ↓
permission check
    ↓
execute tool
    ↓
bound output
    ↓
persist result
    ↓
continue provider turn
```

Never execute an unvalidated model-generated command directly.

---

# 35. LLM provider abstraction

Define a provider-neutral interface similar conceptually to:

```text
Provider
Model
Request
Response
Stream
ToolDefinition
ToolCall
Usage
FinishReason
ProviderError
```

M31A's orchestration layer should not import NVIDIA-specific types.

Provider-specific data belongs under a provider adapter such as:

```text
internal/llm/nvidia/
```

Future providers can implement the same interface:

```text
internal/llm/openai/
internal/llm/anthropic/
internal/llm/google/
internal/llm/ollama/
```

They should not be necessary for v1 completion.

---

# 36. OpenCode's role during development

OpenCode is the implementation assistant/runtime used to develop M31A.

It is not the M31A runtime architecture.

Development flow:

```text
Developer
   ↓
OpenCode
   ↓
implements M31A
   ↓
M31A runtime
   ↓
NVIDIA API
   ↓
Nemotron
```

OpenCode configuration must not become part of M31A's source-level runtime assumptions.

Machine-local OpenCode credentials/configuration must not be committed.

If an `opencode.json` or equivalent local configuration is needed for development, it should remain local unless intentionally creating a sanitized project-level configuration template.

---

# 37. TUI architecture

Use Bubble Tea as the initial TUI foundation.

The TUI should be a client of the application/runtime layer.

Recommended separation:

```text
TUI
  ↓
Application commands / query interface
  ↓
M31A core
```

Do not put orchestration, LLM calls, filesystem mutation, or Git logic directly into Bubble Tea models.

Bubble Tea handles:

- rendering
- input
- local interaction state
- terminal events

Core handles:

- sessions
- runs
- tools
- agents
- execution
- state
- verification

The TUI must remain replaceable by future interfaces.

---

# 38. Terminal UX requirements

M31A must feel like a serious engineering tool, not a terminal log dump.

The TUI should clearly distinguish:

```text
user input
assistant response
agent status
plan
execution
tool execution
checkpoint
verification
errors
warnings
```

For long operations show:

```text
current agent
current task
progress
elapsed time
active tool
files touched
verification state
```

Avoid massive scrolling tool output when a bounded summary is sufficient.

Allow users to expand raw details when needed.

Long-running tool output should be persisted outside the model context and summarized for the model/UI.

---

# 39. Cancellation and concurrency

Every long-running operation must accept Go `context.Context` cancellation.

Cancellation must be cooperative and resource-safe.

Important cancellation cases:

- user presses Ctrl-C
- terminal closes
- model stream stalls
- tool times out
- process exits unexpectedly
- run is paused
- checkpoint interrupts execution

Do not leak goroutines, subprocesses, PTYs, file descriptors, temporary worktrees, or database transactions.

Concurrency must be explicit.

Avoid shared mutable global state.

---

# 40. Process execution

Shell/tool execution is security-sensitive.

The execution layer must:

- use `exec.CommandContext`
- capture stdout/stderr separately when needed
- enforce timeouts
- record exit codes
- preserve signal information
- bound captured output
- stream output to the UI separately from model context
- persist large output to managed files
- redact secrets
- detect process hangs
- clean up child processes when cancelled

A tool's output shown to the model must have a size ceiling.

The complete output may remain available as a managed artifact.

---

# 41. Tool-output management

Tool output is not automatically model context.

M31A should maintain:

```text
bounded model-visible output
        +
full persisted tool artifact
```

For example:

```text
Tool:
  go test ./...

Model output:
  FAIL: 2 tests
  first failure: ...
  full output: .m31a/runs/84f2/tools/abc123.log
```

This avoids context explosions and preserves auditability.

---

# 42. Persistence

Use SQLite for structured machine state.

Conceptual storage:

```text
SQLite
  ├── projects
  ├── repositories
  ├── sessions
  ├── runs
  ├── events
  ├── tasks
  ├── plans
  ├── agents
  ├── tool_calls
  ├── checkpoints
  ├── verifications
  └── artifacts
```

Human-readable engineering state remains in `.m31a/`.

Preferred model:

```text
machine state → SQLite
engineering artifacts → Markdown/JSON
cache/index → local database/files
```

SQLite choice must minimize operational requirements. M31A should remain a local developer tool without requiring a server.

Do not introduce Postgres/Redis/etc. into v1 core unless a concrete requirement proves local SQLite insufficient.

---

# 43. SQLite and multi-process safety

The state layer must be designed for local concurrency.

Avoid assuming that multiple independent M31A processes can safely mutate the same state database without coordination.

A repository may have:

```text
TUI process
background run
CLI command
```

so the runtime must either:

- centralize ownership in one process, or
- use robust SQLite transaction and locking semantics, or
- provide a local coordinator/daemon later.

Do not invent distributed-systems complexity in v1.

---

# 44. Plugin system

Plugins are part of the longer-term architecture.

A plugin should be able to register:

- tools
- commands
- agents
- context sources
- integrations
- verification providers

Plugins must not receive unrestricted access automatically.

Plugin capabilities must pass through the same policy engine as built-in tools.

Version plugin APIs explicitly.

Never let plugin internals become coupled to private M31A structs.

---

# 45. MCP

MCP should be supported through a well-defined adapter, but MCP availability must not be required for core M31A functionality.

MCP tool schemas can be expensive in model context. M31A should use selective materialization and lazy discovery where possible.

MCP servers should be treated as external trust domains.

Every MCP tool invocation must still pass M31A's execution and permission policy.

---

# 46. Failure modes to design for

The following failure classes are mandatory engineering concerns.

## 46.1 Context rot

Cause:
large conversation/history overwhelms useful context.

Mitigation:

- context epochs
- selective context assembly
- fresh-context agents
- compaction into durable state
- bounded tool outputs

## 46.2 Model hallucinated file paths

Mitigation:

- workspace resolver
- path validation
- file existence checks
- graph-backed file lookup

## 46.3 Model invents APIs

Mitigation:

- repository intelligence
- symbol validation
- compilation/type checks
- official docs research
- evidence-based verification

## 46.4 Tool call races

Mitigation:

- tool call IDs
- run/task ownership
- state machine validation
- stale-call rejection
- serialization for non-idempotent operations

## 46.5 Long-running hang

Mitigation:

- heartbeats
- operation deadlines
- subprocess supervision
- cancellation
- watchdogs
- explicit RUNNING/WATCHING states

## 46.6 Workspace root escape

Mitigation:

- canonical root resolution
- path containment checks
- symlink checks
- explicit external path capabilities

## 46.7 Non-Git repositories

Mitigation:

- workspace root is resolved current directory if Git discovery fails
- never use `/` as the fallback root

This exact class of workspace-root bug has appeared in real coding-agent software and can expose unrelated files.

## 46.8 Multi-repository workspaces

Mitigation:

- repository discovery returns multiple repository boundaries
- file-to-repository mapping
- per-repository Git operations
- no assumption that a parent directory owns all repos

## 46.9 Model/API outage

Mitigation:

- retry with backoff for safe transient failures
- preserve run state before retry
- never duplicate non-idempotent tool execution merely because the provider call is retried
- expose provider failure clearly
- allow resume later

## 46.10 API streaming corruption

Mitigation:

- defensive SSE parser
- partial delta handling
- malformed event tolerance
- provider request IDs where available
- durable provider attempts

## 46.11 Secret leakage

Mitigation:

- log redaction
- environment-variable filtering
- output scanners
- secret-safe prompt construction
- no credentials in artifacts

## 46.12 Verification theater

Cause:
agent runs a superficial check and declares success.

Mitigation:

- explicit acceptance criteria
- evidence binding
- independent verifier
- behavior checks for behavior claims
- no “looks good” completion state

## 46.13 Infinite repair loops

Mitigation:

- bounded repair attempts
- failure taxonomy
- replan threshold
- human escalation

## 46.14 Self-confirming verification

Mitigation:

- separate verifier context
- optionally separate model
- verification based on executable evidence

## 46.15 Destructive Git action

Mitigation:

- capability policy
- confirmation checkpoint
- clean diff display
- explicit target branch/repository

## 46.16 Dependency slop

Mitigation:

- package legitimacy research
- authoritative registry lookup
- vulnerability/license checks
- human checkpoint for suspicious packages

## 46.17 Prompt injection in repository

Mitigation:

- repository content is data, not authority
- system policy has higher priority
- explicit instruction-source classification
- do not execute commands from untrusted documents automatically

## 46.18 Tool output context explosion

Mitigation:

- bounded output
- artifact files
- summaries
- selective re-read

## 46.19 SQLite corruption / lock contention

Mitigation:

- transactional writes
- robust journaling configuration
- migration strategy
- integrity checks
- controlled multi-process access

---

# 47. Known lessons from OpenCode issues

M31A should proactively test classes of failure that have appeared in OpenCode and similar coding agents.

Examples include:

- incorrect workspace root detection when no Git repository exists
- undo/rollback models that fail in multi-repository or non-root-Git layouts
- long-running agent runs that appear stuck while the underlying model is not progressing
- oversized context/tool schemas
- stale tool calls after tool registration changes
- complicated session/state lifecycle edge cases

M31A should treat these not as reasons to criticize OpenCode, but as requirements for our own state model and integration tests.

---

# 48. Undo and rollback model

Rollback cannot depend solely on Git.

Why:

- repository may not have Git
- repository may contain multiple Git roots
- user may have unrelated local modifications
- files may exist outside Git tracking
- generated/untracked files can matter

M31A needs a workspace snapshot abstraction.

Possible hierarchy:

```text
WorkspaceSnapshot
 ├── repository snapshots
 ├── untracked file manifests
 ├── created files
 ├── modified files
 └── deleted files
```

Git may be one implementation detail, not the entire rollback contract.

For v1, implement conservative rollback behavior and clearly report unsupported cases instead of claiming full undo.

---

# 49. Requirements traceability

Every non-trivial plan should map to requirements.

Example:

```text
REQ-AUTH-01
REQ-AUTH-02
REQ-AUTH-03
```

Task graph:

```text
T1 → REQ-AUTH-01
T2 → REQ-AUTH-02
T3 → REQ-AUTH-03
```

Verification:

```text
REQ-AUTH-01 → test-auth-001
REQ-AUTH-02 → integration-auth-004
REQ-AUTH-03 → API-check-007
```

This enables gap analysis.

---

# 50. Vertical slicing / tracer principle

For meaningful features, prefer a small end-to-end production-quality tracer slice before expanding horizontally.

Example:

Bad decomposition:

```text
database layer
API layer
service layer
UI layer
```

Better initial slice:

```text
one end-to-end RBAC flow
    ↓
validated through persistence
    ↓
validated through service
    ↓
validated through API
    ↓
validated with test
```

Then expand the implementation.

This reduces the risk of building several disconnected layers that only integrate at the end.

---

# 51. TDD integration

M31A should support TDD planning/execution mode.

For behavior-adding tasks:

```text
write failing test
    ↓
implement behavior
    ↓
make test pass
    ↓
refactor
    ↓
verify
```

TDD mode should be explicit in the TaskGraph so the executor knows the expected lifecycle.

Do not force TDD for tasks where it is inappropriate, such as pure documentation or certain exploratory work.

---

# 52. Package and dependency policy

External dependencies must justify their presence.

Before adding a dependency, M31A should consider:

```text
Does stdlib solve this?
Is the dependency maintained?
Is its license acceptable?
What is its transitive cost?
Does it introduce CGO?
Does it introduce platform-specific requirements?
Is the API stable?
Is it security-sensitive?
```

Prefer Go standard library for stable foundational concerns where reasonable.

Avoid dependency proliferation merely to save a few lines of code.

---

# 53. API and error design

Errors must be typed and actionable.

Avoid opaque strings like:

```text
something went wrong
```

Prefer structured error categories:

```text
ErrProviderUnavailable
ErrProviderUnauthorized
ErrProviderRateLimited
ErrWorkspaceNotFound
ErrPathOutsideWorkspace
ErrToolDenied
ErrToolInvalidArguments
ErrTaskInvalidTransition
ErrVerificationFailed
ErrCheckpointRequired
ErrRunPaused
ErrRunNotFound
```

Errors should carry:

- category
- message
- remediation
- original cause where safe
- operation identity
- relevant run/task/tool ID

Never leak secrets through wrapped errors.

---

# 54. Logging

Use structured logging.

Logs should include:

```text
time
level
component
run_id
session_id
task_id
tool_call_id
operation
message
```

Sensitive values must be redacted.

The normal UI should not dump raw debug logs.

Debug logs must be available through an explicit mode.

---

# 55. Observability

M31A should be observable locally without requiring a cloud service.

Useful metrics:

```text
provider requests
provider latency
stream duration
tool invocations
tool failures
agent executions
retries
replans
verification failures
checkpoint frequency
run duration
context size estimates
stored artifact sizes
```

This telemetry should help diagnose agent quality, not become product analytics infrastructure in v1.

---

# 56. Testing philosophy

M31A must be tested as a runtime, not only as a collection of pure functions.

Required layers:

## Unit

Domain and state-machine logic.

## Integration

- SQLite
- filesystem
- Git
- provider API client
- tool registry

## Contract

- LLM protocol
- tool schema
- provider adapter
- persistence schema

## End-to-end

Realistic repository fixtures and simulated model interactions.

## Failure injection

Test:

- provider timeout
- partial stream
- tool failure
- subprocess crash
- permission denial
- checkpoint denial
- database lock
- interrupted run
- context compaction
- process restart

---

# 57. Evaluation harness

M31A requires a benchmark suite from early development.

Categories:

```text
repository understanding
code navigation
implementation correctness
tool calling
planning quality
impact analysis
verification quality
Git workflows
recovery
security policy
context management
```

Every major architecture change should run the evaluation suite.

Never optimize M31A only against one manually observed conversation.

---

# 58. Golden repository fixtures

Create small but realistic fixtures for:

```text
Go monorepo
TypeScript application
Python service
Rust application
multi-package repo
multi-repository workspace
non-Git project
repository with nested Git roots
large generated directories
symlink-heavy project
project with malicious-looking README instructions
project with secrets fixtures
```

Each fixture should have known expectations for discovery, impact, planning, and verification.

---

# 59. Repository structure

Recommended initial structure:

```text
M31A/
├── cmd/
│   └── m31a/
│       └── main.go
│
├── internal/
│   ├── agent/
│   ├── application/
│   ├── artifact/
│   ├── checkpoint/
│   ├── contextengine/
│   ├── execution/
│   ├── events/
│   ├── git/
│   ├── intelligence/
│   ├── llm/
│   │   ├── nvidia/
│   │   └── protocol/
│   ├── mcp/
│   ├── permissions/
│   ├── persistence/
│   ├── planning/
│   ├── project/
│   ├── research/
│   ├── run/
│   ├── sandbox/
│   ├── session/
│   ├── tool/
│   ├── tui/
│   ├── verification/
│   └── workspace/
│
├── pkg/
│   ├── pluginapi/
│   └── protocol/
│
├── docs/
├── tests/
├── testdata/
├── scripts/
├── .gitignore
├── CONTEXT_M31A.md
├── go.mod
├── go.sum
└── README.md
```

Package names may evolve as the implementation becomes clearer. Do not create empty packages merely to match the diagram.

---

# 60. Dependency principles

Minimize third-party dependencies.

Initial likely dependencies:

- Bubble Tea for TUI
- SQLite driver appropriate for M31A's portability requirements
- Tree-sitter bindings if selected for code intelligence
- Git library only if it gives a concrete benefit over robust `git` subprocess calls
- structured logging library if standard library facilities are insufficient
- TOML library if configuration is TOML

Do not add libraries without evaluating:

- maintenance
- license
- platform behavior
- CGO implications
- security
- performance
- dependency graph size

---

# 61. TUI rendering contract

The UI should have distinct surfaces for:

```text
Header
Context / project state
Conversation
Plan
Execution timeline
Tool activity
Verification
Errors / warnings
Input
```

Example execution screen:

```text
M31A · RUN 84F2
──────────────────────────────────────────────
Implement organization-level RBAC

[1/5] Role model             ✓
[2/5] Authorization         ✓
[3/5] Route guards          ● running
[4/5] Persistence           ○ waiting
[5/5] Verification          ○ waiting

Agent: Implementer
Tool: edit_file
Files: 3
Elapsed: 00:42

──────────────────────────────────────────────
```

Verification screen:

```text
Verification
──────────────────────────────────────────────
Typecheck              ✓
Unit tests             ✓
Integration tests      ✓
Authorization matrix   ✓
Regression scan        ✓

Result: VERIFIED
```

---

# 62. Natural-language workflow example

The canonical M31A experience should look approximately like this.

```text
$ m31a

> Add organization-level RBAC with admin,
  manager and member roles.

M31A
──────────────────────────────────────────────

Analyzing repository...
✓ project mapped
✓ architecture loaded
✓ 137 relevant symbols found
✓ 22 affected files
✓ 4 existing authorization patterns found

Researching...
✓ existing auth model
✓ database constraints
✓ API middleware
✓ test conventions

Plan
──────────────────────────────────────────────
1. Introduce organization role model
2. Add authorization service
3. Integrate route guards
4. Update persistence layer
5. Add test matrix

Risk: MEDIUM
Estimated changes: 18–27 files

Proceed? [y/N]
```

Execution:

```text
Execution
──────────────────────────────────────────────

[1/5] role model       ✓
[2/5] authorization    ✓
[3/5] route guards     ✓
[4/5] persistence      ✓
[5/5] test matrix      ✓
```

Verification:

```text
Verification
──────────────────────────────────────────────

Typecheck             ✓
Unit tests             ✓
Integration tests      ✓
API contract           ✓
Authorization matrix   ✓
Regression scan        ✓

Result: VERIFIED
```

Completion:

```text
Run 84f2 completed.

18 files changed
26 tests added
114 tests passed
0 known regressions

Suggested commit:
feat(authz): add organization role-based access control
```

This is a target UX, not a requirement to reproduce these exact words or layout.

---

# 63. Codebase intelligence example

User:

```text
> what breaks if I change this interface?
```

M31A should use the code graph to produce something like:

```text
Impact: HIGH

Target:
  UserRepository

Direct callers:
  11

Implementations:
  3

Tests:
  8

Public API exposure:
  yes

Affected packages:
  auth
  users
  admin

Likely breakage:
  constructor signatures
  dependency injection
  integration fixtures

Recommended plan:
  3 tasks / 2 execution waves
```

---

# 64. Why-does-this-exist example

```text
$ m31a investigate "why src/auth/token_cache.go exists"
```

M31A should answer from evidence:

```text
Introduced:
  commit 91d2a1

Reason stated in commit:
  Reduce repeated token introspection calls.

Current consumers:
  auth middleware
  websocket gateway

Current tests:
  token_cache_test.go
  auth_integration_test.go

Current assessment:
  The cache still reduces duplicate introspection requests.
  Its TTL differs from the current token lifetime policy.

Potential follow-up:
  Reconcile cache TTL with token expiry configuration.
```

---

# 65. Regression example

```text
$ m31a investigate "the login endpoint started returning 500"
```

M31A should:

```text
1. reproduce the failure
2. inspect recent changes
3. identify candidate commits
4. inspect dependency/data flow
5. form a hypothesis
6. test the hypothesis
7. report evidence
```

Output:

```text
Regression:
  verified

Introducing commit:
  abc123

Root cause:
  token decoder now returns nil metadata for expired tokens;
  middleware assumes metadata is non-nil.

Evidence:
  failing integration test reproduces behavior before fix
  + stack trace
  + diff inspection

Recommended fix:
  handle expired-token metadata as an explicit error state.
```

---

# 66. Git semantic commit example

Given a run that changes many files, M31A should identify logical boundaries.

Instead of:

```text
git commit -am "fix stuff"
```

prefer:

```text
feat(auth): introduce token validator
test(auth): add expiration coverage
refactor(auth): isolate token parsing
```

The exact conventional-commit style is configurable.

M31A must not fabricate commit intent that does not correspond to actual changes.

---

# 67. Development workflow with OpenCode

Development should follow:

```text
1. Read CONTEXT_M31A.md
2. Read the relevant package/module documentation
3. Inspect current repository state
4. Form a plan
5. Implement a minimal vertical slice
6. Run tests
7. Run static analysis
8. Review diff
9. Verify behavior
10. Update project artifacts
```

For significant work, use explicit phase artifacts in `.m31a/`.

OpenCode should be instructed to respect M31A's project constraints rather than generating code opportunistically.

---

# 68. M31A implementation phase model

The following roadmap is the recommended delivery sequence. It is deliberately outcome-oriented.

## Phase 0 — Foundation

Deliver:

- Go module
- CLI entrypoint
- config loading
- structured errors
- logging
- project/workspace detection
- `.m31a/` initialization
- SQLite storage
- test harness

Exit condition:

```bash
m31a doctor
```

works and reports a valid environment.

## Phase 1 — Terminal UX

Deliver:

- Bubble Tea TUI
- command palette
- input/editor
- streaming response surface
- basic session lifecycle
- cancellation
- checkpoint rendering

Exit condition:

interactive terminal session works without embedding orchestration into UI code.

## Phase 2 — NVIDIA LLM Runtime

Deliver:

- provider abstraction
- NVIDIA adapter
- authentication
- streaming
- reasoning metadata handling
- tool calls
- retries
- request IDs
- provider diagnostics

Exit condition:

M31A can reliably run a simple tool-calling loop against the NVIDIA model.

## Phase 3 — Tool System

Deliver:

- typed registry
- filesystem tools
- search tools
- shell tool
- Git inspection
- test execution
- output bounding
- tool permissions

Exit condition:

agent can safely inspect and modify a fixture repository.

## Phase 4 — Session and Run Runtime

Deliver:

- sessions
- runs
- event log
- lifecycle state machine
- resumability
- durable tool calls

Exit condition:

kill and restart M31A during a run and resume safely.

## Phase 5 — Repository Intelligence

Deliver:

- file index
- symbol index
- imports
- calls
- test mapping
- architecture map
- search

Exit condition:

M31A can answer repository-structure questions without scanning everything from scratch.

## Phase 6 — Planning Engine

Deliver:

- intent classification
- requirements
- TaskGraph
- dependencies
- risk
- checkpoints
- plan artifacts

Exit condition:

M31A can convert a feature request into a verified executable TaskGraph.

## Phase 7 — Autonomous Execution

Deliver:

- planner agent
- implementer agent
- task scheduling
- execution waves
- retries
- repair
- replanning

Exit condition:

M31A completes a non-trivial feature in an isolated worktree.

## Phase 8 — Verification Engine

Deliver:

- acceptance criteria
- evidence binding
- tests
- static checks
- behavior checks
- verification reports
- failure routing

Exit condition:

M31A can reject an implementation that does not satisfy verification criteria.

## Phase 9 — Git Intelligence

Deliver:

- semantic diff analysis
- commit generation
- branch analysis
- history reasoning
- regression investigation
- worktree operations

Exit condition:

M31A can manage a complete feature branch safely.

## Phase 10 — Security and Policy

Deliver:

- capability policies
- path safety
- secret redaction
- dangerous Git guards
- prompt-injection defenses
- plugin trust boundaries

Exit condition:

security-focused fixture suite passes.

## Phase 11 — Adversarial Review

Deliver:

- independent reviewer
- risk analysis
- architectural review
- security review
- repair-loop integration

Exit condition:

M31A can find a deliberate seeded defect after implementation.

## Phase 12 — Advanced Intelligence

Deliver:

- causal regression analysis
- decision intelligence
- dependency intelligence
- architecture drift detection
- project learnings

Exit condition:

M31A produces useful historical/codebase explanations that require multiple evidence sources.

## Phase 13 — Plugins and MCP

Deliver:

- plugin API
- MCP adapter
- tool lifecycle
- capability scoping
- versioned external contracts

Exit condition:

third-party tooling can extend M31A without changing the core.

## Phase 14 — v1 Release Hardening

Deliver:

- compatibility guarantees
- installer
- upgrade path
- diagnostics
- documentation
- benchmark suite
- crash recovery
- security audit
- performance profiling

Exit condition:

v1 release verification passes all mandatory acceptance criteria.

---

# 69. v1 scope

M31A v1 must include:

### Core

- Go runtime
- terminal CLI/TUI
- sessions
- runs
- durable state
- event log
- local SQLite
- `.m31a/`

### LLM

- NVIDIA API provider
- `nvidia/nemotron-3-ultra-550b-a55b`
- streaming
- tool calling
- reasoning-aware request handling
- retries
- cancellation

### Tools

- filesystem read/write/edit
- grep/search
- glob
- command execution
- test execution
- Git inspection
- Git mutation with policy

### Intelligence

- repository discovery
- symbol/file indexing
- impact analysis
- Git history inspection
- architecture summary

### Planning

- intent classification
- requirements
- TaskGraph
- risk assessment
- checkpointing

### Execution

- planner
- implementer
- verifier
- retries
- repair
- resume

### Verification

- build
- unit tests
- integration tests
- structural checks
- acceptance criteria
- evidence reports

### Safety

- path safety
- permission policy
- dangerous Git protection
- secret redaction
- prompt injection defenses

---

# 70. Explicit v1 non-goals

Do not delay v1 for:

- VS Code extension
- web dashboard
- desktop app
- multi-provider parity
- distributed execution
- hosted SaaS
- team collaboration server
- cloud database
- autonomous cloud deployment
- perfect multi-language semantic analysis
- fully general-purpose sandboxing
- elaborate plugin marketplace

These may be future capabilities.

The v1 product is the local terminal engineering runtime.

---

# 71. v1 acceptance criteria

M31A v1 is not complete merely because the binary launches.

The following must be true.

## Repository understanding

- detects workspace roots safely
- handles non-Git projects
- handles multiple repositories
- indexes relevant source files
- exposes useful file/symbol search

## LLM runtime

- authenticates to NVIDIA
- streams reliably
- handles reasoning-enabled responses
- handles tool calls
- recovers from transient failures
- does not leak credentials

## Tool runtime

- validates arguments
- enforces permissions
- bounds output
- persists large output
- handles cancellation
- cleans resources

## Planning

- creates explicit TaskGraphs
- tracks requirements
- captures dependencies
- assigns risk
- creates checkpoints when needed

## Execution

- respects task dependencies
- survives process interruption
- supports retry
- supports repair
- supports replan
- never falsely marks a failed task completed

## Verification

- maps acceptance criteria to evidence
- executes configured checks
- distinguishes verified/unverified
- produces durable verification artifacts
- routes failures correctly

## Git

- understands working tree
- respects user changes
- never blindly overwrites unrelated work
- protects destructive operations
- supports isolated worktrees
- handles non-Git repositories gracefully

## UX

- responsive terminal experience
- clear progress
- understandable failures
- checkpoints are impossible to miss
- no raw internal debug noise by default

---

# 72. Non-negotiable engineering rules for agents implementing M31A

1. Read this document before modifying architecture.
2. Never silently change the project language away from Go.
3. Never embed the model provider directly into generic orchestration code.
4. Never store secrets in source, config committed to Git, or `.m31a/` artifacts.
5. Never trust model-generated paths without validation.
6. Never trust model-generated shell commands without policy evaluation.
7. Never mark work complete without evidence.
8. Never make verification mutate code silently.
9. Never allow UI code to own domain logic.
10. Never use the TUI as the only API to the runtime.
11. Never introduce global mutable state unnecessarily.
12. Every long-running operation must support cancellation.
13. Every state transition must be validated.
14. Every non-idempotent operation must have clear retry semantics.
15. Every irreversible operation must have a checkpoint unless explicitly overridden by policy.
16. Every external package must be justified.
17. Every provider-specific behavior belongs in a provider adapter.
18. Every significant run must have a durable identifier.
19. Every tool call must be associated with a run/task/session.
20. Every important engineering decision must be recoverable after context compaction.
21. Do not copy OpenCode or GSD implementation wholesale.
22. Learn principles from them; create M31A's own runtime contracts.
23. Prefer vertical end-to-end slices before horizontal expansion.
24. Fix architectural causes rather than layering patches over symptoms.
25. Avoid TODO placeholders in production code.

---

# 73. Development quality gates

Every implementation phase must finish with:

```text
format
lint
static analysis
tests
integration tests
race checks where applicable
security checks
diff review
```

At minimum:

```bash
gofmt ./...
go vet ./...
go test ./...
go test -race ./...
```

Use project-selected linters and security scanners as they are introduced.

Do not claim a quality gate passed if it was not run.

---

# 74. Performance principles

M31A must remain responsive even during expensive operations.

Use concurrency carefully for:

- repository indexing
- independent research
- independent TaskGraph waves
- tool output processing
- background persistence

Do not parallelize:

- operations with mutable shared state without coordination
- non-idempotent commands without explicit dependency analysis
- concurrent Git mutations against one worktree

The scheduler must understand dependency constraints, not just run everything concurrently.

---

# 75. Resource budgets

Every agent/run should have explicit or inherited limits for:

```text
provider time
provider output
command timeout
total execution time
parallel task count
stored tool output
retry count
repair count
replan count
```

Budget exhaustion is a state transition, not an unhandled exception.

---

# 76. Long-running agent behavior

Long-running runs need watchdog signals.

A healthy run should expose:

```text
last model activity
last tool activity
current task
current state
elapsed time
next expected transition
```

If no progress occurs within a configurable interval, M31A should detect a possible stall.

Possible responses:

```text
warn
inspect
cancel
retry
resume
replan
```

Do not leave an agent spinning indefinitely.

---

# 77. Human checkpoints

Checkpoint UX must show:

```text
WHY
WHAT WILL CHANGE
RISK
AFFECTED RESOURCES
REVERSIBILITY
EXPECTED CONSEQUENCES
```

Example:

```text
CHECKPOINT REQUIRED

Reason:
  database migration is potentially destructive

Change:
  drop legacy_sessions table

Impact:
  production database schema

Reversible:
  NO, unless backup exists

Action:
  Approve / Reject / Inspect Plan
```

A checkpoint request must be durable so the run can resume after terminal interruption.

---

# 78. Project decisions and ADRs

M31A should capture durable architecture decisions.

Decision format should include:

```text
ID
Title
Context
Options
Chosen option
Rationale
Consequences
Reversibility
Status
Date
```

Do not rely on chat history as the permanent record of architecture decisions.

---

# 79. Learnings

Completed runs should be able to contribute durable learnings.

Examples:

```text
This repository requires generation before tests.

The auth service follows a factory pattern.

Database migrations are applied through ./scripts/migrate.sh.

The project uses table-driven tests.
```

Learnings must be evidence-backed and should not become uncontrolled model folklore.

Allow human correction and invalidation.

---

# 80. Configuration philosophy

Configuration should be explicit, typed, validated, and versioned.

Possible config domains:

```toml
[llm]
provider = "nvidia"
model = "nvidia/nemotron-3-ultra-550b-a55b"
base_url = "https://integrate.api.nvidia.com/v1"
reasoning = true

[execution]
max_parallel_tasks = 2
max_retries = 2
max_repairs = 2

[verification]
require_tests = true
require_build = true

[workspace]
isolation = "auto"

[security]
allow_external_paths = false
require_checkpoint_for_destructive_git = true
```

The actual configuration schema can evolve. Do not invent dozens of options before they have a real use case.

---

# 81. Environment variables

Initial supported environment variables:

```text
NVIDIA_API_KEY
```

Optional runtime variables may later include:

```text
M31A_CONFIG
M31A_LOG_LEVEL
M31A_DATA_DIR
M31A_DEBUG
```

Environment-variable secrets must always override or complement file configuration safely without being persisted accidentally.

---

# 82. CLI diagnostics

Provide:

```bash
m31a doctor
m31a version
m31a status
m31a config show
m31a diagnostics
```

`m31a doctor` should detect:

- Go/build not relevant at runtime
- Git availability
- workspace access
- writable `.m31a/`
- SQLite health
- NVIDIA key availability
- NVIDIA endpoint reachability when requested
- model configuration
- required optional tools
- TTY capability

Never print secret contents.

---

# 83. Release model

M31A should ship as a self-contained binary.

Target platforms should include at least:

```text
Linux amd64
Linux arm64
macOS amd64
macOS arm64
Windows amd64
```

Additional platforms can follow after v1.

Release artifacts should include checksums and reproducible version metadata.

---

# 84. Licensing and third-party attribution

M31A must track dependency licenses.

Do not copy code from OpenCode or GSD into M31A unless the legal terms and applicable license obligations explicitly permit it and the copied code is actually necessary.

Preferred approach:

```text
architectural inspiration
≠
code copying
```

Maintain a third-party attribution document when necessary.

---

# 85. Research references

The project was informed by current public documentation and repositories, including:

- OpenCode repository: `https://github.com/anomalyco/opencode`
- OpenCode context/session architecture: `https://github.com/anomalyco/opencode`
- GSD Core repository: `https://github.com/open-gsd/gsd-core`
- GSD Core architecture: `https://github.com/open-gsd/gsd-core/blob/next/docs/ARCHITECTURE.md`
- GSD Core planning artifacts reference: `https://github.com/open-gsd/gsd-core/blob/next/docs/reference/planning-artifacts.md`
- GSD Core command reference: `https://github.com/open-gsd/gsd-core/blob/next/docs/COMMANDS.md`
- GSD Core context engineering: `https://github.com/open-gsd/gsd-core/blob/next/docs/explanation/context-engineering.md`
- GSD Core quickstart: `https://github.com/open-gsd/docs/blob/main/core/quickstart.mdx`
- NVIDIA model catalog: `https://build.nvidia.com/models`
- NVIDIA Nemotron 3 Ultra model page: `https://build.nvidia.com/nvidia/nemotron-3-ultra-550b-a55b`
- NVIDIA Nemotron 3 Ultra model card: `https://build.nvidia.com/nvidia/nemotron-3-ultra-550b-a55b/modelcard`
- NVIDIA Nemotron 3 Ultra API reference: `https://docs.api.nvidia.com/nim/reference/nvidia-nemotron-3-ultra-550b-a55b`
- NVIDIA LLM API reference: `https://docs.api.nvidia.com/nim/reference/llm-apis`
- NVIDIA API Catalog quickstart: `https://docs.api.nvidia.com/nim/docs/api-quickstart`
- Go documentation: `https://go.dev/`
- Go 1.25 API/release information: `https://go.dev/doc/go1.25`
- Bubble Tea documentation: `https://pkg.go.dev/github.com/charmbracelet/bubbletea`

Important: web/API behavior changes over time. Before implementing provider-specific behavior, consult the current authoritative NVIDIA documentation again.

---

# 86. Current external facts that must not be assumed forever

The following facts are current reference points, not eternal constants:

1. NVIDIA currently exposes the requested Nemotron 3 Ultra model through Build.NVIDIA.com.
2. The exact current model identifier is `nvidia/nemotron-3-ultra-550b-a55b`.
3. NVIDIA currently exposes an OpenAI-compatible endpoint at `https://integrate.api.nvidia.com/v1`.
4. The model is documented with up to 1M context.
5. The Build examples currently show streaming and reasoning configuration.
6. NVIDIA explicitly documents a coding-agent compatibility option `force_nonempty_content` for this model family.
7. Provider availability, quotas, rate limits, latency, API behavior, and model versions can change.

Therefore:

- no implementation may assume the free endpoint is unlimited;
- no test may assume network availability;
- provider integration tests must support mocks/fixtures;
- current provider behavior must be revalidated before release;
- model identity and capabilities should be configuration data where practical.

---

# 87. What M31A should ultimately become

The long-term architecture is:

```text
                         M31A
                          │
                         USER
                          │
                    NATURAL LANGUAGE
                          │
                          ▼
                   ┌─────────────┐
                   │ Intent Router│
                   └──────┬──────┘
                          │
                   ┌──────▼──────┐
                   │ Project State│
                   └──────┬──────┘
                          │
                   ┌──────▼──────┐
                   │ Code Graph  │
                   └──────┬──────┘
                          │
                   ┌──────▼──────┐
                   │   Research  │
                   └──────┬──────┘
                          │
                   ┌──────▼──────┐
                   │    Plan     │
                   └──────┬──────┘
                          │
                   ┌──────▼──────┐
                   │  Task Graph │
                   └──────┬──────┘
                          │
             ┌────────────┼────────────┐
             ▼            ▼            ▼
         Implementer   Tester      Git Agent
             │            │            │
             └────────────┼────────────┘
                          ▼
                   ┌─────────────┐
                   │ Verification│
                   └──────┬──────┘
                          │
                   ┌──────▼──────┐
                   │   Evidence  │
                   └──────┬──────┘
                          │
                   ┌──────▼──────┐
                   │ Durable Run │
                   └──────┬──────┘
                          │
                     PROJECT MEMORY
```

M31A should make the coding agent feel less like “a model operating tools” and more like “a software engineer operating a controlled engineering system.”

---

# 88. Final implementation doctrine

The project should optimize for:

```text
correctness
clarity
recoverability
security
auditability
engineering velocity
maintainability
```

in that order for high-risk behavior.

Do not optimize for:

```text
raw feature count
maximum autonomy at any cost
minimum lines of code
prompt cleverness
copying competitor behavior
```

The model can generate code quickly.

M31A's value is making the work **understood, structured, controlled, verified, and resumable**.

That is the central product idea.

---

# 89. First implementation target

The first working vertical slice should be intentionally small but end-to-end.

Target:

```text
m31a
  ↓
NVIDIA provider
  ↓
Nemotron 3 Ultra
  ↓
read/search tools
  ↓
inspect a Go repository
  ↓
produce a TaskGraph
  ↓
make one safe code change
  ↓
run tests
  ↓
verify the change
  ↓
persist the Run
```

The first slice should prove the architecture.

Do not spend the first milestone building a beautiful TUI while the runtime remains an unstructured loop.

Do not spend the first milestone building a massive plugin framework before sessions, runs, tools, state, planning, and verification are sound.

Do not optimize model prompts before the execution semantics are stable.

The first milestone should make M31A **trustworthy**, not merely impressive.

---

# 90. One-sentence product definition

> **M31A is a Go-native, terminal-first, persistent engineering agent that understands a codebase, plans work as an executable graph, safely executes it through capability-controlled tools, verifies the result with evidence, and preserves the complete engineering state for future runs.**
