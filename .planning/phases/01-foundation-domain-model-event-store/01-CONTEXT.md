# Phase 01: Foundation — Domain Model & Event Store - Context

**Gathered:** 2026-08-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Establish canonical domain types (18 explicit types across six planes) and an authoritative SQLite event store as the single source of truth for all durable state. Migrate existing `.planning/` data to `.m31a/` with zero data loss and full traceability. The TUI and all other consumers become pure projections reading from this event store.

</domain>

<decisions>
## Implementation Decisions

### Event Versioning Strategy
- **D-01:** Event payloads include `schema_version` in `EventMetadata`; incremented when payload structure changes; projections handle versions — **Reversibility:** costly — changing schema version affects all projections and migration tools; downstream agents must handle multiple versions
- **D-02:** Projections use graceful degradation when encountering newer schema versions — read known fields, ignore unknown, log warning, continue building state — **Reversibility:** reversible — behavior change isolated to projection logic
- **D-03:** Migration utilities included to upgrade existing events in store (backfill `schema_version`, transform v1→v2 payloads) — **Reversibility:** one-way — modifies immutable event log; requires careful testing and backup before run
- **D-04:** Canonical event type definitions live in `internal/core/types/event.go` alongside domain types — **Reversibility:** costly — moving event types would require updates across all consumers (planners, verifiers, TUI event handlers)

### Concurrency Model
- **D-05:** Single-writer, multi-reader model using SQLite WAL mode; one M31A process writes at a time; multiple can read concurrently — **Reversibility:** costly — changing to per-session databases or per-run locks would require significant architectural changes
- **D-06:** Write conflicts fail fast with clear error (`ErrWriteConflict`) — "Another M31A instance is running; close it or use different workspace" — **Reversibility:** reversible — error message and behavior can be adjusted without data migration
- **D-07:** Process-level lock on entire `.m31a/events.db` via SQLite's built-in WAL serialization; `busy_timeout=5000` in DSN handles contention — **Reversibility:** costly — moving to advisory locks or per-run locks would require lock protocol changes
- **D-08:** Rely on SQLite's built-in locking (WAL mode serializes writers); no Go `flock` wrapper — **Reversibility:** reversible — adding Go-level lock later is additive if needed

### Migration Completeness
- **D-09:** Migrate ALL substantive artifacts as events: REQUIREMENTS.md (118), ROADMAP.md (12 phases), PROJECT.md, STATE.md, CONTEXT.md, decisions/, research/ (4 files), codebase/ maps (7 files) — **Reversibility:** one-way — migration writes events that become authoritative; rolling back requires re-migration
- **D-10:** One event per artifact: `RequirementCreated` × 118, `PhaseCreated` × 12, `DecisionLogged` × N, `ResearchCompleted` × 4, etc.; full traceability and queryability — **Reversibility:** one-way — event granularity is foundational to traceability model
- **D-11:** Strict validation — fail fast on any unparsable requirement/decision; forces manual fix before migration completes; guarantees data integrity — **Reversibility:** reversible — validation strictness can be relaxed in future migrations
- **D-12:** After migration, archive `.planning/` to `.planning.archived.<timestamp>/`; preserves history, `.planning/` available for new projects — **Reversibility:** reversible — archive can be restored if needed

### Projection Snapshots
- **D-13:** Projection checkpoints written every N events (configurable, e.g., every 100); balance rebuild speed and write overhead — **Reversibility:** reversible — checkpoint frequency is a tuning parameter
- **D-14:** Checkpoint contains full projected state JSON (requirements.md content, decisions list, etc.) + `last_seq`; fast resume by loading state + replaying from `last_seq+1` — **Reversibility:** costly — changing checkpoint format requires migration of all existing checkpoints
- **D-15:** Checkpoint writes in same SQLite transaction as event append; guaranteed consistency, no partial state — **Reversibility:** costly — moving to async/separate transactions would require consistency guarantees redesign
- **D-16:** Keep last N checkpoints (configurable, e.g., 10); auto-delete older on new checkpoint write; bounded disk usage — **Reversibility:** reversible — retention policy is a configuration change

### Type File Structure
- **D-17:** 18 domain types organized by six-plane alignment: `domain.go` (Project, Repository, Workspace, Session, Run), `planning.go` (Intent, Requirement, Decision, Plan, Task, TaskGraph), `execution.go` (Agent, Tool, Capability, PermissionPolicy), `assurance.go` (Artifact, Verification, Checkpoint), `event.go` (Event, EventType, EventMetadata), `serialization.go` (JSON helpers) — **Reversibility:** costly — reorganizing types affects imports across all layers
- **D-18:** Event types (`EventType`, `Event`, `EventMetadata`) in `internal/core/types/event.go`; shared vocabulary for downstream agents — **Reversibility:** costly — moving to eventstore would require import updates everywhere
- **D-19:** `EventStore` interface in `internal/core/types/eventstore.go`; `SQLiteEventStore` implementation in `internal/memory/eventstore`; enables testing and future backends — **Reversibility:** one-way — interface definition is a contract; changing it breaks implementations
- **D-20:** Manual JSON marshaling/unmarshaling for all types (no code generation); explicit, no build dependencies, clear control; 18 types manageable — **Reversibility:** reversible — adding code generation later is additive

### the agent's Discretion
- Exact checkpoint interval (N events) and retention count (N) — tunable via config
- Exact `busy_timeout` value in DSN (current: 5000ms) — tune based on observed contention
- Migration archive timestamp format — implementation detail

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Architecture & Domain Model
- `.planning/CONTEXT_M31A.md` §7 — Six-plane architecture (Interaction, Intelligence, Engineering, Execution, Assurance, Memory)
- `.planning/CONTEXT_M31A.md` §8 — Core domain model (18 explicit types with ownership boundaries)
- `.planning/CONTEXT_M31A.md` §11 — Event model (35+ event types, monotonic ordering, append-only)
- `.planning/CONTEXT_M31A.md` §12 — Project memory: `.m31a/` structure, granular Git-tracking policy
- `.planning/CONTEXT_M31A.md` §34 — NVIDIA Build integration (provider, auth, streaming, reasoning)
- `.planning/PROJECT.md` — Validated/Active/Out-of-scope requirements, constraints, key decisions
- `.planning/REQUIREMENTS.md` — 118 v1 requirements with traceability (DOMAIN-01..04, PERSIST-01..07)
- `.planning/ROADMAP.md` — Phase 1 success criteria (5 criteria), dependencies, plans
- `.planning/STATE.md` — Project decisions D-001..D-014, active todos, session continuity

### Research & Risks
- `.planning/research/SUMMARY.md` — Research synthesis: stack, table stakes, pitfalls, confidence
- `.planning/research/STACK.md` — Standard stack: Go 1.26.5, modernc.org/sqlite v1.57.0, koanf/v2 v2.3.6, Bubble Tea v1.3.0
- `.planning/research/PITFALLS.md` — 30 domain-specific pitfalls with warning signs, prevention, phase mapping
- `.planning/research/ARCHITECTURE.md` — Six-plane architecture, component boundaries, data flows, migration strategy

### Codebase Patterns
- `.planning/codebase/STACK.md` — Current implementation stack (Bubble Tea v1.3.0, Bubbles, Lipgloss, Glamour)
- `.planning/codebase/ARCHITECTURE.md` — Current architecture: 7-phase workflow engine, provider registry, tools dispatcher, session manager
- `.planning/codebase/CONVENTIONS.md` — Coding conventions: Go 1.26.5, Bubble Tea Elm architecture, error handling, module design
- `internal/core/types/types.go` — Existing types: RiskLevel, WorkflowPhase, IntentType, Task, Session, Event, Tool, ModelInfo
- `internal/core/config/loader.go` — Config loading: layered TOML/env/flags, koanf/v2, project-local `.m31a/`
- `internal/integrations/keychain/keychain.go` — OS-native keychain: Get/Set/Delete, cached with blacklist TTL
- `internal/integrations/git/git.go` — Git client: status, diff, commit, worktree management

### Validation & Testing
- `.planning/phases/01-foundation-domain-model-event-store/01-VALIDATION.md` — 11 task verification map, Wave 0 requirements, sampling rate

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **OS keychain implementation** (`internal/integrations/keychain/keychain.go`): Get/Set/Delete with platform-specific backends (macOS security, Windows Credential Manager, Linux D-Bus Secret Service); cached with 5min blacklist TTL — use for NVIDIA_API_KEY storage
- **Config loader** (`internal/core/config/loader.go`): Layered loading (defaults → global TOML → workspace TOML → M31A_ env vars → CLI flags); TOML parsing via BurntSushi/toml; post-unmarshal validation — extend for new config sections
- **Git client** (`internal/integrations/git/git.go`): Repository operations (status, diff, commit, branch, worktree); used by workflow engine — extend for semantic commit composition
- **Existing domain types** (`internal/core/types/types.go`): RiskLevel, WorkflowPhase, IntentType, Task, Session, ProjectState, Event, Tool, ToolError, HealReport, Message, StreamChunk — extend rather than replace

### Established Patterns
- **Bubble Tea Elm architecture**: All state mutations in `Update()`, never from goroutines; `tea.Cmd`/`tea.Msg` for async work — event store must follow this for TUI integration
- **Lazy provider initialization**: Providers registered on first LLM call, not at startup — config loader follows similar pattern
- **Project-local sessions**: Session data in `<workDir>/.m31a/` (not global `~/.m31a/`); enables per-project isolation — event store follows this pattern
- **Structured error handling**: `errors.Wrap()` from `internal/core/errors/errors.go`; sentinel errors for common failures; ToolError with hint for LLM recovery — event store errors should follow this
- **Single-threaded UI with message passing**: Goroutines send `tea.Msg` via channels; never mutate state directly — event store subscriptions must use this pattern

### Integration Points
- **Config → EventStore**: Config loader provides `.m31a/events.db` path, WAL settings, backup interval
- **Keychain → Config**: Provider API keys read from keychain at runtime, never persisted to config.toml
- **EventStore → TUI**: TUI subscribes to normalized events via `Subscribe(ctx, afterSeq)`; receives `ProjectInitialized`, `RequirementCreated`, `RunCompleted`, etc.
- **Migration → EventStore**: Migration reads `.planning/` artifacts, emits events via `EventStore.Append()`, writes projections to `.m31a/`
- **EventStore → Projections**: Projection rebuild reads events via `QueryByType`, `QueryByRun`, `QueryBySession`; writes human-readable files to `.m31a/`

</code_context>

<specifics>
## Specific Ideas

- Event schema evolution: `EventMetadata` with `schema_version`, `tags`, `source` fields for future extensibility
- Migration CLI: `m31a migrate [--replay] [--dry-run]` — replays events to rebuild projections, validates completeness
- Hot backup CLI: `m31a backup <dst>` — uses modernc.org/sqlite NewBackup with incremental Step() for non-blocking backup
- Projection rebuild: `m31a project --rebuild` — replays from last checkpoint + incremental events

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

### Reviewed Todos (not folded)
- `internal/core/types/types.go` extension vs replacement — deferred to planner (D-17 covers organization)

</deferred>

---

*Phase: 01-Foundation — Domain Model & Event Store*
*Context gathered: 2026-08-23*