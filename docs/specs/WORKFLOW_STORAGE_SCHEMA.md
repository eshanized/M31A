# Workflow Storage Schema Specification

**Subsystem:** `m31a-storage::workflow_store`, `m31a-storage::migrations`  
**Database:** SQLite 3 (WAL mode)  
**Migration ID:** 15 (`add_workflows_and_orchestration_tables`)  
**Version:** 1.0.0 (Foundation Milestone)

---

## 1. Schema Overview

Migration 15 introduces six relational tables supporting workflow runs, plan revisions, task DAGs, worker leases, append-only event auditing, and content-addressed artifacts:

```
┌─────────────────┐       1:N       ┌────────────────────┐
│    workflows    │────────────────>│   workflow_plans   │
└────────┬────────┘                 └─────────┬──────────┘
         │                                    │
         │ 1:N                                │ 1:N
         │                                    ▼
         │                          ┌────────────────────┐
         │                          │   workflow_tasks   │
         │                          └─────────┬──────────┘
         │                                    │
         │                                    │ 1:N
         │                                    ▼
         │                          ┌────────────────────────────┐
         │                          │ workflow_task_dependencies │
         │                          └────────────────────────────┘
         │
         ├─────────────────────────>┌────────────────────┐
         │ 1:N                      │  workflow_events   │ (Append-Only)
         │                          └────────────────────┘
         │
         └─────────────────────────>┌────────────────────┐
           1:N                      │ workflow_artifacts │ (Content-Addressed)
                                    └────────────────────┘
```

---

## 2. Table Specifications

### 1. `workflows`
Tracks the top-level execution lifecycle of an engineering objective.

```sql
CREATE TABLE IF NOT EXISTS workflows (
    id TEXT PRIMARY KEY,
    thread_id TEXT,
    workspace_root TEXT NOT NULL,
    objective TEXT NOT NULL,
    workflow_type TEXT NOT NULL DEFAULT 'feature',
    state TEXT NOT NULL DEFAULT 'created',
    plan_revision INTEGER NOT NULL DEFAULT 1,
    configuration_json TEXT NOT NULL DEFAULT '{}',
    created_at_epoch_ms INTEGER NOT NULL,
    updated_at_epoch_ms INTEGER NOT NULL,
    started_at_epoch_ms INTEGER,
    completed_at_epoch_ms INTEGER,
    cancellation_reason TEXT,
    overall_result_json TEXT,
    resource_usage_json TEXT
);

CREATE INDEX IF NOT EXISTS idx_workflows_state ON workflows(state);
CREATE INDEX IF NOT EXISTS idx_workflows_created_at ON workflows(created_at_epoch_ms);
CREATE INDEX IF NOT EXISTS idx_workflows_thread_id ON workflows(thread_id);
```

### 2. `workflow_plans`
Stores versioned implementation plans and DAG structures.

```sql
CREATE TABLE IF NOT EXISTS workflow_plans (
    workflow_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    summary TEXT NOT NULL,
    rationale TEXT NOT NULL,
    estimated_waves INTEGER NOT NULL DEFAULT 1,
    dag_json TEXT NOT NULL,
    created_at_epoch_ms INTEGER NOT NULL,
    PRIMARY KEY (workflow_id, revision),
    FOREIGN KEY (workflow_id) REFERENCES workflows(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_workflow_plans_wf ON workflow_plans(workflow_id);
```

### 3. `workflow_tasks`
Discrete executable tasks within a plan DAG, including worker leasing and priority metadata.

```sql
CREATE TABLE IF NOT EXISTS workflow_tasks (
    id TEXT NOT NULL,
    workflow_id TEXT NOT NULL,
    plan_revision INTEGER NOT NULL DEFAULT 1,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    assigned_role TEXT NOT NULL DEFAULT 'general',
    assigned_agent_id TEXT,
    priority INTEGER NOT NULL DEFAULT 0,
    wave_index INTEGER NOT NULL DEFAULT 1,
    dependencies_json TEXT NOT NULL DEFAULT '[]',
    acceptance_criteria_json TEXT NOT NULL DEFAULT '[]',
    required_verification_json TEXT NOT NULL DEFAULT '[]',
    declared_read_paths_json TEXT NOT NULL DEFAULT '[]',
    declared_write_paths_json TEXT NOT NULL DEFAULT '[]',
    workspace_isolation TEXT NOT NULL DEFAULT 'worktree',
    status TEXT NOT NULL DEFAULT 'pending',
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    started_at_epoch_ms INTEGER,
    completed_at_epoch_ms INTEGER,
    worker_id TEXT,
    lease_expires_at_epoch_ms INTEGER,
    error_details TEXT,
    input_artifacts_json TEXT NOT NULL DEFAULT '[]',
    output_artifacts_json TEXT NOT NULL DEFAULT '[]',
    verification_result_json TEXT,
    PRIMARY KEY (workflow_id, id),
    FOREIGN KEY (workflow_id) REFERENCES workflows(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_workflow_tasks_status ON workflow_tasks(workflow_id, status);
CREATE INDEX IF NOT EXISTS idx_workflow_tasks_wave ON workflow_tasks(workflow_id, wave_index);
CREATE INDEX IF NOT EXISTS idx_workflow_tasks_lease ON workflow_tasks(status, lease_expires_at_epoch_ms);
```

### 4. `workflow_task_dependencies`
Normalized dependency edges for high-performance relational querying.

```sql
CREATE TABLE IF NOT EXISTS workflow_task_dependencies (
    workflow_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    depends_on_task_id TEXT NOT NULL,
    PRIMARY KEY (workflow_id, task_id, depends_on_task_id),
    FOREIGN KEY (workflow_id, task_id) REFERENCES workflow_tasks(workflow_id, id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_task_deps_lookup ON workflow_task_dependencies(workflow_id, depends_on_task_id);
```

### 5. `workflow_events`
Append-only forensic event ledger. Entries are immutable.

```sql
CREATE TABLE IF NOT EXISTS workflow_events (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    task_id TEXT,
    agent_id TEXT,
    event_type TEXT NOT NULL,
    timestamp_epoch_ms INTEGER NOT NULL,
    correlation_id TEXT,
    payload_json TEXT NOT NULL DEFAULT '{}',
    schema_version INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY (workflow_id) REFERENCES workflows(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_wf_events_timeline ON workflow_events(workflow_id, timestamp_epoch_ms);
CREATE INDEX IF NOT EXISTS idx_wf_events_task ON workflow_events(workflow_id, task_id);
```

### 6. `workflow_artifacts`
Content-addressed metadata index for generated artifacts.

```sql
CREATE TABLE IF NOT EXISTS workflow_artifacts (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    task_id TEXT,
    name TEXT NOT NULL,
    artifact_type TEXT NOT NULL,
    file_path TEXT NOT NULL,
    content_sha256 TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    mime_type TEXT NOT NULL DEFAULT 'application/octet-stream',
    created_at_epoch_ms INTEGER NOT NULL,
    FOREIGN KEY (workflow_id) REFERENCES workflows(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_wf_artifacts_lookup ON workflow_artifacts(workflow_id, artifact_type);
CREATE INDEX IF NOT EXISTS idx_wf_artifacts_sha ON workflow_artifacts(content_sha256);
```

---

## 3. Storage Invariants and Concurrency Guarantees

1. **ACID Transactions:**  
   Task state transitions, worker lease claiming, and dependency resolutions execute inside immediate SQLite transactions.
2. **Lease Query Performance:**  
   `idx_workflow_tasks_lease` ensures worker claiming (`claim_ready_task`) and stale task detection (`reconcile_stale_tasks`) execute in sub-millisecond time.
3. **Foreign Key Cascading:**  
   Deleting a workflow cleanly cascades deletions to its plans, tasks, dependencies, events, and artifact records without orphaned rows.
