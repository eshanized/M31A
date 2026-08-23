# Project Research Summary

**Project:** M31A
**Domain:** Persistent agentic coding runtime (terminal-first)
**Researched:** 2026-08-23
**Confidence:** HIGH

## Executive Summary

M31A is a terminal-first, persistent software-engineering agent runtime designed for serious engineering work — not chat assistance. It executes multi-step engineering workflows (plan → implement → verify → ship) with durable state, evidence-backed verification, and human control over irreversible actions. Research confirms the product requires a **six-plane architecture** (Interaction, Intelligence, Engineering, Execution, Assurance, Memory) with **event sourcing** as the backbone, **TaskGraph IR** as the executable plan representation, and **worktree-first execution** for workspace safety.

The recommended approach: build the canonical architecture from scratch via a **12-week incremental migration** that replaces the current god-object engine with domain services, establishes SQLite event store as the single source of truth, and implements capability-based permissions evaluated at execution time. The current codebase has 14 confirmed P0/P1 bugs (distributed state ownership, Git safety failures, verification threshold allowing broken code, headless permission bypass, incomplete recovery) that necessitate this architectural reset.

Key risks and mitigations: **Distributed state ownership** → single EventStore authority with projections; **Git operations destroying user work** → worktree isolation + scoped CommitWithFiles only; **Verification theater** → all-or-nothing evidence-backed verdicts with adversarial review; **TUI as state owner** → pure projection architecture subscribing to normalized events; **Permission system broken in headless** → context-aware policy interface with `--permission-mode` flag.

## Key Findings

### Recommended Stack

The stack is opinionated around **static binaries (CGO_ENABLED=0)**, **pure Go dependencies**, and **terminal-native** tooling:

**Core technologies:**
- **Go 1.26+**: Primary language — static binaries, cross-compilation, excellent for concurrent agent runtime
- **Bubble Tea v2 (charm.land/bubbletea/v2)**: TUI framework — declarative View() API, 36-screen spec support, Elm architecture
- **Lip Gloss v2 (charm.land/lipgloss/v2)**: Terminal styling — semantic tokens, composable styles
- **Bubbles v0.18+**: TUI components — text input, viewport, table, list, spinner, progress (26-component library)
- **Zone v0.1+**: Mouse interaction — zone tracking for clickable UI elements
- **modernc.org/sqlite**: Embedded SQLite — pure Go, WAL mode, transaction hooks, hot backups, serialization
- **go-openai (sashabaranov/go-openai)**: LLM client — OpenAI-compatible, streaming, NVIDIA Build API, Azure config
- **go-tree-sitter**: Multi-language parsing — incremental, Tree.Edit, embedded languages, streaming
- **go-lsp (sourcegraph/go-lsp)**: LSP client — semantic analysis, go-to-definition, references, hover
- **go-git v5**: Pure Go Git — worktree support, submodule safety, billy filesystem, no CGO
- **koanf v2**: Configuration — layered providers (TOML/JSON/YAML/env/flags), no global state
- **kong**: CLI parsing — declarative struct tags, subcommands, completion, type-safe
- **slog (stdlib)**: Structured logging — zero dependencies, standard library since Go 1.21
- **Custom event bus + SQLite**: Event sourcing — append-only events, monotonic ordering, replay/recovery
- **Custom permission engine**: Capability-based — typed registry, scopes, risk classes, conditions, execution-time evaluation

### Expected Features

**Must have (table stakes):**
- Natural-language intent entry — primary interaction model
- Repository understanding/onboarding — language-agnostic, incremental, multi-repo
- File read/write/edit/search tools — path validation, workspace containment
- Shell command execution — timeouts, output bounding, cancellation, secret redaction
- Git inspection (status, diff, log, blame, branches) — multi-repo, worktree awareness
- Session persistence & resume — SQLite event sourcing, exact resume reconstruction
- Task/plan visualization — TaskGraph rendering, wave execution, risk indicators
- Checkpoint/approval for risky actions — durable checkpoint state, risk/reversibility display
- Verification with evidence — 5 levels (Structural→Architectural→Human), evidence binding
- Failure recovery (retry/repair/replan) — bounded retries, failure taxonomy, escalation
- Multi-project workspace support — root detection, per-repo Git ops, file→repo mapping
- Responsive terminal UX (80–160+ cols) — 4 layout tiers, sidebar collapse, virtualized lists
- Keyboard-first navigation — universal Esc=back, Ctrl+K palette, ? help
- Provider streaming with reasoning — empty deltas, reasoning chunks, connection recovery
- Capability-based permissions — filesystem/Git/shell/database scopes, risk classes, deny-lists
- Project memory — human-readable `.m31a/` artifacts, evidence-class tagging

**Should have (competitive):**
- Six-plane architecture — clean separation enabling independent evolution
- TaskGraph IR — executable plan prevents "nice plan, different execution" divergence
- Impact analysis — direct/indirect dependents, risk categories, suggests TaskGraph from impact
- Code intelligence graph — symbols, calls, inheritance, tests, APIs, DB, configs, Git history
- Explain/archaeology — source + call graph + Git blame + ADRs + tests → *why*, not just *what*
- Regression intelligence (bisect + causal analysis) — reproduces, hypothesizes, verifies root cause
- Semantic commit composition — logical commits from task boundaries, conventional commits
- Worktree-first isolated execution — user's main worktree clean until verification + approval
- Adversarial review loop — independent reviewer context/model, repair→reverify
- Proof-carrying changes — durable records with intent, plan, diff, tests, evidence, risks, verdict
- Context epochs & compaction — preserves decisions/tasks/risks, discards conversational noise
- Fresh-context specialized agents — 12 roles with contracts, handoff protocol
- Decision intelligence — reversibility, blast radius, migration, contract, risk → ONE_WAY_DOOR/SAFE
- Dependency intelligence — registry lookup, vulns, license, transitive impact, API stability
- Requirements traceability — REQ → TaskGraph → Files → Tests → Verification → VERIFIED
- Event-driven architecture — 35+ event types, monotonic ordering, derivable state
- TUI as pure projection — replaceable UI, headless-testable runtime, no domain logic in UI
- Vertical slice delivery — tracer principle validates full stack before horizontal expansion

**Defer (v2+):**
- VS Code extension / web UI / desktop app — terminal-first is core thesis
- Rust sandbox component — Go handles process isolation for v1
- Multi-model routing — abstraction built but single model (Nemotron 3 Ultra) exercised
- MCP/plugin ecosystem — plugin API designed, adapters later
- Browser/HTTP/database/container tools — core tools solid first
- Package registry / cloud integrations — research engine handles ad-hoc
- Real-time collaboration — single-user terminal; session artifacts shareable via Git
- Mobile/remote access — SSH/tmux works natively

### Architecture Approach

**Six-plane model** with strict component boundaries and event sourcing as the backbone:

**Major components:**
1. **Interaction Plane** — Bubble Tea TUI (36 screens, 26 components), CLI, command parsing; owns ONLY presentation state (focus, scroll, selections); subscribes to normalized application events
2. **Intelligence Plane** — LLM providers (NVIDIA/OpenRouter/Zen), ContextEngine, CodeIntel (Tree-sitter + LSP), ImpactAnalyzer, ResearchEngine; provides read-only projections to Engineering
3. **Engineering Plane** — Requirements, Plans, TaskGraph IR, Decisions, Constraints, Objectives; owns authoritative mutable state; ContextEngine assembles cross-plane context
4. **Execution Plane** — AgentRuntime (12 agents), ToolDispatcher (18 tools), GitClient, WorktreeManager, CapabilityPolicyEngine; executes TaskGraph in isolated worktrees
5. **Assurance Plane** — VerificationEngine (5 levels + Human UAT), AdversarialReviewer, TestRunner, StaticAnalyzer, CheckpointGate; evidence-backed verdicts, independent verifier
6. **Memory Plane** — EventStore (SQLite events.db), ProjectArtifacts (`.m31a/`), SessionManager, RecoveryService, DecisionLog; events are single source of truth, flat files are projections

**Critical patterns:**
- **Event-sourced domain model** — all durable changes append events; state derived via projections
- **Fresh context per agent** — specialized agents receive minimal relevant context from durable state
- **Capability permissions at execution time** — evaluated when tool runs, not when presented to model
- **Worktree-first execution** — isolated worktrees for risky changes; user's main worktree clean until approval
- **TaskGraph as executable IR** — strongly typed with objectives, requirements, constraints, tasks, dependencies, preconditions, acceptance criteria, verification steps, risk levels, checkpoints

### Critical Pitfalls

1. **Distributed State Ownership (Split-Brain)** — Multiple components own overlapping run state (Engine, WorkflowState, SessionManager, Recovery, TUI). Prevention: Single EventStore authority; every subsystem has exactly one owner; TUI consumes events only.

2. **Git as Full-Worktree Owner** — Public `AddAll()`/`Commit()` stages user's unrelated changes. Prevention: Remove/privatize `Commit()`/`AddAll()`; all mutations use `CommitWithFiles(paths...)` with tracked agent files; bisect/rollback in isolated worktrees.

3. **State Machine Validation Bypass** — `SetPhase()` allows arbitrary transitions; corrupted recovery can jump to Ship from Idle. Prevention: `RestorePhase(phase, expectedFrom)` with validation; store `fromPhase` in checkpoint; session ID validation on restore.

4. **Verification Threshold Allows Broken Code** — 90% pass threshold ships known failures. Prevention: Remove threshold (`allOK = len(failedTasks) == 0`); explicit opt-in `allow_partial_verify=false` with TUI confirmation.

5. **Permission System Coupled to TUI** — Headless mode blocks forever on unbuffered channel. Prevention: `PermissionPolicy` interface with `InteractivePolicy`, `HeadlessDenyPolicy`, `HeadlessAllowPolicy`, `CIPolicy`; `--permission-mode=auto|deny|allow` flag.

6. **Incomplete Task State Restoration** — Checkpoint misses task status, heal counts, acceptance results. Prevention: Checkpoint includes complete task array; `LoadCheckpointData()` reinitializes TaskRunner with restored states.

7. **Inconsistent Path Validation** — Scattered validation; symlink escapes in Bash, git commit, verify. Prevention: Central `ValidateTaskFiles()` at task creation AND before every use; `EvalSymlinks` on both paths.

8. **Shell Command Validation via Regex** — Misses `${VAR}`, `$((...))`, nested `$()`. Prevention: Use `mvdan.cc/sh/v3` parser; walk AST rejecting CommandSubst, ArithmExpr, dangerous redirections.

9. **Event-Driven Without Durable Store** — Events only in-memory; lost on crash. Prevention: SQLite `events.db` append-only; every transition emits durable event; TUI reads projections.

10. **Provider Abstraction Leaking Specifics** — NVIDIA `chat_template_kwargs` scattered in generic code. Prevention: Provider adapter owns ALL translation; interface models capabilities abstractly (`SupportsTools()`, `GetContextWindow()`).

## Implications for Roadmap

Based on research, suggested phase structure (12-week migration):

### Phase 1: Foundation — Core Domain Model & Event Store
**Rationale:** All other planes depend on canonical domain types and authoritative persistence. Establishes single source of truth.
**Delivers:** `internal/core/types` (Project, Repository, Workspace, Session, Run, Intent, Requirement, Decision, Plan, Task, TaskGraph, Agent, Tool, Capability, PermissionPolicy, Artifact, Verification, Checkpoint, Event, enums); SQLite EventStore with schema, append, projections, migration from flat files; Config/Keychain with OS-native credential storage.
**Addresses:** Table stakes — session persistence, project memory, multi-project workspace.
**Avoids:** Pitfall 1 (Distributed State), Pitfall 9 (Event Store), Pitfall 17 (SQLite Migration), Pitfall 25 (Cross-Workflow Recovery).
**Uses:** Go 1.26, modernc.org/sqlite, koanf, keychain integration.

### Phase 2: Intelligence Plane — LLM Providers, Context Engine, Code Intelligence
**Rationale:** Intelligence feeds Engineering and Execution; provider abstraction must be clean before agents use it.
**Delivers:** `LLMProvider` interface + NVIDIA/OpenRouter/Zen adapters with capability metadata from API; ContextEngine with fresh per-agent assembly, incremental compaction, ContextSnapshot attribution; CodeIntel orchestrating Tree-sitter + LSP for symbol graph, impact analysis, architecture violations.
**Addresses:** Table stakes — repository understanding, provider streaming, project memory. Differentiators — impact analysis, code intelligence graph, explain/archaeology.
**Avoids:** Pitfall 10 (Provider Leak), Pitfall 11 (Context as Prompt Manipulation), Pitfall 18 (Heuristic Capabilities), Pitfall 21 (Incremental Compaction), Pitfall 22 (Token Estimation).
**Uses:** go-openai, go-tree-sitter, go-lsp, gopls, provider APIs.

### Phase 3: Engineering & Execution Planes — Permissions, Tools, Agents, TaskGraph, Verification
**Rationale:** Core execution engine with safety guarantees. Permission system must work in headless mode.
**Delivers:** CapabilityPolicyEngine (execution-time evaluation, scopes, risk classes, conditions); ToolDispatcher + 18 typed tools with central `ValidateTaskFiles()`; AgentRuntime with 12 agents (Supervisor, Explorer, Researcher, Architect, Planner, Implementer, Tester, Debugger, Verifier, Reviewer, SecurityAuditor, GitSpecialist, ReleaseEngineer) and explicit contracts; TaskGraph IR with execution state machine (PLANNED→READY→RUNNING→WAITING→VERIFYING→COMPLETED/FAILED), Kahn's algorithm, wave scheduling, checkpoint gates; VerificationEngine with 5 levels + Human UAT, evidence-backed verdicts, adversarial review loop, proof-carrying changes.
**Addresses:** Table stakes — file/shell/git tools, task visualization, checkpoints, verification, failure recovery, capabilities. Differentiators — TaskGraph IR, worktree-first, adversarial review, proof-carrying changes, decision intelligence.
**Avoids:** Pitfall 2 (Git Safety), Pitfall 3 (State Machine), Pitfall 4 (Verification Threshold), Pitfall 5 (Permission/TUI), Pitfall 6 (Task Recovery), Pitfall 7 (Path Validation), Pitfall 8 (Shell Safety), Pitfall 12 (Markdown Plan), Pitfall 13 (Model Confidence), Pitfall 15 (Role-Based Permissions), Pitfall 16 (Ad-Hoc Recovery), Pitfall 24 (God Object).
**Uses:** go-git v5, billy, mvdan.cc/sh/v3, testify, gomock, rate limiter, semaphore.

### Phase 4: Interaction Plane & Git Intelligence — TUI Foundation, Worktree Manager
**Rationale:** TUI must be a pure projection consuming events; Git intelligence enables worktree-first execution and semantic commits.
**Delivers:** TUI Foundation per UI-SPEC.md — 36 screens (S01–S36), 26 components, global shell, responsive layouts (4 tiers), keyboard contract, event subscription (session.*, run.*, agent.*, task.*, tool.*, checkpoint.*, verification.*, git.*, workspace.*, provider.*); Application Command/Query API boundary; GitClient with worktree-first execution, semantic commit composition, safety policies (block force-push, require checkpoint for reset --hard), rollback tracking agent files only, bisect in isolated worktree, multi-repo workspace support.
**Addresses:** Table stakes — responsive UX, keyboard navigation, Git inspection. Differentiators — worktree-first, semantic commits, TUI as projection, regression intelligence.
**Avoids:** Pitfall 14 (TUI as State Owner), Pitfall 19 (Emitter Backpressure), Pitfall 20 (Bisect/Rollback on Primary), Pitfall 26 (Config Watcher), Pitfall 27 (Todo Sync), Pitfall 28 (Stash), Pitfall 29 (Diff Baseline), Pitfall 30 (Nested Repos).
**Uses:** Bubble Tea v2, Lip Gloss v2, Bubbles, Zone, go-git worktree APIs.

### Phase 5: Migration & Cutover — Integration, Validation, Cleanup
**Rationale:** Incremental ownership transfer; switch consumers to new interfaces; verify end-to-end; remove old architecture.
**Delivers:** Migration harness replaying `.m31a/` sessions into EventStore; TUI cutover to new runtime; full workflow validation (intent → plan → execute → verify → ship); crash recovery test; headless mode with permissions; all 14 P0/P1 bugs fixed; obsolete code removed (god-object Engine, WorkflowState, legacy checkpoint/recovery, flat-file authority).
**Addresses:** All table stakes and differentiators integrated.
**Avoids:** Pitfall 17 (Split-Brain Persistence), all migration anti-patterns.
**Uses:** All prior stack; goreleaser for cross-compilation verification.

### Phase Ordering Rationale

1. **Foundation first** — Domain types and EventStore are prerequisites for every other plane. Without authoritative persistence, no plane can have durable state.
2. **Intelligence before Execution** — Agents need provider abstraction, context assembly, and code intelligence before they can execute tasks. ContextEngine feeds all agent roles.
3. **Execution & Assurance together** — TaskGraph execution and verification are tightly coupled (verification validates execution output). Adversarial review depends on verification evidence model.
4. **Interaction last** — TUI is a consumer of the runtime API. Building it before the runtime API is stable leads to TUI owning state (Pitfall 14). TUI subscribes to event projections from the EventStore.
5. **Migration cutover as explicit phase** — Prevents big-bang rewrite; interfaces-first approach ensures consumers migrate before old code deletion; verification gates at each week catch regressions early.

### Research Flags

**Phases needing deeper research during planning (`--research-phase`):**
- **Phase 2 (Code Intelligence):** Tree-sitter + LSP orchestration for 10+ languages; incremental indexing performance; architecture violation detection rules — needs spike with real codebases
- **Phase 3 (Adversarial Review):** Independent verifier context/model separation; repair→reverify loop mechanics; proof-carrying change schema — novel pattern, limited prior art
- **Phase 3 (Decision Intelligence):** Reversibility/blast radius classification heuristics; ONE_WAY_DOOR detection accuracy — needs validation on real engineering decisions
- **Phase 4 (TUI Performance):** 36-screen rendering at 160+ cols with virtualized lists; event subscription throughput under load — needs load testing

**Phases with standard patterns (skip research-phase):**
- **Phase 1 (Foundation):** Go domain modeling, SQLite event store, config/keychain — well-established patterns
- **Phase 2 (LLM Providers):** go-openai adapter, provider capability metadata — standard OpenAI-compatible integration
- **Phase 3 (Tools/Permissions/Agents/TaskGraph):** Capability-based permissions, Kahn's algorithm, typed tool interfaces — established patterns
- **Phase 4 (Git Intelligence):** go-git worktree operations, semantic commit composition — standard Git operations
- **Phase 5 (Migration):** Incremental ownership transfer, interface-first migration — standard legacy modernization

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All technologies verified via Context7/official docs; pure-Go constraint eliminates CGO alternatives; versions pinned to latest stable |
| Features | HIGH | Sourced from PROJECT.md, CONTEXT_M31A.md, UI-SPEC.md (canonical requirements); MVP prioritization validated against architectural dependencies |
| Architecture | HIGH | Six-plane model from CONTEXT_M31A.md §§7-34; migration strategy from ARCHITECTURE_RESET.md + CONCERNS.md; 12-week plan with verification gates |
| Pitfalls | HIGH | 20 pitfalls sourced from AUDIT_REPORT.md (17 confirmed findings), AUDIT_ROOT_CAUSES.md, AUDIT_VALIDATION_SUMMARY.md, R1/R2 remediation summaries; P0/P1 bugs reproduced |

**Overall confidence:** HIGH

### Gaps to Address

- **E2E workflow with real API keys** — No credentials in audit environment; cannot validate full NVIDIA Build integration. *Handle:* Phase 2 verification gate requires streaming with reasoning on real API.
- **Cross-compilation targets** — Goreleaser not tested; windows/arm64 excluded but listed. *Handle:* Phase 5 verification gate includes `make cross` and goreleaser dry-run.
- **Concurrent sessions** — Multiple M31A instances in same project not tested. *Handle:* Phase 1 EventStore schema includes session isolation; Phase 5 adds concurrent session test.
- **Subagent worktree isolation** — Parallel agent isolation not validated. *Handle:* Phase 3 TaskGraph wave execution with worktree per wave; Phase 5 stress test.
- **Windows path handling** — Path validation on Windows not verified. *Handle:* Phase 1 adds Windows CI; `ValidateTaskFiles()` uses `filepath.EvalSymlinks` cross-platform.
- **Large session performance** — Memory/context growth under 1000+ messages unknown. *Handle:* Phase 2 ContextEngine incremental compaction; Phase 5 load test with 10k message sessions.
- **Network partition behavior** — Provider timeouts, retries, partial streams not tested. *Handle:* Phase 2 provider adapter implements retry/timeout/circuit breaker; Phase 5 chaos test.
- **MCP/plugin trust domains** — Extension points exist but no implementation to validate. *Handle:* Deferred to v2; Phase 3 designs plugin API with capability scoping only.

## Sources

### Primary (HIGH confidence)
- **CONTEXT_M31A.md** — Canonical architecture (6 planes, domain model, event model, execution state machine, agent contracts, TaskGraph IR, verification model, capability/permission model, workspace/Git safety, TUI/runtime boundary, NVIDIA Build integration)
- **UI-SPEC.md** — 36-screen TUI specification, component library, interaction model, event model exposed to TUI, mandatory user journeys
- **PROJECT.md** — Project requirements, constraints, key decisions, validated/active
- **AUDIT_REPORT.md** — Forensic audit with 17 confirmed findings (3 P0, 8 P1, 4 P2, 2 P3)
- **AUDIT_ROOT_CAUSES.md** — 7 root cause clusters with architectural analysis
- **AUDIT_VALIDATION_SUMMARY.md** — Independent validation reproducing all critical findings
- **R1_REMEDIATION_SUMMARY.md** — Git safety boundary fixes (Commit() privatized, AddAll fallback removed)
- **R2_REMEDIATION_SUMMARY.md** — State machine validation, session ID recovery fixes
- **M31A_ARCHITECTURE_RESET.md** — Authoritative problem diagnosis: distributed state ownership

### Secondary (MEDIUM confidence)
- **.planning/codebase/ARCHITECTURE.md** — Current implementation analysis: layers, data flow, abstractions, anti-patterns
- **.planning/codebase/CONCERNS.md** — Known tech debt, bugs, security issues, test gaps
- **.planning/codebase/STRUCTURE.md** — Directory layout, package purposes, dependency rules
- **.planning/codebase/STACK.md** — Technology stack: Go 1.26.5, Bubble Tea, SQLite, NVIDIA Build
- **.planning/codebase/CONVENTIONS.md** — Code style, architectural patterns, anti-patterns
- **.planning/codebase/INTEGRATIONS.md** — External integrations audit
- **AGENTS.md** — Build requirements, workflow engine, provider layer, tools, dependency rules
- **OpenCode repository analysis** — Architectural inspiration: durable sessions, provider abstraction, typed tool registry, permission-aware materialization, event streams
- **GSD Core methodology** — Planning artifacts, verification workflow, persistent project state, fresh-context agents

### Tertiary (LOW confidence)
- **NVIDIA Nemotron 3 Ultra documentation** — Model capabilities, streaming, reasoning, tool calling, coding-agent compatibility (needs validation on actual API)
- **Bubble Tea v2 upgrade guide** — Context7, MEDIUM confidence
- **modernc.org/sqlite docs** — Context7, MEDIUM confidence
- **go-tree-sitter docs** — Context7, MEDIUM confidence
- **go-openai docs** — Context7, MEDIUM confidence
- **go-git docs** — Context7, MEDIUM confidence

---

*Research completed: 2026-08-23*
*Ready for roadmap: yes*