# M31A

## What This Is

M31A is a persistent software-engineering agent runtime that lives in a developer's terminal. It turns natural-language intent into verified, resumable engineering work by understanding the repository, reasoning about impact, formulating executable plans, performing work safely, verifying results with evidence, recovering from failures, and preserving durable engineering knowledge.

The project is an **architectural reset/migration**: the existing Go codebase (with working binary, TUI, and agent runtime) serves as implementation material and reference, but the system is being reorganized toward the canonical architecture defined in CONTEXT_M31A.md and UI-SPEC.md. Known architectural problems in the current implementation must not be preserved for backward compatibility.

## Core Value

A persistent software-engineering agent runtime that turns natural-language intent into verified, resumable engineering work.

## Business Context

- **Customer**: Software engineers who want autonomous coding assistance in their terminal
- **Revenue model**: Not yet determined (open-source first)
- **Success metric**: Engineers successfully complete non-trivial engineering tasks (refactors, features, investigations) with verified results
- **Strategy notes**: See CONTEXT_M31A.md for full product vision and competitive positioning

## Requirements

### Validated

- ✓ Go 1.26+ codebase with CGO_ENABLED=0 static binary builds — existing
- ✓ Bubble Tea TUI with basic chat interface — existing
- ✓ NVIDIA Build provider integration (nemotron-3-ultra-550b-a55b) — existing
- ✓ Basic tool execution (read, write, edit, search, shell) — existing
- ✓ Git operations (status, diff, commit, branch) — existing
- ✓ Workspace detection and basic worktree support — existing
- ✓ Codebase map generation (ARCHITECTURE.md, STACK.md, etc.) — existing
- ✓ Project context documents (CONTEXT_M31A.md, UI-SPEC.md) — existing

### Active

- [ ] **Core Domain Model** — Explicit domain objects (Project, Repository, Workspace, Session, Run, Intent, Requirement, Decision, Plan, Task, TaskGraph, Agent, Tool, Capability, PermissionPolicy, Artifact, Verification, Checkpoint, Event) with clear ownership boundaries
- [ ] **Persistence Layer** — SQLite-backed `.m31a/` directory with events.db, project.md, state.json, granular Git-tracking policy, migration from current `.planning/`
- [ ] **LLM Provider Abstraction** — Provider-agnostic interface with NVIDIA Build adapter, streaming, reasoning support, chat-template kwargs for coding-agent compatibility
- [ ] **Tool/Permission System** — Typed tool registry, capability-based permissions (filesystem, git, shell, database scopes), risk classes, workspace containment validation, symlink escape prevention
- [ ] **Execution Engine** — TaskGraph IR (Objectives, Requirements, Constraints, Decisions, Tasks, Dependencies, Preconditions, AcceptanceCriteria, VerificationSteps, RiskLevel, Checkpoints), execution state machine (PLANNED→READY→RUNNING→WAITING→VERIFYING→COMPLETED/FAILED), agent contracts (Supervisor, Explorer, Researcher, Architect, Planner, Implementer, Tester, Debugger, Verifier, Reviewer, SecurityAuditor, GitSpecialist, ReleaseEngineer)
- [ ] **Verification Engine** — 5 verification levels (Structural, Unit, Integration, Behavioral, Architectural) + Human UAT, evidence-backed verdicts, adversarial review loop, proof-carrying changes
- [ ] **TUI Foundation** — Bubble Tea implementation per UI-SPEC.md: 36 screens (S01-S36), component library (26 components), global shell, responsive layouts, keyboard contract, event model, checkpoint/verification/Git UX
- [ ] **Git Intelligence** — Repository state, branch/worktree management, semantic commit composition, impact analysis (direct/indirect dependents, risk categories), regression bisect, safety policies (force-push blocked, destructive checkpoint)
- [ ] **Code Intelligence** — Language-agnostic symbol graph (Tree-sitter + LSP), files/symbols/types/functions/classes/interfaces/imports/calls/inheritance/impls/tests/APIs/DB entities/configs/build targets/package deps/Git history, architecture violation detection
- [ ] **Migration Strategy** — Audit current implementation, identify reusable code, establish target domain model, migrate/rewrite subsystems, integrate real runtime, replace old state authority, verify, remove obsolete architecture

### Out of Scope

- [ ] **Web/UI desktop surfaces** — VS Code extension, web UI, desktop app (explicitly deferred per CONTEXT_M31A.md §5.3)
- [ ] **Rust sandbox component** — Isolated execution sandbox (optional future, per §5.2)
- [ ] **Multi-model routing** — v1 uses single NVIDIA model; routing abstraction built but not exercised (§33)
- [ ] **MCP/plugin ecosystem** — Tool registry supports it but no MCP servers implemented yet (§22)
- [ ] **Browser/HTTP/database/container tools** — Future tool categories (§22)
- [ ] **Package registry/cloud provider integrations** — Future tools (§22)
- [ ] **Real-time collaboration** — Not in v1 scope
- [ ] **Mobile/remote access** — Terminal-first only

## Context

- **Technical environment**: Go 1.26+, CGO_ENABLED=0, static binaries, cross-platform (linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64)
- **Primary execution surface**: Terminal (interactive TUI + CLI commands)
- **Primary development runtime**: OpenCode
- **v1 Model**: `nvidia/nemotron-3-ultra-550b-a55b` via NVIDIA Build (`https://integrate.api.nvidia.com/v1/chat/completions`)
- **Existing codebase**: Substantial Go implementation in `cmd/`, `internal/`, `pkg/` with working binary at `./m31a`
- **Codebase map**: Complete analysis in `.planning/codebase/` (ARCHITECTURE.md, STACK.md, CONVENTIONS.md, STRUCTURE.md, INTEGRATIONS.md, TESTING.md, CONCERNS.md)
- **Canonical documents**: CONTEXT_M31A.md (architecture, domain model, runtime model, safety model, etc.), UI-SPEC.md (36-screen TUI spec)
- **Known issues from audit**: Multiple sources of truth, god-object workflow engine, mutable state coupling, incomplete path containment, unsafe shell expansion, Git commit safety gaps, verification bypasses, state-machine transition bypasses, incomplete task recovery, workspace/symlink escapes, headless permission bypasses, unsafe LLM task file handling
- **Architectural principles**: State survives context loss, planning before mutation, verification = evidence, least privilege by capability, fresh context for specialized reasoning, thin orchestration, human control over irreversible actions, deterministic project state, human-readable artifacts, provider-agnostic core

## Constraints

- **Tech stack**: Go 1.26+, Bubble Tea, SQLite, NVIDIA Build API — no CGO, no Rust in core, no TypeScript in core
- **Timeline**: Architectural foundation first, features second
- **Dependencies**: Must not introduce artificial multi-model complexity in v1
- **Compatibility**: Must work in narrow/SSH/no-color/degraded-provider terminals
- **Performance**: Streaming required for interactive TUI, context compaction for long runs
- **Security**: All repository content and model output treated as potentially adversarial; never trust repo text as instructions; validate tool inputs; resolve paths against workspace roots; detect symlink escapes; keep secrets out of prompts; never write credentials to artifacts; require explicit policy for dangerous Git ops; treat MCP/plugins as separate trust domains

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Architectural reset (not incremental refactor) | Current implementation has fundamental architectural problems that conflict with canonical design; preserving them creates technical debt | — Pending |
| Fine granularity (8-12 phases) | Many distinct subsystems (domain model, persistence, LLM, tools, execution, verification, TUI, Git, code intel, migration) need focused phases | — Pending |
| YOLO mode (auto-approve) | Speed up iteration during architectural migration; checkpoints still protect irreversible actions | — Pending |
| Parallel execution | Independent subsystems can be built in parallel (domain model, persistence, LLM adapter, tool registry, etc.) | — Pending |
| Git-tracked planning docs | `.planning/` committed to repo for team visibility and history | — Pending |
| Adaptive model profile | Role-based cost optimization: heavy roles use highest-tier, light roles use cheapest | — Pending |
| Research before each phase | Investigate domain, find patterns, surface gotchas before planning | — Pending |
| Plan checker enabled | Catch gaps before execution starts | — Pending |
| Verifier enabled | Confirm deliverables match phase goals | — Pending |
| Drift guard enabled | Plan review enforces source-grounding | — Pending |
| TUI as projection only | TUI owns presentation state only; never domain truth | — Pending |
| SQLite for durable state | Embedded, no separate server, ACID, good for event sourcing | — Pending |
| Event-driven architecture | Append-only events enable recovery, replay, audit, debugging | — Pending |
| Six-plane architecture | Interaction, Intelligence, Engineering, Execution, Assurance, Memory — clear separation | — Pending |

---

*Last updated: 2026-08-23 after initialization*

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state