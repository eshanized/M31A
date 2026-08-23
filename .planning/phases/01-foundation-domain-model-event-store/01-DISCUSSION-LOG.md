# Phase 01: Foundation — Domain Model & Event Store - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-08-23
**Phase:** 01-foundation-domain-model-event-store
**Areas discussed:** Event versioning strategy, Concurrency model, Migration completeness, Projection snapshots, Type file structure

---

## Event versioning strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Schema version in metadata (Recommended) | Add `schema_version` to `EventMetadata`; increment on payload changes; projections handle versions | ✓ |
| Separate event type per version | Separate event types per version (RequirementCreated_v1, RequirementCreated_v2) | |
| Type-per-field with optional JSON | No version field; optional JSON fields with defaults; simpler but less explicit | |
| You decide | I'll describe my preference | |

**User's choice:** Schema version in metadata (Recommended)
**Notes:** Chosen for explicit version tracking and projection compatibility

---

## Event versioning strategy (Q2)

| Option | Description | Selected |
|--------|-------------|----------|
| Graceful degradation (Recommended) | Projection reads known fields, ignores unknown, logs warning; continues building state | ✓ |
| Strict validation | Projection fails hard on unknown schema version; requires code update | |
| Skip and continue | Projection skips events with newer schema entirely; may result in incomplete state | |
| You decide | I'll describe my preference | |

**User's choice:** Graceful degradation (Recommended)
**Notes:** Chosen for resilience during schema evolution

---

## Event versioning strategy (Q3)

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, include migration tools (Recommended) | `migrate-events` command to upgrade events in-place; essential for long-lived event store | ✓ |
| No, never modify events | Events are immutable; write new correction events; migration operates on projections only | |
| Only backfill metadata | Add `schema_version` to old events (safe no-op), but never transform payloads | |
| You decide | I'll describe my preference | |

**User's choice:** Yes, include migration tools (Recommended)
**Notes:** Chosen for long-term event store maintainability

---

## Event versioning strategy (Q4)

| Option | Description | Selected |
|--------|-------------|----------|
| With domain types (Recommended) | Event types are part of shared vocabulary; `internal/core/types/event.go` keeps all domain types together | ✓ |
| In eventstore package | Event types are implementation detail of event store; `internal/memory/eventstore/types.go` encapsulates them | |
| Both | Public event types in `internal/core/types/event.go`; eventstore has internal variants with store-specific fields | |
| You decide | I'll describe my preference | |

**User's choice:** With domain types (Recommended)
**Notes:** Chosen for shared vocabulary across all downstream agents

---

## Concurrency model

| Option | Description | Selected |
|--------|-------------|----------|
| Single-writer, multi-reader (Recommended) | One M31A process writes; multiple read via WAL; advisory lock on write; matches single-session assumption | ✓ |
| Per-session database | Each session gets own `.m31a/sessions/<id>/events.db`; no writer contention; complicates cross-session queries | |
| Global lock with queue | SQLite serialized writes via mutex; simple but limits throughput; fine for v1 scale | |
| You decide | I'll describe my preference | |

**User's choice:** Single-writer, multi-reader (Recommended)
**Notes:** Matches single-session assumption and leverages WAL mode

---

## Concurrency model (Q2)

| Option | Description | Selected |
|--------|-------------|----------|
| Fail fast with clear error (Recommended) | Return `ErrWriteConflict` with hint: "Another M31A instance is running; close it or use different workspace" | ✓ |
| Wait with timeout | Block for N seconds (configurable), then fail; allows brief overlaps but adds latency | |
| Auto-create isolated workspace | Detect conflict, create `worktree/m31a/<run-id>-<timestamp>` with separate `.m31a/`; seamless but complex | |
| You decide | I'll describe my preference | |

**User's choice:** Fail fast with clear error (Recommended)
**Notes:** Clear user action required; no silent failures

---

## Concurrency model (Q3)

| Option | Description | Selected |
|--------|-------------|----------|
| Process-level lock (Recommended) | Single `flock` on `.m31a/events.db`; one writer at a time regardless of run; simpler | ✓ |
| Per-run advisory lock | SQLite advisory locks; different runs interleave writes if no conflict; more complex | |
| Per-session file | Each session gets `.m31a/sessions/<id>/events.db`; natural isolation; complexity for cross-session queries | |
| You decide | I'll describe my preference | |

**User's choice:** Process-level lock (Recommended)
**Notes:** Simpler, matches single-session assumption

---

## Concurrency model (Q4)

| Option | Description | Selected |
|--------|-------------|----------|
| SQLite built-in (Recommended) | WAL mode already serializes writers; `busy_timeout=5000` in DSN handles contention; no extra code | ✓ |
| Go `flock` wrapper | Explicit `flock` on `.m31a/events.db.lock` file; more control; useful for cross-platform | |
| Both | SQLite for normal operation + Go `flock` as safety net; defense in depth | |
| You decide | I'll describe my preference | |

**User's choice:** SQLite built-in (Recommended)
**Notes:** Leverages WAL mode's built-in serialization; no additional code

---

## Migration completeness

| Option | Description | Selected |
|--------|-------------|----------|
| All substantive artifacts (Recommended) | REQUIREMENTS.md, ROADMAP.md, PROJECT.md, STATE.md, CONTEXT.md, decisions/, research/, codebase/ maps; each → events | ✓ |
| Core only | REQUIREMENTS.md, ROADMAP.md, PROJECT.md, STATE.md; skip decisions/, research/, codebase/ maps | |
| Everything including transient | Include graphs/, caches, .env files; maximum completeness but pollutes event log | |
| You decide | I'll describe my preference | |

**User's choice:** All substantive artifacts (Recommended)
**Notes:** Full traceability and zero data loss

---

## Migration completeness (Q2)

| Option | Description | Selected |
|--------|-------------|----------|
| One event per artifact (Recommended) | `RequirementCreated` × 118, `PhaseCreated` × 12, `DecisionLogged` × N, `ResearchCompleted` × 4, etc.; full traceability | ✓ |
| Single summary event | One `MigrationCompleted` with counts; simpler but loses per-item traceability | |
| Both | Per-artifact events + summary `MigrationCompleted`; best of both but more events | |
| You decide | I'll describe my preference | |

**User's choice:** One event per artifact (Recommended)
**Notes:** Full traceability and queryability

---

## Migration completeness (Q3)

| Option | Description | Selected |
|--------|-------------|----------|
| Strict (Recommended) | Fail fast on any unparsable requirement/decision; forces manual fix; guarantees data integrity | ✓ |
| Lenient | Log warnings for unparsable items, continue with valid ones; incomplete but unblocked | |
| Interactive | Pause on each error, let user decide fix/skip/abort; maximum control but slow | |
| You decide | I'll describe my preference | |

**User's choice:** Strict (Recommended)
**Notes:** Guarantees data integrity

---

## Migration completeness (Q4)

| Option | Description | Selected |
|--------|-------------|----------|
| Archive with timestamp (Recommended) | Move to `.planning.archived.<timestamp>/`; preserves history, `.planning/` available for new projects | ✓ |
| Delete | Remove entirely; clean break; `.planning/` only for new projects | |
| Keep as read-only | Leave `.planning/` in place but document `.m31a/` is authoritative; risk of confusion | |
| You decide | I'll describe my preference | |

**User's choice:** Archive with timestamp (Recommended)
**Notes:** Preserves history; explicit about migration

---

## Projection snapshots

| Option | Description | Selected |
|--------|-------------|----------|
| Every N events (Recommended) | Configurable (e.g., every 100 events); balance rebuild speed and write overhead; simple | ✓ |
| Time-based | Every T seconds (e.g., 30s); guarantees bounded rebuild time regardless of event rate | |
| On significant events | Only after `RunCompleted`, `PhaseCompleted`, `SessionCreated`; fewer snapshots but longer replay | |
| You decide | I'll describe my preference | |

**User's choice:** Every N events (Recommended)
**Notes:** Simple, tunable balance

---

## Projection snapshots (Q2)

| Option | Description | Selected |
|--------|-------------|----------|
| Projected state JSON + last seq (Recommended) | Store full projection output (requirements.md, decisions, etc.) + `last_seq`; fast resume | ✓ |
| Last seq only | Only store `last_seq`; always rebuild from scratch; simplest but slower | |
| Delta events | Store events since last checkpoint; more complex but enables partial rebuild | |
| You decide | I'll describe my preference | |

**User's choice:** Projected state JSON + last seq (Recommended)
**Notes:** Fast resume by loading state + replaying from `last_seq+1`

---

## Projection snapshots (Q3)

| Option | Description | Selected |
|--------|-------------|----------|
| Same transaction (Recommended) | Append event + update projection checkpoint in same SQLite transaction; guaranteed consistency | ✓ |
| Separate transaction | Event appended first, checkpoint updated after; slight risk of checkpoint lag; simpler errors | |
| Async/background | Event appended, checkpoint updated in background; fastest writes; eventual consistency | |
| You decide | I'll describe my preference | |

**User's choice:** Same transaction (Recommended)
**Notes:** Guaranteed consistency; no partial state

---

## Projection snapshots (Q4)

| Option | Description | Selected |
|--------|-------------|----------|
| Keep last N (Recommended) | Configurable (e.g., keep last 10); auto-delete older on new checkpoint; bounded disk | ✓ |
| Keep all | Never delete; full history of projection states; unbounded growth | |
| Time-based retention | Keep checkpoints from last T days (e.g., 7 days); auto-cleanup by timestamp | |
| You decide | I'll describe my preference | |

**User's choice:** Keep last N (Recommended)
**Notes:** Bounded disk usage; tunable

---

## Type file structure

| Option | Description | Selected |
|--------|-------------|----------|
| By six-plane alignment (Recommended) | `domain.go`, `planning.go`, `execution.go`, `assurance.go`, `event.go`, `serialization.go`; mirrors architecture | ✓ |
| By layer | `types.go` (core), `events.go` (events), `config.go` (config); current codebase style | |
| Single file with sections | One `types.go` with `// Domain`, `// Planning`, `// Execution`, `// Assurance`, `// Events` comment headers | |
| You decide | I'll describe my preference | |

**User's choice:** By six-plane alignment (Recommended)
**Notes:** Mirrors six-plane architecture; clear ownership boundaries

---

## Type file structure (Q2)

| Option | Description | Selected |
|--------|-------------|----------|
| In internal/core/types (Recommended) | Event types are shared vocabulary; downstream agents need them; eventstore imports from core | ✓ |
| In internal/memory/eventstore | Event types are implementation detail of event store; core stays pure domain | |
| Both | Public event types in `internal/core/types/event.go`; eventstore has internal variants with `Seq` field | |
| You decide | I'll describe my preference | |

**User's choice:** In internal/core/types (Recommended)
**Notes:** Shared vocabulary for downstream agents

---

## Type file structure (Q3)

| Option | Description | Selected |
|--------|-------------|----------|
| Interfaces in core, impl in eventstore (Recommended) | `EventStore` interface in `internal/core/types/eventstore.go`; `SQLiteEventStore` in `internal/memory/eventstore`; enables testing, future backends | ✓ |
| Only in eventstore | `EventStore` interface in `internal/memory/eventstore/eventstore.go`; less public API surface | |
| No interface | Direct struct usage; simplest but couples to SQLite; not testable with mocks | |
| You decide | I'll describe my preference | |

**User's choice:** Interfaces in core, impl in eventstore (Recommended)
**Notes:** Enables testing and future backends

---

## Type file structure (Q4)

| Option | Description | Selected |
|--------|-------------|----------|
| Manual (Recommended) | Current codebase uses manual `MarshalJSON`/`UnmarshalJSON`; explicit, no build deps; 18 types manageable | ✓ |
| Code generation | Use `stringer` for `EventType` string conversion, `jsonenums` for enum JSON; reduces boilerplate | |
| Hybrid | Generate for enums only (`EventType`, `RiskLevel`, `WorkflowPhase`); manual for structs | |
| You decide | I'll describe my preference | |

**User's choice:** Manual (Recommended)
**Notes:** Explicit, no build dependencies, matches current codebase style

---

## the agent's Discretion

- Exact checkpoint interval (N events) and retention count (N) — tunable via config
- Exact `busy_timeout` value in DSN (current: 5000ms) — tune based on observed contention
- Migration archive timestamp format — implementation detail

---

## Deferred Ideas

None — discussion stayed within phase scope.