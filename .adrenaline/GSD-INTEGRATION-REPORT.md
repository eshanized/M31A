# Integration Report: GSD (Get Shit Done) → M31A

**Date:** 2026-06-13
**Scope:** Feasibility analysis, architectural mapping, and phased integration plan for incorporating GSD patterns into M31A
**Source:** `gsd-build/get-shit-done` (archived; project continues at `open-gsd/gsd-core`)

---

## Executive Summary

GSD is a **meta-prompting framework** (Node.js, ~10,700-line installer, 86+ commands, 33 agents, 11 hooks) that sits between users and AI coding agents (Claude Code, Gemini CLI, Codex, Copilot, etc.). It provides context engineering, multi-agent orchestration, spec-driven development, and file-based state management across 15+ AI coding runtimes.

M31A is a **Go-native CLI coding assistant** (97,366 lines of Go, 335 files, Bubble Tea TUI) with its own six-phase workflow engine, dual-provider architecture, and a polished terminal UI.

Both projects share deep architectural DNA: six-phase workflows, file-based state, spec-driven development, agent orchestration, and verification gates. But they occupy different niches — GSD is a **layer above** AI agents (meta-prompts for Claude/Codex/Gemini), while M31A **is** the AI agent itself.

This report identifies **8 high-value integration patterns** from GSD, maps them to M31A's architecture, and proposes a **4-phase integration plan** with effort estimates.

---

## 1. Comparative Analysis

### 1.1 Architecture Comparison

| Dimension | GSD | M31A | Overlap |
|-----------|-----|------|---------|
| **Language** | Node.js (CJS + TypeScript SDK) | Go 1.24 | None |
| **Workflow phases** | 6 phases (discuss → plan → execute → verify → ship → milestone) | 6 phases (Initialize → Discuss → Plan → Execute → Verify → Ship) | **High** — nearly identical |
| **State management** | `.planning/` (Markdown + JSON) | `~/.m31a/sessions/<id>/planning/` (Markdown + JSON) | **High** — same philosophy |
| **Agent model** | 33 markdown-defined agents, spawned as subagents by host runtime | 13 Go-native tools + planned subagent goroutines | **Medium** — GSD richer |
| **LLM provider** | Runtime-agnostic (delegates to Claude, Gemini, Codex, etc.) | Dual-provider (OpenRouter + OpenCode Zen) | **Low** — different layers |
| **UI** | Host runtime's UI (Claude Code TUI, Gemini CLI, etc.) | Own Bubble Tea TUI with 10 screens | **None** — different approach |
| **Commands** | 86+ slash commands + 6 namespace meta-skills | ~25 slash commands | **Medium** — GSD broader |
| **Multi-project** | Workstreams, workspaces, multi-repo | Single project per session | **Low** — GSD more mature |
| **Hooks** | 11 hooks (statusline, context monitor, guards, etc.) | Bubble Tea event loop + channel-based events | **Low** — different model |
| **Quality gates** | Plan checker, verifier, Nyquist, UAT, code review, security audit, UI review | Self-heal (2 attempts), git bisect, verification checks | **Medium** — GSD richer |
| **Cross-session learning** | Global learnings store, user profiling, debug knowledge base | Cross-session learning ledger (`LEDGER.md`) | **High** — both have it |
| **Brownfield support** | 7-file codebase mapping (`/gsd-map-codebase`) | None | **None** — GSD unique |
| **Knowledge graph** | Graphify integration (BFS, snapshots, diff) | None | **None** — GSD unique |

### 1.2 Philosophy Alignment

Both projects share these core principles:

1. **Fresh context per phase** — GSD spawns agents with clean 200K windows; M31A prunes context per phase
2. **File-based state** — both persist planning data as human-readable Markdown/JSON
3. **Spec-driven development** — both generate structured plans before execution
4. **Defense in depth** — both verify before shipping (GSD: 4 gate types; M31A: self-heal + bisect)
5. **Absent = enabled** — GSD's config philosophy maps to M31A's graceful defaults
6. **FOSS mandate** — both MIT-licensed, no vendor lock-in

### 1.3 Key Divergences

| Divergence | Impact on Integration |
|------------|----------------------|
| GSD is a meta-prompting layer; M31A is a self-contained agent | Cannot directly reuse GSD commands — must extract patterns and re-implement in Go |
| GSD delegates LLM calls to host runtime; M31A owns its provider layer | GSD's model-profile system must be adapted to M31A's `LLMProvider` interface |
| GSD's agents are markdown files read by the host; M31A's tools are Go structs | Agent definitions must be translated to Go-native prompt templates |
| GSD's CLI tools are Node.js (`.cjs`); M31A is pure Go | Must port algorithms, not code |

---

## 2. High-Value Integration Patterns

### 2.1 Brownfield Codebase Mapping — HIGH VALUE, MEDIUM EFFORT

**GSD Feature:** `/gsd-map-codebase` spawns 4 parallel mapper agents to analyze an existing codebase:
- Tech stack detection (frameworks, languages, build tools)
- Architecture mapping (modules, layers, dependencies)
- Code quality assessment (anti-patterns, complexity)
- Concerns identification (technical debt, security issues)

Output: 7 structured Markdown files in `.planning/codebase/` (STACK.md, ARCHITECTURE.md, CONVENTIONS.md, CONCERNS.md, STRUCTURE.md, TESTING.md, INTEGRATIONS.md)

**M31A Gap:** M31A has no brownfield support. The Initialize phase detects project type from file markers (`package.json`, `go.mod`, etc.) but doesn't deeply analyze existing codebases. This limits M31A to greenfield projects.

**Integration Plan:**
1. Add a `CodebaseMapper` tool in `internal/tools/codebase.go` that spawns parallel analysis goroutines
2. Each goroutine uses Grep + FileRead + Glob to scan different dimensions
3. Output structured Markdown to `~/.m31a/sessions/<id>/planning/codebase/`
4. Inject codebase intel into Plan phase context (like GSD's `init.plan-phase` does)
5. Add post-execute drift gate (GSD #2003) — detect when new code diverges from mapped architecture

**Files to create:**
- `internal/tools/codebase.go` — mapper tool implementation
- `internal/workflow/codebase_mapping.go` — workflow integration
- `internal/tui/codebase_view.go` — TUI screen for mapping results

**Effort:** ~3-5 days
**Impact:** Unlocks brownfield projects (the majority of real-world use cases)

---

### 2.2 Structured Agent Definitions with Model Profiles — HIGH VALUE, LOW EFFORT

**GSD Feature:** 33 agents defined in Markdown with YAML frontmatter specifying:
- `name`, `description`, `tools`, `color`
- Model profile assignments (`opus`, `sonnet`, `haiku`, `inherit`)
- Per-phase-type model selection (plan phases use stronger models, verify uses faster ones)
- Adaptive context enrichment (1M models get richer context than 200K models)

**M31A Gap:** M31A uses a single model for all operations. The cost arbitrage engine suggests cheaper models per-task, but there's no structured agent definition system or per-phase model routing.

**Integration Plan:**
1. Define agent profiles in `internal/workflow/agents/` as embedded Go structs (or TOML files)
2. Each profile specifies: system prompt template, recommended model tier, tool permissions, context budget
3. Extend `LLMProvider` with `SelectModelForPhase(phase WorkflowPhase)` that picks the right model tier
4. Add `adaptive_context` config flag — when using a 500K+ model, inject richer context (prior summaries, research docs)

**Files to modify:**
- `internal/workflow/engine.go` — add agent profile lookup per phase
- `internal/provider/registry.go` — add model tier selection
- `internal/types/constants.go` — add agent profile types

**Effort:** ~2-3 days
**Impact:** Smarter model routing, reduced costs, better per-phase quality

---

### 2.3 Enhanced Verification Gates — HIGH VALUE, MEDIUM EFFORT

**GSD Feature:** Multi-layered verification beyond basic pass/fail:
- **Plan Checker agent** — validates plan structure before execution (max 3 iterations)
- **Nyquist Validation** — ensures test coverage maps to all requirements
- **Requirements Coverage Gate** — verifies REQ-IDs map to plans
- **Decision Coverage Gate** — verifies CONTEXT.md decisions map to shipped artifacts
- **Post-Execute Codebase Drift Gate** — detects structural changes after execution
- **Schema Drift Detection** — ORM pattern monitoring (Prisma, Drizzle, etc.)
- **Package Legitimacy Gate** — supply-chain protection against slopsquatting

**M31A Gap:** M31A's Verify phase checks file existence, syntax, tests, and imports. It has self-heal (2 attempts) and git bisect. But it lacks:
- Pre-execution plan validation beyond JSON schema
- Requirements traceability
- Decision coverage tracking
- Supply-chain protection

**Integration Plan:**
1. Add a `PlanChecker` step in the Plan phase — validates plan before user approval:
   - No dangling file references
   - Estimated complexity within model capability
   - Requirements coverage (each task maps to a goal criterion)
2. Add Requirements Traceability to `TASKS.md`:
   - Each task gets a `req_ids` field linking to acceptance criteria from Discuss phase
   - Verify phase checks all req_ids have corresponding passing tasks
3. Add Package Legitimacy Gate to Execute phase:
   - When a task installs packages, run `npm view` / `pip index versions` / `cargo search`
   - Tag packages from WebSearch as `[ASSUMED]` — require human confirmation before install
4. Add Decision Coverage to Verify:
   - Track key decisions from Discuss phase in `PROJECT.md`
   - Verify phase checks each decision has a corresponding artifact

**Files to modify:**
- `internal/workflow/engine_verify.go` — add gate pipeline
- `internal/workflow/engine_parse.go` — add plan validation rules
- `internal/workflow/prompts/verify.md` — add gate prompt templates

**Effort:** ~4-5 days
**Impact:** Catches failures before execution, prevents supply-chain attacks

---

### 2.4 Debug Session Persistence — MEDIUM VALUE, LOW EFFORT

**GSD Feature:** `/gsd-debug` creates persistent debug sessions:
- Tracks evidence, eliminated hypotheses, and resolution state
- Supports `list`, `status <slug>`, `continue <slug>` subcommands
- TDD mode integration (failing test before fix)
- Knowledge base accumulation across debug sessions (`.planning/debug/knowledge-base.md`)
- Diagnosis-only mode (`--diagnose`)

**M31A Gap:** M31A has no dedicated debug workflow. Bugs are handled as ad-hoc tasks within the REPL or as part of a workflow.

**Integration Plan:**
1. Add `/debug` slash command that creates a debug session:
   - `~/.m31a/sessions/<id>/debug/<slug>.md` with structured fields: Description, Evidence, Hypotheses, Eliminated, Resolution, Next Action
2. Add `debug list`, `debug status`, `debug continue` subcommands
3. Integrate with Verify phase — when self-heal triggers, create a debug session automatically
4. Add debug knowledge base — persist learnings across sessions in `~/.m31a/DEBUG-KNOWLEDGE.md`

**Files to create:**
- `pkg/debug/` — debug session management (similar to `pkg/ledger/`)
- `internal/tui/debug_view.go` — debug session browser
- `internal/tui/repl_commands.go` — add `/debug` command handler

**Effort:** ~2-3 days
**Impact:** Structured debugging, persistent debug knowledge

---

### 2.5 Progress Auto-Routing (`/gsd-progress --next`) — MEDIUM VALUE, LOW EFFORT

**GSD Feature:** `/gsd-progress --next` automatically determines and dispatches the next logical workflow step:
- No project → suggests `/gsd-new-project`
- Phase needs discussion → runs `/gsd-discuss-phase`
- Phase needs planning → runs `/gsd-plan-phase`
- Phase needs execution → runs `/gsd-execute-phase`
- Phase needs verification → runs `/gsd-verify-work`
- All phases complete → suggests `/gsd-complete-milestone`
- `--forensic` flag adds a 6-check integrity audit

**M31A Gap:** M31A has `/workflow` to show current state, but no auto-routing. The user must manually invoke `/plan`, `/execute`, `/verify`, `/ship`.

**Integration Plan:**
1. Add `/next` slash command (or extend `/workflow` with `--next` flag):
   - Read `STATE.md` to determine current phase
   - Check if next phase prerequisites are met
   - Auto-dispatch to the appropriate phase command
2. Add `--forensic` flag that runs integrity checks:
   - STATE.md consistency with TASKS.md
   - Orphaned planning files
   - Uncommitted work detection
   - Deferred scope drift

**Files to modify:**
- `internal/tui/repl_commands.go` — add `/next` command
- `internal/workflow/engine.go` — add `NextAction()` method

**Effort:** ~1-2 days
**Impact:** Smoother workflow, less cognitive overhead

---

### 2.6 Socratic Exploration (`/gsd-explore`) — MEDIUM VALUE, LOW EFFORT

**GSD Feature:** `/gsd-explore` provides a Socratic ideation session:
- Guides an idea through probing questions
- Optionally spawns research agents
- Routes output to the right artifact (notes, todos, seeds, requirements, new phase)

**M31A Gap:** M31A jumps directly from user goal to Initialize phase. There's no lightweight exploration step for when the user isn't sure what they want to build.

**Integration Plan:**
1. Add `/explore` slash command:
   - LLM generates probing questions based on the topic
   - User answers iteratively
   - At conclusion, offer routing: create as a todo, start a new workflow, or save as a note
2. Store exploration output in `~/.m31a/sessions/<id>/explorations/`

**Files to create:**
- `internal/workflow/explore.go` — exploration workflow
- `internal/tui/explore_view.go` — exploration UI

**Effort:** ~1-2 days
**Impact:** Better for ambiguous goals, reduces wasted planning cycles

---

### 2.7 Cross-AI Review (`/gsd-review`) — MEDIUM VALUE, HIGH EFFORT

**GSD Feature:** `/gsd-review` sends phase plans to external AI CLIs for peer review:
- Supports Gemini, Claude, Codex, OpenCode, Qwen, Cursor, Ollama, LM Studio, llama.cpp
- Each reviewer runs as a subprocess with its own context
- Reviews are collected in `REVIEWS.md`
- `/gsd-plan-review-convergence` loops plan → review → replan until no HIGH concerns remain

**M31A Gap:** M31A uses a single LLM for all operations. There's no cross-validation from other models.

**Integration Plan:**
1. Add `/review` slash command with `--gemini`, `--codex`, `--all` flags
2. Implement subprocess spawning for external AI CLIs (similar to how Bash tool works)
3. Collect review output, parse for concern levels (HIGH, MEDIUM, LOW)
4. Feed HIGH concerns back to the Plan phase for revision
5. Add convergence loop — max 3 cycles, stall detection

**Files to create:**
- `internal/tools/external_ai.go` — subprocess management for external CLIs
- `internal/workflow/review.go` — cross-AI review workflow
- `internal/tui/review_view.go` — review results display

**Effort:** ~5-7 days
**Impact:** Higher plan quality, catches blind spots

---

### 2.8 Workstream Support — LOW-MEDIUM VALUE, MEDIUM EFFORT

**GSD Feature:** `/gsd-workstreams` enables parallel work on different milestone areas:
- Create, switch, list, status, progress, complete, resume
- Each workstream has its own `.planning/` directory
- Session-scoped active pointer
- Workstream inventory builder for fast status projection

**M31A Gap:** M31A has single-project sessions. No concept of parallel workstreams.

**Integration Plan:**
1. Extend session model with workstream concept:
   - `~/.m31a/sessions/<id>/workstreams/<name>/` directories
   - Active workstream pointer in `session.json`
2. Add `/workstreams` command with `create`, `switch`, `list`, `status` subcommands
3. Each workstream gets its own `TASKS.md`, `STATE.md`, `PROJECT.md`

**Files to modify:**
- `pkg/session/manager.go` — add workstream CRUD
- `internal/types/types.go` — add workstream types
- `internal/tui/` — add workstream selector UI

**Effort:** ~3-4 days
**Impact:** Multi-feature parallel development

---

## 3. Patterns NOT Recommended for Integration

### 3.1 Multi-Runtime Installer
GSD supports 15+ AI coding runtimes with a 10,700-line installer. M31A is its own runtime — it doesn't need to install into other tools. **Skip.**

### 3.2 Two-Stage Namespace Meta-Skills
GSD's namespace routing (`/gsd-workflow`, `/gsd-project`, etc.) reduces token cost for eager skill listing. M31A's ~25 commands don't need hierarchical routing — a flat command palette is sufficient. **Skip.**

### 3.3 Hook System (Node.js / Shell)
GSD's 11 hooks are tightly coupled to Claude Code's / Gemini CLI's hook event system. M31A's Bubble Tea event loop already handles these concerns natively. **Skip.**

### 3.4 Cross-AI Execution Delegation
GSD's `--cross-ai` flag delegates entire phase execution to external CLIs. This contradicts M31A's self-contained architecture. **Skip** (but cross-AI *review* is valuable — see §2.7).

### 3.5 Knowledge Graph (Graphify)
GSD's graphify integration is an external dependency that builds code knowledge graphs. M31A's planned V1.2 plugin system would be a better home for this — implement as a plugin, not core. **Defer to V1.2 plugin.**

### 3.6 MVP Mode / SPIDR Splitting / Walking Skeleton
GSD's MVP mode enforces vertical-slice planning with TDD gates. This is a workflow philosophy that's too opinionated for M31A's general-purpose design. **Skip** — M31A's task system already supports this pattern organically.

---

## 4. Phased Integration Plan

### Phase 1: Quick Wins (V1.0 Patch) — ~4-6 days

| # | Pattern | Effort | Impact | Files |
|---|---------|--------|--------|-------|
| 1 | Progress Auto-Routing (`/next`) | 1-2 days | Medium | 2 files modified |
| 2 | Socratic Exploration (`/explore`) | 1-2 days | Medium | 2 new files |
| 3 | Debug Session Persistence (`/debug`) | 2-3 days | Medium | 3 new files |

### Phase 2: Core Enhancement (V1.1) — ~6-8 days

| # | Pattern | Effort | Impact | Files |
|---|---------|--------|--------|-------|
| 4 | Structured Agent Definitions + Model Profiles | 2-3 days | High | 3 files modified |
| 5 | Enhanced Verification Gates | 4-5 days | High | 3 files modified |

### Phase 3: Major Feature (V1.1 or V1.2) — ~3-5 days

| # | Pattern | Effort | Impact | Files |
|---|---------|--------|--------|-------|
| 6 | Brownfield Codebase Mapping | 3-5 days | High | 3 new files |

### Phase 4: Advanced (V1.2+) — ~8-11 days

| # | Pattern | Effort | Impact | Files |
|---|---------|--------|--------|-------|
| 7 | Cross-AI Review | 5-7 days | Medium | 3 new files |
| 8 | Workstream Support | 3-4 days | Low-Medium | 3 files modified |

---

## 5. Architectural Seams for Integration

### 5.1 Where GSD Patterns Map to M31A Code

```
GSD Concept                    → M31A Integration Point
──────────────────────────────────────────────────────────
commands/gsd/*.md              → internal/workflow/ (Go prompt templates)
agents/*.md                    → internal/workflow/agents/ (Go structs)
get-shit-done/workflows/*.md   → internal/workflow/prompts/ (extend existing)
get-shit-done/references/*.md  → internal/workflow/prompts/ (new reference files)
get-shit-done/bin/lib/*.cjs    → pkg/ (port algorithms to Go)
.planning/                     → ~/.m31a/sessions/<id>/planning/ (extend)
.planning/codebase/            → ~/.m31a/sessions/<id>/planning/codebase/ (new)
.planning/debug/               → ~/.m31a/sessions/<id>/debug/ (new)
config.json                    → ~/.m31a/config.toml (add new sections)
gsd-sdk query                  → internal Go-native queries (no subprocess)
hooks/*.js                     → Bubble Tea event handlers (native)
```

### 5.2 Config Schema Extensions

```toml
# New sections to add to ~/.m31a/config.toml

[workflow]
plan_check = true              # Pre-execution plan validation
nyquist_validation = true      # Test coverage mapping
decision_coverage = true       # Decision → artifact tracking
package_legitimacy_gate = true # Supply-chain protection
debug_persistence = true       # Persistent debug sessions

[codebase]
mapping_enabled = false        # Brownfield codebase mapping
drift_detection = true         # Post-execute architecture drift check
drift_threshold = 3            # Structural changes before drift warning

[agents]
model_routing = true           # Per-phase model tier selection
adaptive_context = false       # Richer context for 500K+ models

[review]
cross_ai_enabled = false       # Cross-AI review
default_reviewers = []         # Default external reviewers
max_cycles = 3                 # Review convergence limit
```

### 5.3 Session Directory Extensions

```
~/.m31a/sessions/<session-id>/
├── (existing files...)
├── planning/
│   ├── (existing: PROJECT.md, TASKS.md, STATE.md)
│   ├── codebase/              # NEW: brownfield mapping
│   │   ├── STACK.md
│   │   ├── ARCHITECTURE.md
│   │   ├── CONVENTIONS.md
│   │   ├── CONCERNS.md
│   │   ├── STRUCTURE.md
│   │   ├── TESTING.md
│   │   └── INTEGRATIONS.md
│   └── reviews/               # NEW: cross-AI reviews
│       └── PHASE-{N}-REVIEWS.md
├── debug/                     # NEW: persistent debug sessions
│   ├── <slug>.md
│   └── resolved/
├── explorations/              # NEW: Socratic exploration output
│   └── <slug>.md
└── workstreams/               # NEW: parallel workstreams (Phase 4)
    └── <name>/
        ├── TASKS.md
        ├── STATE.md
        └── PROJECT.md
```

---

## 6. Risk Assessment

### 6.1 Integration Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| GSD patterns assume external host runtime | High | All patterns must be re-implemented natively in Go — no code sharing |
| Feature creep from 86+ GSD commands | Medium | Strict 8-pattern selection; reject multi-runtime, hooks, namespace routing |
| Performance regression from new verification gates | Medium | Gates run in parallel goroutines; non-blocking by default |
| Config complexity increase | Low | New sections are opt-in (absent = enabled philosophy from GSD) |
| Breaking existing session format | Low | New directories are additive; existing sessions unaffected |

### 6.2 Maintenance Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| GSD is archived (moved to open-gsd/gsd-core) | Medium | Patterns are well-documented in the archived repo; no upstream dependency |
| GSD's Node.js algorithms may not translate cleanly to Go | Medium | Port logic, not code — Go's type system catches issues Node misses |
| Divergence from upstream GSD improvements | Low | GSD is archived; no future divergence expected |

---

## 7. Key Takeaways

1. **GSD and M31A are architecturally aligned** — same philosophy (fresh context, file-based state, spec-driven, defense in depth), same workflow shape (6 phases). This makes pattern extraction natural.

2. **Port patterns, not code** — GSD is Node.js; M31A is Go. The value is in the *design patterns* (brownfield mapping, verification gates, agent profiles, debug persistence), not the implementation.

3. **Start with quick wins** — Progress auto-routing (`/next`), exploration (`/explore`), and debug persistence (`/debug`) can be implemented in ~5 days with high user-facing impact.

4. **Brownfield mapping is the highest-value major feature** — M31A currently only works well for greenfield projects. Adding codebase mapping unlocks the majority of real-world use cases.

5. **Cross-AI review is powerful but expensive** — The pattern of sending plans to multiple AI models for peer review is unique and valuable, but requires significant implementation effort and external CLI dependencies.

6. **Skip the meta-prompting layer** — GSD's multi-runtime installer, hook system, and namespace routing are irrelevant to M31A's self-contained architecture. Don't be tempted to add them.

7. **GSD's archived status is an advantage** — The patterns are frozen and well-documented. No risk of upstream breaking changes. M31A can adopt at its own pace.
