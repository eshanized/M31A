# Feature Landscape

**Domain:** Persistent software-engineering agent runtime (terminal-first)
**Researched:** 2026-08-23

## Table Stakes

Features users expect. Missing = product feels incomplete or untrustworthy.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **Natural-language intent entry** | Primary interaction model; users won't learn a CLI syntax for engineering tasks | Medium | Must handle ambiguous, multi-sentence, context-dependent requests |
| **Repository understanding (onboarding)** | Agent must know the codebase before acting; re-scanning every prompt is unacceptable | High | Language-agnostic; Tree-sitter + LSP; incremental updates; handles multi-repo workspaces |
| **File read/write/edit/search tools** | Basic coding agent capability; without these, nothing can be built | Medium | Path validation, workspace containment, symlink escape prevention, output bounding |
| **Shell command execution** | Running tests, builds, linting, Git ops, arbitrary dev commands | Medium | Context-aware timeouts, output capture/bounding, cancellation, secret redaction |
| **Git inspection (status, diff, log, blame, branches)** | Understanding repo state is prerequisite for safe changes | Medium | Multi-repo support; worktree awareness; never blind `git add .` |
| **Session persistence & resume** | Terminal crashes, network drops, context compaction must not lose work | High | SQLite event sourcing; durable run state; exact resume point reconstruction |
| **Task/plan visualization** | Users need to see *what will happen* before it happens; trust requires visibility | Medium | TaskGraph rendering; wave-based execution; dependency display; risk indicators |
| **Checkpoint/approval for risky actions** | Destructive Git, migrations, one-way-door decisions require human control | Medium | Durable checkpoint state; persists across terminal close; shows risk/reversibility/options |
| **Verification with evidence** | "Done" without evidence is theater; users need proof tests pass, types compile, behavior works | High | 5 verification levels (Structural→Architectural→Human); evidence binding; independent verifier |
| **Failure recovery (retry/repair/replan)** | Agents fail; users expect autonomous recovery within policy bounds | High | Bounded retries; failure taxonomy; replan threshold; human escalation path |
| **Multi-project workspace support** | Real repos are monorepos, multi-repo, or non-Git directories | Medium | Workspace root detection (never fallback to `/`); per-repo Git ops; file→repo mapping |
| **Responsive terminal UX (80–160+ cols)** | Engineers work in SSH, tmux, small panes, wide screens | Medium | 4 layout tiers; sidebar collapse; virtualized lists; lazy diff/log loading |
| **Keyboard-first navigation** | Terminal users expect full keyboard control; mouse is additive | Low | Universal `Esc`=back; `Ctrl+K` palette; `?` help; no mouse-required flows |
| **Provider streaming with reasoning** | Interactive feel requires token streaming; reasoning metadata for transparency | Medium | Handles empty deltas, reasoning-only chunks, mixed transitions, connection recovery |
| **Capability-based permissions** | "Allow shell" toggle is insufficient; need scoped, risk-aware, condition-based policy | High | Filesystem/Git/shell/database scopes; risk classes; deny-lists (.env, *.pem, ~/.ssh) |
| **Project memory (decisions, research, learnings)** | Context loss between sessions destroys accumulated knowledge | Medium | Human-readable `.m31a/` artifacts; evidence-class tagging (VERIFIED/INFERRED/ASSUMED/UNRESOLVED) |

---

## Differentiators

Features that set M31A apart. Not expected by default, but create competitive advantage.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Six-plane architecture (Interaction, Intelligence, Engineering, Execution, Assurance, Memory)** | Clean separation enables independent evolution, testing, replacement of each plane | High | Architectural principle, not a single feature; drives all subsystem boundaries |
| **TaskGraph IR (Objectives, Requirements, Constraints, Decisions, Tasks, Dependencies, Preconditions, AcceptanceCriteria, VerificationSteps, RiskLevel, Checkpoints)** | Executable plan representation prevents "nice-looking plan, different execution" divergence | High | Compiled from LLM intent; runtime executes IR not Markdown; enables scheduling, verification, replay |
| **Impact analysis with direct/indirect dependents & risk categories** | Answers "what breaks if I change this?" before mutation; drives planning scope | Very High | Requires code graph (calls, imports, tests, inheritance, Git history); suggests TaskGraph from impact |
| **Code intelligence graph (symbols, types, functions, calls, inheritance, tests, APIs, DB entities, configs, build targets, package deps, Git history)** | Enables impact, explain, regression, architecture violation detection, dependency intelligence | Very High | Language-agnostic via Tree-sitter + LSP; incremental indexing; architecture map generation |
| **Explain/archaeology ("why does this exist?")** | Combines source, call graph, Git blame, commit history, ADRs, tests to answer *why*, not just *what* | High | Distinguishes evidence from inference; cites sources; assesses current rationale validity |
| **Regression intelligence (bisect + causal analysis)** | "Find the regression after commit X" → reproduces, hypothesizes, verifies, reports root cause | High | Distinguishes "likely cause" from "verified cause"; integrates with verification engine |
| **Semantic commit composition from task boundaries** | Logical commits (feat/test/refactor grouped) instead of `git commit -am "fix stuff"` | Medium | Change grouping by semantic boundaries; conventional commits; linked to verified tasks |
| **Worktree-first isolated execution** | User's main worktree stays clean; changes applied after verification + approval | High | Auto worktree creation; diff production; safe merge; multi-repo worktree coordination |
| **Adversarial review loop (independent reviewer context/model)** | Finds assumptions, missed edge cases, race conditions, security boundary changes post-implementation | Very High | Separate verifier context; optionally separate model; repair→reverify loop |
| **Proof-carrying changes (durable change records with intent, plan, diff, tests, evidence, risks, verdict)** | Every completed task produces auditable engineering artifact; enables learning, audit, rollback | Medium | Structured run artifacts; requirement traceability (REQ→TASK→FILE→TEST→VERIFICATION) |
| **Context epochs & compaction with durable fact extraction** | Prevents context rot; preserves decisions/tasks/risks/plan/verification across compaction | High | Extracts engineering facts; discards conversational noise; never relies on prose summary alone |
| **Fresh-context specialized agents (Supervisor, Explorer, Researcher, Architect, Planner, Implementer, Tester, Debugger, Verifier, Reviewer, SecurityAuditor, GitSpecialist, ReleaseEngineer)** | Each role gets focused context assembled from durable state; no giant shared conversation | High | Contract-based (requires/produces/must_not); handoff protocol with loaded context summary |
| **Decision intelligence (reversibility, blast radius, data migration, public contract, operational risk → ONE_WAY_DOOR/SAFE_AUTOMATION)** | Classifies decisions; enforces checkpoints for irreversible actions automatically | Medium | Durable decision metadata; drives checkpoint policy; human-readable ADR format |
| **Dependency intelligence (registry lookup, maintenance, vulns, license, transitive impact, API stability)** | Prevents dependency slop; human checkpoint for suspicious packages before install | Medium | Authoritative registry queries; vulnerability/license scans; popularity signals |
| **Requirements traceability (REQ → TaskGraph → Files → Tests → Verification → VERIFIED)** | Gap analysis; proves every requirement has verified implementation | Medium | Enables "what's not covered?" queries; drives verification coverage reporting |
| **Event-driven architecture with append-only event log** | Recovery, replay, audit, debugging, deterministic reconstruction from events | High | Minimum 35 event types; monotonic ordering; state derivable from events |
| **TUI as pure projection (no domain logic in UI)** | TUI replaceable; runtime testable headless; UI never owns truth | Medium | Bubble Tea handles rendering/input only; commands/queries → application layer → runtime |
| **Vertical slice delivery (tracer principle)** | End-to-end RBAC flow validated through persistence→service→API→test before horizontal expansion | Process | Reduces integration risk; proves architecture early |

---

## Anti-Features

Features to explicitly NOT build (v1 scope boundaries from CONTEXT_M31A.md §70, §5.3).

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **VS Code extension / web UI / desktop app** | Terminal-first is core thesis; UI clients duplicate agent engine; diverts focus from runtime | Build Go runtime with clean command/query API; clients (VS Code, web) added later as thin consumers |
| **Rust sandbox component in core** | Adds split-language complexity prematurely; Go handles process isolation for v1 | Optional future `m31a-sandbox` in Rust; core stays Go-only |
| **Multi-model routing in v1** | Only one configured model (Nemotron 3 Ultra); routing abstraction built but not exercised | Build provider abstraction; keep default implementation single-model |
| **MCP/plugin ecosystem** | Tool registry supports it but no MCP servers implemented; trust boundaries undefined | Design plugin API with capability scoping; implement adapters later |
| **Browser/HTTP/database/container tools** | Expands tool surface before core tools are solid; security surface grows | Core tools: read/write/edit/search/shell/test/Git; others deferred |
| **Package registry / cloud provider integrations** | External integrations before core engineering loop works | Research engine handles registry lookups; tools added when needed |
| **Real-time collaboration** | Terminal-first, single-user; multiplayer adds distributed state complexity | Deferred; session/run artifacts are shareable via Git/`.m31a/` |
| **Mobile/remote access** | Terminal is primary surface; remote access via SSH/tmux works natively | Ensure SSH-friendly input, NO_COLOR, ASCII fallback, resize handling |
| **Unrestricted shell access** | Security model forbids "allow shell" toggle; every command needs policy evaluation | Capability-based permissions with risk classes, scopes, conditions |
| **Model as sole source of truth** | Context windows are disposable; project state must be durable | All engineering state in SQLite/`.m31a/`; model receives assembled context |
| **Automatic blind `git add . && git commit`** | Destroys semantic commit boundaries; mixes unrelated changes | Semantic commit composer from task boundaries + change groups |
| **Verification theater (superficial check → "done")** | Undermines trust; core differentiator is evidence-backed verification | Explicit acceptance criteria → evidence binding → independent verifier |
| **Hidden chain-of-thought as user transcript** | Provider-private reasoning must not be exposed as diagnostics | Show compact status: `reasoning: active`; safe summary only |
| **Global mutable state in TUI** | Bubble Tea is single-threaded; shared mutable state from goroutines breaks | TUI owns presentation state only; all domain mutations via `tea.Cmd`/`tea.Msg` |
| **`/` as workspace fallback root** | Known failure class in coding agents; exposes unrelated machine files | Workspace detector prefers resolved current directory when no Git root |
| **Prose summaries for recovery-critical information** | Never rely on model summary for decisions/tasks/risks/plan/verification | Durable structured state in SQLite; human-readable artifacts in `.m31a/` |

---

## Feature Dependencies

```
Repository Intelligence (Phase 5)
    ├── File/Symbol Index → Impact Analysis
    ├── Call Graph + Tests → Regression Intelligence
    ├── Import/Inheritance Graph → Architecture Violation Detection
    └── Git History → Explain/Archaeology

TaskGraph IR (Phase 6)
    ├── Intent Classification → Requirements
    ├── Requirements + Impact → Tasks + Dependencies
    ├── Risk Assessment → Checkpoints
    └── Acceptance Criteria → Verification Steps

Execution Engine (Phase 7)
    ├── TaskGraph → Agent Scheduling (waves)
    ├── Agent Contracts → Capability Scoping
    ├── Retry/Repair/Replan → Failure Taxonomy
    └── Worktree Isolation → Git Intelligence

Verification Engine (Phase 8)
    ├── Acceptance Criteria (from TaskGraph) → Evidence Binding
    ├── 5 Verification Levels → Independent Verifier Context
    ├── Failure Routing → Repair/Replan/Escalate
    └── Evidence Artifacts → Proof-Carrying Changes

Git Intelligence (Phase 9)
    ├── Semantic Diff → Commit Composer
    ├── Branch/Worktree State → Worktree Manager
    ├── History + Code Graph → Regression Bisect
    └── Safety Policy → Destructive Operation Guards

Security & Policy (Phase 10)
    ├── Capability Registry → All Tool Execution
    ├── Path Containment → Filesystem Tools
    ├── Secret Redaction → All Output/Logging
    └── Prompt Injection Defense → Context Assembly

Adversarial Review (Phase 11)
    ├── Independent Verifier Context → Review Loop
    ├── Risk Analysis → Architectural/Security Review
    └── Repair Integration → Re-verification

Advanced Intelligence (Phase 12)
    ├── Decision Metadata → Decision Intelligence
    ├── Dependency Research → Dependency Intelligence
    ├── Architecture Map + Git History → Drift Detection
    └── Run Artifacts → Project Learnings
```

---

## MVP Recommendation

**Prioritize (vertical tracer slice):**

1. **Repository onboarding + file/symbol index** (enables all intelligence)
2. **TaskGraph IR + planner agent** (converts intent → executable plan)
3. **Implementer agent + core tools (read/write/edit/search/shell/test)** (executes one task)
4. **Verification engine (structural + unit + build)** (evidence-backed completion)
5. **Session/Run persistence + resume** (survives interruption)
6. **Checkpoint for destructive Git** (human control over irreversible actions)

**Defer:**
- Adversarial review (Phase 11): Requires independent verifier context; build after verification engine solid
- Dependency intelligence (Phase 12): Research engine can handle ad-hoc; systematic intelligence later
- Plugin/MCP (Phase 13): Core runtime must be stable first
- Multi-model routing (out of v1 scope): Abstraction built, single model exercised
- Web/VS Code clients (out of v1 scope): Terminal runtime is the product

---

## Sources

- PROJECT.md (validated/active requirements, constraints, key decisions)
- CONTEXT_M31A.md (complete product vision, architecture, domain model, runtime model, safety model, v1 scope/non-goals/acceptance criteria)
- UI-SPEC.md (36-screen TUI specification, component library, interaction model, mandatory user journeys)
- OpenCode repository analysis (architectural inspiration: durable sessions, provider abstraction, typed tool registry, permission-aware materialization, event streams)
- GSD Core methodology (planning artifacts, verification workflow, persistent project state, fresh-context agents)
- NVIDIA Nemotron 3 Ultra documentation (model capabilities, streaming, reasoning, tool calling, coding-agent compatibility)