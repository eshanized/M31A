---
phase: 01-foundation-domain-model-event-store
plan: 01
subsystem: types
tags:
  - domain-model
  - event-sourcing
  - types
  - serialization
requires:
  - DOMAIN-01
  - DOMAIN-02
  - DOMAIN-03
  - DOMAIN-04
provides:
  - 18 canonical domain types
  - event envelope with monotonic SEQ
  - EventStore interface
  - JSON serialization helpers
affects:
  - internal/core/types
  - internal/core/errors
tech-stack:
  added:
    - github.com/google/uuid
  patterns:
    - six-plane domain organization
    - event-sourced architecture
    - JSON marshaling with nil-slice safety
key-decisions:
  - 18 types organized by six-plane: domain.go (Interaction/Memory), planning.go (Engineering), execution.go (Execution), assurance.go (Assurance)
  - IntentType, ComplexityLevel, RunStatus, TaskGraphStatus, AgentRole, VerificationLevel, CheckpointType, ArtifactType enums defined
  - EventStore interface with Append, Query, Subscribe, Backup
  - Sentinel errors for event store operations
requirements-completed:
  - DOMAIN-01
  - DOMAIN-02
  - DOMAIN-03
  - DOMAIN-04
duration: 45 min
completed: "2026-08-24T00:00:00.000Z"
---

# Phase 01 Plan 01: Domain Types Summary

## One-liner
Created 18 canonical domain types organized by six-plane architecture, event envelope with monotonic sequencing, EventStore interface, and JSON serialization helpers — establishing the type vocabulary for all downstream planes.

## Duration
- Start: 2026-08-23T23:15:00Z
- End: 2026-08-24T00:00:00Z
- Duration: 45 min

## Task Count
3 tasks completed

## Files Created/Modified
**Created (9 files):**
- `internal/core/types/domain.go` — Project, Repository, Workspace, Session, Run (Interaction/Memory planes)
- `internal/core/types/planning.go` — Intent, Requirement, Decision, Plan, Task, TaskGraph (Engineering plane)
- `internal/core/types/execution.go` — Agent, ToolSpec, Capability, PermissionPolicy (Execution plane)
- `internal/core/types/assurance.go` — Artifact, Verification, Checkpoint, Research (Assurance plane)
- `internal/core/types/event.go` — Event envelope, 35+ EventType constants, EventMetadata, Query
- `internal/core/types/eventstore.go` — EventStore interface + sentinel errors
- `internal/core/types/serialization.go` — MarshalEventPayload, UnmarshalEventPayload generic helpers
- `internal/core/types/domain_test.go` — Unit tests for DOMAIN-01..03
- `internal/core/types/event_test.go` — Unit tests for DOMAIN-04, event serialization

**Modified (2 files):**
- `internal/core/types/types.go` — Removed legacy types, kept shared utilities (RiskLevel, WorkflowPhase, IntentResult, Tool interface, etc.)
- `internal/core/types/plan.go` — Renamed legacy Plan to PlanDocument for backward compatibility
- `internal/core/errors/errors.go` — Added event store sentinel errors

**Updated references (7 files):**
- `internal/engine/workflow/coverage_gates.go` — Updated to use PlanDocument
- `internal/engine/workflow/execute.go` — Updated to use PlanDocument
- `internal/engine/workflow/plan.go` — Updated to use PlanDocument
- `internal/engine/workflow/plan_check.go` — Updated to use PlanDocument
- `internal/engine/workflow/plan_parser.go` — Updated to return PlanDocument
- `internal/engine/workflow/replan.go` — Updated to use PlanDocument
- `internal/engine/workflow/workflow_cache.go` — Updated to cache PlanDocument

## Accomplishments

### Task 1: Create 18 domain types in six-plane organized files
- **domain.go (5 types)**: Project, Repository, Workspace, Session, Run — Interaction/Memory planes
- **planning.go (6 types)**: Intent, Requirement, Decision, Plan, Task, TaskGraph — Engineering plane  
- **execution.go (4 types)**: Agent, ToolSpec, Capability, PermissionPolicy — Execution plane
- **assurance.go (4 types)**: Artifact, Verification, Checkpoint, Research — Assurance plane
- **Enums**: IntentType, ComplexityLevel, RunStatus, TaskGraphStatus, AgentRole, Capability, PermissionPolicy, ResourceScope, MutationClass, RequirementStatus, DecisionStatus, PlanStatus, TaskStatus, ArtifactType, VerificationLevel, VerificationVerdict, CheckpointType, CheckpointStatus
- All types use uuid.UUID for IDs, time.Time for timestamps, proper JSON tags with omitempty for optional fields
- Custom MarshalJSON methods ensure nil slices serialize as empty arrays

### Task 2: Create event envelope, event types, and EventStore interface
- **Event struct**: ID, Seq (monotonic int64), Type (EventType), Timestamp, RunID, SessionID, Payload (json.RawMessage), Metadata (EventMetadata)
- **35+ EventType constants**: ProjectInitialized through BackupCompleted + migration events
- **EventMetadata**: SchemaVersion, Tags, Source for extensibility
- **EventStore interface**: Append, Query, Subscribe, Backup, Close with context.Context
- **Query struct**: RunID, SessionID, Type, AfterSeq, BeforeSeq, Limit, Offset for flexible querying
- **Sentinel errors**: ErrEventStoreUnavailable, ErrEventNotFound, ErrInvalidEventPayload, ErrMigrationFailed, ErrWriteConflict, ErrBackupFailed, ErrProjectionNotFound, ErrProjectionApplyFailed, ErrArtifactParseFailed, ErrKeyNotFound, ErrKeychainUnavailable

### Task 3: Write unit tests for domain types and event serialization
- **domain_test.go**: TestDomainTypes (all 18 types instantiate), TestOwnership (all in types package), TestSerialization (JSON round-trip for all types with nil-slice handling), TestEnums (all enum constants)
- **event_test.go**: TestEventEnvelope (full event structure), TestEventTypes (35+ unique types), TestEventMetadata (schema_version, tags, source), TestEventStoreInterface (compile-time check), TestEventSerialization (MarshalEventPayload/UnmarshalEventPayload round-trip), TestQuery (filter combinations)
- All tests pass: `go test ./internal/core/types/... -short`

## Verification
- `go build ./internal/core/types/...` — PASS
- `go test ./internal/core/types/... -short` — PASS (11 tests)
- All DOMAIN-01..04 requirements covered by tests

## Deviations from Plan
None - plan executed exactly as written.

## Total Deviations
0 auto-fixed. Impact: None.

## Next Steps
Ready for Plan 01-02: Config system with EventStore/Migration sections, layered loading with koanf/v2, OS keychain integration for API keys.