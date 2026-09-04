# Roadmap: M31A

**Project:** M31A  
**Core Value:** A persistent software-engineering agent runtime that turns natural-language intent into verified, resumable engineering work.  
**Granularity:** fine (12 phases)  
**Mode:** yolo  
**Created:** 2026-08-23  

---

## Phases

- [ ] **Phase 1: Foundation — Domain Model & Event Store** - Canonical domain types and SQLite event store as single source of truth
- [x] **Phase 2: LLM Provider Abstraction** - Provider-agnostic interface with NVIDIA Build adapter, streaming, and reasoning support (completed 2026-08-24)
- [x] **Phase 3: Code Intelligence Graph** - Language-agnostic symbol graph via Tree-sitter + LSP with incremental indexing (completed 2026-08-25)
- [ ] **Phase 4: Intelligence Features** - Explain/archaeology, regression intelligence, dependency intelligence
- [ ] **Phase 5: TaskGraph IR & Execution Engine** - Executable plan IR with state machine, Kahn's scheduling, wave execution
- [ ] **Phase 6: Agent Contracts & Runtime** - 12 specialized agents with explicit contracts and fresh-context handoffs
- [ ] **Phase 7: Tool Registry & Permissions** - 18 typed tools, capability-based permissions, execution-time evaluation
- [ ] **Phase 8: Worktree & Git Safety** - Worktree-first execution, semantic commits, safety policies, bisect in isolation
- [ ] **Phase 9: Verification Engine** - 5 verification levels + Human UAT, evidence-backed verdicts, adversarial review
- [ ] **Phase 10: TUI Foundation** - Global shell, 26 components, 4 responsive layouts, keyboard contract, event model
- [ ] **Phase 11: TUI Screens** - All 36 screens (S01–S36) per UI-SPEC.md implementing mandatory user journeys
- [ ] **Phase 12: Migration & Cutover** - Incremental ownership transfer, end-to-end validation, obsolete code removal

---

## Phase Details

### Phase 1: Foundation — Domain Model & Event Store

**Goal**: System has canonical domain types and an authoritative SQLite event store; all durable state derives from events; migration from `.planning/` to `.m31a/` completes with zero data loss.  
**Depends on**: Nothing (first phase)  
**Requirements**: DOMAIN-01, DOMAIN-02, DOMAIN-03, DOMAIN-04, PERSIST-01, PERSIST-02, PERSIST-03, PERSIST-04, PERSIST-05, PERSIST-06, PERSIST-07  
**Success Criteria** (what must be TRUE):

  1. User runs `m31a init` in a new repository and `.m31a/` directory is created with `events.db`, `config.toml`, `project.md`
  2. User runs `m31a migrate` and all existing `.planning/` data (requirements, roadmap, decisions, research) appears in `.m31a/` with traceability preserved
  3. User creates a session, adds a requirement, makes a decision — all produce append-only events in `events.db` queryable by run/session/type
  4. User restarts `m31a` and previous session state (requirements, decisions, runs) is fully reconstructed from event log
  5. User configures NVIDIA API key via `m31a config set provider.key` and it is stored in OS keychain, never written to disk

**Plans**: 5/6 plans executed
Plans:

- [x] 01-01-PLAN.md — Domain types (18 types), event envelope, EventStore interface, serialization
- [x] 01-02-PLAN.md — Config system: EventStore/Migration sections, layered loading, keychain integration
- [x] 01-03-PLAN.md — Event store core: schema, WAL mode, transactional append, range queries
- [x] 01-04-PLAN.md — Event subscription (TUI real-time), hot backup (non-blocking)
- [x] 01-05-PLAN.md — Projection manager with checkpoints, artifact writers (project.md, decisions/, research/), .gitignore
- [ ] 01-06-PLAN.md — Migration engine: parse .planning/, emit events, rebuild projections, archive .planning/, CLI command

### Phase 2: LLM Provider Abstraction

**Goal**: Provider-agnostic interface with NVIDIA Build adapter supporting streaming, reasoning, and coding-agent kwargs; credentials never touch disk or logs.  
**Depends on**: Phase 1  
**Requirements**: LLM-01, LLM-02, LLM-03, LLM-04, LLM-05, LLM-06  
**Success Criteria** (what must be TRUE):

  1. User runs `m31a chat "hello"` and receives streamed response from NVIDIA Nemotron 3 Ultra with reasoning chunks visible in TUI
  2. User sets `NVIDIA_API_KEY` in environment and it is read at runtime, never persisted to `.m31a/config.toml` or logs
  3. User switches provider config (temperature, max_tokens, reasoning) and subsequent requests use new parameters
  4. Network interruption during streaming triggers automatic retry with exponential backoff and resumes from last chunk
  5. User runs `m31a models list` and sees dynamically discovered models from NVIDIA API with capability metadata (context length, tool calling, reasoning)

**Plans**: 6 plans
Plans:

- [x] 02-00-PLAN.md — Wave 0 test infrastructure (7 test files for all verify commands)
- [x] 02-01-PLAN.md — Core interface, types, NVIDIA ultra config, tracer end-to-end streaming path
- [x] 02-02-PLAN.md — Model profiles config, layered loading, BaseClient profile merging
- [x] 02-03-PLAN.md — Streaming resilience (empty deltas, retry strategies), capability detection API enrichment
- [x] 02-04-PLAN.md — Provider selection (GetProviderForRequest, FallbackMode), CLI models list command
- [x] 02-05-PLAN.md — Credential security verification, Wave 0 test infrastructure

### Phase 3: Code Intelligence Graph

**Goal**: Language-agnostic symbol graph built via Tree-sitter + LSP covering 10 languages; incremental indexing on file changes; impact analysis and architecture violation detection operational.  
**Depends on**: Phase 1  
**Requirements**: CODE-01, CODE-02, CODE-03, CODE-04, CODE-05, CODE-06, CODE-07  
**Success Criteria** (what must be TRUE):

  1. User runs `m31a index` in a Go/TypeScript/Python/Rust repository and symbol graph is built with files, symbols, types, functions, classes, interfaces, imports, calls, inheritance, implementations, tests, APIs, DB entities, configs, build targets, package dependencies, Git history
  2. User edits a file and runs `m31a index --incremental` — only changed files are re-parsed, progress reported for large repos
  3. User runs `m31a impact "funcName"` and receives direct callers (file:line), indirect dependents (transitive), affected tests, risk categories (API, runtime, test)
  4. User runs `m31a arch check` and architecture violations are reported: forbidden imports, layer boundary crossings, circular dependencies, public API changes
  5. User opens a multi-repo workspace and `m31a index` produces per-repo indexes with cross-repo import tracking and unified query API

**Plans**: 4/4 plans complete
Plans:
**Wave 1**

- [x] 03-01-PLAN.md — Core tracer: EventStore events, CodeGraph edges, projection rebuild

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 03-02-PLAN.md — LSP client infrastructure: JSON-RPC, connection pool, lifecycle manager
- [x] 03-03-PLAN.md — Impact analysis and incremental file watching

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 03-04-PLAN.md — Architecture violation detection, multi-repo workspace, CLI commands

### Phase 4: Intelligence Features

**Goal**: Explain/archaeology, regression intelligence, and dependency intelligence commands operational with evidence-backed outputs distinguishing citations from inference.  
**Depends on**: Phase 1, Phase 3  
**Requirements**: EXPLAIN-01, EXPLAIN-02, EXPLAIN-03, EXPLAIN-04, REGRESS-01, REGRESS-02, REGRESS-03, DEPEND-01, DEPEND-02, DEPEND-03, DEPEND-04  
**Success Criteria** (what must be TRUE):

  1. User runs `m31a explain "auth middleware"` and receives answer combining source, call graph, Git blame, commit history, ADRs, tests with evidence citations (file:line, commit SHA) and inference marked explicitly
  2. User runs `m31a explain "why file exists"` and sees introduction commit, original purpose, current consumers, removal impact assessment
  4. User runs `m31a investigate "panic after commit abc123"` and system bisects candidates, inspects causal changes, constructs hypothesis, runs verification, reports root cause with confidence labeling ("likely cause" vs "verified cause")
  5. User runs `m31a deps check github.com/new/dep` and receives registry existence, project age, releases, maintenance, license, vulnerabilities, transitive impact, API stability, popularity — with human checkpoint required for high-risk findings

**Plans**: 7/7 plans executed
Plans:
**Wave 1**

- [x] 04-01-PLAN.md — Shared vocabulary: Confidence enum, EvidencePack/Citation types, intelligence events, [intelligence] config with defaults

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 04-02-PLAN.md — Tracer: git blame porcelain parser + explain symbol-mode end-to-end (collector, single LLM synthesis, marker validation, render, CLI)
- [x] 04-06-PLAN.md — Deps registry clients (deps.dev/OSV/GitHub) + config-driven risk classification (D-13, D-16)

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 04-03-PLAN.md — Explain expansion: file/topic modes, rationale-validity signals (D-08), ADR scanning, disambiguation
- [x] 04-04-PLAN.md — Investigate engine core: temp-worktree isolation (D-10), repro resolution chain (D-09), window-bounded bisect (D-11)

**Wave 4** *(blocked on Wave 3 completion)*

- [x] 04-05-PLAN.md — Investigate attribution + two-tier report (D-12) + events + CLI wiring

**Wave 5** *(blocked on Wave 4 completion)*

- [x] 04-07-PLAN.md — Deps verdict assembly, event-backed cache (D-15), human checkpoint gate (D-14), CLI wiring

### Phase 5: TaskGraph IR & Execution Engine

**Goal**: TaskGraph IR with full schema (objectives, requirements, constraints, decisions, tasks, dependencies, preconditions, acceptance, verification, risk, checkpoints) executable via validated state machine with wave-based parallel execution.  
**Depends on**: Phase 1  
**Requirements**: TASKGRAPH-01, TASKGRAPH-02, TASKGRAPH-03, TASKGRAPH-04, TASKGRAPH-05, TASKGRAPH-06  
**Success Criteria** (what must be TRUE):

  1. User creates a plan via `m31a plan "add auth"` and resulting TaskGraph contains all IR fields with valid topological order (Kahn's algorithm)
  2. User runs `m31a execute` and tasks progress through PLANNED → READY → RUNNING → WAITING → VERIFYING → COMPLETED/FAILED with validated transitions only
  3. User encounters a task failure and system offers RETRY (bounded), REPAIR (patch + rerun), REPLAN (new TaskGraph), ESCALATE (human) — each produces correct subsequent state
  4. User runs parallel execution and independent tasks in same wave execute concurrently with correct dependency ordering
  5. User hits a ONE_WAY_DOOR checkpoint and execution blocks until explicit human approval via TUI; checkpoint gate records decision in event log

**Plans**: TBD

### Phase 6: Agent Contracts & Runtime

**Goal**: 12 specialized agents (Supervisor, Explorer, Researcher, Architect, Planner, Implementer, Tester, Debugger, Verifier, Reviewer, SecurityAuditor, GitSpecialist, ReleaseEngineer) with explicit requires/produces/must_not contracts, fresh-context assembly, and structured handoff protocol.  
**Depends on**: Phase 1, Phase 2, Phase 5  
**Requirements**: AGENTS-01, AGENTS-02, AGENTS-03, AGENTS-04, AGENTS-05  
**Success Criteria** (what must be TRUE):

  1. User runs a multi-step plan and observes Supervisor → Planner → Implementer → Tester → Verifier handoff sequence with each agent receiving fresh context (plan + N files + M constraints) from durable state
  2. User inspects agent activity and sees each agent declares requires/produces/must_not capabilities (e.g., Implementer: filesystem.write, shell.test, git.read; forbidden: git.push, destructive.reset)
  3. User triggers agent handoff and handoff artifact includes loaded context summary with no conversational history shared between roles
  4. User runs in headless mode and agents execute without TUI coupling — fresh context assembled from EventStore projections only
  5. User reviews proof-carrying change record after task completion and sees intent, plan, changed files, diff, commands, tests, verification evidence, risks, verdict durably stored

**Plans**: TBD

### Phase 7: Tool Registry & Permissions

**Goal**: 18 built-in tools registered with typed schemas; capability-based permissions evaluated at execution time with scopes, risk classes, deny-lists; shell injection prevented; path traversal/symlink escapes closed.  
**Depends on**: Phase 1, Phase 5  
**Requirements**: TOOLS-01, TOOLS-02, TOOLS-03, TOOLS-04, TOOLS-05, TOOLS-06, TOOLS-07  
**Success Criteria** (what must be TRUE):

  1. User runs `m31a tools list` and sees 18 tools with name, description, input/output schemas, capabilities, risk class, resource scope, mutation class, side effects, idempotency
  2. User attempts `write_file` outside workspace root and operation is denied with structured error code; `ValidateTaskFiles()` with `EvalSymlinks` on both paths prevents escape
  3. User runs shell command with `${VAR}` or `$((...))` and `mvdan.cc/sh/v3` parser rejects it — only allow-listed built-ins execute with arguments passed as array
  4. User executes tool producing >64KB output and output is bounded with secret redaction (API keys, tokens, passwords) applied automatically
  5. User runs concurrent tasks and dispatcher enforces permissions, rate limiting, concurrency limits, idempotency keys per tool

**Plans**: TBD

### Phase 8: Worktree & Git Safety

**Goal**: Worktree-first execution isolates every run; semantic commit composition groups changes by task; safety policies block force-push, require checkpoint for destructive ops; bisect runs in isolated worktrees.  
**Depends on**: Phase 1, Phase 7  
**Requirements**: WORKTREE-01, WORKTREE-02, WORKTREE-03, WORKTREE-04, WORKTREE-05, WORKTREE-06, WORKTREE-07  
**Success Criteria** (what must be TRUE):

  1. User runs `m31a execute` and `worktree/m31a/<run-id>` is created; user's main worktree remains clean with no staged/unstaged changes from agent
  2. User completes a run with multiple tasks and `m31a git commit` produces semantic commits grouped by task boundaries: `feat/auth:...`, `test/auth:...`, `refactor/auth:...`
  3. User attempts `git push --force` in agent worktree and operation is blocked; `reset --hard` requires explicit checkpoint approval
  4. User has uncommitted changes in main worktree and `m31a execute` detects overlap, shows conflict, and refuses to silently overwrite
  5. User runs `m31a investigate "regression"` and regression bisect creates temporary worktrees, runs verification, reports culprit commit without affecting main worktree

**Plans**: TBD

### Phase 9: Verification Engine

**Goal**: 5 verification levels (Structural, Unit, Integration, Behavioral, Architectural) + Human UAT with evidence-backed all-or-nothing verdicts; independent verifier with adversarial review loop; proof-carrying changes; requirements traceability chain queryable.  
**Depends on**: Phase 1, Phase 5, Phase 6  
**Requirements**: VERIFY-01, VERIFY-02, VERIFY-03, VERIFY-04, VERIFY-05, VERIFY-06, VERIFY-07, VERIFY-08  
**Success Criteria** (what must be TRUE):

  1. User runs `m31a verify` on completed task and sees L0–L4 checks with expectation, observed result, evidence source (test name, command, file), provenance (agent, run, timestamp)
  2. User sees verification verdict: VERIFIED only when ALL acceptance criteria have evidence (no threshold); FAILED when any criterion lacks evidence
  3. User triggers adversarial review and independent verifier agent (separate context) finds assumptions, missed edges, race conditions, security boundary changes, untested behavior — produces repair plan
  4. User runs `m31a trace REQ-123` and sees complete chain: REQ → TaskGraph → Files → Tests → Verification → VERIFIED
  5. User modifies a file after verification and system invalidates old evidence, triggers fresh verification run automatically

**Plans**: TBD

### Phase 10: TUI Foundation

**Goal**: Bubble Tea TUI with global shell (TopBar, NavigationRail, MainViewport, ContextPanel, InputBar, NotificationLayer, ModalLayer), 26 typed components, 4 responsive layout tiers, universal keyboard contract, event subscription model — all as pure projection consuming normalized events.  
**Depends on**: Phase 1  
**Requirements**: TUISHELL-01, TUISHELL-02, TUISHELL-03, TUISHELL-04, TUISHELL-05, TUICOMP-01, TUICOMP-02, TUILAY-01, TUILAY-02, TUILAY-03, TUILAY-04, TUILAY-05, TUIKEY-01, TUIKEY-02, TUIKEY-03, TUIKEY-04, TUIEVT-01, TUIEVT-02, TUIEVT-03, TUIEVT-04  
**Success Criteria** (what must be TRUE):

  1. User launches `m31a` and sees TopBar with product/project/branch/run/mode/provider state with semantic status badges
  2. User navigates 10 major routes via NavigationRail (Chat, Plan, Run, Verify, Intelligence, Git, Runs, Project, Settings, Diagnostics) — collapses at <120 cols
  3. User resizes terminal from 80 to 160+ cols and layout recomputes through 4 tiers (single-column → primary+status → nav+main+panel → full three-region) with focus/scroll preserved
  4. User presses `?` and sees contextual help; `Ctrl+K` opens command palette; `Esc` goes back/cancels; `Enter` confirms; `Tab`/`Shift+Tab` cycles focus — mouse never required
  5. User runs headless `m31a --headless` and TUI event subscription receives normalized events (session.*, run.*, agent.*, task.*, tool.*, checkpoint.*, verification.*, git.*, workspace.*, provider.*) with monotonic ordering — no domain logic in UI

**UI hint**: yes
**Plans**: TBD

### Phase 11: TUI Screens

**Goal**: All 36 screens (S01–S36) per UI-SPEC.md implemented with mandatory user journeys: onboarding → chat → plan → run → verify → git → intelligence → project state.  
**Depends on**: Phase 10  
**Requirements**: TUISCRN-01, TUISCRN-02, TUISCRN-03, TUISCRN-04, TUISCRN-05, TUISCRN-06, TUISCRN-07, TUISCRN-08, TUISCRN-09, TUISCRN-10, TUISCRN-11, TUISCRN-12, TUISCRN-13, TUISCRN-14, TUISCRN-15, TUISCRN-16, TUISCRN-17, TUISCRN-18, TUISCRN-19, TUISCRN-20, TUISCRN-21, TUISCRN-22, TUISCRN-23, TUISCRN-24, TUISCRN-25, TUISCRN-26, TUISCRN-27, TUISCRN-28, TUISCRN-29, TUISCRN-30, TUISCRN-31, TUISCRN-32, TUISCRN-33, TUISCRN-34, TUISCRN-35, TUISCRN-36  
**Success Criteria** (what must be TRUE):

  1. User launches `m31a` in new directory and sees S01 Welcome/Project Discovery with Git detection, provider health, onboard/new project/quit options
  2. User enters natural language in S02 Main Chat and sees structured plan/impact/run/checkpoint/verification cards rendered inline
  3. User navigates S06 Plan Overview → S07 Plan Detail → S08 Plan Review and sees objective, risk, impact, waves, requirements, checkpoints, acceptance criteria, requirement coverage, dependency consistency, pattern consistency, one-way-door decisions, verification coverage
  4. User runs execution and sees S09 Run Dashboard with task graph, active task, agent, tool, files, elapsed, controls (pause/cancel/diff/verify) — S10 Task Detail shows execution state, current step, files, tools, retries
  5. User hits checkpoint and sees S13 Checkpoint with risk, reversibility, options, evidence, cancel; verification shows S14 Overview + S15 Detail with expectation/observed/evidence/command/file/provenance per check
  6. User views S18 Git Overview → S19 Commit Composer → S20 Branch/Worktree Manager with semantic commits, isolated workspaces, safe merge
  7. User explores S21 Code Intelligence → S22 Symbol Explorer → S23 Impact Analysis → S24 Dependency Explorer with packages, symbols, references, callers, risk categories, suggested TaskGraph
  8. User investigates regression via S25 Git History/Regression with commit candidates, bisect status, hypothesis, evidence
  9. User accesses S27 Run History → S28 Run Detail with complete lifecycle: tasks, tools, verification, Git, artifacts
  10. User manages project via S29 Project State → S30 Decisions → S31 Research → S32 Configuration → S33 Command Palette → S34 Help → S35 Logs/Diagnostics → S36 Error Boundary (recovers preserving run/state context)

**UI hint**: yes
**Plans**: TBD

### Phase 12: Migration & Cutover

**Goal**: Incremental ownership transfer from legacy architecture to canonical six-plane runtime; all consumers switched to new interfaces; 14 P0/P1 bugs fixed; obsolete code (god-object Engine, WorkflowState, legacy checkpoint/recovery, flat-file authority) removed; cross-compilation verified.  
**Depends on**: Phase 1–11  
**Requirements**: (Integration phase — validates all prior requirements end-to-end)  
**Success Criteria** (what must be TRUE):

  1. User runs `m31a migrate --replay` and all `.m31a/` sessions replay into EventStore with zero data loss; legacy `.planning/` authority fully retired
  2. User executes full workflow: `m31a plan "..."` → `m31a execute` → `m31a verify` → `m31a ship` with TUI consuming new runtime API (no legacy code paths)
  3. User triggers crash mid-execution, restarts `m31a`, and `m31a resume` reconstructs exact task states, heal counts, acceptance results from checkpoints
  4. User runs `make cross` and `goreleaser --dry-run` — static binaries produce for linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64
  5. User runs headless with `--permission-mode=auto|deny|allow` and permission policies (InteractivePolicy, HeadlessDenyPolicy, HeadlessAllowPolicy, CIPolicy) evaluate correctly without TUI coupling

**Plans**: TBD

---

## Progress Table

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation — Domain Model & Event Store | 5/6 | In Progress|  |
| 2. LLM Provider Abstraction | 6/6 | Complete    | 2026-08-24 |
| 3. Code Intelligence Graph | 4/4 | Complete   | 2026-08-25 |
| 4. Intelligence Features | 7/7 | In Progress|  |
| 5. TaskGraph IR & Execution Engine | 0/0 | Not started | - |
| 6. Agent Contracts & Runtime | 0/0 | Not started | - |
| 7. Tool Registry & Permissions | 0/0 | Not started | - |
| 8. Worktree & Git Safety | 0/0 | Not started | - |
| 9. Verification Engine | 0/0 | Not started | - |
| 10. TUI Foundation | 0/0 | Not started | - |
| 11. TUI Screens | 0/0 | Not started | - |
| 12. Migration & Cutover | 0/0 | Not started | - |

---
