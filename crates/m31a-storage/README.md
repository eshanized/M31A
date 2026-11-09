# `m31a-storage`

> Embedded SQLite persistence engine, transaction management, checkpoint DAGs, multi-agent pub-sub message boards, and workflow artifact storage for M31A.

---

## Why Does This Crate Exist?

Autonomous coding sessions are not transient chat prompts. When an agent refactors an enterprise repository, it may run for dozens of turns, spawn sub-agents, mutate dozens of files, run compiler checks, and take atomic snapshot checkpoints.

If the user hits `Ctrl+C`, their laptop battery dies, or an agent makes a mistake that needs to be rolled back, you cannot rely on in-memory state. You need durable, relational, zero-mock persistence.

`m31a-storage` provides embedded SQLite persistence (currently running **Migration 15**) with zero external database dependencies. It stores session threads, conversation turns, file checkpoints, task graphs with worker leases, content-addressed artifacts, concurrency locks, and distributed message boards.

---

## Core Concepts & How It Works

### 1. Embedded SQLite & Automated Migrations (`migrations.rs`)
`m31a-storage` embeds SQLite via `rusqlite` (compiled with the `bundled` feature). On first startup, `MigrationEngine` runs in a transaction, migrating from schema version 1 through 15. Every schema change is versioned, atomic, and backward-compatible.

### 2. Checkpoint DAG & Atomic Rollbacks (`db.rs`)
Every time an agent completes a turn or executes a mutating patch, `m31a-storage` can record a snapshot checkpoint containing:
- Git HEAD commit SHA and working tree status.
- List of modified, created, and deleted files.
- Conversation turn parent pointers forming a directed acyclic graph (DAG).
This allows instant session rewinds via `m31a undo` or `m31a revert <checkpoint_id>`.

### 3. Multi-Agent Pub-Sub Message Board (`BoardPost`, `ChannelRecord`)
When running agent swarms (`m31a swarm`), child agents need to coordinate without clobbering each other's work. `m31a-storage` provides channels and posts where agents publish status updates, claim tasks, and request assistance asynchronously.

### 4. Path-Based Concurrency Locks (`PathLockRecord`)
Prevents two concurrent agents from modifying the same file or directory simultaneously. Locks include lease durations and automatic stale lease expiration.

### 5. Autonomous Workflow Store (`workflow_store.rs`)
Backs `m31a workflow` with:
- Task dependency DAGs.
- Worker leases (`ExecutionClaim`) preventing duplicate executions.
- Append-only event trails (`StoredEvidenceRecord`).
- Content-addressed artifact deduplication using SHA-256 digests.

---

## Key Modules & Types

| Module | Key Structs / Traits | Purpose |
| :--- | :--- | :--- |
| [`src/db.rs`](src/db.rs) | `Storage`, `ThreadRecord`, `TurnRecord`, `CheckpointRecord` | Primary SQLite connection wrapper with queries for sessions and checkpoints. |
| [`src/workflow_store.rs`](src/workflow_store.rs) | `WorkflowStore`, `ExecutionClaim` | Persistent storage for workflow DAG tasks, worker leases, and phase states. |
| [`src/migrations.rs`](src/migrations.rs) | `MigrationEngine`, `Migration`, `BUILTIN_MIGRATIONS` | Automated sequential migration runner (Migrations 1 through 15). |
| [`src/attachment_store.rs`](src/attachment_store.rs) | `AttachmentStore`, `LocalDiskAttachmentStore` | Stores screenshots, diff artifacts, and diagnostic logs with MIME detection. |
| [`src/history_recovery.rs`](src/history_recovery.rs) | `HistoryRecoveryStore` | Fast fuzzy full-text search across past prompt histories and turns. |
| [`src/transcript_export.rs`](src/transcript_export.rs) | `TranscriptExporter` | Formats and exports conversation sessions to Markdown, HTML, and JSONL. |
| [`src/worktree.rs`](src/worktree.rs) | `WorktreeManager`, `ManagedWorktree` | Tracks ephemeral Git worktrees allocated for sandboxed execution. |

---

## Practical Usage Example

Here is how to initialize storage, create a session thread, record a turn, and save a checkpoint:

```rust
use m31a_storage::db::Storage;
use std::path::Path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Initialize embedded SQLite database at ~/.m31a/state.db
    let db_path = Path::new("test_session.db");
    let storage = Storage::open(db_path)?;

    // 2. Create a new conversation thread session
    let thread = storage.create_thread(
        "Refactor auth layer",
        "/home/developer/project",
        "nvidia/nemotron-3.5-lightning-30b-a3b"
    )?;
    println!("Created thread session: {}", thread.id);

    // 3. Record a user turn
    let turn = storage.record_turn(
        &thread.id,
        "user",
        "Inspect JWT token validation and add unit tests"
    )?;
    println!("Recorded turn #{}", turn.turn_number);

    // 4. Create a snapshot checkpoint after modifying files
    let checkpoint = storage.create_checkpoint(
        &thread.id,
        "Added JWT expiration tests",
        &["tests/auth_test.rs"],
        Some("git_commit_sha_abc123")
    )?;
    println!("Saved checkpoint: {}", checkpoint.id);

    Ok(())
}
```

---

## Integration in M31A Monorepo

- **Upstream Dependencies**: Depends on [`m31a-core`](../m31a-core), `rusqlite`, `tokio`, `sha2`, and `serde`.
- **Downstream Consumers**: Used by [`m31a-agent-engine`](../m31a-agent-engine), [`m31a-app-server`](../m31a-app-server), [`m31a-cli`](../m31a-cli), and [`m31a-tui`](../m31a-tui).

---

## Testing & Verification

Storage tests enforce the **Zero-Mock Policy** by creating real temporary SQLite files, executing migrations, asserting foreign key integrity, and verifying rollbacks:

```bash
cargo test -p m31a-storage
```
