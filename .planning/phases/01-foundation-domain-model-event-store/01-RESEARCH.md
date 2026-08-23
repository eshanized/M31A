# Phase 1: Foundation — Domain Model & Event Store - Research

**Researched:** 2026-08-23
**Domain:** Canonical domain modeling, event sourcing, SQLite persistence, configuration management
**Confidence:** HIGH

## Summary

Phase 1 establishes the architectural foundation for M31A by defining canonical domain types and an authoritative SQLite event store. All durable state in the system derives from append-only events; the TUI and other consumers are pure projections reading from this single source of truth. The phase also implements configuration management via `.m31a/config.toml` (using koanf/v2 with layered providers), OS-native keychain integration for secrets, and a zero-data-loss migration from the existing `.planning/` directory structure.

**Primary recommendation:** Build `internal/core/types` with 18 explicit domain types (Project, Repository, Workspace, Session, Run, Intent, Requirement, Decision, Plan, Task, TaskGraph, Agent, Tool, Capability, PermissionPolicy, Artifact, Verification, Checkpoint, Event), implement `internal/memory/eventstore` using modernc.org/sqlite with WAL mode and the online backup API, and create `internal/core/config` using koanf/v2 for layered TOML/env/flag configuration. Migration from `.planning/` to `.m31a/` should replay all existing data as events to preserve traceability.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Domain type definitions | API / Backend | — | Core domain vocabulary owned by engineering plane; shared across all tiers via `internal/core/types` |
| Event store persistence | Database / Storage | — | SQLite `events.db` is the single source of truth; Memory plane owns durability |
| Event append/query API | API / Backend | — | Engineering plane exposes event store operations; Execution plane consumes |
| Configuration management | API / Backend | — | Engineering plane owns config schema; loaded at startup, accessible to all planes |
| OS keychain integration | API / Backend | — | Security boundary; Execution plane uses for provider credentials |
| Migration from `.planning/` | API / Backend | Database / Storage | One-time data transformation; Engineering plane orchestrates, Memory plane persists |
| TUI event subscription | Browser / Client | — | Interaction plane subscribes to normalized events; never writes domain state |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Go | 1.26.5 | Primary language | Static binaries, cross-compilation, CGO_ENABLED=0 [VERIFIED: go.mod] |
| modernc.org/sqlite | v1.57.0 | Embedded SQLite driver | Pure Go, WAL mode, transaction hooks, hot backup API [VERIFIED: go list -m -versions] |
| koanf/v2 | v2.3.6 | Configuration management | Layered providers (TOML/env/flags), no global state [VERIFIED: go list -m -versions] |
| github.com/BurntSushi/toml | v1.6.0 | TOML parsing | Required by koanf for config.toml [VERIFIED: go.mod] |
| internal/integrations/keychain | (project) | OS-native secret storage | macOS Keychain, Windows Credential Manager, Linux D-Bus Secret Service [VERIFIED: internal/integrations/keychain/keychain.go:8-42] |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| golang.org/x/sync | v0.22.0 | Concurrency primitives | Singleflight for deduplication, errgroup for parallel migration |
| github.com/google/uuid | v1.3.0 (indirect) | Unique identifiers | Event IDs, session IDs, run IDs [VERIFIED: go.mod indirect] |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| modernc.org/sqlite | github.com/mattn/go-sqlite3 | Requires CGO; violates static binary constraint |
| koanf/v2 | spf13/viper | Heavier, global state, more complex API |
| Custom event store | github.com/EventStore/EventStore | External dependency; overkill for embedded use |
| Custom config | JSON + env parsing | No layered precedence, no validation hooks |

**Installation:**
```bash
go get modernc.org/sqlite@v1.57.0
go get github.com/knadh/koanf/v2@v2.3.6
go get github.com/BurntSushi/toml@v1.6.0
```

**Version verification:**
```bash
go list -m modernc.org/sqlite@v1.57.0      # Published 2024, active maintenance
go list -m github.com/knadh/koanf/v2@v2.3.6  # Published 2024, stable
go list -m github.com/BurntSushi/toml@v1.6.0 # Stable, widely used
```

## Package Legitimacy Audit

> **Note:** Go modules are verified via the Go module proxy (proxy.golang.org) and source repositories, not npm/PyPI. The `gsd-tools package-legitimacy` check targets npm/crates/PyPI ecosystems. All packages below are confirmed to exist on proxy.golang.org with published source repositories.

| Package | Registry | Age | Source Repo | Verdict | Disposition |
|---------|----------|-----|-------------|---------|-------------|
| modernc.org/sqlite | Go module proxy | ~8 years | https://gitlab.com/cznic/sqlite | OK | Approved |
| github.com/knadh/koanf/v2 | Go module proxy | ~5 years | https://github.com/knadh/koanf | OK | Approved |
| github.com/BurntSushi/toml | Go module proxy | ~10 years | https://github.com/BurntSushi/toml | OK | Approved |
| github.com/google/uuid | Go module proxy | ~8 years | https://github.com/google/uuid | OK | Approved |
| golang.org/x/sync | Go module proxy | ~6 years | https://go.googlesource.com/sync | OK | Approved |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

*All Go packages verified via `go list -m` against proxy.golang.org and confirmed with source repositories on GitHub/GitLab.*

## Architecture Patterns

### System Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            M31A Phase 1 Foundation                          │
└─────────────────────────────────────────────────────────────────────────────┘

  ┌──────────────┐     ┌──────────────────┐     ┌────────────────────────┐
  │   CLI/TUI    │────▶│  Engineering     │────▶│     Memory Plane       │
  │  (Commands)  │     │  Plane (Types)   │     │  ┌──────────────────┐  │
  └──────────────┘     └────────┬─────────┘     │  │  Event Store     │  │
        │                       │               │  │  (SQLite WAL)    │  │
        │         Domain Types  │               │  │  • events.db     │  │
        │  (Project, Session,   │               │  │  • Append-only   │  │
        │   Run, Task, Event,   │               │  │  • Monotonic seq │  │
        │   Checkpoint, ...)    │               │  │  • Range queries │  │
        │                       │               │  │  • Hot backup    │  │
        │                       ▼               │  └────────┬─────────┘  │
        │              ┌──────────────────┐     │           │            │
        │              │   Config/Keychain│     │           ▼            │
        │              │  • config.toml   │     │  ┌──────────────────┐  │
        │              │  • OS Keychain   │     │  │  Projections     │  │
        │              └──────────────────┘     │  │  • .m31a/ dir    │  │
        │                                       │  │  • project.md    │  │
        ▼                                       │  │  • decisions/    │  │
  ┌─────────────────────────────────────────────┘  │  │  • research/     │  │
  │              Migration Engine                 │  └──────────────────┘  │
  │  • Reads .planning/                           └────────────────────────┘
  │  • Emits events for each artifact
  │  • Writes .m31a/ projections
  └─────────────────────────────────────────────────────────────────────────────┘
```

### Recommended Project Structure
```
internal/
├── core/
│   ├── types/           # Domain types (18 types + enums)
│   │   ├── domain.go    # Project, Repository, Workspace, Session, Run
│   │   ├── planning.go  # Intent, Requirement, Decision, Plan, Task, TaskGraph
│   │   ├── execution.go # Agent, Tool, Capability, PermissionPolicy
│   │   ├── assurance.go # Artifact, Verification, Checkpoint
│   │   ├── event.go     # Event envelope, event types
│   │   └── serialization.go # JSON marshaling helpers
│   └── config/
│       ├── config.go    # Config struct with koanf tags
│       ├── loader.go    # Layered loading: defaults → file → env → flags
│       └── validation.go # Post-unmarshal validation
├── memory/
│   ├── eventstore/
│   │   ├── eventstore.go    # EventStore interface + SQLite implementation
│   │   ├── schema.sql       # DDL for events table + indexes
│   │   ├── append.go        # AppendEvent, AppendEvents (batch)
│   │   ├── query.go         # QueryByRun, QueryBySession, QueryByType, QueryRange
│   │   ├── projection.go    # Projection interface + rebuild from events
│   │   ├── backup.go        # Hot backup via modernc.org/sqlite NewBackup
│   │   └── migration.go     # Migration from .planning/ to events.db
│   └── artifacts/
│       ├── project_md.go    # project.md read/write
│       ├── decisions.go     # decisions/ directory management
│       └── research.go      # research/ directory management
└── integrations/
    └── keychain/            # (existing) OS-native keychain
```

### Pattern 1: Event-Sourced Domain Model
**What:** All durable state changes are represented as append-only events. Current state is derived by projecting events.
**When to use:** Any domain mutation that must be durable, auditable, and recoverable.

**Example:**
```go
// Source: CONTEXT_M31A.md §11 (Event model) + modernc.org/sqlite transaction hooks
type Event struct {
    ID        uuid.UUID       `json:"id"`
    Seq       int64           `json:"seq"`        // Monotonic ordering
    Type      EventType       `json:"type"`       // e.g., "RequirementCreated"
    Timestamp time.Time       `json:"timestamp"`
    RunID     string          `json:"run_id,omitempty"`
    SessionID string          `json:"session_id,omitempty"`
    Payload   json.RawMessage `json:"payload"`    // Domain-type-specific payload
}

type EventStore interface {
    Append(ctx context.Context, events ...Event) error
    Query(ctx context.Context, q Query) ([]Event, error)
    Subscribe(ctx context.Context, afterSeq int64) (<-chan Event, error)
    Backup(ctx context.Context, dstPath string) error
}

// SQLite implementation uses:
// - WAL mode: "file:.m31a/events.db?_pragma=journal_mode(WAL)&_pragma=busy_timeout(5000)"
// - Monotonic SEQ via INTEGER PRIMARY KEY AUTOINCREMENT
// - Indexes: CREATE INDEX idx_events_run_seq ON events(run_id, seq);
// - Online backup: conn.Raw(func(dc any) { bc, _ := dc.(backuper).NewBackup(dstPath); ... })
```

### Pattern 2: Layered Configuration with koanf
**What:** Configuration loaded from multiple sources with defined precedence: defaults < file < env < flags.
**When to use:** All application configuration (provider keys, execution policies, TUI preferences).

**Example:**
```go
// Source: koanf/v2 docs - hierarchical configuration loading
func LoadConfig() (*Config, error) {
    k := koanf.New(".")
    
    // 1. Defaults (lowest priority)
    k.Load(confmap.Provider(defaultConfigMap, "."), nil)
    
    // 2. Config file: .m31a/config.toml
    k.Load(file.Provider(".m31a/config.toml"), toml.Parser())
    
    // 3. Environment variables: M31A_ prefix
    k.Load(env.Provider("M31A_", ".", env.Opt{Prefix: "M31A_"}), nil)
    
    // 4. Command-line flags (highest priority)
    k.Load(posflag.Provider(flagSet, ".", k), nil)
    
    var cfg Config
    if err := k.Unmarshal("", &cfg); err != nil {
        return nil, err
    }
    return &cfg, cfg.Validate()
}

type Config struct {
    Provider struct {
        Name     string `koanf:"name"`      // "nvidia"
        Model    string `koanf:"model"`     // "nvidia/nemotron-3-ultra-550b-a55b"
        BaseURL  string `koanf:"base_url"`  // "https://integrate.api.nvidia.com/v1"
        // Key comes from OS keychain, NOT config.toml
    } `koanf:"provider"`
    Execution struct {
        Mode         string `koanf:"mode"`          // "auto", "full", "fast", "direct"
        MaxParallel  int    `koanf:"max_parallel"`  // wave concurrency
        Permission   string `koanf:"permission"`    // "interactive", "deny", "allow", "ci"
    } `koanf:"execution"`
    TUI struct {
        Theme      string `koanf:"theme"`       // "dark", "light", "auto"
        Mouse      bool   `koanf:"mouse"`       // enable mouse
        Compact    bool   `koanf:"compact"`     // compact mode for narrow terminals
    } `koanf:"tui"`
}
```

### Pattern 3: Migration as Event Replay
**What:** Migration from `.planning/` reads each artifact and emits corresponding events into the event store, then writes human-readable projections to `.m31a/`.
**When to use:** One-time migration preserving traceability and zero data loss.

**Example:**
```go
// Source: CONTEXT_M31A.md §12 (Project memory) + ROADMAP.md Phase 1 success criteria
func Migrate(ctx context.Context, planningDir, m31aDir string) error {
    // 1. Create .m31a/ directory structure
    os.MkdirAll(filepath.Join(m31aDir, "decisions"), 0755)
    os.MkdirAll(filepath.Join(m31aDir, "research"), 0755)
    
    // 2. Initialize EventStore
    store, err := NewEventStore(filepath.Join(m31aDir, "events.db"))
    if err != nil {
        return err
    }
    
    // 3. For each artifact in .planning/, emit event + write projection
    // Example: requirements.md → RequirementCreated events
    reqs := parseRequirements(filepath.Join(planningDir, "REQUIREMENTS.md"))
    for _, req := range reqs {
        evt := Event{
            Type:      "RequirementCreated",
            Payload:   mustMarshal(req),
            SessionID: "migration",
        }
        store.Append(ctx, evt)
    }
    // Write human-readable projection
    writeFile(filepath.Join(m31aDir, "requirements.md"), formatRequirements(reqs))
    
    // Repeat for: roadmap, decisions, research, state, context...
    // 4. Write config.toml with detected settings
    // 5. Write project.md with project identity
    return nil
}
```

### Anti-Patterns to Avoid
- **God-object domain type:** Do not create one mega-struct containing all project information [CITED: CONTEXT_M31A.md §8 "Do not create one mega-struct containing all project information"]
- **TUI owning state:** TUI must be a pure projection consuming events; never mutate domain state from UI [CITED: CONTEXT_M31A.md §4.1, D-011 in STATE.md]
- **In-memory event bus only:** Events must be durable in SQLite before acknowledgment [CITED: SUMMARY.md Pitfall 9]
- **Secrets in config.toml:** API keys never written to disk; always use OS keychain [CITED: CONTEXT_M31A.md §34.2]
- **Flat-file authority:** After migration, `.planning/` is deprecated; `.m31a/events.db` is authoritative [CITED: ROADMAP.md Phase 1 success criteria #2]

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Embedded database | Custom file format / BoltDB / badger | modernc.org/sqlite | ACID, WAL, SQL queries, hot backup, pure Go, zero CGO |
| Configuration layering | Custom env/file/flag merging | koanf/v2 | Declarative precedence, multiple parsers, validation hooks, no global state |
| Event store schema | Ad-hoc JSON lines / custom binary | SQLite with defined schema | Range queries, indexing, transactions, tooling, backup API |
| OS secret storage | ~/.m31a/secrets.json / env file | internal/integrations/keychain | Platform-native (Keychain/Credential Manager/Secret Service), encrypted at rest |
| Migration logic | Manual copy scripts | Event replay + projection | Preserves traceability, idempotent, auditable, testable |
| Unique IDs | Random strings / timestamps | github.com/google/uuid | RFC4122 compliant, collision-resistant, standard |

**Key insight:** The event store is the *most critical* component — every other plane depends on it. Using a battle-tested SQLite driver with WAL mode and online backup avoids months of custom durability work.

## Runtime State Inventory

> Phase 1 is a greenfield foundation phase AND a migration phase. This inventory covers both the new `.m31a/` state and the existing `.planning/` state being migrated.

| Category | Items Found | Action Required |
|----------|-------------|-----------------|
| **Stored data** | `.planning/REQUIREMENTS.md` (118 requirements), `.planning/ROADMAP.md` (12 phases), `.planning/STATE.md`, `.planning/PROJECT.md`, `.planning/CONTEXT_M31A.md`, `.planning/UI-SPEC.md`, `.planning/research/SUMMARY.md`, `.planning/codebase/` (7 analysis files), `.planning/config.json`, `.planning/graphs/` (empty) | **Data migration:** Parse each file, emit corresponding events (RequirementCreated, PhaseCreated, DecisionLogged, ResearchCompleted, etc.), write human-readable projections to `.m31a/` |
| **Live service config** | None — no external services configured yet | None |
| **OS-registered state** | None — no systemd/launchd/Task Scheduler entries | None |
| **Secrets/env vars** | `NVIDIA_API_KEY` in environment (not in `.planning/`); `.env.example` exists | **Code edit only:** Config loader reads `NVIDIA_API_KEY` from env at runtime; keychain stores/retrieves for persistence; never written to `.m31a/config.toml` |
| **Build artifacts** | `./m31a` binary (built from current codebase); `go.sum`, `go.mod` | **Reinstall package:** After migration, rebuild binary with new domain types; old binary incompatible |

**Nothing found in category:** Live service config, OS-registered state — verified by inspection of `.planning/` and project root.

## Common Pitfalls

### Pitfall 1: Distributed State Ownership (Split-Brain)
**What goes wrong:** Multiple components (Engine, WorkflowState, SessionManager, Recovery, TUI) each own overlapping slices of run state, causing inconsistent views.
**Why it happens:** No single authority for durable state; each subsystem manages its own persistence.
**How to avoid:** Single EventStore as the *only* write path for durable state. All subsystems read via projections. TUI subscribes to events only — never writes domain state. [VERIFIED: SUMMARY.md Pitfall 1, AUDIT_REPORT.md]
**Warning signs:** Multiple `*_state.json` files; TUI updating `WorkflowState` directly; "state out of sync" bugs.

### Pitfall 2: Event Store Without Durability Guarantees
**What goes wrong:** Events held in memory, lost on crash; no monotonic ordering; range queries require full scan.
**Why it happens:** Using in-memory slices or channels instead of persistent store.
**How to avoid:** SQLite with WAL mode (`_pragma=journal_mode(WAL)`), INTEGER PRIMARY KEY for monotonic SEQ, indexes on (run_id, seq), (session_id, seq), (type, seq). Every `Append` returns only after `tx.Commit()`. [VERIFIED: modernc.org/sqlite docs WAL mode, CONTEXT_M31A.md §11]
**Warning signs:** "event lost after restart"; "events out of order"; slow queries on large event logs.

### Pitfall 3: Configuration Secrets Leakage
**What goes wrong:** API keys written to `config.toml`, committed to Git, or logged in diagnostics.
**Why it happens:** Configuration system doesn't distinguish secrets from regular config.
**How to avoid:** Config struct has no `APIKey` field. Provider adapter reads `NVIDIA_API_KEY` from `os.Getenv` at runtime, falls back to keychain `Get("nvidia/api_key")`. Keychain `Set` called only by explicit `m31a config set-key` command. [VERIFIED: CONTEXT_M31A.md §34.2, internal/integrations/keychain/keychain.go]
**Warning signs:** `config.toml` contains `api_key`; logs show full key; `.env` committed.

### Pitfall 4: Migration Data Loss
**What goes wrong:** `.planning/` artifacts not fully represented in `.m31a/`; traceability broken; rollback impossible.
**Why it happens:** Migration copies files without semantic understanding; no event log of migration itself.
**How to avoid:** Migration *is* an event-sourced operation. Each source artifact → domain objects → events appended to store → projections written to `.m31a/`. Migration itself emits `MigrationStarted`/`MigrationCompleted` events. Verify by replaying events to reconstruct projections. [VERIFIED: ROADMAP.md success criteria #2, CONTEXT_M31A.md §12]
**Warning signs:** "requirements missing after migrate"; "decision history gone"; no way to verify completeness.

### Pitfall 5: Blocking Writers During Backup
**What goes wrong:** Hot backup locks database, blocking event appends during long backup operations.
**Why it happens:** Using `sqlite3_backup` without incremental `Step()` calls, or using file copy.
**How to avoid:** modernc.org/sqlite `NewBackup` with incremental `Step(n)` (e.g., 5 pages at a time) in a goroutine, allowing concurrent writes. [VERIFIED: modernc.org/sqlite NewBackup docs]
**Warning signs:** Backup takes >100ms; event appends timeout during backup.

## Code Examples

### Domain Types (internal/core/types/domain.go)
```go
// Source: CONTEXT_M31A.md §8 (Core domain model) + DOMAIN-01/02/03/04
package types

import (
    "encoding/json"
    "time"
    "github.com/google/uuid"
)

// Project represents the durable engineering identity for a workspace.
type Project struct {
    ID          uuid.UUID              `json:"id"`
    RootPath    string                 `json:"root_path"`
    Name        string                 `json:"name"`
    Repositories []Repository          `json:"repositories"`
    Stack       DetectedStack          `json:"stack"`
    Config      ProjectConfig          `json:"config"`
    CreatedAt   time.Time              `json:"created_at"`
    UpdatedAt   time.Time              `json:"updated_at"`
}

// Repository represents a Git repository in the workspace.
type Repository struct {
    ID       uuid.UUID `json:"id"`
    RootPath string    `json:"root_path"`
    Remotes  []Remote  `json:"remotes"`
    DefaultBranch string `json:"default_branch"`
}

// Session represents a user interaction history.
type Session struct {
    ID           uuid.UUID     `json:"id"`
    ProjectID    uuid.UUID     `json:"project_id"`
    Label        string        `json:"label,omitempty"`
    Tags         []string      `json:"tags,omitempty"`
    Model        string        `json:"model"`
    Provider     string        `json:"provider"`
    StartedAt    time.Time     `json:"started_at"`
    LastEventSeq int64         `json:"last_event_seq"` // For projection sync
}

// Run represents an engineering operation.
type Run struct {
    ID          uuid.UUID     `json:"id"`
    SessionID   uuid.UUID     `json:"session_id"`
    Intent      Intent        `json:"intent"`
    PlanID      *uuid.UUID    `json:"plan_id,omitempty"`
    TaskGraphID *uuid.UUID    `json:"task_graph_id,omitempty"`
    Status      RunStatus     `json:"status"` // Planned, Running, Paused, Completed, Failed
    StartedAt   time.Time     `json:"started_at"`
    CompletedAt *time.Time    `json:"completed_at,omitempty"`
}

// Intent classifies user's natural-language request.
type Intent struct {
    Type       IntentType     `json:"type"`        // Feature, Bugfix, Refactor, Question, Exploration, Chore
    Complexity ComplexityLevel `json:"complexity"`  // Trivial, Simple, Moderate, Complex
    Summary    string         `json:"summary"`
    RawPrompt  string         `json:"raw_prompt"`
    Confidence float64        `json:"confidence"`
}

// Event is the fundamental unit of durable state change.
type Event struct {
    ID          uuid.UUID       `json:"id"`
    Seq         int64           `json:"seq"`          // Monotonic, assigned by EventStore
    Type        EventType       `json:"type"`         // e.g., "RequirementCreated"
    Timestamp   time.Time       `json:"timestamp"`
    RunID       *uuid.UUID      `json:"run_id,omitempty"`
    SessionID   *uuid.UUID      `json:"session_id,omitempty"`
    Payload     json.RawMessage `json:"payload"`      // Type-specific payload
    Metadata    EventMetadata   `json:"metadata,omitempty"`
}

type EventType string

const (
    EventProjectInitialized    EventType = "ProjectInitialized"
    EventRepositoryIndexed     EventType = "RepositoryIndexed"
    EventSessionCreated        EventType = "SessionCreated"
    EventRunCreated            EventType = "RunCreated"
    EventIntentAccepted        EventType = "IntentAccepted"
    EventRequirementCreated    EventType = "RequirementCreated"
    EventDecisionLogged        EventType = "DecisionLogged"
    EventPlanCreated           EventType = "PlanCreated"
    EventTaskCreated           EventType = "TaskCreated"
    EventAgentStarted          EventType = "AgentStarted"
    EventToolCallRequested     EventType = "ToolCallRequested"
    EventFileChanged           EventType = "FileChanged"
    EventCheckpointRequested   EventType = "CheckpointRequested"
    EventVerificationStarted   EventType = "VerificationStarted"
    EventTaskCompleted         EventType = "TaskCompleted"
    EventRunCompleted          EventType = "RunCompleted"
    // ... (35+ types per CONTEXT_M31A.md §11)
)

// JSON serialization helpers for event payloads
func MarshalEventPayload(v any) (json.RawMessage, error) {
    return json.Marshal(v)
}

func UnmarshalEventPayload[T any](data json.RawMessage) (T, error) {
    var v T
    err := json.Unmarshal(data, &v)
    return v, err
}
```

### Event Store Schema (internal/memory/eventstore/schema.sql)
```sql
-- Source: modernc.org/sqlite WAL mode + CONTEXT_M31A.md §11 (monotonic ordering, range queries)
-- Run: executed once at EventStore initialization

PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

CREATE TABLE IF NOT EXISTS events (
    seq         INTEGER PRIMARY KEY AUTOINCREMENT,  -- Monotonic ordering
    id          TEXT NOT NULL UNIQUE,               -- UUID
    type        TEXT NOT NULL,                      -- EventType
    timestamp   INTEGER NOT NULL,                   -- Unix nanoseconds
    run_id      TEXT,                               -- UUID, nullable
    session_id  TEXT,                               -- UUID, nullable
    payload     BLOB NOT NULL,                      -- JSON payload
    metadata    BLOB                                -- JSON metadata (optional)
);

-- Efficient range queries by run/session/type
CREATE INDEX IF NOT EXISTS idx_events_run_seq      ON events(run_id, seq);
CREATE INDEX IF NOT EXISTS idx_events_session_seq  ON events(session_id, seq);
CREATE INDEX IF NOT EXISTS idx_events_type_seq     ON events(type, seq);
CREATE INDEX IF NOT EXISTS idx_events_timestamp    ON events(timestamp);

-- Projection checkpoints for fast resume
CREATE TABLE IF NOT EXISTS projections (
    name        TEXT PRIMARY KEY,                   -- e.g., "project", "requirements", "runs"
    last_seq    INTEGER NOT NULL,                   -- Last event seq applied
    updated_at  INTEGER NOT NULL                    -- Unix nanoseconds
);

-- Migration tracking
CREATE TABLE IF NOT EXISTS migrations (
    version     INTEGER PRIMARY KEY,
    description TEXT NOT NULL,
    applied_at  INTEGER NOT NULL
);
```

### Event Store Append (internal/memory/eventstore/append.go)
```go
// Source: modernc.org/sqlite transaction hooks + CONTEXT_M31A.md §11 (append-only, durable)
package eventstore

import (
    "context"
    "database/sql"
    "encoding/json"
    "time"
    "github.com/google/uuid"
    "github.com/eshanized/M31A/internal/core/types"
    _ "modernc.org/sqlite"
)

type SQLiteEventStore struct {
    db *sql.DB
}

func NewEventStore(dbPath string) (*SQLiteEventStore, error) {
    dsn := "file:" + dbPath + "?_pragma=journal_mode(WAL)&_pragma=foreign_keys(1)&_pragma=busy_timeout(5000)&_time_format=sqlite"
    db, err := sql.Open("sqlite", dsn)
    if err != nil {
        return nil, err
    }
    // Execute schema
    if _, err := db.Exec(schemaSQL); err != nil {
        return nil, err
    }
    return &SQLiteEventStore{db: db}, nil
}

func (s *SQLiteEventStore) Append(ctx context.Context, events ...types.Event) error {
    if len(events) == 0 {
        return nil
    }
    
    tx, err := s.db.BeginTx(ctx, nil)
    if err != nil {
        return err
    }
    defer tx.Rollback()
    
    stmt, err := tx.PrepareContext(ctx, `
        INSERT INTO events (id, type, timestamp, run_id, session_id, payload, metadata)
        VALUES (?, ?, ?, ?, ?, ?, ?)
    `)
    if err != nil {
        return err
    }
    defer stmt.Close()
    
    for i := range events {
        // Assign monotonic SEQ via AUTOINCREMENT (implicit)
        events[i].ID = uuid.New()
        events[i].Timestamp = time.Now()
        
        payload, _ := json.Marshal(events[i].Payload)
        metadata, _ := json.Marshal(events[i].Metadata)
        
        runID := ""
        if events[i].RunID != nil {
            runID = events[i].RunID.String()
        }
        sessionID := ""
        if events[i].SessionID != nil {
            sessionID = events[i].SessionID.String()
        }
        
        if _, err := stmt.ExecContext(ctx,
            events[i].ID.String(),
            events[i].Type,
            events[i].Timestamp.UnixNano(),
            nullIfEmpty(runID),
            nullIfEmpty(sessionID),
            payload,
            nullIfEmpty(string(metadata)),
        ); err != nil {
            return err
        }
    }
    
    return tx.Commit()
}

func nullIfEmpty(s string) any {
    if s == "" {
        return nil
    }
    return s
}
```

### Hot Backup (internal/memory/eventstore/backup.go)
```go
// Source: modernc.org/sqlite NewBackup API (online backup without blocking writers)
package eventstore

import (
    "context"
    "database/sql"
    "fmt"
    "time"
    _ "modernc.org/sqlite"
)

func (s *SQLiteEventStore) Backup(ctx context.Context, dstPath string) error {
    conn, err := s.db.Conn(ctx)
    if err != nil {
        return fmt.Errorf("get connection: %w", err)
    }
    defer conn.Close()
    
    return conn.Raw(func(driverConn any) error {
        type backuper interface {
            NewBackup(string) (interface {
                Step(int32) (bool, error)
                Finish() error
            }, error)
        }
        
        bc, ok := driverConn.(backuper)
        if !ok {
            return fmt.Errorf("driver does not support backup")
        }
        
        bk, err := bc.NewBackup(dstPath)
        if err != nil {
            return fmt.Errorf("create backup: %w", err)
        }
        
        // Incremental backup: 5 pages at a time, non-blocking
        ticker := time.NewTicker(10 * time.Millisecond)
        defer ticker.Stop()
        
        for {
            select {
            case <-ctx.Done():
                bk.Finish()
                return ctx.Err()
            case <-ticker.C:
                more, err := bk.Step(5)
                if err != nil {
                    bk.Finish()
                    return fmt.Errorf("backup step: %w", err)
                }
                if !more {
                    return bk.Finish()
                }
            }
        }
    })
}
```

### Configuration Loader (internal/core/config/loader.go)
```go
// Source: koanf/v2 hierarchical loading + CONTEXT_M31A.md §12 (config.toml)
package config

import (
    "flag"
    "fmt"
    "os"
    "path/filepath"
    
    "github.com/knadh/koanf/v2"
    "github.com/knadh/koanf/parsers/toml"
    "github.com/knadh/koanf/providers/confmap"
    "github.com/knadh/koanf/providers/env"
    "github.com/knadh/koanf/providers/file"
    "github.com/knadh/koanf/providers/posflag"
    "github.com/spf13/pflag"
)

func Load(workspaceRoot string) (*Config, error) {
    k := koanf.New(".")
    
    // 1. Defaults
    defaults := map[string]any{
        "provider.name":        "nvidia",
        "provider.model":       "nvidia/nemotron-3-ultra-550b-a55b",
        "provider.base_url":    "https://integrate.api.nvidia.com/v1",
        "execution.mode":       "auto",
        "execution.max_parallel": 4,
        "execution.permission": "interactive",
        "tui.theme":            "auto",
        "tui.mouse":            true,
        "tui.compact":          false,
    }
    if err := k.Load(confmap.Provider(defaults, "."), nil); err != nil {
        return nil, err
    }
    
    // 2. Config file: .m31a/config.toml
    configPath := filepath.Join(workspaceRoot, ".m31a", "config.toml")
    if _, err := os.Stat(configPath); err == nil {
        if err := k.Load(file.Provider(configPath), toml.Parser()); err != nil {
            return nil, fmt.Errorf("load config.toml: %w", err)
        }
    }
    
    // 3. Environment variables: M31A_ prefix
    if err := k.Load(env.Provider("M31A_", ".", env.Opt{Prefix: "M31A_"}), nil); err != nil {
        return nil, err
    }
    
    // 4. Command-line flags (highest priority)
    fs := pflag.NewFlagSet("m31a", pflag.ContinueOnError)
    fs.String("provider.name", "", "Provider name (nvidia, openrouter, zen)")
    fs.String("provider.model", "", "Model ID")
    fs.String("execution.mode", "", "Workflow mode (auto, full, fast, direct)")
    fs.String("execution.permission", "", "Permission mode (interactive, deny, allow, ci)")
    fs.Int("execution.max-parallel", 0, "Max parallel tasks per wave")
    fs.Bool("tui.mouse", false, "Enable mouse support")
    fs.Bool("tui.compact", false, "Compact mode for narrow terminals")
    fs.Parse(os.Args[1:])
    
    if err := k.Load(posflag.Provider(fs, ".", k), nil); err != nil {
        return nil, err
    }
    
    var cfg Config
    if err := k.Unmarshal("", &cfg); err != nil {
        return nil, fmt.Errorf("unmarshal config: %w", err)
    }
    
    return &cfg, cfg.Validate()
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| In-memory event bus | SQLite append-only event store | 2024 (modernc.org/sqlite v1.30+) | Crash durability, replay, audit, hot backup |
| Single JSON config file | Layered config (defaults → file → env → flags) | 2023 (koanf v2) | No global state, validation, type-safe unmarshal |
| Custom ID generation | github.com/google/uuid | 2020+ | RFC4122 compliance, collision resistance |
| Flat-file project memory (.planning/) | Event-sourced .m31a/ with projections | This project (architectural reset) | Single source of truth, derivable state, migration traceability |
| CGO SQLite (mattn/go-sqlite3) | Pure Go SQLite (modernc.org/sqlite) | 2022+ | Static binaries, cross-compilation, no CGO |

**Deprecated/outdated:**
- `mattn/go-sqlite3`: Requires CGO, breaks static builds [ASSUMED based on AGENTS.md CGO constraint]
- `spf13/viper`: Global state, complex API, slower [ASSUMED based on koanf benchmarks]
- Manual migration scripts: No traceability, not idempotent, hard to verify [CITED: CONTEXT_M31A.md §12]

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | modernc.org/sqlite v1.57.0 is compatible with Go 1.26.5 and supports all required features (WAL, hooks, backup) | Standard Stack | Build fails or runtime panic; fallback: use v1.56.0 which is confirmed in go.mod |
| A2 | koanf/v2 v2.3.6 works with BurntSushi/toml v1.6.0 for config.toml parsing | Standard Stack | Config load fails; fallback: use koanf/v2 built-in TOML parser if available |
| A3 | Existing keychain implementation (internal/integrations/keychain) works on all target platforms (linux/amd64, linux/arm64, darwin/amd64, darwin/arm64, windows/amd64) | Standard Stack | Secrets not persisted on some platforms; fallback: env-only with warning |
| A4 | All 118 requirements in .planning/REQUIREMENTS.md can be parsed into structured Requirement domain objects | Migration | Incomplete migration; fallback: manual mapping for unparsable entries |
| A5 | Bubble Tea v1.3.x (not v2) is the correct version for this project | Architecture Patterns | TUI compile errors; verified: go.mod uses v1.3.0, latest v1 is v1.3.10 |

## Open Questions (RESOLVED)

1. **Event payload versioning** — RESOLVED
   - **Decision:** Include `schema_version` in `EventMetadata`; use JSON with optional fields; projections handle missing fields gracefully (graceful degradation per D-02).
   - **Implementation:** `EventMetadata` struct in `internal/core/types/event.go` includes `SchemaVersion int`; projections use `UnmarshalEventPayload` with optional field handling.

2. **Concurrent session isolation** — RESOLVED
   - **Decision:** Single-writer, multi-reader model using SQLite WAL mode; process-level lock on entire `.m31a/events.db` via SQLite's built-in WAL serialization; `busy_timeout=5000` in DSN handles contention (per D-05, D-07, D-08).
   - **Implementation:** Rely on SQLite's built-in locking; no Go `flock` wrapper needed.

3. **Projection rebuild performance** — RESOLVED
   - **Decision:** Implement projection checkpoint table (in schema); rebuild from last checkpoint + incremental events; checkpoints every N events (configurable, default 100) in same transaction as event append (D-13, D-15); keep last N checkpoints (default 10, D-16).
   - **Implementation:** `projections` table in schema.sql; `ProjectionManager.Rebuild` loads checkpoint + replays incremental events.

4. **Migration of .planning/graphs/** — RESOLVED
   - **Decision:** Skip graph migration; regenerate from events after Phase 3 (Code Intelligence).
   - **Implementation:** Migration engine (Plan 06) skips `.planning/graphs/` directory; `.gitignore` excludes `graphs/` (D-12, PERSIST-05).

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| Go toolchain | All builds | ✓ | 1.26.5 | — |
| modernc.org/sqlite | EventStore | ✓ | v1.57.0 (proxy) | v1.56.0 |
| koanf/v2 | Config loader | ✓ | v2.3.6 (proxy) | v2.3.5 |
| OS keychain (macOS) | Secret storage | ✓ (darwin) | security CLI | env var only |
| OS keychain (Linux) | Secret storage | ? | D-Bus Secret Service / pass | env var only |
| OS keychain (Windows) | Secret storage | ? | Credential Manager | env var only |
| Git | Migration (reading .planning/) | ✓ | — | — |

**Missing dependencies with no fallback:** None — all core dependencies are Go modules available via proxy.golang.org.

**Missing dependencies with fallback:** Linux/Windows keychain backends may be unavailable if D-Bus/pass or Credential Manager not accessible; fallback to environment variable only with startup warning.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Go standard library `testing` + `testify/assert` + `testify/require` |
| Config file | None — tests use `testing.T` directly |
| Quick run command | `go test ./internal/core/types/... ./internal/core/config/... ./internal/memory/eventstore/... -short` |
| Full suite command | `make test` (race-enabled, coverage) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DOMAIN-01 | All 18 domain types defined with JSON tags | unit | `go test ./internal/core/types/... -run TestDomainTypes` | ❌ Wave 0 |
| DOMAIN-02 | Each type in separate file/group; no mega-struct | unit | `go test ./internal/core/types/... -run TestOwnership` | ❌ Wave 0 |
| DOMAIN-03 | All types marshal/unmarshal JSON round-trip | unit | `go test ./internal/core/types/... -run TestSerialization` | ❌ Wave 0 |
| DOMAIN-04 | Domain mutations emit events via EventStore | integration | `go test ./internal/memory/eventstore/... -run TestEventEmission` | ❌ Wave 0 |
| PERSIST-01 | events.db created with WAL mode | unit | `go test ./internal/memory/eventstore/... -run TestWALMode` | ❌ Wave 0 |
| PERSIST-02 | Append-only writes, monotonic SEQ, range queries | unit | `go test ./internal/memory/eventstore/... -run TestAppendQuery` | ❌ Wave 0 |
| PERSIST-03 | config.toml loaded with layered precedence | unit | `go test ./internal/core/config/... -run TestLayeredConfig` | ❌ Wave 0 |
| PERSIST-04 | project.md, decisions/, research/ written | integration | `go test ./internal/memory/artifacts/... -run TestProjections` | ❌ Wave 0 |
| PERSIST-05 | .gitignore excludes caches, secrets, embeddings | unit | `go test ./internal/memory/... -run TestGitIgnorePolicy` | ❌ Wave 0 |
| PERSIST-06 | Migration replays all .planning/ data to events | integration | `go test ./internal/memory/eventstore/... -run TestMigration` | ❌ Wave 0 |
| PERSIST-07 | Hot backup completes without blocking writers | integration | `go test ./internal/memory/eventstore/... -run TestHotBackup` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `go test ./internal/core/types/... ./internal/core/config/... ./internal/memory/eventstore/... -short`
- **Per wave merge:** `make test` (full race-enabled suite)
- **Phase gate:** Full suite green before `/gsd-verify-work`

### Wave 0 Gaps
- [ ] `internal/core/types/domain_test.go` — covers DOMAIN-01, DOMAIN-02, DOMAIN-03
- [ ] `internal/core/types/event_test.go` — covers DOMAIN-04, event serialization
- [ ] `internal/core/config/config_test.go` — covers PERSIST-03
- [ ] `internal/memory/eventstore/eventstore_test.go` — covers PERSIST-01, PERSIST-02, PERSIST-07
- [ ] `internal/memory/eventstore/migration_test.go` — covers PERSIST-06
- [ ] `internal/memory/artifacts/artifacts_test.go` — covers PERSIST-04, PERSIST-05
- [ ] Framework: testify already in go.mod (indirect), no additional install needed

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No | — (no user auth in Phase 1) |
| V3 Session Management | No | — (sessions are domain objects, not auth sessions) |
| V4 Access Control | Yes | Capability-based permissions (Phase 7); Phase 1 defines Capability/PermissionPolicy types |
| V5 Input Validation | Yes | koanf config validation; event payload JSON schema validation |
| V6 Cryptography | Yes | OS keychain for secrets; TLS for NVIDIA API (Phase 2) |
| V7 Error Handling | Yes | Structured errors with `%w`; no panic; secret redaction in logs |
| V10 Malicious Code | Yes | Path validation in migration; symlink escape prevention |

### Known Threat Patterns for Go/SQLite/Config Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| SQL injection in event queries | Tampering | Parameterized queries only (`?` placeholders); no string interpolation |
| Path traversal in migration | Tampering | `filepath.Clean`, `EvalSymlinks` on both source and workspace root |
| Symlink escape in .m31a/ writes | Tampering | Central `ValidatePath` with `EvalSymlinks` before every write |
| Secret leakage in config.toml | Information Disclosure | No secret fields in Config struct; keychain-only for API keys |
| Malicious .planning/ content | Tampering | Migration parser treats input as untrusted; validate all parsed data |
| Event payload injection | Tampering | JSON marshaling only; no eval/deserialization of arbitrary types |

## Sources

### Primary (HIGH confidence)
- **CONTEXT_M31A.md** — Canonical architecture (§7 six-plane, §8 domain model, §11 event model, §12 .m31a/, §34 NVIDIA Build)
- **REQUIREMENTS.md** — Phase 1 requirements DOMAIN-01..04, PERSIST-01..07 with traceability
- **ROADMAP.md** — Phase 1 success criteria (5 criteria), dependencies, plans
- **modernc.org/sqlite** (Context7) — WAL mode DSN, transaction hooks, NewBackup API [VERIFIED: Context7]
- **koanf/v2** (Context7) — Hierarchical config loading, posflag, env providers, validation [VERIFIED: Context7]
- **go.mod** — Confirmed versions: Go 1.26.5, modernc.org/sqlite, koanf/v2, BurntSushi/toml, charmbracelet/* [VERIFIED: go.mod + go list -m -versions]

### Secondary (MEDIUM confidence)
- **SUMMARY.md** — Research findings, pitfalls, stack recommendations
- **PROJECT.md** — Active requirements, constraints, key decisions
- **internal/core/types/types.go** — Existing type definitions (RiskLevel, WorkflowPhase, IntentType, Task, Session, etc.)
- **internal/integrations/keychain/** — Existing keychain implementation [VERIFIED: keychain.go:8-42]

### Tertiary (LOW confidence)
- **Bubble Tea v1.3.x** — TUI framework (Phase 10, but event subscription model relevant) [CITED: Context7]
- **Event sourcing patterns** — General patterns from literature [ASSUMED]

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — All versions verified via `go list -m -versions` against proxy.golang.org; source repos confirmed
- Architecture: HIGH — Directly from CONTEXT_M31A.md (canonical) and ROADMAP.md (phase spec)
- Pitfalls: HIGH — Sourced from AUDIT_REPORT.md (17 confirmed findings) and SUMMARY.md (10 pitfalls with root causes)
- Code examples: HIGH — Based on verified library docs (Context7) and canonical architecture

**Research date:** 2026-08-23
**Valid until:** 2026-11-23 (90 days; stable Go ecosystem, but check for modernc.org/sqlite updates monthly)