---
phase: 01-foundation-domain-model-event-store
plan: 03
subsystem: database
tags: [sqlite, wal, event-sourcing, monotonic-seq, modernc.org/sqlite]

# Dependency graph
requires:
  - phase: 01
    provides: [Domain types (Event, EventType, EventMetadata, Query), EventStore interface, Error types]
  - phase: 02
    provides: [EventStore config section (path, WAL mode, busy timeout), Config validation]
provides:
  - SQLiteEventStore implementation with WAL mode
  - Schema with events, projections, migrations tables and indexes
  - Transactional AppendEvents with monotonic SEQ
  - Query with filtering (run_id, session_id, type, seq range) and pagination
affects: [01-04-subscription-backup, 01-05-projection-artifacts, 01-06-migration-engine]

# Actuals
actuals:
  tokens: 42000
  tasks: 3
  commits: 4

# Tech tracking
tech-stack:
  added: [modernc.org/sqlite v1.57.0]
  patterns: [WAL mode for concurrent reads/writes, AUTOINCREMENT for monotonic ordering, Parameterized queries for SQL injection prevention, Composite indexes for range queries]

key-files:
  created:
    - internal/memory/eventstore/schema.sql
    - internal/memory/eventstore/eventstore.go
    - internal/memory/eventstore/append.go
    - internal/memory/eventstore/query.go
    - internal/memory/eventstore/eventstore_test.go
  modified:
    - go.mod, go.sum (added modernc.org/sqlite)

key-decisions:
  - "Use modernc.org/sqlite (pure Go, no CGO) for portability"
  - "WAL mode with busy_timeout=5000 for concurrent access"
  - "Monotonic SEQ via SQLite AUTOINCREMENT (not manually assigned)"
  - "Composite indexes (run_id, seq), (session_id, seq), (type, seq) for efficient range queries"
  - "Parameterized queries only - no string interpolation for SQL injection prevention"
  - "Event timestamp stored as Unix nanoseconds (int64) for precision"
  - "Schema executed at NewEventStore initialization"
  - "Subscribe and Backup stubbed for Plan 04 implementation"

patterns-established:
  - "Embedded schema SQL as string constant in eventstore.go"
  - "Transaction-per-batch with automatic rollback on error"
  - "Dynamic SQL building with ? placeholders for Query filters"
  - "sql.NullString for nullable UUID columns"
  - "json.RawMessage for Payload, custom unmarshal for Metadata"
  - "Error wrapping with coreerrors.Wrap/Wrapf for context"
  - "Test helper newTestStore for isolated temp databases"

requirements-completed:
  - PERSIST-01
  - PERSIST-02

coverage:
  - id: D1
    description: "SQLite schema with WAL mode, events/projections/migrations tables, and composite indexes"
    requirement: "PERSIST-01"
    verification:
      - kind: unit
        ref: "internal/memory/eventstore/eventstore_test.go#TestWALMode"
        status: pass
    human_judgment: false
  - id: D2
    description: "NewEventStore initializes DB with WAL mode, busy_timeout=5000, foreign_keys=ON"
    requirement: "PERSIST-01"
    verification:
      - kind: unit
        ref: "internal/memory/eventstore/eventstore_test.go#TestWALMode"
        status: pass
    human_judgment: false
  - id: D3
    description: "AppendEvents writes batch atomically in transaction, assigns monotonic SEQ via AUTOINCREMENT"
    requirement: "PERSIST-02"
    verification:
      - kind: unit
        ref: "internal/memory/eventstore/eventstore_test.go#TestAppendBatch"
        status: pass
      - kind: unit
        ref: "internal/memory/eventstore/eventstore_test.go#TestAppendSingle"
        status: pass
      - kind: unit
        ref: "internal/memory/eventstore/eventstore_test.go#TestAppendEmpty"
        status: pass
    human_judgment: false
  - id: D4
    description: "Query supports all filter combinations (run_id, session_id, type, after_seq, before_seq) with pagination (limit/offset)"
    requirement: "PERSIST-02"
    verification:
      - kind: unit
        ref: "internal/memory/eventstore/eventstore_test.go#TestQueryByRunID"
        status: pass
      - kind: unit
        ref: "internal/memory/eventstore/eventstore_test.go#TestQueryBySessionID"
        status: pass
      - kind: unit
        ref: "internal/memory/eventstore/eventstore_test.go#TestQueryByType"
        status: pass
      - kind: unit
        ref: "internal/memory/eventstore/eventstore_test.go#TestQueryAfterSeq"
        status: pass
      - kind: unit
        ref: "internal/memory/eventstore/eventstore_test.go#TestQueryBeforeSeq"
        status: pass
      - kind: unit
        ref: "internal/memory/eventstore/eventstore_test.go#TestQueryLimit"
        status: pass
      - kind: unit
        ref: "internal/memory/eventstore/eventstore_test.go#TestQueryOffset"
        status: pass
      - kind: unit
        ref: "internal/memory/eventstore/eventstore_test.go#TestQueryCombined"
        status: pass
      - kind: unit
        ref: "internal/memory/eventstore/eventstore_test.go#TestQueryEmpty"
        status: pass
    human_judgment: false
  - id: D5
    description: "QueryByRun convenience method for projection replay"
    requirement: "PERSIST-02"
    verification:
      - kind: unit
        ref: "internal/memory/eventstore/eventstore_test.go#TestQueryByRun"
        status: pass
    human_judgment: false

duration: 55min
completed: 2026-08-24
status: complete
---

# Phase 01 Plan 03: SQLite Event Store Core

**Implemented SQLite event store with WAL mode, monotonic SEQ, transactional batch append, and indexed range queries — the durability foundation for all downstream planes**

## Performance

- **Duration:** 55 min
- **Started:** 2026-08-24T03:25:00Z
- **Completed:** 2026-08-24T04:20:00Z
- **Tasks:** 3
- **Files modified:** 5 created, 2 modified (go.mod/go.sum)

## Accomplishments
- SQLite schema with WAL mode, foreign keys, busy_timeout=5000, and composite indexes for efficient range queries
- SQLiteEventStore implementation with NewEventStore initialization executing embedded schema
- Transactional AppendEvents with batch support, automatic UUID/Timestamp assignment, and monotonic SEQ via AUTOINCREMENT
- Query engine with dynamic SQL building supporting all filter combinations (run_id, session_id, type, seq range) and pagination
- Comprehensive test suite covering PERSIST-01 (schema/init) and PERSIST-02 (append/query) with 19 test cases

## Task Commits

Each task was committed atomically:

1. **Task 1: Create SQLite schema and EventStore initialization** - `feat(01-03): create SQLite schema and EventStore initialization with WAL mode`
2. **Task 2: Implement transactional batch append with monotonic SEQ** - `test(01-03): add AppendEvents tests for PERSIST-02`
3. **Task 3: Implement range queries with filtering and pagination** - `feat(01-03): implement Query with filtering, pagination, and QueryByRun helper`

**Plan metadata:** `docs(01-03): complete 01-03 plan summary`

## Files Created/Modified
- `internal/memory/eventstore/schema.sql` - Embedded schema with events, projections, migrations tables and indexes
- `internal/memory/eventstore/eventstore.go` - SQLiteEventStore struct, NewEventStore, schemaSQL constant, Close/Subscribe/Backup stubs
- `internal/memory/eventstore/append.go` - AppendEvents with transaction, batch support, parameterized INSERT
- `internal/memory/eventstore/query.go` - Query with dynamic WHERE clauses, ORDER BY seq ASC, LIMIT/OFFSET, QueryByRun helper
- `internal/memory/eventstore/eventstore_test.go` - 19 tests for PERSIST-01 (schema/init) and PERSIST-02 (append/query)
- `go.mod, go.sum` - Added modernc.org/sqlite v1.57.0

## Decisions Made
- Use modernc.org/sqlite (pure Go, no CGO) for portability and static binary compatibility
- WAL mode with busy_timeout=5000 for concurrent reads/writes with single-writer model
- Monotonic SEQ assigned by SQLite AUTOINCREMENT — not manually set, ensuring durability ordering
- Composite indexes (run_id, seq), (session_id, seq), (type, seq) for efficient range queries by any dimension
- Parameterized queries only — no string interpolation, preventing SQL injection
- Schema executed at NewEventStore initialization; Subscribe and Backup stubbed for Plan 04
- Error wrapping with coreerrors.Wrap/Wrapf for consistent error context

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed timestamp scanning from int64 to time.Time**
- **Found during:** TestQueryAll - scan error storing driver.Value type int64 into *time.Time
- **Issue:** SQLite returns timestamp as int64 (Unix nanos), but Event.Timestamp is time.Time
- **Fix:** Scan timestamp into int64 variable, then convert with time.Unix(0, timestamp).UTC()
- **Files modified:** internal/memory/eventstore/query.go
- **Verification:** All Query tests pass
- **Committed in:** feat(01-03): fix timestamp scanning in Query

**2. [Rule 2 - Missing Critical] Initialize events slice as empty not nil**
- **Found during:** TestQueryEmpty - expected non-nil slice
- **Issue:** var events []types.Event is nil when no rows returned
- **Fix:** Initialize with events := make([]types.Event, 0)
- **Files modified:** internal/memory/eventstore/query.go
- **Verification:** TestQueryEmpty passes
- **Committed in:** feat(01-03): return empty slice not nil from Query

**3. [Rule 3 - Blocking] Removed redundant type assertion in test helper**
- **Found during:** Build failure - invalid operation: store (type *SQLiteEventStore) is not an interface
- **Issue:** NewEventStore returns concrete *SQLiteEventStore, not interface
- **Fix:** Removed unnecessary type assertion in newTestStore helper
- **Files modified:** internal/memory/eventstore/eventstore_test.go
- **Verification:** All tests compile and pass
- **Committed in:** test(01-03): fix test helper type assertion

---

**Total deviations:** 3 auto-fixed (1 bug, 1 missing critical, 1 blocking)
**Impact on plan:** All auto-fixes essential for correctness and test reliability. No scope creep.

## Issues Encountered
- modernc.org/sqlite returns timestamp as int64 requiring conversion to time.Time — handled in Query scan
- Empty query results return nil slice by default — fixed by initializing as empty slice
- Test helper type assertion redundant since NewEventStore returns concrete type — removed

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Event store core complete and tested (PERSIST-01, PERSIST-02)
- Ready for Plan 01-04: Event subscription (TUI real-time) and hot backup
- Ready for Plan 01-05: Projection manager with checkpoints and artifact writers
- Ready for Plan 01-06: Migration engine using projections table

---
*Phase: 01-foundation-domain-model-event-store*
*Completed: 2026-08-24*