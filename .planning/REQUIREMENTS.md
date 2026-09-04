# Requirements: M31A

**Defined:** 2026-08-23
**Core Value:** A persistent software-engineering agent runtime that turns natural-language intent into verified, resumable engineering work.

## v1 Requirements

Requirements for initial architectural foundation. Each maps to roadmap phases.

### Core Domain Model

- [ ] **DOMAIN-01**: System defines explicit domain types for Project, Repository, Workspace, Session, Run, Intent, Requirement, Decision, Plan, Task, TaskGraph, Agent, Tool, Capability, PermissionPolicy, Artifact, Verification, Checkpoint, Event with clear ownership boundaries
- [ ] **DOMAIN-02**: No single component owns multiple domain types — each type has one authoritative owner
- [ ] **DOMAIN-03**: Domain types are serializable to/from JSON for event storage and TUI projection
- [ ] **DOMAIN-04**: All domain mutations produce events; current state is derivable from event log

### Persistence Layer

- [ ] **PERSIST-01**: SQLite database at `.m31a/events.db` with WAL mode enabled for concurrent read/write
- [ ] **PERSIST-02**: Event store supports append-only writes, monotonic ordering, and efficient range queries by run/session/type
- [ ] **PERSIST-03**: Configuration stored in `.m31a/config.toml` with provider keys, execution policies, TUI preferences
- [ ] **PERSIST-04**: Project metadata in `.m31a/project.md` (human-readable), decisions in `.m31a/decisions/`, research in `.m31a/research/`
- [ ] **PERSIST-05**: Granular Git-tracking policy: project.md, requirements.md, roadmap.md, architecture/, decisions/, selected verification reports tracked; embeddings, caches, secrets, transient indexes ignored
- [ ] **PERSIST-06**: Migration from existing `.planning/` to `.m31a/` with zero data loss and traceability preserved
- [ ] **PERSIST-07**: Hot backup API for `.m31a/events.db` without blocking writers

### LLM Provider Abstraction

- [x] **LLM-01**: Provider-agnostic interface with `ChatCompletion`, `StreamChatCompletion`, `ListModels` methods
- [x] **LLM-02**: NVIDIA Build adapter implements interface with `nvidia/nemotron-3-ultra-550b-a55b`, streaming, reasoning (`enable_thinking`), coding-agent kwargs (`force_nonempty_content`)
- [x] **LLM-03**: Request options (temperature, top_p, max_tokens, reasoning) configurable per provider/model profile
- [x] **LLM-04**: Streaming handles empty deltas, reasoning-only chunks, content-only chunks, tool-call deltas, mixed transitions, connection recovery with retry policy
- [x] **LLM-05**: Provider credentials from environment (`NVIDIA_API_KEY`) never written to disk, never in logs, redacted in diagnostics
- [x] **LLM-06**: Model capability detection from API metadata (context length, tool calling, reasoning, streaming)

### Code Intelligence Graph

- [ ] **CODE-01**: Language-agnostic symbol graph built via Tree-sitter parsers (Go, TypeScript, Python, Rust, JavaScript, JSON, YAML, TOML, Markdown, Shell)
- [ ] **CODE-02**: LSP client integration for semantic analysis (go to definition, references, call hierarchy, type hierarchy) per language
- [ ] **CODE-03**: Graph includes files, symbols, types, functions, classes, interfaces, imports, calls, inheritance, implementations, tests, APIs, database entities, configurations, build targets, package dependencies, Git history
- [ ] **CODE-04**: Incremental indexing on file changes; full re-index on demand; progress reporting for large repos
- [ ] **CODE-05**: Impact analysis: given a symbol, returns direct callers (with file/line), indirect dependents (transitive), affected tests, risk categories (API, runtime, test)
- [ ] **CODE-06**: Architecture violation detection: forbidden imports, layer boundary crossings, circular dependencies, public API changes
- [ ] **CODE-07**: Multi-repo workspace support: per-repo indexes, cross-repo import tracking, unified query API

### Explain / Archaeology

- [x] **EXPLAIN-01**: `m31a explain "topic"` combines source, call graph, Git blame, commit history, ADRs, tests to answer why code exists
- [x] **EXPLAIN-02**: Output distinguishes evidence (citations with file:line, commit SHA) from inference (marked as inferred)
- [x] **EXPLAIN-03**: Assesses current rationale validity: "rationale still appears valid" / "likely obsolete" with confidence
- [x] **EXPLAIN-04**: `m31a investigate "why file exists"` shows introduction commit, original purpose, current consumers, removal impact

### Regression Intelligence

- [x] **REGRESS-01**: `m31a investigate "symptom after commit X"` identifies failing behavior, bisects candidate commits, inspects causal changes, constructs hypothesis, runs verification, reports root cause
- [x] **REGRESS-02**: Distinguishes "likely cause" from "verified cause" with explicit confidence labeling
- [x] **REGRESS-03**: Output includes regression-introducing commit, likely mechanism, affected components, recommended fix

### Dependency Intelligence

- [x] **DEPEND-01**: Before adding dependency, investigates registry existence, project age, recent releases, maintenance activity, source repo, license, known vulnerabilities, transitive impact, API stability, popularity
- [x] **DEPEND-02**: Package suggestions from model memory marked as unverified; authoritative registry queries required before install
- [x] **DEPEND-03**: Human checkpoint required for high-risk/ambiguous dependencies (vulnerabilities, unmaintained, license issues)
- [x] **DEPEND-04**: Dependency verdicts cached with timestamp; re-evaluated on version change or policy update

### TaskGraph IR & Execution Engine

- [ ] **TASKGRAPH-01**: TaskGraph IR contains Objectives, Requirements[], Constraints[], Decisions[], Tasks[], Dependencies[], Preconditions[], AcceptanceCriteria[], VerificationSteps[], RiskLevel, Checkpoints[]
- [ ] **TASKGRAPH-02**: Execution state machine enforces PLANNED → READY → RUNNING → WAITING → VERIFYING → COMPLETED/FAILED with validated transitions
- [ ] **TASKGRAPH-03**: Failure states support RETRY (bounded), REPAIR (patch + rerun), REPLAN (new TaskGraph), ESCALATE (human)
- [ ] **TASKGRAPH-04**: Kahn's algorithm for topological sort; wave-based parallel execution of independent tasks
- [ ] **TASKGRAPH-05**: Checkpoint gates block execution until human approval for ONE_WAY_DOOR decisions
- [ ] **TASKGRAPH-06**: Task state (objective, files, dependencies, acceptance, verification) persisted at each transition for exact resume

### Agent Contracts

- [ ] **AGENTS-01**: 12 specialized agents with explicit contracts: Supervisor, Explorer, Researcher, Architect, Planner, Implementer, Tester, Debugger, Verifier, Reviewer, SecurityAuditor, GitSpecialist, ReleaseEngineer
- [ ] **AGENTS-02**: Each agent declares requires/produces/must_not; fresh context assembled from durable state per role
- [ ] **AGENTS-03**: Agent handoff protocol includes loaded context summary (plan + N files + M constraints)
- [ ] **AGENTS-04**: No agent shares conversational history; only structured handoff artifacts pass between roles
- [ ] **AGENTS-05**: Capability scoping per agent role (e.g., Implementer: filesystem.write, shell.test, git.read; forbidden: git.push, destructive.reset)

### Tool Registry & Permissions

- [ ] **TOOLS-01**: 18 built-in tools registered with typed schemas: read_file, write_file, edit_file, search_text, glob_files, list_directory, run_command, run_tests, inspect_git, create_worktree, apply_patch, read_git_history, inspect_symbol, inspect_dependency_graph, request_checkpoint, plus 3 future slots
- [ ] **TOOLS-02**: Each tool declares name, description, input/output schemas, capabilities, risk class, resource scope, mutation class, side effects, idempotency
- [ ] **TOOLS-03**: Capability-based permissions evaluated at execution time: filesystem.write (repo scope, medium risk, deny .env/*.pem/~/.ssh), git.write (allowed: branch/commit/stage; checkpoint: push/force-push/reset), database.migrate (checkpoint required)
- [ ] **TOOLS-04**: Central `ValidateTaskFiles()` with `EvalSymlinks` on both requested path and workspace root closes path traversal/symlink escape gaps
- [ ] **TOOLS-05**: Shell injection prevention: command arguments passed as array, no shell metacharacter expansion, allow-list for built-ins
- [ ] **TOOLS-06**: Output bounding (default 64KB), secret redaction (API keys, tokens, passwords), structured error codes
- [ ] **TOOLS-07**: Dispatcher handles permissions, rate limiting, concurrency, idempotency keys

### Worktree + Git Safety

- [ ] **WORKTREE-01**: Worktree-first execution: `worktree/m31a/<run-id>` created for each run; user's main worktree stays clean
- [ ] **WORKTREE-02**: Semantic commit composer groups changes by task boundaries: feat/auth:..., test/auth:..., refactor/auth:...
- [ ] **WORKTREE-03**: Safety policies: force-push blocked, `reset --hard` requires checkpoint, rebase shared branch requires checkpoint, destructive ops require approval
- [ ] **WORKTREE-04**: Git state exposure: branch, upstream, working tree (modified/added/deleted), ahead/behind, safety policy status
- [ ] **WORKTREE-05**: Dirty worktree overlap detection shows conflict before isolated worktree creation; never silently overwrites user changes
- [ ] **WORKTREE-06**: Multi-repo workspace: per-repo worktrees, coordinated commits, atomic cross-repo operations
- [ ] **WORKTREE-07**: Regression bisect in isolation: creates temporary worktrees, runs verification, reports culprit without affecting main worktree

### Verification Engine

- [ ] **VERIFY-01**: 5 verification levels with evidence binding: L0 Structural (files exist, imports resolve, types compile), L1 Unit (unit tests pass), L2 Integration (API/DB tests), L3 Behavioral (CLI/HTTP/runtime), L4 Architectural (dependency constraints, layer boundaries, forbidden imports, API invariants), L5 Human (UAT, manual checkpoint, visual)
- [ ] **VERIFY-02**: Evidence for each check: expectation, observed result, evidence source (test name, command, file), provenance (agent, run, timestamp)
- [ ] **VERIFY-03**: All-or-nothing verdict: no verification threshold; every acceptance criterion must have evidence for VERIFIED
- [ ] **VERIFY-04**: Independent verifier agent with separate context (requirements, acceptance criteria, task graph, diff, test output, runtime behavior, invariants)
- [ ] **VERIFY-05**: Adversarial review loop: post-verification independent review finds assumptions, missed edges, race conditions, security boundary changes, untested behavior → repair plan → re-verification
- [ ] **VERIFY-06**: Proof-carrying changes: each completed task produces durable record (intent, plan, changed files, diff, commands, tests, verification evidence, risks, verdict)
- [ ] **VERIFY-07**: Requirements traceability: REQ → TaskGraph → Files → Tests → Verification → VERIFIED chain queryable
- [ ] **VERIFY-08**: File changes during verification invalidate old evidence and trigger fresh verification run

### TUI Global Shell

- [ ] **TUISHELL-01**: TopBar shows product/project/branch/run/mode/provider state with semantic status badges
- [ ] **TUISHELL-02**: NavigationRail with 10 major routes (Chat, Plan, Run, Verify, Intelligence, Git, Runs, Project, Settings, Diagnostics) collapsible <120 cols
- [ ] **TUISHELL-03**: MainViewport renders current screen; ContextPanel shows screen-aware inspector (run: task/agent/tool; plan: tasks/risk; verify: evidence; git: branch/tree)
- [ ] **TUISHELL-04**: InputBar for natural-language and explicit command entry with Ctrl+K command palette
- [ ] **TUISHELL-05**: NotificationLayer for transient feedback; ModalLayer for blocking decisions (checkpoints, confirmations)

### TUI Screens (36)

- [ ] **TUISCRN-01**: S01 Welcome/Project Discovery — current path, Git detection, M31A state, provider health, onboard/new project/quit
- [ ] **TUISCRN-02**: S02 Main Chat — conversation + structured plan/impact/run/checkpoint/verification cards
- [ ] **TUISCRN-03**: S03 Context Explorer — project map, requirements, Git state, architecture, symbol graph, active context
- [ ] **TUISCRN-04**: S04 Project Overview — milestone, phase, requirements, active runs, decisions, recent verification, health
- [ ] **TUISCRN-05**: S05 Onboarding/Repository Mapping — progressive discovery with pause/resume/restart/discard
- [ ] **TUISCRN-06**: S06 Plan Overview — objective, risk, impact, waves, requirements, checkpoints, acceptance criteria
- [ ] **TUISCRN-07**: S07 Plan Detail — task dependencies, files, actions, acceptance, verification per TaskGraph node
- [ ] **TUISCRN-08**: S08 Plan Review — requirement coverage, dependency consistency, pattern consistency, risk, one-way-door decisions, verification coverage
- [ ] **TUISCRN-09**: S09 Run Dashboard — task graph, active task, agent, tool, files, elapsed, controls (pause/cancel/diff/verify)
- [ ] **TUISCRN-10**: S10 Task Detail — execution state, current step, files, tools, retries, controls
- [ ] **TUISCRN-11**: S11 Agent Activity — specialist agents, current contract, capabilities, forbidden actions
- [ ] **TUISCRN-12**: S12 Tool Activity — structured timeline, collapsed output, policy inspection, safe rerun
- [ ] **TUISCRN-13**: S13 Checkpoint — blocking decision with risk, reversibility, options, evidence, cancel
- [ ] **TUISCRN-14**: S14 Verification Overview — overall verdict, checks, evidence count, automated check status
- [ ] **TUISCRN-15**: S15 Verification Detail — expectation, observed, evidence, command, file, provenance per check
- [ ] **TUISCRN-16**: S16 Failure/Recovery — causal failure surface with retry/repair/replan/inspect/stop options
- [ ] **TUISCRN-17**: S17 Diff Review — semantic diff, file list, change groups, symbol changes, lazy raw diff
- [ ] **TUISCRN-18**: S18 Git Overview — repo/branch/upstream/dirty-state/distance/safety policy
- [ ] **TUISCRN-19**: S19 Commit Composer — logical commits from task boundaries, cohesive grouping, tests with behavior
- [ ] **TUISCRN-20**: S20 Branch/Worktree Manager — isolated workspaces, active runs, lifecycle, safe merge
- [ ] **TUISCRN-21**: S21 Code Intelligence — packages/files/symbols/imports/tests, hotspots, architecture issues
- [ ] **TUISCRN-22**: S22 Symbol Explorer — search/browse symbols, references, callers, history, impact
- [ ] **TUISCRN-23**: S23 Impact Analysis — direct/indirect dependents, risk categories, suggested TaskGraph
- [ ] **TUISCRN-24**: S24 Dependency Explorer — package inventory, version, risk, verdict, compare alternatives
- [ ] **TUISCRN-25**: S25 Git History/Regression — commit candidates, bisect status, hypothesis, evidence
- [ ] **TUISCRN-26**: S26 Search Results — combined literal/symbol/semantic with filters
- [ ] **TUISCRN-27**: S27 Run History — paged runs with status, objective, duration, branch, risk filters
- [ ] **TUISCRN-28**: S28 Run Detail — complete lifecycle: tasks, tools, verification, Git, artifacts
- [ ] **TUISCRN-29**: S29 Project State — current phase, objective, blockers, open work, decisions, handoff
- [ ] **TUISCRN-30**: S30 Decisions — durable architecture/product decisions with rationale and status
- [ ] **TUISCRN-31**: S31 Research — questions, sources, findings, risks, recommendations, evidence classes
- [ ] **TUISCRN-32**: S32 Configuration — provider/model/execution/safety/Git/context/TUI/plugin/MCP settings
- [ ] **TUISCRN-33**: S33 Command Palette — fuzzy expert launcher with category, availability, context relevance
- [ ] **TUISCRN-34**: S34 Help/Keyboard Reference — contextual help + global keyboard reference
- [ ] **TUISCRN-35**: S35 Logs/Diagnostics — component health, structured events, stack traces, process tree, storage, provider
- [ ] **TUISCRN-36**: S36 Error Boundary — recoverable fatal UI/runtime boundary preserving run/state context

### TUI Component Library

- [ ] **TUICOMP-01**: All 26 components implemented with typed input models, deterministic render, focus behavior, empty/loading/error treatment, keyboard behavior, truncation rules, accessibility labels
- [ ] **TUICOMP-02**: Components: TopBar, NavigationRail, ContextPanel, InputBar, StatusBadge, ProgressMeter, TaskRow, Timeline, ToolRow, AgentBadge, EvidenceRow, RiskIndicator, KeyHintBar, EmptyState, ErrorState, Citation, PlanCard, RunCard, VerificationCard, CheckpointCard, DiffStats, TaskGraph, VerificationMatrix, RequirementTrace, PolicyInspector, ModelStatus, ContextMeter, SearchList, SymbolView, ImpactSummary, DependencyRow, GitStatus, CommitProposal, WorktreeRow, UATCase, Notification

### TUI Responsive Layouts

- [ ] **TUILAY-01**: <80 cols: compact single-column, hide secondary metadata
- [ ] **TUILAY-02**: 80-119 cols: primary content + compact status region
- [ ] **TUILAY-03**: 120-159 cols: navigation + main + contextual panel
- [ ] **TUILAY-04**: 160+ cols: full three-region layout with richer telemetry
- [ ] **TUILAY-05**: Layout recomputed on resize; focus and scroll preserved; modal switches to full-screen compact if needed

### TUI Keyboard Contract

- [ ] **TUIKEY-01**: Enter=open/submit/confirm, Esc=back/cancel/close, Tab=next focus, Shift+Tab=prev focus
- [ ] **TUIKEY-02**: Ctrl+K=command palette, Ctrl+L=redraw, Ctrl+C=cancel (2nd press=exit), ?=contextual help
- [ ] **TUIKEY-03**: /=local search, ↑↓=selection, j/k=list navigation
- [ ] **TUIKEY-04**: Every primary action has keyboard path; mouse support additive, never required

### TUI Runtime Event Model

- [ ] **TUIEVT-01**: Subscribes to normalized events: session/run/agent/task/tool/checkpoint/verification/git/workspace/provider
- [ ] **TUIEVT-02**: Events carry monotonic ordering/freshness; stale events never move rendered state backwards
- [ ] **TUIEVT-03**: Screen state contract: initializing/loading/ready/empty/running/paused/waiting/success/warning/error/recoverable_error/offline_or_unavailable/terminal_resized
- [ ] **TUIEVT-04**: Unknown provider state displayed as "unavailable" not empty field

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Advanced Intelligence

- [ ] **ADVINT-01**: Multi-model routing exercised (provider abstraction built in v1, single model used)
- [ ] **ADVINT-02**: MCP/plugin ecosystem with capability scoping and trust boundaries
- [ ] **ADVINT-03**: Browser/HTTP/database/container tools with policy integration
- [ ] **ADVINT-04**: Package registry/cloud provider integrations as tools
- [ ] **ADVINT-05**: Real-time collaboration via shared session/run artifacts
- [ ] **ADVINT-06**: Remote LSP pool for distributed symbol graph at scale
- [ ] **ADVINT-07**: Rust sandbox component for isolated execution

### Platform Expansion

- [ ] **PLAT-01**: VS Code extension as thin consumer of command/query API
- [ ] **PLAT-02**: Web UI for run visualization and project dashboard
- [ ] **PLAT-03**: Desktop application surfaces
- [ ] **PLAT-04**: Mobile/remote access optimizations

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| VS Code extension / web UI / desktop app | Terminal-first is core thesis; UI clients duplicate agent engine; diverts focus from runtime |
| Rust sandbox component in core | Adds split-language complexity prematurely; Go handles process isolation for v1 |
| Multi-model routing in v1 | Only one configured model (Nemotron 3 Ultra); routing abstraction built but not exercised |
| MCP/plugin ecosystem | Tool registry supports it but no MCP servers implemented; trust boundaries undefined |
| Browser/HTTP/database/container tools | Expands tool surface before core tools are solid; security surface grows |
| Package registry / cloud provider integrations | External integrations before core engineering loop works |
| Real-time collaboration | Terminal-first, single-user; multiplayer adds distributed state complexity |
| Mobile/remote access | Terminal is primary surface; remote access via SSH/tmux works natively |
| Unrestricted shell access | Security model forbids "allow shell" toggle; every command needs policy evaluation |
| Model as sole source of truth | Context windows are disposable; project state must be durable |
| Automatic blind `git add . && git commit` | Destroys semantic commit boundaries; mixes unrelated changes |
| Verification theater (superficial check → "done") | Undermines trust; core differentiator is evidence-backed verification |
| Hidden chain-of-thought as user transcript | Provider-private reasoning must not be exposed as diagnostics |
| Global mutable state in TUI | Bubble Tea is single-threaded; shared mutable state from goroutines breaks |
| `/` as workspace fallback root | Known failure class in coding agents; exposes unrelated machine files |
| Prose summaries for recovery-critical information | Never rely on model summary for decisions/tasks/risks/plan/verification |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| DOMAIN-01 | Phase 1 | Pending |
| DOMAIN-02 | Phase 1 | Pending |
| DOMAIN-03 | Phase 1 | Pending |
| DOMAIN-04 | Phase 1 | Pending |
| PERSIST-01 | Phase 1 | Pending |
| PERSIST-02 | Phase 1 | Pending |
| PERSIST-03 | Phase 1 | Pending |
| PERSIST-04 | Phase 1 | Pending |
| PERSIST-05 | Phase 1 | Pending |
| PERSIST-06 | Phase 1 | Pending |
| PERSIST-07 | Phase 1 | Pending |
| LLM-01 | Phase 2 | Complete |
| LLM-02 | Phase 2 | Complete |
| LLM-03 | Phase 2 | Complete |
| LLM-04 | Phase 2 | Complete |
| LLM-05 | Phase 2 | Complete |
| LLM-06 | Phase 2 | Complete |
| CODE-01 | Phase 3 | Pending |
| CODE-02 | Phase 3 | Pending |
| CODE-03 | Phase 3 | Pending |
| CODE-04 | Phase 3 | Pending |
| CODE-05 | Phase 3 | Pending |
| CODE-06 | Phase 3 | Pending |
| CODE-07 | Phase 3 | Pending |
| EXPLAIN-01 | Phase 4 | Complete |
| EXPLAIN-02 | Phase 4 | Complete |
| EXPLAIN-03 | Phase 4 | Complete |
| EXPLAIN-04 | Phase 4 | Complete |
| REGRESS-01 | Phase 4 | Complete |
| REGRESS-02 | Phase 4 | Complete |
| REGRESS-03 | Phase 4 | Complete |
| DEPEND-01 | Phase 4 | Complete |
| DEPEND-02 | Phase 4 | Complete |
| DEPEND-03 | Phase 4 | Complete |
| DEPEND-04 | Phase 4 | Complete |
| TASKGRAPH-01 | Phase 5 | Pending |
| TASKGRAPH-02 | Phase 5 | Pending |
| TASKGRAPH-03 | Phase 5 | Pending |
| TASKGRAPH-04 | Phase 5 | Pending |
| TASKGRAPH-05 | Phase 5 | Pending |
| TASKGRAPH-06 | Phase 5 | Pending |
| AGENTS-01 | Phase 6 | Pending |
| AGENTS-02 | Phase 6 | Pending |
| AGENTS-03 | Phase 6 | Pending |
| AGENTS-04 | Phase 6 | Pending |
| AGENTS-05 | Phase 6 | Pending |
| TOOLS-01 | Phase 7 | Pending |
| TOOLS-02 | Phase 7 | Pending |
| TOOLS-03 | Phase 7 | Pending |
| TOOLS-04 | Phase 7 | Pending |
| TOOLS-05 | Phase 7 | Pending |
| TOOLS-06 | Phase 7 | Pending |
| TOOLS-07 | Phase 7 | Pending |
| WORKTREE-01 | Phase 8 | Pending |
| WORKTREE-02 | Phase 8 | Pending |
| WORKTREE-03 | Phase 8 | Pending |
| WORKTREE-04 | Phase 8 | Pending |
| WORKTREE-05 | Phase 8 | Pending |
| WORKTREE-06 | Phase 8 | Pending |
| WORKTREE-07 | Phase 8 | Pending |
| VERIFY-01 | Phase 9 | Pending |
| VERIFY-02 | Phase 9 | Pending |
| VERIFY-03 | Phase 9 | Pending |
| VERIFY-04 | Phase 9 | Pending |
| VERIFY-05 | Phase 9 | Pending |
| VERIFY-06 | Phase 9 | Pending |
| VERIFY-07 | Phase 9 | Pending |
| VERIFY-08 | Phase 9 | Pending |
| TUISHELL-01 | Phase 10 | Pending |
| TUISHELL-02 | Phase 10 | Pending |
| TUISHELL-03 | Phase 10 | Pending |
| TUISHELL-04 | Phase 10 | Pending |
| TUISHELL-05 | Phase 10 | Pending |
| TUICOMP-01 | Phase 10 | Pending |
| TUICOMP-02 | Phase 10 | Pending |
| TUILAY-01 | Phase 10 | Pending |
| TUILAY-02 | Phase 10 | Pending |
| TUILAY-03 | Phase 10 | Pending |
| TUILAY-04 | Phase 10 | Pending |
| TUILAY-05 | Phase 10 | Pending |
| TUIKEY-01 | Phase 10 | Pending |
| TUIKEY-02 | Phase 10 | Pending |
| TUIKEY-03 | Phase 10 | Pending |
| TUIKEY-04 | Phase 10 | Pending |
| TUIEVT-01 | Phase 10 | Pending |
| TUIEVT-02 | Phase 10 | Pending |
| TUIEVT-03 | Phase 10 | Pending |
| TUIEVT-04 | Phase 10 | Pending |
| TUISCRN-01 | Phase 11 | Pending |
| TUISCRN-02 | Phase 11 | Pending |
| TUISCRN-03 | Phase 11 | Pending |
| TUISCRN-04 | Phase 11 | Pending |
| TUISCRN-05 | Phase 11 | Pending |
| TUISCRN-06 | Phase 11 | Pending |
| TUISCRN-07 | Phase 11 | Pending |
| TUISCRN-08 | Phase 11 | Pending |
| TUISCRN-09 | Phase 11 | Pending |
| TUISCRN-10 | Phase 11 | Pending |
| TUISCRN-11 | Phase 11 | Pending |
| TUISCRN-12 | Phase 11 | Pending |
| TUISCRN-13 | Phase 11 | Pending |
| TUISCRN-14 | Phase 11 | Pending |
| TUISCRN-15 | Phase 11 | Pending |
| TUISCRN-16 | Phase 11 | Pending |
| TUISCRN-17 | Phase 11 | Pending |
| TUISCRN-18 | Phase 11 | Pending |
| TUISCRN-19 | Phase 11 | Pending |
| TUISCRN-20 | Phase 11 | Pending |
| TUISCRN-21 | Phase 11 | Pending |
| TUISCRN-22 | Phase 11 | Pending |
| TUISCRN-23 | Phase 11 | Pending |
| TUISCRN-24 | Phase 11 | Pending |
| TUISCRN-25 | Phase 11 | Pending |
| TUISCRN-26 | Phase 11 | Pending |
| TUISCRN-27 | Phase 11 | Pending |
| TUISCRN-28 | Phase 11 | Pending |
| TUISCRN-29 | Phase 11 | Pending |
| TUISCRN-30 | Phase 11 | Pending |
| TUISCRN-31 | Phase 11 | Pending |
| TUISCRN-32 | Phase 11 | Pending |
| TUISCRN-33 | Phase 11 | Pending |
| TUISCRN-34 | Phase 11 | Pending |
| TUISCRN-35 | Phase 11 | Pending |
| TUISCRN-36 | Phase 11 | Pending |

**Coverage:**

- v1 requirements: 124 total
- Mapped to phases: 124
- Unmapped: 0 ✓

---

*Requirements defined: 2026-08-23*
*Last updated: 2026-08-23 after roadmap creation (12 phases, 100% coverage)*
