# Architecture Patterns

**Domain:** Persistent agentic coding runtime (M31A)
**Researched:** 2026-08-23

## Recommended Architecture: Six-Plane Model

```
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

### Plane Responsibilities

| Plane | Core Responsibility | Key Components |
|-------|---------------------|----------------|
| **Interaction** | Terminal I/O, TUI, CLI, command parsing, progress, checkpoints | Bubble Tea AppState, Screen Router, 36 Screens (S01–S36), 26 Components, InputBar, NotificationLayer |
| **Intelligence** | LLM providers, model routing, context assembly, repository intelligence, symbol search, code graph, research, impact analysis | Provider Registry (OpenRouter/Zen/NVIDIA), ContextEngine, CodeIntel (Tree-sitter + LSP), ImpactAnalyzer, ResearchEngine |
| **Engineering** | Requirements, plans, TaskGraph IR, project state, architecture state, decisions, constraints, work breakdown | Project, Requirement, Plan, TaskGraph, Decision, Constraint, Objective |
| **Execution** | Agent runtime, tool execution, shell, filesystem, Git, worktrees, MCP, plugins, workspace isolation | AgentRuntime (12 agents), ToolDispatcher (18 built-ins), GitClient, WorktreeManager, CapabilityPolicyEngine |
| **Assurance** | Tests, static analysis, build checks, security, impact validation, verification, adversarial review, UAT | VerificationEngine (5 levels), AdversarialReviewer, TestRunner, StaticAnalyzer, CheckpointGate |
| **Memory** | Session history, runs, event log, durable state, project artifacts, decisions, learnings, evidence, telemetry | EventStore (SQLite), ProjectArtifacts (.m31a/), SessionManager, RecoveryService, DecisionLog |

---

## Component Boundaries

### Interaction Plane ↔ Runtime (Application Command/Query API)

```
Bubble Tea TUI
      │
      ▼
Application Command / Query API
      │
      ▼
M31A Runtime
 ├─ Sessions          → SessionManager
 ├─ Runs              → RunService
 ├─ Planning          → PlanService
 ├─ Agents            → AgentRuntime
 ├─ Tools             → ToolDispatcher
 ├─ Policy            → CapabilityPolicyEngine
 ├─ Git/Workspaces    → GitClient, WorktreeManager
 ├─ Intelligence      → CodeIntel, ImpactAnalyzer
 ├─ Verification      → VerificationEngine
 └─ Persistence       → EventStore, ArtifactStore
      │
      ▼
Events + Query Projections
```

**Boundary Rules:**
- TUI owns **only** transient presentation state: focus, selected rows, open overlays, scroll offsets, local draft input, terminal dimensions, cached view projections
- TUI **never** owns domain truth (no workflow state, no task state, no agent state, no Git state)
- All domain mutations flow through Application Commands; all reads flow through Query Projections
- Communication: `tea.Cmd` / `tea.Msg` only — no direct struct sharing

### Intelligence Plane ↔ Engineering Plane

```
ContextEngine (assembles context)
      │
      ├─► ProjectState (Engineering)
      ├─► RepositoryGraph (Intelligence)
      ├─► Requirements (Engineering)
      ├─► ResearchArtifacts (Intelligence)
      ├─► Active Plan/TaskGraph (Engineering)
      ├─► Relevant Symbols/Files (Intelligence)
      └─► Decisions/Constraints (Engineering)
      │
      ▼
Specialized Agent Contexts (fresh per agent)
```

**Boundary Rules:**
- Intelligence provides *read-only* projections (symbol graph, impact analysis, dependencies)
- Engineering owns *authoritative* mutable state (requirements, plans, decisions)
- ContextEngine is the **only** component that assembles cross-plane context for agents
- Agents receive *fresh, minimal* context — never the full conversation history

### Execution Plane ↔ Assurance Plane

```
AgentRuntime executes TaskGraph
      │
      ├─► ToolDispatcher (filesystem, shell, git, search)
      ├─► WorktreeManager (isolated workspace)
      └─► CapabilityPolicyEngine (permission checks at execution time)
      │
      ▼
VerificationEngine validates results
      │
      ├─► Level 0: Structural (types, schemas, imports)
      ├─► Level 1: Unit (unit tests)
      ├─► Level 2: Integration (API, DB, contract tests)
      ├─► Level 3: Behavioral (CLI, HTTP, state transitions)
      ├─► Level 4: Architectural (layer boundaries, forbidden imports)
      └─► Level 5: Human (UAT, manual checkpoint, visual)
      │
      ▼
Evidence-backed verdict → TaskGraph state transition
```

**Boundary Rules:**
- Execution **never** mutates verification state
- Verification **never** mutates source code (repair → explicit replan/execute transition)
- CapabilityPolicyEngine evaluates permissions **at execution time**, not at tool registration
- WorktreeManager isolates execution; user's main worktree remains clean until integration

### Memory Plane (Event Sourcing Backbone)

```
All Planes → EventStore (SQLite: events.db)
      │
      ├─► Append-only events (ProjectInitialized, RunCreated, TaskCreated, ToolCallCompleted, ...)
      ├─► Current state derivable from events (CQRS read models)
      ├─► Recovery: replay events to reconstruct Run state
      ├─► Audit: complete trace of every decision and action
      └─► Projections: TUI queries read from materialized views
```

**Boundary Rules:**
- Events are the **single source of truth** for durable state
- Flat files (`.m31a/project.md`, `requirements.md`, `decisions/`, `plans/`) are **projections/artifacts**, not authority
- SQLite provides ACID, embedded, no separate server — ideal for event sourcing
- Never store secrets/API keys in events or `.m31a/`

---

## Data Flow

### Primary Request Flow (Interactive TUI)

```
1. User Input (S02 Main Chat InputBar)
       │
       ▼
2. AppState.Update() → parses intent, creates RunPhaseCmd
       │
       ▼
3. tea.Cmd runs Engine.RunPhase() in background goroutine
       │
       ▼
4. Engine.PrePhaseSetup() → validates transition, persists recovery
       │
       ▼
5. PromptBuilder + ContextBuilder → assembles LLM context
       │          (ProjectState + RepositoryGraph + Requirements + Plan + Relevant Symbols)
       │
       ▼
6. Provider.ChatCompletionStream() → StreamIterator
       │
       ▼
7. Tool calls → Dispatcher.Execute() → Tool implementations
       │         (permission check at execution time via CapabilityPolicyEngine)
       │
       ▼
8. Results emitted as tea.Msg via MsgEmitter channel
       │
       ▼
9. AppState.Update() receives messages → updates presentation state → re-renders
       │
       ▼
10. Engine.PostPhaseExecution() → clears recovery on success, emits PhaseResultMsg
```

### Headless Mode (`--prompt` / `--goal`)

```
CLI Parse → runHeadless()/runHeadlessWorkflow() → Creates Engine (no TUI)
       │
       ▼
Same Engine.RunPhase() sequence, but:
- MsgEmitter writes to stdout/stderr instead of TUI channel
- Permission mode: --permission-mode=auto|deny|allow (default: deny for dangerous tools)
- No streaming UI; final result printed as JSON or markdown
```

### Session Persistence & Recovery

```
On Phase Transition:
  Engine.persistRecovery() → writes CheckpointData to .m31a/recovery.json
       (includes: phase, goal, plan, messages, checkpoint, sessionID)

On Shutdown:
  AppState.Shutdown() → SessionManager.SaveSession()
       (session.json, messages.json, tasks, workflow state)

On Resume (startup with recovery.json):
  Manager.LoadRecoveryBytes() → Engine.Recover()
       → validates sessionID matches
       → validates phase has corresponding tasks
       → restores TaskRunner with task states (Status, HealsAttempted)
       → resumes from exact interruption point
```

### Event Flow (Memory Plane)

```
Domain Action (e.g., TaskCompleted)
       │
       ▼
EventStore.Append(Event{Type: "TaskCompleted", Payload: ..., Timestamp, RunID, CausationID})
       │
       ├─► Projection: TaskView (for TUI Run Dashboard)
       ├─► Projection: RunSummary (for S27 Run History)
       ├─► Projection: VerificationEvidence (for S14/S15 Verification screens)
       ├─► Projection: DecisionLog (for S30 Decisions)
       └─► Projection: CodeIntelIndex (symbol graph updates)
       │
       ▼
ArtifactStore writes human-readable projections:
  .m31a/project.md, requirements.md, roadmap.md
  .m31a/decisions/0001-*.md
  .m31a/plans/<run-id>.json
  .m31a/runs/<run-id>/
  .m31a/verification/<run-id>.json
```

---

## Build Order & Dependencies

### Phase 1: Foundation (No Dependencies)
```
1. Core Domain Model (internal/core/types)
   ├─ Project, Repository, Workspace, Session, Run
   ├─ Intent, Requirement, Decision, Plan, Task, TaskGraph
   ├─ Agent, Tool, Capability, PermissionPolicy
   ├─ Artifact, Verification, Checkpoint, Event
   └─ WorkflowPhase, RiskLevel, TaskStatus enums

2. Event Store (SQLite)
   ├─ events.db schema (append-only, indexed by run_id, timestamp, causation_id)
   ├─ EventStore interface + SQLite implementation
   ├─ Projection engine (materialized views for TUI queries)
   └─ Migration from current .m31a/ flat files

3. Configuration & Keychain
   ├─ Multi-layer config loader (TOML, JSON, env, keychain)
   ├─ OS keychain integration (macOS security, Windows Credential Manager, libsecret)
   └─ Provider credential resolution (never plaintext)
```

### Phase 2: Intelligence & Engineering Planes (Depends on Phase 1)
```
4. LLM Provider Abstraction
   ├─ LLMProvider interface (ChatCompletionStream, FetchModels, HealthCheck)
   ├─ NVIDIA Build adapter (nemotron-3-ultra-550b-a55b, chat_template_kwargs)
   ├─ OpenRouter/Zen adapters (for future)
   ├─ Model metadata + capabilities from API (not heuristics)
   └─ Streaming: empty deltas, reasoning deltas, tool-call deltas, mixed transitions

5. Context Engine
   ├─ ContextEpoch model (baseline + projected history + sources + tool outputs)
   ├─ Context compaction (incremental, preserves: tasks, plan, decisions, verification, active files)
   ├─ Per-agent context assembly (Planner ≠ Implementer ≠ Verifier ≠ Reviewer)
   └─ Token estimation with safety margin (90% of provider context_window)

6. Repository Intelligence (CodeIntel)
   ├─ Language-agnostic symbol graph (Tree-sitter + LSP orchestration)
   ├─ Files, symbols, types, functions, classes, interfaces, imports, calls, inheritance, impls, tests, APIs, DB entities, configs, build targets, package deps, Git history
   ├─ ImpactAnalyzer (direct/indirect dependents, risk categories)
   └─ Architecture violation detection (layer boundaries, forbidden imports)
```

### Phase 3: Execution & Assurance Planes (Depends on Phases 1-2)
```
7. Capability-Based Permission System
   ├─ Capability registry (filesystem.read/write, git.read/write, shell, database.migrate)
   ├─ Scope: repository, workspace, project, global
   ├─ Risk class: safe, medium, dangerous, critical
   ├─ Conditions: deny patterns (.env, *.pem, ~/.ssh/*), require checkpoint for destructive ops
   ├─ PolicyEngine evaluates at EXECUTION TIME (not registration)
   └─ Headless mode: --permission-mode flag

8. Tool Dispatcher & Built-in Tools
   ├─ Typed Tool interface (Name, Description, InputSchema, OutputSchema, Capabilities, RiskClass, ResourceScope, MutationClass, SideEffects, Idempotency)
   ├─ 18 built-in tools: read_file, write_file, edit_file, search_text, glob_files, list_directory, run_command, run_tests, inspect_git, create_worktree, apply_patch, read_git_history, inspect_symbol, inspect_dependency_graph, request_checkpoint, ...
   ├─ Dispatcher: permission check, rate limiting (golang.org/x/time/rate), concurrency (semaphore)
   ├─ Path validation: ResolveAndContainPath + EvalSymlinks on BOTH sides (central ValidateTaskFiles)
   └─ Shell injection prevention: mvdan.cc/sh/v3 syntax parser (complete metacharacter detection)

9. Agent Runtime & Contracts
   ├─ 12 agents: Supervisor, Explorer, Researcher, Architect, Planner, Implementer, Tester, Debugger, Verifier, Reviewer, SecurityAuditor, GitSpecialist, ReleaseEngineer
   ├─ Explicit contracts: requires[], produces[], must_not[]
   ├─ Fresh context per agent (no shared conversational context)
   ├─ AgentRuntime orchestrates: spawn → assemble context → execute → handoff
   └─ Supervisor coordinates: dependency resolution, wave scheduling, checkpoint gates

10. TaskGraph IR & Execution Engine
    ├─ TaskGraph: Objectives, Requirements[], Constraints[], Decisions[], Tasks[], Dependencies[], Preconditions[], AcceptanceCriteria[], VerificationSteps[], RiskLevel, Checkpoints[]
    ├─ Execution State Machine: PLANNED→READY→RUNNING→WAITING→VERIFYING→COMPLETED/FAILED
    ├─ Failure transitions: FAILED→retry/repair/replan/escalate; VERIFYING→FAILED→diagnose/patch/rerun
    ├─ TaskRunner: Kahn's algorithm (topological sort), parallel execution with semaphore, retry logic
    ├─ Wave-based execution: independent tasks in parallel, dependent tasks sequential
    └─ Checkpoint gates: one-way-door decisions require human approval (persisted before presentation)

11. Verification Engine
    ├─ 5 Levels + Human UAT (Structural → Unit → Integration → Behavioral → Architectural → Human)
    ├─ Evidence-backed verdicts (expectation, observed, evidence source, command, file, task, agent provenance)
    ├─ Adversarial Review loop: Implementation → Verification → Adversarial Review → Repair Plan → Execution → Verification
    ├─ Independent model/context for verifier when practical
    ├─ Verification invalidation: if files change during verification, old evidence discarded
    └─ Proof-carrying changes: every task produces durable change record (intent, plan, diff, commands, tests, evidence, risks, verdict)
```

### Phase 4: Interaction Plane & Integration (Depends on Phases 1-3)
```
12. TUI Foundation (per UI-SPEC.md)
    ├─ Global Shell: TopBar, NavigationRail, MainViewport, ContextPanel, InputBar, NotificationLayer, ModalLayer
    ├─ 36 Screens (S01–S36): Welcome, Chat, Context Explorer, Project Overview, Onboarding, Plan Overview/Detail/Review, Run Dashboard, Task Detail, Agent/Tool Activity, Checkpoint, Verification Overview/Detail, Failure/Recovery, Diff Review, Git Overview, Commit Composer, Branch/Worktree Manager, Code Intelligence, Symbol Explorer, Impact Analysis, Dependency Explorer, Regression, Search, Run History/Detail, Project State, Decisions, Research, Configuration, Command Palette, Help, Diagnostics, Error Boundary
    ├─ 26 Components: TopBar, NavigationRail, ContextPanel, InputBar, StatusBadge, ProgressMeter, TaskRow, Timeline, ToolRow, AgentBadge, EvidenceRow, RiskIndicator, KeyHintBar, EmptyState, ErrorState, Citation, PlanCard, RunCard, VerificationCard, CheckpointCard, DiffStats, TaskGraph, VerificationMatrix, RequirementTrace, PolicyInspector, ModelStatus, ContextMeter, SearchList, SymbolView, ImpactSummary, DependencyRow, GitStatus, CommitProposal, WorktreeRow, UATCase, Notification
    ├─ Responsive: <80 compact, 80-119 primary+status, 120-159 three-region, 160+ full telemetry
    ├─ Keyboard contract: Enter/Escape/Tab/Ctrl+K/Ctrl+L/Ctrl+C/?//j/k
    ├─ Event subscription: session.*, run.*, agent.*, task.*, tool.*, checkpoint.*, verification.*, git.*, workspace.*, provider.*
    └─ TUI as projection ONLY — no domain mutations in Update()

13. Git Intelligence & Workspace Safety
    ├─ GitClient: status, diff, commit, branch, worktree, blame, history, bisect
    ├─ Worktree-first execution: create worktree → analyze → plan → execute → test → verify → diff → apply/merge after approval
    ├─ Semantic commit composition: logical groups from task boundaries
    ├─ Safety policies: force-push blocked, reset --hard requires checkpoint, destructive ops blocked
    ├─ Rollback: tracks agent-modified files only (git checkout <commit> -- <files>), preserves user changes
    ├─ Bisect: runs in isolated worktree (never modifies user repo bisect state)
    └─ Multi-repo workspace support (first-class)
```

### Phase 5: Migration & Cutover (Depends on All Above)
```
14. Migration Strategy
    ├─ Audit current implementation (CONCERNS.md: 14 P0/P1 bugs, 4 fragile areas)
    ├─ Identify reusable code (provider adapters, git client, keychain, tree-sitter, token estimation, theme system, components)
    ├─ Establish target domain model as authoritative (EngineeringRun, TaskGraph, EventStore)
    ├─ Migrate/rewrite subsystems in dependency order (Phase 1→2→3→4)
    ├─ Integrate real runtime (replace mock TUI bridges with real Application API)
    ├─ Replace old state authority (session.json/messages.json → SQLite events.db + projections)
    ├─ Verify: run full workflow end-to-end, all P0/P1 bugs fixed, recovery works
    └─ Remove obsolete architecture (god-object Engine, WorkflowState, legacy checkpoint/recovery)
```

---

## Migration Strategy: From Current to Canonical

### Current Architecture Problems (from CONCERNS.md)

| Problem | Current State | Target State |
|---------|---------------|--------------|
| **Multiple Sources of Truth** | Engine (30+ fields), WorkflowState, SessionManager, Checkpoint, TUI AppState all hold overlapping state | Single EventStore (SQLite) as authority; all else are projections |
| **God-Object Engine** | `Engine` holds workflow, state, LLM, tools, Git, metrics, hooks, code-intel, compaction, session, recovery | Domain services: TaskScheduler, AgentRuntime, ContextEngine, ModelGateway, ToolRuntime, VerificationService, RecoveryService |
| **State Machine Bypass** | `SetPhase()` skips validation; checkpoint restore corrupts state | `SetPhaseWithValidation(from, to)`; sessionID in recovery; task consistency check |
| **Path Containment Gaps** | Git commit/verify use raw paths; symlink escape in Bash | Central `ValidateTaskFiles()` at task creation + before every use; `EvalSymlinks` on both paths |
| **Verification Threshold** | 90% pass = success (P0 bug) | All tasks must pass; explicit `allow_partial_verify` flag with TUI confirmation |
| **Headless Permission Bypass** | Blocks forever or fails silently | `--permission-mode=auto|deny|allow` with dangerous-tool defaults |
| **Recovery Incomplete** | Restores phase but not task states; no sessionID validation | Full task state restoration; sessionID validation; plan/task consistency check |
| **Rollback Unsafe** | Stashes ALL user changes; bisect leaves repo in bisect mode | Track agent-modified files; selective revert; bisect in isolated worktree |

### Reusable Code Inventory (Keep & Adapt)

| Component | Current Location | Migration Action |
|-----------|------------------|------------------|
| NVIDIA Provider Adapter | `internal/integrations/provider/nvidia/client.go` | **Keep** — implements `LLMProvider` interface; add capability metadata from API |
| OpenRouter/Zen Adapters | `internal/integrations/provider/{openrouter,zen}/client.go` | **Keep** — same interface |
| Git Client | `internal/integrations/git/git.go` | **Adapt** — add `CommitWithFiles` enforcement, worktree support, agent-file tracking |
| Keychain | `internal/integrations/keychain/keychain.go` | **Keep** — OS-native credential storage |
| Tree-sitter Integration | `internal/integrations/codeintel/` | **Adapt** — integrate into CodeIntel service with LSP orchestration |
| Token Estimation | `internal/engine/tokens/estimator.go` | **Adapt** — use provider `context_window`, add 90% safety margin, incremental compaction |
| Theme System | `internal/ui/tui/theme/` | **Keep** — 26 semantic tokens, responsive, accessible |
| 26 UI Components | `internal/ui/tui/components/` | **Keep** — typed input models, deterministic render, keyboard contracts |
| TaskRunner (Kahn's) | `internal/engine/taskrunner/runner.go` | **Adapt** — integrate with TaskGraph IR, checkpoint gates, wave scheduling |
| Bisect/Rollback | `internal/engine/{bisect,rollback}/` | **Rewrite** — worktree isolation, agent-file tracking, crash-safe |
| Extensions Protocol | `pkg/extensions/protocol.go` | **Keep** — JSON-RPC subprocess for MCP/plugins |

### Code to Remove/Replace

| Component | Reason | Replacement |
|-----------|--------|-------------|
| `internal/engine/workflow/engine.go` (30+ fields) | God object, mixed responsibilities | Domain services + Application API |
| `internal/engine/workflow/workflow_state.go` | Separate mutable state with mutex hierarchy | EventStore projections |
| `internal/engine/session/manager.go` (session.json, messages.json) | Flat-file authority, slow at scale, no event sourcing | SQLite EventStore + ArtifactStore projections |
| `internal/engine/workflow/engine_checkpoint.go` | Partial checkpoint (no tasks), no sessionID | Full CheckpointData in EventStore |
| `internal/engine/workflow/recovery.go` | Loads phase without validation | RecoveryService with sessionID + task consistency |
| `internal/ui/tui/app.go` (AppState with domain fields) | Owns domain truth (workflowPhase, run, tasks, agents) | Pure presentation model; domain via Query API |
| `internal/tools/dispatcher.go` (permission at registration) | Permission checked at tool presentation, not execution | CapabilityPolicyEngine at execution time |
| `internal/tools/exec/bash.go` (incomplete shell validation) | Regex misses `${VAR}`, `$((...))`, etc. | mvdan.cc/sh/v3 parser or complete deny-list |

### Migration Execution Order

```
WEEK 1-2: Foundation
  □ Domain types (internal/core/types) — new canonical types
  □ EventStore (SQLite) — schema, append, projections
  □ Config/Keychain — verify current works, add provider-agnostic profile

WEEK 3-4: Intelligence Plane
  □ LLMProvider interface + NVIDIA adapter — extract from current, add capabilities
  □ ContextEngine — fresh context per agent, incremental compaction
  □ CodeIntel — Tree-sitter + LSP orchestration, symbol graph, impact analysis

WEEK 5-6: Engineering + Execution Planes
  □ CapabilityPolicyEngine — central permission evaluation at execution time
  □ ToolDispatcher + 18 tools — migrate to new Tool interface, central path validation
  □ AgentRuntime + 12 contracts — fresh context, explicit requires/produces/must_not
  □ TaskGraph IR + Execution State Machine — typed, validated transitions
  □ TaskRunner — integrate with TaskGraph, waves, checkpoints

WEEK 7-8: Assurance Plane
  □ VerificationEngine — 5 levels + Human, evidence-backed, adversarial review
  □ Proof-carrying changes — durable task records

WEEK 9-10: Interaction Plane
  □ TUI Foundation — 36 screens, 26 components, global shell, responsive
  □ Application Command/Query API — TUI talks to runtime via this boundary only
  □ Event subscription — TUI reads from projections, not Engine fields

WEEK 11-12: Git + Migration Cutover
  □ GitClient + WorktreeManager — worktree-first, semantic commits, safety policies
  □ Migration harness — replay current .m31a/ sessions into EventStore
  □ Cutover — switch TUI to new runtime, verify end-to-end, remove old code
```

### Verification Gates (Per Migration Week)

| Gate | Criteria |
|------|----------|
| **Foundation** | Domain types compile; EventStore persists/appends/replays; config loads from keychain |
| **Intelligence** | NVIDIA provider streams with reasoning; ContextEngine assembles per-agent context; CodeIntel indexes test repo |
| **Engineering/Execution** | ToolDispatcher executes with permission checks; AgentRuntime runs Planner→Implementer handoff; TaskGraph executes waves with checkpoints |
| **Assurance** | VerificationEngine runs all 5 levels on sample task; adversarial review finds injected bug; proof-carrying record produced |
| **Interaction** | All 36 screens render; responsive at 80/120/160 cols; keyboard contract complete; TUI receives events from projections |
| **Cutover** | Full workflow: intent → plan → execute → verify → ship; recovery from crash mid-execute; headless mode with permissions; all P0/P1 bugs fixed |

---

## Patterns to Follow

### Pattern 1: Event-Sourced Domain Model
**What:** All durable state changes are append-only events; current state derived via projections.
**When:** Any domain object that must survive process crash, terminal close, or context loss.
**Example:**
```go
// Domain action produces event
func (s *RunService) CompleteTask(runID, taskID string, result TaskResult) error {
    evt := Event{
        Type:        "TaskCompleted",
        RunID:       runID,
        CausationID: taskID,
        Payload:     TaskCompletedPayload{TaskID: taskID, Result: result},
        Timestamp:   time.Now(),
    }
    return s.eventStore.Append(evt)
}

// Projection reads for TUI
func (p *TaskProjection) GetTaskView(runID string) ([]TaskView, error) {
    events, _ := p.eventStore.Query("TaskCreated|TaskStarted|TaskCompleted|TaskFailed", runID)
    return p.materialize(events), nil
}
```

### Pattern 2: Fresh Context Per Agent
**What:** Specialized agents receive minimal, relevant context assembled from durable state — not shared conversation history.
**When:** Spawning any of the 12 agent roles.
**Example:**
```go
func (a *AgentRuntime) AssembleContext(agent AgentRole, runID string) AgentContext {
    base := a.contextEngine.GetBaseline(runID) // project, requirements, architecture, decisions
    switch agent {
    case Planner:
        return base.Add(a.requirements.Get(runID)).
                   Add(a.research.GetArtifacts(runID)).
                   Add(a.codeIntel.GetRelevantSymbols(runID, "planning")).
                   Add(a.plan.GetCurrent(runID))
    case Implementer:
        return base.Add(a.taskGraph.GetCurrentTask(runID)).
                   Add(a.codeIntel.GetFilesForTask(runID)).
                   Add(a.verification.GetAcceptanceCriteria(runID)).
                   Add(a.git.GetDiff(runID))
    case Verifier:
        return base.Add(a.requirements.Get(runID)).
                   Add(a.taskGraph.Get(runID)).
                   Add(a.git.GetDiff(runID)).
                   Add(a.testRunner.GetResults(runID)).
                   Add(a.verification.GetEvidence(runID))
    }
}
```

### Pattern 3: Capability-Based Permission at Execution Time
**What:** Permissions evaluated when tool executes, not when presented to model.
**When:** Every tool invocation.
**Example:**
```go
func (d *Dispatcher) Execute(ctx context.Context, call ToolCall) ToolResult {
    tool := d.registry.Get(call.Name)
    caps := tool.Capabilities()
    
    for _, cap := range caps {
        decision := d.policyEngine.Evaluate(PermissionRequest{
            Capability: cap.Name,
            Scope:      cap.Scope,
            Risk:       cap.RiskClass,
            Resource:   call.Input.ExtractResource(),
            Agent:      call.AgentID,
        })
        if decision == Deny {
            return ToolResult{Error: "permission denied: " + cap.Name}
        }
        if decision == Ask && !d.headlessAutoApprove {
            return d.requestHumanApproval(call, cap)
        }
    }
    return tool.Execute(ctx, call.Input)
}
```

### Pattern 4: Worktree-First Execution
**What:** Substantial changes execute in isolated worktree; user's main worktree clean until integration.
**When:** Any Run with risk ≥ MEDIUM or >3 files modified.
**Example:**
```go
func (e *ExecutionEngine) ExecuteInWorktree(runID string, taskGraph TaskGraph) error {
    wt := e.worktreeManager.Create(fmt.Sprintf("m31a/%s", runID))
    defer func() {
        if !e.policy.AllowAutoMerge(runID) {
            wt.LeaveIntact() // user reviews diff in TUI
        }
    }()
    
    // All file ops, git ops, shell commands scoped to worktree
    ctx := WithWorktree(context.Background(), wt.Path)
    return e.taskRunner.Run(ctx, taskGraph)
}
```

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: God Object Engine
**What:** Single `Engine` struct holds workflow orchestration, state management, LLM prompting, tool dispatch, Git, metrics, hooks, code-intel, compaction, context registry, session persistence, recovery, pause/cancel channels.
**Why bad:** Violates single responsibility; coupling makes testing difficult; changes cascade; 30+ fields with mixed concerns.
**Instead:** Extract domain services with explicit interfaces: `TaskScheduler`, `AgentRuntime`, `ContextEngine`, `ModelGateway`, `ToolRuntime`, `VerificationService`, `RecoveryService`. Engine becomes thin orchestrator.

### Anti-Pattern 2: TUI Owning Domain State
**What:** `AppState` contains `workflowPhase`, `currentRun`, `tasks`, `agents`, `plan`, `messages` — mutates them directly.
**Why bad:** UI becomes source of truth; recovery impossible; headless mode diverges; testing requires full TUI.
**Instead:** TUI holds only presentation state (focus, scroll, selections). Domain state in EventStore; TUI queries projections via Application API.

### Anti-Pattern 3: Permission at Registration Time
**What:** Tools ask permission when registered/presented to LLM; headless mode blocks on unbuffered channel.
**Why bad:** Context changes between presentation and execution (files modified, Git state changed); headless broken; TOCTOU.
**Instead:** `CapabilityPolicyEngine.Evaluate()` at `Dispatcher.Execute()` time with current resource state.

### Anti-Pattern 4: Flat-File Session Authority
**What:** `session.json`, `messages.json`, `recovery.json` are authoritative; rewritten on every phase transition.
**Why bad:** Slow at scale; no ACID; no event sourcing; recovery partial (misses task states); cross-workflow corruption.
**Instead:** SQLite EventStore as authority; flat files as human-readable projections/artifacts only.

### Anti-Pattern 5: Unvalidated State Machine Transitions
**What:** `SetPhase(newPhase)` directly assigns without checking `validTransitions[from]`; checkpoint restore calls this.
**Why bad:** Corrupted recovery file can jump to `PhaseShip` from `PhaseIdle`; bypasses all safety gates.
**Instead:** `SetPhaseWithValidation(expectedFrom, newPhase)`; store `fromPhase` in checkpoint; validate on restore.

---

## Scalability Considerations

| Concern | At 1 Project | At 10 Projects | At 100 Projects |
|---------|--------------|----------------|-----------------|
| **EventStore** | Single SQLite per project (`.m31a/events.db`) | Per-project SQLite; global index optional | Per-project SQLite; consider centralized event log for analytics |
| **CodeIntel** | In-process Tree-sitter + LSP per project | Language servers pooled; symbol graph cached per project | Remote LSP pool; distributed symbol graph (future) |
| **TUI** | Single process | Single process per terminal | Unchanged (terminal-per-developer) |
| **Provider** | NVIDIA Build single model | Same; routing abstraction ready | Multi-model routing by task type (abstraction built) |
| **Tools** | 18 built-in, local execution | Same; MCP/plugin subprocesses isolated | MCP servers per project; plugin sandbox (future Rust) |
| **Sessions** | `.m31a/` per project | Per-project isolation maintained | Same; global config at `~/.m31a/config.toml` |

---

## Sources

- CONTEXT_M31A.md (Canonical architecture: §§7-34 — Six planes, Domain model, Event model, Execution state machine, Agent contracts, TaskGraph IR, Verification model, Capability/permission model, Workspace/Git safety, TUI/runtime boundary, NVIDIA Build integration)
- UI-SPEC.md (36-screen TUI specification, component library, interaction model, event model exposed to TUI)
- .planning/codebase/ARCHITECTURE.md (Current implementation analysis: layers, data flow, abstractions, anti-patterns)
- .planning/codebase/CONCERNS.md (14 P0/P1 bugs, 4 fragile areas, security considerations, migration approach)
- .planning/codebase/STRUCTURE.md (Directory layout, package purposes, dependency rules)
- .planning/codebase/STACK.md (Technology stack: Go 1.26.5, Bubble Tea, SQLite, NVIDIA Build)
- .planning/codebase/CONVENTIONS.md (Code style, architectural patterns, anti-patterns)
- AGENTS.md (Build requirements, workflow engine, provider layer, tools, dependency rules)