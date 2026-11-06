//! SQLite relational database engine managing threads, turns, messages, and session metadata.
//!
//! Provides transactional database access, query indexes, and schema management for conversation sessions.

use anyhow::{Context, Result, anyhow};
use m31a_core::types::SessionMetadata;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use uuid::Uuid;

#[derive(Clone)]
pub struct Storage {
    conn: Arc<Mutex<Connection>>,
    /// Directory holding the primary database file (`None` for in-memory).
    db_dir: Option<PathBuf>,
    /// §6.1: optional JSONL rollout mirror root. `None` disables mirroring
    /// (default); set via `set_rollout_root`.
    rollout_root: Arc<RwLock<Option<PathBuf>>>,
    /// Per-thread rollout writers, shared across Storage clones.
    rollout_writers: Arc<Mutex<HashMap<String, crate::rollout::RolloutWriter>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSpawnEdgeRecord {
    pub parent_thread_id: String,
    pub child_thread_id: String,
    pub agent_role: String,
    pub status: String,
    pub created_at: i64,
    pub depth: u32,
}

/// §5.5: a host-owned fact that survives compaction outside the
/// summarization contract (verified Q/A, retained user text, compaction
/// checkpoints). `ordinal` is scoped per (thread, kind, origin) and must
/// not be compared across origins.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetainedFact {
    pub id: String,
    pub thread_id: String,
    pub kind: String,
    pub origin: String,
    pub ordinal: i64,
    pub content: String,
    pub model_hash: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadRecord {
    pub id: String,
    pub title: String,
    pub model: String,
    pub cwd: String,
    pub sandbox_policy: String,
    pub approval_mode: String,
    pub is_archived: bool,
    pub parent_thread_id: Option<String>,
    pub section_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSectionRecord {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionTodoRecord {
    pub id: String,
    pub thread_id: String,
    pub content: String,
    pub status: String,
    pub position: i64,
    pub created_at_epoch_ms: i64,
    pub updated_at_epoch_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub thread_id: String,
    pub goal_id: String,
    pub objective: String,
    pub status: String, // 'active', 'paused', 'blocked', 'budget_limited', 'complete'
    pub token_budget: Option<u64>,
    pub tokens_used: u64,
    pub time_used_seconds: u64,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionClaim {
    pub claim_id: String,
    pub thread_id: String,
    pub turn_index: u32,
    pub status: String, // 'claimed', 'running', 'completed', 'aborted', 'crashed'
    pub pid: u32,
    pub prompt_preview: Option<String>,
    pub started_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxItem {
    pub id: String,
    pub thread_id: String,
    pub kind: String, // 'prompt', 'steer', 'system'
    pub payload: String,
    pub status: String, // 'queued', 'active', 'consumed', 'dropped'
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathLockRecord {
    pub id: String,
    pub thread_id: String,
    pub path: String,
    pub is_dir: bool,
    pub locked_by: String,
    pub created_at_epoch_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredEvidenceRecord {
    pub id: String,
    pub requirement_id: String,
    pub evidence_type: String,
    pub description: String,
    pub verified: bool,
    pub turn_index: Option<u32>,
    pub timestamp_epoch_ms: i64,
    pub details_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredSessionMemory {
    pub id: String,
    pub topic: String,
    pub detail: String,
    pub citation_count: u32,
    pub last_used_at_epoch_ms: Option<i64>,
    pub created_at_epoch_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnRecord {
    pub id: String,
    pub thread_id: String,
    pub turn_index: usize,
    pub status: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointRecord {
    pub id: String,
    pub thread_id: String,
    pub label: String,
    pub git_commit: Option<String>,
    pub turn_index: usize,
    pub modified_files: Vec<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThreadStats {
    pub total_turns: usize,
    pub total_items: usize,
    pub user_messages: usize,
    pub assistant_messages: usize,
    pub tool_calls: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelRecord {
    pub name: String,
    pub topic: String,
    pub created_at: i64,
    pub post_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSummaryRecord {
    pub root_id: String,
    pub channel: String,
    pub author: String,
    pub root_body: String,
    pub reply_count: usize,
    pub last_activity_timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardPost {
    pub seq: i64,
    pub board: String,
    pub id: String,
    pub channel: String,
    pub root: String,
    pub author: String,
    pub timestamp: i64,
    pub body: String,
    pub payload: serde_json::Value,
}

impl Storage {
    pub fn open_default() -> Result<Self> {
        let dir = m31a_core::Config::config_dir();
        fs::create_dir_all(&dir)?;
        let db_path = dir.join("state.db");
        let storage = Self::open(&db_path)?;
        // §6.1: mirror sessions to append-only JSONL rollouts beside the
        // SQLite index (~/.m31a/sessions/). Ephemeral/unpersisted sessions
        // never enqueue, so they leave no rollout file behind.
        storage.set_rollout_root(dir.join("sessions"));
        Ok(storage)
    }

    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open SQLite database at {:?}", path))?;

        let storage = Self {
            conn: Arc::new(Mutex::new(conn)),
            db_dir: path.parent().map(|p| p.to_path_buf()),
            rollout_root: Arc::new(RwLock::new(None)),
            rollout_writers: Arc::new(Mutex::new(HashMap::new())),
        };
        storage.migrate()?;
        Ok(storage)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let storage = Self {
            conn: Arc::new(Mutex::new(conn)),
            db_dir: None,
            rollout_root: Arc::new(RwLock::new(None)),
            rollout_writers: Arc::new(Mutex::new(HashMap::new())),
        };
        storage.migrate()?;
        Ok(storage)
    }

    pub fn notes(&self) -> Result<crate::notes::RolloutNotesManager> {
        crate::notes::RolloutNotesManager::new(Arc::clone(&self.conn))
            .map_err(|e| anyhow::anyhow!(e))
    }

    pub fn history(&self) -> Result<crate::history_recovery::HistoryRecoveryStore> {
        crate::history_recovery::HistoryRecoveryStore::new(Arc::clone(&self.conn))
            .map_err(|e| anyhow::anyhow!(e))
    }

    pub fn queue(&self) -> crate::queue_store::QueueStore {
        crate::queue_store::QueueStore::new(Arc::clone(&self.conn))
    }

    pub fn workflow_store(&self) -> crate::workflow_store::WorkflowStore {
        crate::workflow_store::WorkflowStore::new(Arc::clone(&self.conn))
    }

    pub fn log_db(&self) -> crate::log_db::LogDatabase {
        // §6.2: dedicated logs pool (separate file beside the main DB) so log
        // writes contend with the primary connection as little as possible.
        // Falls back to the shared connection (in-memory storage).
        if let Some(dir) = &self.db_dir
            && let Ok(db) = crate::log_db::LogDatabase::open_dedicated(&dir.join("logs.db"))
        {
            return db;
        }
        crate::log_db::LogDatabase::new(Arc::clone(&self.conn))
    }

    pub fn transcript_exporter(&self) -> crate::transcript_export::TranscriptExporter {
        crate::transcript_export::TranscriptExporter::new(Arc::clone(&self.conn))
    }

    pub fn live_sync(&self, debounce_threshold: usize) -> crate::live_sync::LiveSyncManager {
        crate::live_sync::LiveSyncManager::new(Arc::clone(&self.conn), debounce_threshold)
    }

    /// §6.1: enables the JSONL rollout mirror under `dir` (one
    /// `rollout-<timestamp>-<thread_id>.jsonl` per thread). Best-effort:
    /// mirror failures never block SQLite persistence.
    pub fn set_rollout_root(&self, dir: PathBuf) {
        *self.rollout_root.write().unwrap() = Some(dir);
    }

    pub fn rollout_root(&self) -> Option<PathBuf> {
        self.rollout_root.read().unwrap().clone()
    }

    fn mirror_rollout(&self, thread_id: &str, kind: &str, payload: &serde_json::Value) {
        let Some(root) = self.rollout_root() else {
            return;
        };
        let mut writers = self.rollout_writers.lock().unwrap();
        let writer = writers
            .entry(thread_id.to_string())
            .or_insert_with(|| crate::rollout::RolloutWriter::new(&root, thread_id));
        writer.enqueue(kind, payload);
        if let Err(e) = writer.write_pending_with_recovery() {
            eprintln!("rollout mirror write failed (non-fatal): {e}");
        }
    }

    /// Replays a thread's mirrored rollout history (§6.1).
    pub fn read_thread_rollout(&self, thread_id: &str) -> Result<Vec<crate::rollout::RolloutItem>> {
        let root = self.rollout_root().context("rollout mirror not enabled")?;
        crate::rollout::read_rollout(&root, thread_id)
    }

    fn migrate(&self) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA busy_timeout = 5000;
            PRAGMA cache_size = -64000;
            PRAGMA temp_store = MEMORY;
            PRAGMA mmap_size = 268435456;
            "#,
        )?;

        crate::migrations::MigrationEngine::apply_all(
            &mut conn,
            crate::migrations::BUILTIN_MIGRATIONS,
        )?;
        Ok(())
    }

    pub fn save_thread(
        &self,
        meta: &SessionMetadata,
        rollout_path: &str,
        sandbox: &str,
        approval: &str,
    ) -> Result<()> {
        // §9.1: Resolve external metadata outside the database connection critical section
        let git_meta = if self.rollout_root().is_some() {
            let run = |args: &[&str]| -> Option<String> {
                let out = std::process::Command::new("git")
                    .args(args)
                    .current_dir(&meta.cwd)
                    .output()
                    .ok()?;
                if out.status.success() {
                    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
                } else {
                    None
                }
            };
            let branch = run(&["rev-parse", "--abbrev-ref", "HEAD"]);
            let commit = run(&["rev-parse", "HEAD"]);
            if branch.is_none() && commit.is_none() {
                None
            } else {
                Some(serde_json::json!({ "branch": branch, "commit": commit }))
            }
        } else {
            None
        };

        {
            let conn = self.conn.lock().unwrap();
            conn.execute(
                r#"
                INSERT OR REPLACE INTO threads (
                    id, rollout_path, created_at, updated_at, source, model_provider, cwd, title, sandbox_policy, approval_mode, is_archived, parent_thread_id
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, NULL)
                "#,
                params![
                    meta.id,
                    rollout_path,
                    meta.created_at.timestamp(),
                    meta.updated_at.timestamp(),
                    "m31a-cli",
                    meta.model,
                    meta.cwd,
                    meta.title,
                    sandbox,
                    approval
                ],
            )?;
        }

        // §6.1 / §9.1: Best-effort session-meta rollout record mirrored outside the database mutex
        if self.rollout_root().is_some() {
            self.mirror_rollout(
                &meta.id,
                "session_meta",
                &serde_json::json!({
                    "thread_id": meta.id,
                    "title": meta.title,
                    "model": meta.model,
                    "cwd": meta.cwd,
                    "sandbox": sandbox,
                    "approval": approval,
                    "git": git_meta,
                }),
            );
        }
        Ok(())
    }

    pub fn get_thread(&self, id: &str) -> Result<Option<ThreadRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, title, model_provider, cwd, sandbox_policy, approval_mode, is_archived, parent_thread_id, section_id, created_at, updated_at
            FROM threads
            WHERE id = ?1
            "#,
        )?;

        let res = stmt
            .query_row(params![id], |row| {
                Ok(ThreadRecord {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    model: row.get(2)?,
                    cwd: row.get(3)?,
                    sandbox_policy: row.get(4)?,
                    approval_mode: row.get(5)?,
                    is_archived: row.get::<_, i64>(6)? != 0,
                    parent_thread_id: row.get(7)?,
                    section_id: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })
            .optional()?;

        Ok(res)
    }

    pub fn list_threads(&self, limit: usize) -> Result<Vec<ThreadRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, title, model_provider, cwd, sandbox_policy, approval_mode, is_archived, parent_thread_id, section_id, created_at, updated_at
            FROM threads
            WHERE is_archived = 0
            ORDER BY updated_at DESC
            LIMIT ?1
            "#,
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(ThreadRecord {
                id: row.get(0)?,
                title: row.get(1)?,
                model: row.get(2)?,
                cwd: row.get(3)?,
                sandbox_policy: row.get(4)?,
                approval_mode: row.get(5)?,
                is_archived: row.get::<_, i64>(6)? != 0,
                parent_thread_id: row.get(7)?,
                section_id: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn archive_thread(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE threads SET is_archived = 1 WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn unarchive_thread(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE threads SET is_archived = 0 WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn delete_thread(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM threads WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn set_thread_owner(&self, thread_id: &str, client_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            r#"
            INSERT OR REPLACE INTO thread_ownership (thread_id, owner_client_id, created_at_epoch_ms, updated_at_epoch_ms)
            VALUES (?1, ?2, COALESCE((SELECT created_at_epoch_ms FROM thread_ownership WHERE thread_id = ?1), ?3), ?3)
            "#,
            params![thread_id, client_id, now],
        )?;
        Ok(())
    }

    pub fn get_thread_owner(&self, thread_id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT owner_client_id FROM thread_ownership WHERE thread_id = ?1")?;
        let res = stmt
            .query_row(params![thread_id], |row| row.get(0))
            .optional()?;
        Ok(res)
    }

    pub fn list_thread_owners(&self) -> Result<std::collections::HashMap<String, String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT thread_id, owner_client_id FROM thread_ownership")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        let mut map = std::collections::HashMap::new();
        for r in rows {
            let (k, v) = r?;
            map.insert(k, v);
        }
        Ok(map)
    }

    pub fn save_session_todos(&self, thread_id: &str, todos: &[SessionTodoRecord]) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM session_todos WHERE thread_id = ?1",
            params![thread_id],
        )?;
        let now = chrono::Utc::now().timestamp_millis();
        for (idx, item) in todos.iter().enumerate() {
            tx.execute(
                r#"
                INSERT INTO session_todos (id, thread_id, content, status, position, created_at_epoch_ms, updated_at_epoch_ms)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
                params![
                    item.id,
                    thread_id,
                    item.content,
                    item.status,
                    idx as i64,
                    if item.created_at_epoch_ms > 0 { item.created_at_epoch_ms } else { now },
                    now
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_session_todos(&self, thread_id: &str) -> Result<Vec<SessionTodoRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, thread_id, content, status, position, created_at_epoch_ms, updated_at_epoch_ms
            FROM session_todos
            WHERE thread_id = ?1
            ORDER BY position ASC
            "#,
        )?;
        let rows = stmt.query_map(params![thread_id], |row| {
            Ok(SessionTodoRecord {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                content: row.get(2)?,
                status: row.get(3)?,
                position: row.get(4)?,
                created_at_epoch_ms: row.get(5)?,
                updated_at_epoch_ms: row.get(6)?,
            })
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn fork_thread(&self, parent_id: &str, new_title: &str) -> Result<ThreadRecord> {
        self.fork_thread_at(parent_id, new_title, None)
    }

    pub fn fork_thread_at(
        &self,
        parent_id: &str,
        new_title: &str,
        up_to_turn: Option<usize>,
    ) -> Result<ThreadRecord> {
        let parent = self
            .get_thread(parent_id)?
            .ok_or_else(|| anyhow!("Parent thread not found: {parent_id}"))?;

        let new_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();

        let forked = ThreadRecord {
            id: new_id.clone(),
            title: new_title.to_string(),
            model: parent.model.clone(),
            cwd: parent.cwd.clone(),
            sandbox_policy: parent.sandbox_policy.clone(),
            approval_mode: parent.approval_mode.clone(),
            is_archived: false,
            parent_thread_id: Some(parent_id.to_string()),
            section_id: parent.section_id.clone(),
            created_at: now,
            updated_at: now,
        };

        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO threads (
                id, rollout_path, created_at, updated_at, source, model_provider, cwd, title, sandbox_policy, approval_mode, is_archived, parent_thread_id
            ) VALUES (?1, '', ?2, ?3, 'fork', ?4, ?5, ?6, ?7, ?8, 0, ?9)
            "#,
            params![
                forked.id,
                forked.created_at,
                forked.updated_at,
                forked.model,
                forked.cwd,
                forked.title,
                forked.sandbox_policy,
                forked.approval_mode,
                parent_id
            ],
        )?;

        // Duplicate previous turns & items up to up_to_turn into forked thread
        if let Some(max_turn) = up_to_turn {
            conn.execute(
                r#"
                INSERT INTO thread_turns (id, thread_id, turn_index, status, created_at)
                SELECT lower(hex(randomblob(16))), ?1, turn_index, status, created_at
                FROM thread_turns
                WHERE thread_id = ?2 AND turn_index <= ?3
                "#,
                params![forked.id, parent_id, max_turn as i64],
            )?;

            conn.execute(
                r#"
                INSERT INTO thread_items (id, thread_id, turn_id, item_type, content, created_at)
                SELECT lower(hex(randomblob(16))), ?1, turn_id, item_type, content, created_at
                FROM thread_items
                WHERE thread_id = ?2 AND (turn_id IS NULL OR turn_id IN (
                    SELECT id FROM thread_turns WHERE thread_id = ?2 AND turn_index <= ?3
                ))
                "#,
                params![forked.id, parent_id, max_turn as i64],
            )?;
        } else {
            conn.execute(
                r#"
                INSERT INTO thread_turns (id, thread_id, turn_index, status, created_at)
                SELECT lower(hex(randomblob(16))), ?1, turn_index, status, created_at
                FROM thread_turns
                WHERE thread_id = ?2
                "#,
                params![forked.id, parent_id],
            )?;

            conn.execute(
                r#"
                INSERT INTO thread_items (id, thread_id, turn_id, item_type, content, created_at)
                SELECT lower(hex(randomblob(16))), ?1, turn_id, item_type, content, created_at
                FROM thread_items
                WHERE thread_id = ?2
                "#,
                params![forked.id, parent_id],
            )?;
        }

        Ok(forked)
    }

    /// §6.3 CAS helper: atomically replaces `rollout_path` ONLY if the
    /// current value matches `expected`. Returns true on success (CAS
    /// succeeded). Fails the entire operation if someone else changed the
    /// pointer in the interim — callers should retry with the new expected
    /// value.
    fn replace_rollout_path_if_current(
        conn: &mut rusqlite::Connection,
        thread_id: &str,
        expected: &str,
        new_path: &str,
    ) -> Result<bool> {
        let changed = conn.execute(
            "UPDATE threads SET rollout_path = ?1 WHERE id = ?2 AND rollout_path = ?3",
            params![new_path, thread_id, expected],
        )?;
        Ok(changed > 0)
    }

    /// §6.3 Immutable revert + CAS pointer cutover.
    /// When the rollout mirror is enabled:
    ///   1. Materialize a NEW rollout file containing ONLY the retained
    ///      prefix (turns ≤ target_turn_index) plus a revert_point meta
    ///      record. The old rollout file is NEVER mutated.
    ///   2. CAS the threads.rollout_path pointer to the new file.
    ///   3. Update forked_from_ordinal_exclusive to MIN(existing,
    ///      first_dropped_ordinal) — shrink-only (revert cannot grow the
    ///      inherited prefix).
    ///
    /// SQLite index truncation remains (rollback of the index is cheap and
    /// expected).
    pub fn revert_thread(&self, thread_id: &str, target_turn_index: usize) -> Result<usize> {
        // Phase 1: read-only — gather turn IDs and the first-dropped turn_index
        // (used as ordinal for forked_from_ordinal_exclusive shrink).
        let (turn_ids, first_dropped_turn_index) = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT id, turn_index FROM thread_turns WHERE thread_id = ?1 AND turn_index > ?2 ORDER BY turn_index ASC",
            )?;
            let mut ids = Vec::new();
            let mut first_idx: Option<i64> = None;
            for row in stmt.query_map(params![thread_id, target_turn_index as i64], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })? {
                let (id, idx) = row?;
                if first_idx.is_none() {
                    first_idx = Some(idx);
                }
                ids.push(id);
            }
            (ids, first_idx)
        };

        // Phase 2: SQLite index truncation (in-place, always done).
        let count = turn_ids.len();
        {
            let conn = self.conn.lock().unwrap();
            for tid in &turn_ids {
                conn.execute("DELETE FROM thread_items WHERE turn_id = ?1", params![tid])?;
            }
            conn.execute(
                "DELETE FROM thread_turns WHERE thread_id = ?1 AND turn_index > ?2",
                params![thread_id, target_turn_index as i64],
            )?;

            // §6.3 fork lineage: shrink the inherited prefix.
            if let Some(idx) = first_dropped_turn_index {
                conn.execute(
                    "UPDATE threads SET forked_from_ordinal_exclusive = MIN(COALESCE(forked_from_ordinal_exclusive, ?1), ?1) WHERE id = ?2",
                    params![idx, thread_id],
                )?;
            }

            let now = chrono::Utc::now().timestamp();
            conn.execute(
                "UPDATE threads SET updated_at = ?1 WHERE id = ?2",
                params![now, thread_id],
            )?;
        }

        // Phase 3: mirror path — if enabled, materialize the new immutable
        // file and CAS the pointer. Writers and conn are NEVER held
        // simultaneously (lock discipline).
        if let Some(root) = self.rollout_root() {
            // Read the retained prefix from the current rollout file(s).
            let retained: Vec<crate::rollout::RolloutItem> = self.read_thread_rollout(thread_id)?;

            // Write a fresh rollout file with only the retained prefix + a
            // revert_point record.
            let ts = chrono::Local::now().format("%Y-%m-%dT%H-%M-%S");
            let name = format!(
                "rollout-{ts}-{}_revert-{}.jsonl",
                thread_id, target_turn_index
            );
            let new_path = root.join(name);
            std::fs::create_dir_all(&root)?;
            {
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&new_path)
                    .with_context(|| {
                        format!("Failed creating revert rollout: {}", new_path.display())
                    })?;
                for item in &retained {
                    let line = item.to_line();
                    file.write_all(line.as_bytes())?;
                    file.write_all(b"\n")?;
                }
                // Revert point meta record (not a normal item).
                let revert_meta = crate::rollout::RolloutItem {
                    ordinal: retained.last().map(|i| i.ordinal + 1).unwrap_or(0),
                    ts_ms: chrono::Utc::now().timestamp_millis(),
                    kind: "revert_point".to_string(),
                    payload: serde_json::json!({
                        "target_turn_index": target_turn_index,
                        "dropped_turn_count": count,
                    }),
                };
                let line = revert_meta.to_line();
                file.write_all(line.as_bytes())?;
                file.write_all(b"\n")?;
            }

            // CAS cutover — retry on contention.
            let new_path_str = new_path.to_string_lossy().to_string();
            let mut cas_succeeded = false;
            for _ in 0..10 {
                let expected = {
                    let conn = self.conn.lock().unwrap();
                    conn.query_row(
                        "SELECT rollout_path FROM threads WHERE id = ?1",
                        params![thread_id],
                        |r| r.get::<_, String>(0),
                    )
                    .unwrap_or_default()
                };
                let mut conn = self.conn.lock().unwrap();
                cas_succeeded = Self::replace_rollout_path_if_current(
                    &mut conn,
                    thread_id,
                    &expected,
                    &new_path_str,
                )?;
                if cas_succeeded {
                    break;
                }
                // Small backoff before retry.
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            if !cas_succeeded {
                eprintln!(
                    "rollback CAS failed for thread {} after retries; old rollout file retained at {}",
                    thread_id,
                    new_path.display()
                );
            }
        }

        Ok(count)
    }

    /// §5.5: persists a host-owned fact that survives compaction outside the
    /// summarization contract (verified Q/A, retained user text, compaction
    /// checkpoints). Ordinals are assigned by the store, scoped per
    /// (thread, kind, origin); they must not be compared across origins.
    pub fn save_retained_fact(
        &self,
        thread_id: &str,
        kind: &str,
        origin: &str,
        content: &str,
        model_hash: Option<&str>,
    ) -> Result<RetainedFact> {
        let conn = self.conn.lock().unwrap();
        let id = format!("rf-{}", Uuid::new_v4());
        let now = chrono::Utc::now().timestamp_millis();

        let ordinal: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(ordinal), -1) + 1 FROM retained_facts
                 WHERE thread_id = ?1 AND kind = ?2 AND origin = ?3",
                params![thread_id, kind, origin],
                |row| row.get(0),
            )
            .unwrap_or(0);

        conn.execute(
            "INSERT INTO retained_facts (id, thread_id, kind, origin, ordinal, content, model_hash, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, thread_id, kind, origin, ordinal, content, model_hash, now],
        )?;

        Ok(RetainedFact {
            id,
            thread_id: thread_id.to_string(),
            kind: kind.to_string(),
            origin: origin.to_string(),
            ordinal,
            content: content.to_string(),
            model_hash: model_hash.map(|s| s.to_string()),
            created_at: now,
        })
    }

    /// §5.5: lists retained facts for a thread. Malformed rows (e.g.
    /// checkpoints with a missing model_hash) are RETURNED, not filtered —
    /// visible data beats silently discarded evidence (§5.5 checkpoint
    /// compatibility).
    pub fn list_retained_facts(&self, thread_id: &str, kind: &str) -> Result<Vec<RetainedFact>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, thread_id, kind, origin, ordinal, content, model_hash, created_at
             FROM retained_facts WHERE thread_id = ?1 AND kind = ?2
             ORDER BY origin ASC, ordinal ASC",
        )?;
        let rows = stmt.query_map(params![thread_id, kind], |row| {
            Ok(RetainedFact {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                kind: row.get(2)?,
                origin: row.get(3)?,
                ordinal: row.get(4)?,
                content: row.get(5)?,
                model_hash: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Creates a checkpoint snapshot capturing git commit, turn offset, modified files, and optional metadata.
    pub fn create_checkpoint(
        &self,
        thread_id: &str,
        label: &str,
        git_commit: Option<&str>,
        turn_index: usize,
        modified_files: &[String],
        metadata: Option<&serde_json::Value>,
    ) -> Result<CheckpointRecord> {
        let conn = self.conn.lock().unwrap();
        let id = format!("cp-{}", Uuid::new_v4());
        let now = chrono::Utc::now().timestamp_millis();
        let modified_json =
            serde_json::to_string(modified_files).unwrap_or_else(|_| "[]".to_string());
        let meta_json = metadata.map(|m| serde_json::to_string(m).unwrap_or_default());

        conn.execute(
            r#"
            INSERT INTO checkpoints (
                id, thread_id, label, git_commit, turn_index, modified_files, metadata, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                id,
                thread_id,
                label,
                git_commit,
                turn_index as i64,
                modified_json,
                meta_json,
                now
            ],
        )?;

        Ok(CheckpointRecord {
            id,
            thread_id: thread_id.to_string(),
            label: label.to_string(),
            git_commit: git_commit.map(str::to_string),
            turn_index,
            modified_files: modified_files.to_vec(),
            metadata: metadata.cloned(),
            created_at: now,
        })
    }

    /// Retrieves a single checkpoint by its ID.
    pub fn get_checkpoint(&self, checkpoint_id: &str) -> Result<Option<CheckpointRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, thread_id, label, git_commit, turn_index, modified_files, metadata, created_at
            FROM checkpoints
            WHERE id = ?1
            "#,
        )?;

        let res = stmt
            .query_row(params![checkpoint_id], |row| {
                let files_str: String = row.get(5)?;
                let meta_str: Option<String> = row.get(6)?;
                let modified_files: Vec<String> =
                    serde_json::from_str(&files_str).unwrap_or_default();
                let metadata: Option<serde_json::Value> =
                    meta_str.and_then(|s| serde_json::from_str(&s).ok());

                Ok(CheckpointRecord {
                    id: row.get(0)?,
                    thread_id: row.get(1)?,
                    label: row.get(2)?,
                    git_commit: row.get(3)?,
                    turn_index: row.get::<_, i64>(4)? as usize,
                    modified_files,
                    metadata,
                    created_at: row.get(7)?,
                })
            })
            .optional()?;

        Ok(res)
    }

    /// Lists all checkpoints associated with a thread ordered newest first.
    pub fn list_checkpoints(&self, thread_id: &str) -> Result<Vec<CheckpointRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, thread_id, label, git_commit, turn_index, modified_files, metadata, created_at
            FROM checkpoints
            WHERE thread_id = ?1
            ORDER BY created_at DESC, rowid DESC
            "#,
        )?;

        let rows = stmt.query_map(params![thread_id], |row| {
            let files_str: String = row.get(5)?;
            let meta_str: Option<String> = row.get(6)?;
            let modified_files: Vec<String> = serde_json::from_str(&files_str).unwrap_or_default();
            let metadata: Option<serde_json::Value> =
                meta_str.and_then(|s| serde_json::from_str(&s).ok());

            Ok(CheckpointRecord {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                label: row.get(2)?,
                git_commit: row.get(3)?,
                turn_index: row.get::<_, i64>(4)? as usize,
                modified_files,
                metadata,
                created_at: row.get(7)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Deletes a checkpoint snapshot by ID.
    pub fn delete_checkpoint(&self, checkpoint_id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "DELETE FROM checkpoints WHERE id = ?1",
            params![checkpoint_id],
        )?;
        Ok(rows > 0)
    }

    /// Restores a checkpoint snapshot, optionally reverting the conversation turns and history in SQLite.
    pub fn restore_checkpoint(
        &self,
        checkpoint_id: &str,
        revert_conversation: bool,
    ) -> Result<CheckpointRecord> {
        let cp = self
            .get_checkpoint(checkpoint_id)?
            .ok_or_else(|| anyhow!("Checkpoint '{}' not found", checkpoint_id))?;

        if revert_conversation {
            self.revert_thread(&cp.thread_id, cp.turn_index)?;
        }

        Ok(cp)
    }

    /// Resolves the ancestor fork lineage of a thread back to the root thread.
    pub fn thread_lineage(&self, thread_id: &str) -> Result<Vec<ThreadRecord>> {
        let mut lineage = Vec::new();
        let mut current_id = Some(thread_id.to_string());
        let mut seen = std::collections::HashSet::new();

        while let Some(id) = current_id {
            if seen.contains(&id) {
                break; // Cycle detected
            }
            seen.insert(id.clone());

            if let Some(record) = self.get_thread(&id)? {
                current_id = record.parent_thread_id.clone();
                lineage.push(record);
            } else {
                break;
            }
        }

        Ok(lineage)
    }

    pub fn search_threads(&self, query: &str, limit: usize) -> Result<Vec<ThreadRecord>> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("%{}%", query);
        let mut stmt = conn.prepare(
            r#"
            SELECT id, title, model_provider, cwd, sandbox_policy, approval_mode, is_archived, parent_thread_id, section_id, created_at, updated_at
            FROM threads
            WHERE title LIKE ?1 OR id LIKE ?1
            ORDER BY updated_at DESC
            LIMIT ?2
            "#,
        )?;
        let rows = stmt.query_map(params![pattern, limit as i64], |row| {
            Ok(ThreadRecord {
                id: row.get(0)?,
                title: row.get(1)?,
                model: row.get(2)?,
                cwd: row.get(3)?,
                sandbox_policy: row.get(4)?,
                approval_mode: row.get(5)?,
                is_archived: row.get::<_, i64>(6)? != 0,
                parent_thread_id: row.get(7)?,
                section_id: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn create_thread_section(
        &self,
        name: &str,
        color: Option<&str>,
        icon: Option<&str>,
    ) -> Result<ThreadSectionRecord> {
        let conn = self.conn.lock().unwrap();
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT INTO thread_sections (id, name, color, icon, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, name, color, icon, now],
        )?;
        Ok(ThreadSectionRecord {
            id,
            name: name.to_string(),
            color: color.map(|s| s.to_string()),
            icon: icon.map(|s| s.to_string()),
            created_at: now,
        })
    }

    pub fn list_thread_sections(&self) -> Result<Vec<ThreadSectionRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, color, icon, created_at FROM thread_sections ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ThreadSectionRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                icon: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn rename_thread_section(
        &self,
        id: &str,
        new_name: &str,
        color: Option<&str>,
        icon: Option<&str>,
    ) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute(
            "UPDATE thread_sections SET name = ?1, color = ?2, icon = ?3 WHERE id = ?4",
            params![new_name, color, icon, id],
        )?;
        Ok(count > 0)
    }

    pub fn delete_thread_section(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let _ = conn.execute(
            "UPDATE threads SET section_id = NULL WHERE section_id = ?1",
            params![id],
        );
        let count = conn.execute("DELETE FROM thread_sections WHERE id = ?1", params![id])?;
        Ok(count > 0)
    }

    pub fn move_thread_to_section(
        &self,
        thread_id: &str,
        section_id: Option<&str>,
    ) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute(
            "UPDATE threads SET section_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![section_id, chrono::Utc::now().timestamp(), thread_id],
        )?;
        Ok(count > 0)
    }

    pub fn list_threads_by_section(
        &self,
        section_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ThreadRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut list = Vec::new();
        if let Some(sec) = section_id {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, title, model_provider, cwd, sandbox_policy, approval_mode, is_archived, parent_thread_id, section_id, created_at, updated_at
                FROM threads
                WHERE is_archived = 0 AND section_id = ?1
                ORDER BY updated_at DESC
                LIMIT ?2
                "#,
            )?;
            let rows = stmt.query_map(params![sec, limit as i64], |row| {
                Ok(ThreadRecord {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    model: row.get(2)?,
                    cwd: row.get(3)?,
                    sandbox_policy: row.get(4)?,
                    approval_mode: row.get(5)?,
                    is_archived: row.get::<_, i64>(6)? != 0,
                    parent_thread_id: row.get(7)?,
                    section_id: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })?;
            for r in rows {
                list.push(r?);
            }
        } else {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, title, model_provider, cwd, sandbox_policy, approval_mode, is_archived, parent_thread_id, section_id, created_at, updated_at
                FROM threads
                WHERE is_archived = 0 AND section_id IS NULL
                ORDER BY updated_at DESC
                LIMIT ?1
                "#,
            )?;
            let rows = stmt.query_map(params![limit as i64], |row| {
                Ok(ThreadRecord {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    model: row.get(2)?,
                    cwd: row.get(3)?,
                    sandbox_policy: row.get(4)?,
                    approval_mode: row.get(5)?,
                    is_archived: row.get::<_, i64>(6)? != 0,
                    parent_thread_id: row.get(7)?,
                    section_id: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })?;
            for r in rows {
                list.push(r?);
            }
        }
        Ok(list)
    }

    pub fn get_thread_stats(&self, thread_id: &str) -> Result<ThreadStats> {
        let conn = self.conn.lock().unwrap();
        let total_turns: usize = conn
            .query_row(
                "SELECT COUNT(*) FROM thread_turns WHERE thread_id = ?1",
                params![thread_id],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let mut stmt = conn.prepare(
            "SELECT item_type, COUNT(*) FROM thread_items WHERE thread_id = ?1 GROUP BY item_type",
        )?;
        let mut stats = ThreadStats {
            total_turns,
            ..Default::default()
        };

        let rows = stmt.query_map(params![thread_id], |row| {
            let item_type: String = row.get(0)?;
            let count: usize = row.get(1)?;
            Ok((item_type, count))
        })?;

        for r in rows {
            let (item_type, count) = r?;
            stats.total_items += count;
            match item_type.as_str() {
                "user_message" => stats.user_messages += count,
                "agent_response" => stats.assistant_messages += count,
                "tool_call" => stats.tool_calls += count,
                _ => {}
            }
        }

        Ok(stats)
    }

    pub fn save_turn(
        &self,
        turn_id: &str,
        thread_id: &str,
        turn_idx: usize,
        status: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT OR REPLACE INTO thread_turns (id, thread_id, turn_index, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![turn_id, thread_id, turn_idx as i64, status, now],
        )?;
        Ok(())
    }

    pub fn list_turns(&self, thread_id: &str) -> Result<Vec<TurnRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, thread_id, turn_index, status, created_at FROM thread_turns WHERE thread_id = ?1 ORDER BY turn_index ASC",
        )?;
        let rows = stmt.query_map(params![thread_id], |row| {
            Ok(TurnRecord {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                turn_index: row.get::<_, i64>(2)? as usize,
                status: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn get_turn(&self, turn_id: &str) -> Result<Option<TurnRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, thread_id, turn_index, status, created_at FROM thread_turns WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![turn_id], |row| {
            Ok(TurnRecord {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                turn_index: row.get::<_, i64>(2)? as usize,
                status: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        if let Some(r) = rows.next() {
            Ok(Some(r?))
        } else {
            Ok(None)
        }
    }

    pub fn save_item(
        &self,
        item_id: &str,
        thread_id: &str,
        turn_id: Option<&str>,
        item_type: &str,
        content: &str,
    ) -> Result<()> {
        // §3.5: redact credential material at the rollout boundary. Persisted
        // items are the replay/log surface (TUI history restore, app-server
        // listing, checkpoints), so nothing reaches disk unredacted — matching
        // upstream `secrets/src/sanitizer.rs` rollout behavior.
        let content = m31a_core::redact_secrets(content);
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT OR REPLACE INTO thread_items (id, thread_id, turn_id, item_type, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![item_id, thread_id, turn_id, item_type, content, now],
        )?;

        // §6.1: best-effort JSONL rollout mirror. `content` is already
        // redacted; the 64 KiB persist truncation applies here.
        if self.rollout_root().is_some() {
            self.mirror_rollout(
                thread_id,
                "item",
                &serde_json::json!({
                    "item_id": item_id,
                    "turn_id": turn_id,
                    "item_type": item_type,
                    "content": crate::rollout::truncate_payload_for_persist(&content),
                }),
            );
        }
        Ok(())
    }

    pub fn list_items(&self, thread_id: &str) -> Result<Vec<(String, String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT item_type, content, id FROM thread_items WHERE thread_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![thread_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn clear_thread_items(&self, thread_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM thread_items WHERE thread_id = ?1",
            params![thread_id],
        )?;
        Ok(())
    }

    pub fn replace_thread_items(&self, thread_id: &str, items: &[(String, String)]) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM thread_items WHERE thread_id = ?1",
            params![thread_id],
        )?;
        let base_time = chrono::Utc::now().timestamp();
        for (idx, (item_type, content)) in items.iter().enumerate() {
            let item_id = format!("item_{}_{}", Uuid::new_v4().simple(), idx);
            let time = base_time + idx as i64;
            tx.execute(
                "INSERT INTO thread_items (id, thread_id, turn_id, item_type, content, created_at) VALUES (?1, ?2, NULL, ?3, ?4, ?5)",
                params![item_id, thread_id, item_type, content, time],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn record_spawn_edge(&self, parent: &str, child: &str, role: &str) -> Result<()> {
        self.upsert_spawn_edge(parent, child, role, "open", None)
    }

    /// Upsert a spawn edge. If depth is None, compute it from parent's depth + 1.
    pub fn upsert_spawn_edge(
        &self,
        parent: &str,
        child: &str,
        role: &str,
        status: &str,
        depth: Option<u32>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp();
        let depth = match depth {
            Some(d) => d,
            None => {
                // Compute depth from parent
                let mut stmt = conn
                    .prepare("SELECT depth FROM thread_spawn_edges WHERE child_thread_id = ?1")?;
                stmt.query_row(params![parent], |row| row.get(0))
                    .unwrap_or(0)
                    + 1
            }
        };
        conn.execute(
            "INSERT OR REPLACE INTO thread_spawn_edges (parent_thread_id, child_thread_id, agent_role, status, created_at, depth) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![parent, child, role, status, now, depth as i64],
        )?;
        Ok(())
    }

    /// Get the depth of an agent by its thread ID
    pub fn get_agent_depth(&self, agent_thread_id: &str) -> Result<u32> {
        let conn = self.conn.lock().unwrap();
        let depth: i64 = conn.query_row(
            "SELECT depth FROM thread_spawn_edges WHERE child_thread_id = ?1",
            params![agent_thread_id],
            |row| row.get(0),
        )?;
        Ok(depth as u32)
    }

    pub fn set_spawn_edge_status(&self, child: &str, status: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE thread_spawn_edges SET status = ?1 WHERE child_thread_id = ?2",
            params![status, child],
        )?;
        Ok(())
    }

    pub fn list_subagents(&self, parent: &str) -> Result<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT child_thread_id, agent_role FROM thread_spawn_edges WHERE parent_thread_id = ?1 AND status = 'open' ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![parent], |row| Ok((row.get(0)?, row.get(1)?)))?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn list_spawn_children(
        &self,
        parent: &str,
        status_filter: Option<&str>,
    ) -> Result<Vec<ThreadSpawnEdgeRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut list = Vec::new();
        if let Some(status) = status_filter {
            let mut stmt = conn.prepare(
                "SELECT parent_thread_id, child_thread_id, agent_role, status, created_at, depth FROM thread_spawn_edges WHERE parent_thread_id = ?1 AND status = ?2 ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map(params![parent, status], |row| {
                Ok(ThreadSpawnEdgeRecord {
                    parent_thread_id: row.get(0)?,
                    child_thread_id: row.get(1)?,
                    agent_role: row.get(2)?,
                    status: row.get(3)?,
                    created_at: row.get(4)?,
                    depth: row.get(5)?,
                })
            })?;
            for r in rows {
                list.push(r?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT parent_thread_id, child_thread_id, agent_role, status, created_at, depth FROM thread_spawn_edges WHERE parent_thread_id = ?1 ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map(params![parent], |row| {
                Ok(ThreadSpawnEdgeRecord {
                    parent_thread_id: row.get(0)?,
                    child_thread_id: row.get(1)?,
                    agent_role: row.get(2)?,
                    status: row.get(3)?,
                    created_at: row.get(4)?,
                    depth: row.get(5)?,
                })
            })?;
            for r in rows {
                list.push(r?);
            }
        }
        Ok(list)
    }

    /// List spawned descendants depth-first or breadth-first using an atomic SQLite Recursive CTE.
    pub fn list_spawn_descendants(
        &self,
        root: &str,
        status_filter: Option<&str>,
    ) -> Result<Vec<ThreadSpawnEdgeRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut list = Vec::new();

        let query = r#"
            WITH RECURSIVE subtree(parent_thread_id, child_thread_id, agent_role, status, created_at, depth) AS (
                SELECT parent_thread_id, child_thread_id, agent_role, status, created_at, depth
                FROM thread_spawn_edges
                WHERE parent_thread_id = ?1
                UNION ALL
                SELECT e.parent_thread_id, e.child_thread_id, e.agent_role, e.status, e.created_at, e.depth
                FROM thread_spawn_edges e
                JOIN subtree s ON e.parent_thread_id = s.child_thread_id
            )
            SELECT parent_thread_id, child_thread_id, agent_role, status, created_at, depth
            FROM subtree
            WHERE (?2 IS NULL OR status = ?2)
            ORDER BY depth ASC, created_at ASC
        "#;

        let mut stmt = conn.prepare(query)?;
        let rows = stmt.query_map(rusqlite::params![root, status_filter], |row| {
            Ok(ThreadSpawnEdgeRecord {
                parent_thread_id: row.get(0)?,
                child_thread_id: row.get(1)?,
                agent_role: row.get(2)?,
                status: row.get(3)?,
                created_at: row.get(4)?,
                depth: row.get(5)?,
            })
        })?;

        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Terminate an entire agent tree rooted at root_child_id in bottom-up order (depth DESC)
    pub fn terminate_agent_tree(&self, root_child_id: &str) -> Result<usize> {
        let mut terminated_count = 0;
        let mut descendants = self.list_spawn_descendants(root_child_id, Some("open"))?;
        // Sort descending by depth so leaf subagents terminate before parents
        descendants.sort_by_key(|a| std::cmp::Reverse(a.depth));

        for desc in descendants {
            self.set_spawn_edge_status(&desc.child_thread_id, "terminated")?;
            terminated_count += 1;
        }
        self.set_spawn_edge_status(root_child_id, "terminated")?;
        terminated_count += 1;
        Ok(terminated_count)
    }

    pub fn set_goal(&self, thread_id: &str, objective: &str, budget: Option<u64>) -> Result<Goal> {
        let conn = self.conn.lock().unwrap();
        let goal_id = Uuid::new_v4().to_string();
        let now_ms = chrono::Utc::now().timestamp_millis();

        conn.execute(
            r#"
            INSERT OR REPLACE INTO thread_goals (
                thread_id, goal_id, objective, status, token_budget, tokens_used, time_used_seconds, created_at_ms
            ) VALUES (?1, ?2, ?3, 'active', ?4, 0, 0, ?5)
            "#,
            params![thread_id, goal_id, objective, budget, now_ms],
        )?;

        Ok(Goal {
            thread_id: thread_id.to_string(),
            goal_id,
            objective: objective.to_string(),
            status: "active".to_string(),
            token_budget: budget,
            tokens_used: 0,
            time_used_seconds: 0,
            created_at_ms: now_ms,
        })
    }

    pub fn get_goal(&self, thread_id: &str) -> Result<Option<Goal>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT thread_id, goal_id, objective, status, token_budget, tokens_used, time_used_seconds, created_at_ms
            FROM thread_goals
            WHERE thread_id = ?1
            "#,
        )?;

        let goal = stmt
            .query_row(params![thread_id], |row| {
                let budget_i64: Option<i64> = row.get(4)?;
                Ok(Goal {
                    thread_id: row.get(0)?,
                    goal_id: row.get(1)?,
                    objective: row.get(2)?,
                    status: row.get(3)?,
                    token_budget: budget_i64.map(|b| b as u64),
                    tokens_used: row.get::<_, i64>(5)? as u64,
                    time_used_seconds: row.get::<_, i64>(6)? as u64,
                    created_at_ms: row.get(7)?,
                })
            })
            .optional()?;

        Ok(goal)
    }

    pub fn update_goal_status(&self, thread_id: &str, status: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE thread_goals SET status = ?2 WHERE thread_id = ?1",
            params![thread_id, status],
        )?;
        Ok(())
    }

    pub fn update_goal_tokens(
        &self,
        thread_id: &str,
        delta_tokens: u64,
        delta_secs: u64,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            UPDATE thread_goals
            SET tokens_used = tokens_used + ?2,
                time_used_seconds = time_used_seconds + ?3
            WHERE thread_id = ?1
            "#,
            params![thread_id, delta_tokens, delta_secs],
        )?;
        Ok(())
    }

    pub fn clear_goal(&self, thread_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM thread_goals WHERE thread_id = ?1",
            params![thread_id],
        )?;
        Ok(())
    }

    pub fn create_execution_claim(
        &self,
        thread_id: &str,
        turn_index: u32,
        pid: u32,
        prompt_preview: Option<&str>,
    ) -> Result<ExecutionClaim> {
        let conn = self.conn.lock().unwrap();
        let claim_id = Uuid::new_v4().to_string();
        let now_ms = chrono::Utc::now().timestamp_millis();

        conn.execute(
            r#"
            INSERT INTO thread_execution_claims (
                claim_id, thread_id, turn_index, status, pid, prompt_preview, started_at_ms, updated_at_ms
            ) VALUES (?1, ?2, ?3, 'running', ?4, ?5, ?6, ?6)
            "#,
            params![claim_id, thread_id, turn_index, pid, prompt_preview, now_ms],
        )?;

        Ok(ExecutionClaim {
            claim_id,
            thread_id: thread_id.to_string(),
            turn_index,
            status: "running".to_string(),
            pid,
            prompt_preview: prompt_preview.map(|s| s.to_string()),
            started_at_ms: now_ms,
            updated_at_ms: now_ms,
        })
    }

    pub fn update_execution_claim_status(&self, claim_id: &str, status: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now_ms = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "UPDATE thread_execution_claims SET status = ?2, updated_at_ms = ?3 WHERE claim_id = ?1",
            params![claim_id, status, now_ms],
        )?;
        Ok(())
    }

    /// Heartbeats an active execution claim by refreshing its updated_at_ms timestamp.
    pub fn heartbeat_claim(&self, claim_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now_ms = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "UPDATE thread_execution_claims SET updated_at_ms = ?2 WHERE claim_id = ?1 AND status IN ('claimed', 'running')",
            params![claim_id, now_ms],
        )?;
        Ok(())
    }

    /// Atomically finalizes both a turn and its associated execution claim in a single SQLite transaction.
    pub fn finalize_turn_and_claim(
        &self,
        thread_id: &str,
        turn_id: &str,
        turn_index: usize,
        claim_id: &str,
        terminal_status: &str,
    ) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let now_ms = chrono::Utc::now().timestamp_millis();

        // 1. Update or insert turn record
        let now_sec = chrono::Utc::now().timestamp();
        tx.execute(
            r#"
            INSERT INTO thread_turns (id, thread_id, turn_index, status, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status
            "#,
            params![
                turn_id,
                thread_id,
                turn_index as i64,
                terminal_status,
                now_sec
            ],
        )?;

        // 2. Finalize execution claim
        let claim_status = match terminal_status {
            "completed" => "completed",
            "aborted" | "cancelled" => "aborted",
            _ => "crashed",
        };
        tx.execute(
            "UPDATE thread_execution_claims SET status = ?2, updated_at_ms = ?3 WHERE claim_id = ?1",
            params![claim_id, claim_status, now_ms],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn get_active_claim(&self, thread_id: &str) -> Result<Option<ExecutionClaim>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT claim_id, thread_id, turn_index, status, pid, prompt_preview, started_at_ms, updated_at_ms
            FROM thread_execution_claims
            WHERE thread_id = ?1 AND status IN ('claimed', 'running')
            ORDER BY started_at_ms DESC
            LIMIT 1
            "#,
        )?;

        let claim = stmt
            .query_row(params![thread_id], |row| {
                Ok(ExecutionClaim {
                    claim_id: row.get(0)?,
                    thread_id: row.get(1)?,
                    turn_index: row.get(2)?,
                    status: row.get(3)?,
                    pid: row.get(4)?,
                    prompt_preview: row.get(5)?,
                    started_at_ms: row.get(6)?,
                    updated_at_ms: row.get(7)?,
                })
            })
            .optional()?;

        Ok(claim)
    }

    pub fn recover_crashed_claims(&self) -> Result<Vec<ExecutionClaim>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT claim_id, thread_id, turn_index, status, pid, prompt_preview, started_at_ms, updated_at_ms
            FROM thread_execution_claims
            WHERE status IN ('claimed', 'running')
            "#,
        )?;

        let claims: Vec<ExecutionClaim> = stmt
            .query_map([], |row| {
                Ok(ExecutionClaim {
                    claim_id: row.get(0)?,
                    thread_id: row.get(1)?,
                    turn_index: row.get(2)?,
                    status: row.get(3)?,
                    pid: row.get(4)?,
                    prompt_preview: row.get(5)?,
                    started_at_ms: row.get(6)?,
                    updated_at_ms: row.get(7)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        drop(stmt);

        let mut recovered = Vec::new();
        let my_pid = std::process::id();
        let now_ms = chrono::Utc::now().timestamp_millis();

        for claim in claims {
            let pid_alive = if claim.pid == my_pid {
                true
            } else {
                #[cfg(target_os = "linux")]
                {
                    std::path::Path::new(&format!("/proc/{}", claim.pid)).exists()
                }
                #[cfg(not(target_os = "linux"))]
                {
                    false
                }
            };

            // An active process with a living PID is never crashed, regardless of elapsed time.
            // Only claims with dead/nonexistent PIDs are marked as crashed.
            if !pid_alive {
                conn.execute(
                    "UPDATE thread_execution_claims SET status = 'crashed', updated_at_ms = ?2 WHERE claim_id = ?1",
                    params![claim.claim_id, now_ms],
                )?;
                let mut c = claim;
                c.status = "crashed".to_string();
                c.updated_at_ms = now_ms;
                recovered.push(c);
            }
        }

        Ok(recovered)
    }

    pub fn enqueue_inbox(&self, thread_id: &str, kind: &str, payload: &str) -> Result<InboxItem> {
        let conn = self.conn.lock().unwrap();
        let id = Uuid::new_v4().to_string();
        let now_ms = chrono::Utc::now().timestamp_millis();

        conn.execute(
            r#"
            INSERT INTO session_inbox (id, thread_id, kind, payload, status, created_at_ms)
            VALUES (?1, ?2, ?3, ?4, 'queued', ?5)
            "#,
            params![id, thread_id, kind, payload, now_ms],
        )?;

        Ok(InboxItem {
            id,
            thread_id: thread_id.to_string(),
            kind: kind.to_string(),
            payload: payload.to_string(),
            status: "queued".to_string(),
            created_at_ms: now_ms,
        })
    }

    pub fn dequeue_inbox(&self, thread_id: &str) -> Result<Option<InboxItem>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, thread_id, kind, payload, status, created_at_ms
            FROM session_inbox
            WHERE thread_id = ?1 AND status = 'queued'
            ORDER BY created_at_ms ASC
            LIMIT 1
            "#,
        )?;

        let item = stmt
            .query_row(params![thread_id], |row| {
                Ok(InboxItem {
                    id: row.get(0)?,
                    thread_id: row.get(1)?,
                    kind: row.get(2)?,
                    payload: row.get(3)?,
                    status: row.get(4)?,
                    created_at_ms: row.get(5)?,
                })
            })
            .optional()?;

        drop(stmt);

        if let Some(ref it) = item {
            conn.execute(
                "UPDATE session_inbox SET status = 'active' WHERE id = ?1",
                params![it.id],
            )?;
        }

        Ok(item)
    }

    pub fn list_inbox(&self, thread_id: &str) -> Result<Vec<InboxItem>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, thread_id, kind, payload, status, created_at_ms
            FROM session_inbox
            WHERE thread_id = ?1
            ORDER BY created_at_ms ASC
            "#,
        )?;

        let items: Result<Vec<InboxItem>, _> = stmt
            .query_map(params![thread_id], |row| {
                Ok(InboxItem {
                    id: row.get(0)?,
                    thread_id: row.get(1)?,
                    kind: row.get(2)?,
                    payload: row.get(3)?,
                    status: row.get(4)?,
                    created_at_ms: row.get(5)?,
                })
            })?
            .collect();

        Ok(items?)
    }

    pub fn lock_path(
        &self,
        thread_id: &str,
        path: &str,
        is_dir: bool,
        locked_by: &str,
    ) -> Result<PathLockRecord> {
        let conn = self.conn.lock().unwrap();
        let id = Uuid::new_v4().to_string();
        let now_ms = chrono::Utc::now().timestamp_millis();
        let norm_path = path
            .trim()
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_string();

        conn.execute(
            r#"
            INSERT OR REPLACE INTO path_locks (id, thread_id, path, is_dir, locked_by, created_at_epoch_ms)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![id, thread_id, norm_path, if is_dir { 1 } else { 0 }, locked_by, now_ms],
        )?;

        Ok(PathLockRecord {
            id,
            thread_id: thread_id.to_string(),
            path: norm_path,
            is_dir,
            locked_by: locked_by.to_string(),
            created_at_epoch_ms: now_ms,
        })
    }

    pub fn unlock_path(&self, thread_id: &str, path: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let norm_path = path
            .trim()
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_string();
        let rows = conn.execute(
            "DELETE FROM path_locks WHERE (thread_id = ?1 OR thread_id = '') AND (path = ?2 OR path = ?3)",
            params![thread_id, path, norm_path],
        )?;
        Ok(rows > 0)
    }

    pub fn list_locks(&self, thread_id: &str) -> Result<Vec<PathLockRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, thread_id, path, is_dir, locked_by, created_at_epoch_ms
            FROM path_locks
            WHERE thread_id = ?1 OR thread_id = ''
            ORDER BY created_at_epoch_ms ASC
            "#,
        )?;

        let records: Result<Vec<PathLockRecord>, _> = stmt
            .query_map(params![thread_id], |row| {
                Ok(PathLockRecord {
                    id: row.get(0)?,
                    thread_id: row.get(1)?,
                    path: row.get(2)?,
                    is_dir: row.get::<_, i64>(3)? != 0,
                    locked_by: row.get(4)?,
                    created_at_epoch_ms: row.get(5)?,
                })
            })?
            .collect();

        records.map_err(Into::into)
    }

    pub fn is_path_locked(
        &self,
        thread_id: &str,
        query_path: &str,
    ) -> Result<Option<PathLockRecord>> {
        let locks = self.list_locks(thread_id)?;
        let norm_query = query_path
            .trim()
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_string();

        for lock in locks {
            let norm_lock = lock
                .path
                .trim()
                .replace('\\', "/")
                .trim_end_matches('/')
                .to_string();
            if norm_query == norm_lock {
                return Ok(Some(lock));
            }
            if lock.is_dir && norm_query.starts_with(&format!("{norm_lock}/")) {
                return Ok(Some(lock));
            }
        }
        Ok(None)
    }

    pub fn post_to_board(
        &self,
        board: &str,
        channel: &str,
        author: &str,
        body: &str,
        payload: serde_json::Value,
    ) -> Result<String> {
        let conn = self.conn.lock().unwrap();
        let id = Uuid::new_v4().to_string();
        let req_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();
        let payload_str = payload.to_string();

        conn.execute(
            r#"
            INSERT INTO posts (
                board, id, channel, root, author, timestamp, body_search, payload, request_id, request
            ) VALUES (?1, ?2, ?3, ?2, ?4, ?5, ?6, ?7, ?8, '')
            "#,
            params![board, id, channel, author, now, body, payload_str, req_id],
        )?;

        Ok(id)
    }

    pub fn list_posts(&self, board: &str, channel: &str, limit: usize) -> Result<Vec<BoardPost>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT seq, board, id, channel, root, author, timestamp, body_search, payload
            FROM posts
            WHERE board = ?1 AND channel = ?2
            ORDER BY seq ASC
            LIMIT ?3
            "#,
        )?;

        let rows = stmt.query_map(params![board, channel, limit as i64], |row| {
            let payload_str: String = row.get(8)?;
            let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap_or_default();
            Ok(BoardPost {
                seq: row.get(0)?,
                board: row.get(1)?,
                id: row.get(2)?,
                channel: row.get(3)?,
                root: row.get(4)?,
                author: row.get(5)?,
                timestamp: row.get(6)?,
                body: row.get(7)?,
                payload,
            })
        })?;

        let mut posts = Vec::new();
        for r in rows {
            posts.push(r?);
        }
        Ok(posts)
    }

    pub fn create_channel(&self, name: &str, topic: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT OR REPLACE INTO channels (name, topic, created_at) VALUES (?1, ?2, ?3)",
            params![name, topic, now],
        )?;
        Ok(())
    }

    pub fn list_channels(&self) -> Result<Vec<ChannelRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT c.name, c.topic, c.created_at, COUNT(p.id)
            FROM channels c
            LEFT JOIN posts p ON p.channel = c.name
            GROUP BY c.name
            ORDER BY c.created_at ASC
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            let count: i64 = row.get(3)?;
            Ok(ChannelRecord {
                name: row.get(0)?,
                topic: row.get(1)?,
                created_at: row.get(2)?,
                post_count: count as usize,
            })
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn post_message(
        &self,
        board: &str,
        channel: &str,
        author: &str,
        body: &str,
        reply_to_id: Option<&str>,
        payload: serde_json::Value,
    ) -> Result<String> {
        let conn = self.conn.lock().unwrap();
        let id = Uuid::new_v4().to_string();
        let req_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();
        let payload_str = payload.to_string();

        // Determine thread root: if replying to an existing post, find its root. Otherwise this post is root.
        let root = if let Some(parent_id) = reply_to_id {
            let mut stmt = conn.prepare("SELECT root FROM posts WHERE id = ?1 LIMIT 1")?;
            let root_res: Option<String> = stmt
                .query_row(params![parent_id], |row| row.get(0))
                .optional()?;
            root_res.unwrap_or_else(|| parent_id.to_string())
        } else {
            id.clone()
        };

        conn.execute(
            r#"
            INSERT INTO posts (
                board, id, channel, root, author, timestamp, body_search, payload, request_id, request
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '')
            "#,
            params![board, id, channel, root, author, now, body, payload_str, req_id],
        )?;

        // Ensure channel exists
        let _ = conn.execute(
            "INSERT OR IGNORE INTO channels (name, topic, created_at) VALUES (?1, '', ?2)",
            params![channel, chrono::Utc::now().timestamp()],
        );

        Ok(id)
    }

    pub fn list_board_threads(
        &self,
        board: &str,
        channel: &str,
        limit: usize,
    ) -> Result<Vec<ThreadSummaryRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT p.root, p.channel, p.author, p.body_search,
                   (SELECT COUNT(*) - 1 FROM posts WHERE root = p.root),
                   MAX(posts_all.timestamp)
            FROM posts p
            JOIN posts posts_all ON posts_all.root = p.root
            WHERE p.board = ?1 AND p.channel = ?2 AND p.id = p.root
            GROUP BY p.root
            ORDER BY MAX(posts_all.timestamp) DESC
            LIMIT ?3
            "#,
        )?;
        let rows = stmt.query_map(params![board, channel, limit as i64], |row| {
            let count: i64 = row.get(4)?;
            Ok(ThreadSummaryRecord {
                root_id: row.get(0)?,
                channel: row.get(1)?,
                author: row.get(2)?,
                root_body: row.get(3)?,
                reply_count: count.max(0) as usize,
                last_activity_timestamp: row.get(5)?,
            })
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn read_thread(&self, board: &str, root_id: &str) -> Result<Vec<BoardPost>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT seq, board, id, channel, root, author, timestamp, body_search, payload
            FROM posts
            WHERE board = ?1 AND root = ?2
            ORDER BY seq ASC
            "#,
        )?;
        let rows = stmt.query_map(params![board, root_id], |row| {
            let payload_str: String = row.get(8)?;
            let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap_or_default();
            Ok(BoardPost {
                seq: row.get(0)?,
                board: row.get(1)?,
                id: row.get(2)?,
                channel: row.get(3)?,
                root: row.get(4)?,
                author: row.get(5)?,
                timestamp: row.get(6)?,
                body: row.get(7)?,
                payload,
            })
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn search_posts(&self, board: &str, query: &str, limit: usize) -> Result<Vec<BoardPost>> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("%{}%", query);
        let mut stmt = conn.prepare(
            r#"
            SELECT seq, board, id, channel, root, author, timestamp, body_search, payload
            FROM posts
            WHERE board = ?1 AND body_search LIKE ?2
            ORDER BY timestamp DESC
            LIMIT ?3
            "#,
        )?;
        let rows = stmt.query_map(params![board, pattern, limit as i64], |row| {
            let payload_str: String = row.get(8)?;
            let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap_or_default();
            Ok(BoardPost {
                seq: row.get(0)?,
                board: row.get(1)?,
                id: row.get(2)?,
                channel: row.get(3)?,
                root: row.get(4)?,
                author: row.get(5)?,
                timestamp: row.get(6)?,
                body: row.get(7)?,
                payload,
            })
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn subscribe(&self, board: &str, channel: &str, subscriber: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT OR REPLACE INTO subscriptions (board, channel, subscriber, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![board, channel, subscriber, now],
        )?;
        Ok(())
    }

    pub fn unsubscribe(&self, board: &str, channel: &str, subscriber: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM subscriptions WHERE board = ?1 AND channel = ?2 AND subscriber = ?3",
            params![board, channel, subscriber],
        )?;
        Ok(())
    }

    pub fn list_subscriptions(&self, subscriber: &str) -> Result<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT board, channel FROM subscriptions WHERE subscriber = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![subscriber], |row| Ok((row.get(0)?, row.get(1)?)))?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    // =========================================================================
    // Evidence Ledger Persistence
    // =========================================================================

    pub fn store_evidence_record(&self, record: &StoredEvidenceRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT OR REPLACE INTO evidence_records (
                id, requirement_id, evidence_type, description, verified, turn_index, timestamp_epoch_ms, details_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                record.id,
                record.requirement_id,
                record.evidence_type,
                record.description,
                if record.verified { 1 } else { 0 },
                record.turn_index,
                record.timestamp_epoch_ms,
                record.details_json,
            ],
        )?;
        Ok(())
    }

    pub fn query_evidence_by_requirement(
        &self,
        requirement_id: &str,
    ) -> Result<Vec<StoredEvidenceRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, requirement_id, evidence_type, description, verified, turn_index, timestamp_epoch_ms, details_json
            FROM evidence_records
            WHERE requirement_id = ?1
            ORDER BY timestamp_epoch_ms ASC
            "#,
        )?;
        let rows = stmt.query_map(params![requirement_id], |row| {
            let verified_int: i32 = row.get(4)?;
            Ok(StoredEvidenceRecord {
                id: row.get(0)?,
                requirement_id: row.get(1)?,
                evidence_type: row.get(2)?,
                description: row.get(3)?,
                verified: verified_int != 0,
                turn_index: row.get(5)?,
                timestamp_epoch_ms: row.get(6)?,
                details_json: row.get(7)?,
            })
        })?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    // =========================================================================
    // Session Memory Persistence
    // =========================================================================

    pub fn store_memory(&self, memory: &StoredSessionMemory) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT OR REPLACE INTO session_memories (
                id, topic, detail, citation_count, last_used_at_epoch_ms, created_at_epoch_ms
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                memory.id,
                memory.topic,
                memory.detail,
                memory.citation_count,
                memory.last_used_at_epoch_ms,
                memory.created_at_epoch_ms,
            ],
        )?;
        Ok(())
    }

    pub fn query_memories(&self, query: &str) -> Result<Vec<StoredSessionMemory>> {
        let conn = self.conn.lock().unwrap();
        let q = query.trim();
        let mut stmt = if q.is_empty() {
            conn.prepare(
                r#"
                SELECT id, topic, detail, citation_count, last_used_at_epoch_ms, created_at_epoch_ms
                FROM session_memories
                ORDER BY citation_count DESC, created_at_epoch_ms DESC
                "#,
            )?
        } else {
            conn.prepare(
                r#"
                SELECT id, topic, detail, citation_count, last_used_at_epoch_ms, created_at_epoch_ms
                FROM session_memories
                WHERE topic LIKE ?1 OR detail LIKE ?1
                ORDER BY citation_count DESC, created_at_epoch_ms DESC
                "#,
            )?
        };

        let map_row = |row: &rusqlite::Row| {
            Ok(StoredSessionMemory {
                id: row.get(0)?,
                topic: row.get(1)?,
                detail: row.get(2)?,
                citation_count: row.get(3)?,
                last_used_at_epoch_ms: row.get(4)?,
                created_at_epoch_ms: row.get(5)?,
            })
        };

        let rows = if q.is_empty() {
            stmt.query_map([], map_row)?
        } else {
            let like_pattern = format!("%{}%", q);
            stmt.query_map(params![like_pattern], map_row)?
        };

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn get_memory_by_id(&self, id: &str) -> Result<Option<StoredSessionMemory>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, topic, detail, citation_count, last_used_at_epoch_ms, created_at_epoch_ms
            FROM session_memories
            WHERE id = ?1
            "#,
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(StoredSessionMemory {
                id: row.get(0)?,
                topic: row.get(1)?,
                detail: row.get(2)?,
                citation_count: row.get(3)?,
                last_used_at_epoch_ms: row.get(4)?,
                created_at_epoch_ms: row.get(5)?,
            })
        })?;
        if let Some(res) = rows.next() {
            Ok(Some(res?))
        } else {
            Ok(None)
        }
    }

    pub fn increment_memory_citation(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        let affected = conn.execute(
            r#"
            UPDATE session_memories
            SET citation_count = citation_count + 1, last_used_at_epoch_ms = ?1
            WHERE id = ?2
            "#,
            params![now, id],
        )?;
        Ok(affected > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_lifecycle() -> Result<()> {
        let storage = Storage::open_in_memory()?;

        // Path lock hierarchical checks
        let lock = storage.lock_path("t1", "src/auth", true, "developer")?;
        assert_eq!(lock.path, "src/auth");
        assert!(lock.is_dir);

        // Child path must be detected as locked
        let child_locked = storage.is_path_locked("t1", "src/auth/jwt.rs")?;
        assert!(child_locked.is_some());
        assert_eq!(child_locked.unwrap().locked_by, "developer");

        // Unrelated path is not locked
        let other_locked = storage.is_path_locked("t1", "src/utils.rs")?;
        assert!(other_locked.is_none());

        // Unlock
        let unlocked = storage.unlock_path("t1", "src/auth")?;
        assert!(unlocked);
        assert!(storage.is_path_locked("t1", "src/auth/jwt.rs")?.is_none());

        // Post to board
        let post_id = storage.post_to_board(
            "default",
            "dev",
            "agent_1",
            "Test message",
            serde_json::json!({"status": "ok"}),
        )?;
        assert!(!post_id.is_empty());

        let posts = storage.list_posts("default", "dev", 10)?;
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].body, "Test message");
        assert_eq!(posts[0].author, "agent_1");

        // Thread and Fork
        let meta = SessionMetadata {
            id: "root-thread-1".to_string(),
            title: "Root Session".to_string(),
            model: "meta/llama-3.3-70b-instruct".to_string(),
            cwd: "/tmp".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            extra: std::collections::HashMap::new(),
        };
        storage.save_thread(&meta, "/tmp/rollout", "workspace-write", "on-request")?;

        let forked = storage.fork_thread("root-thread-1", "Forked Branch")?;
        assert_eq!(forked.parent_thread_id, Some("root-thread-1".to_string()));
        assert_eq!(forked.title, "Forked Branch");

        // Goals
        let goal = storage.set_goal(&meta.id, "Build M31A Codex parity", Some(50000))?;
        assert_eq!(goal.objective, "Build M31A Codex parity");
        assert_eq!(goal.token_budget, Some(50000));

        let retrieved_goal = storage.get_goal(&meta.id)?.unwrap();
        assert_eq!(retrieved_goal.status, "active");

        // Save turns and items to root thread
        storage.save_turn("turn-1", "root-thread-1", 1, "completed")?;
        storage.save_item(
            "item-1",
            "root-thread-1",
            Some("turn-1"),
            "user_message",
            "Hello",
        )?;
        storage.save_item(
            "item-2",
            "root-thread-1",
            Some("turn-1"),
            "agent_response",
            "Hi there",
        )?;

        storage.save_turn("turn-2", "root-thread-1", 2, "completed")?;
        storage.save_item(
            "item-3",
            "root-thread-1",
            Some("turn-2"),
            "tool_call",
            "exec_command",
        )?;

        // Fork thread up to turn 1
        let forked_t1 = storage.fork_thread_at("root-thread-1", "Forked At Turn 1", Some(1))?;
        assert_eq!(
            forked_t1.parent_thread_id,
            Some("root-thread-1".to_string())
        );
        let t1_items = storage.list_items(&forked_t1.id)?;
        assert_eq!(t1_items.len(), 2);

        // Search threads
        let search_res = storage.search_threads("Root", 10)?;
        assert_eq!(search_res.len(), 1);
        assert_eq!(search_res[0].id, "root-thread-1");

        // Thread stats
        let stats = storage.get_thread_stats("root-thread-1")?;
        assert_eq!(stats.total_turns, 2);
        assert_eq!(stats.total_items, 3);
        assert_eq!(stats.user_messages, 1);
        assert_eq!(stats.assistant_messages, 1);
        assert_eq!(stats.tool_calls, 1);

        // Agent Graph Store tests
        storage.upsert_spawn_edge("root-agent", "child-1", "architect", "open", Some(1))?;
        storage.upsert_spawn_edge("root-agent", "child-2", "coder", "open", Some(1))?;
        storage.upsert_spawn_edge("child-2", "grandchild-1", "reviewer", "open", Some(2))?;

        let root_children = storage.list_spawn_children("root-agent", None)?;
        assert_eq!(root_children.len(), 2);
        assert_eq!(root_children[0].child_thread_id, "child-1");
        assert_eq!(root_children[0].agent_role, "architect");

        let descendants = storage.list_spawn_descendants("root-agent", Some("open"))?;
        assert_eq!(descendants.len(), 3);
        assert_eq!(descendants[0].child_thread_id, "child-1");
        assert_eq!(descendants[1].child_thread_id, "child-2");
        assert_eq!(descendants[2].child_thread_id, "grandchild-1");

        // Terminate child-2 tree (should terminate child-2 and grandchild-1)
        let term_count = storage.terminate_agent_tree("child-2")?;
        assert_eq!(term_count, 2);

        let open_descendants = storage.list_spawn_descendants("root-agent", Some("open"))?;
        assert_eq!(open_descendants.len(), 1);
        assert_eq!(open_descendants[0].child_thread_id, "child-1");

        // Message Board tests
        storage.create_channel("arch", "Architecture discussion")?;
        let channels = storage.list_channels()?;
        assert!(
            channels
                .iter()
                .any(|c| c.name == "arch" && c.topic == "Architecture discussion")
        );

        let root_post = storage.post_message(
            "default",
            "arch",
            "agent_lead",
            "Master plan RFC",
            None,
            serde_json::json!({}),
        )?;
        let reply_post = storage.post_message(
            "default",
            "arch",
            "agent_coder",
            "Looks good, I will implement phase 1",
            Some(&root_post),
            serde_json::json!({}),
        )?;

        let threads = storage.list_board_threads("default", "arch", 10)?;
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].root_id, root_post);
        assert_eq!(threads[0].reply_count, 1);

        let thread_messages = storage.read_thread("default", &root_post)?;
        assert_eq!(thread_messages.len(), 2);
        assert_eq!(thread_messages[0].id, root_post);
        assert_eq!(thread_messages[1].id, reply_post);

        let search_hits = storage.search_posts("default", "RFC", 10)?;
        assert_eq!(search_hits.len(), 1);
        assert_eq!(search_hits[0].id, root_post);

        storage.subscribe("default", "arch", "agent_lead")?;
        let subs = storage.list_subscriptions("agent_lead")?;
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0], ("default".to_string(), "arch".to_string()));

        storage.unsubscribe("default", "arch", "agent_lead")?;
        let subs_after = storage.list_subscriptions("agent_lead")?;
        assert_eq!(subs_after.len(), 0);

        Ok(())
    }

    #[test]
    fn test_thread_sections_lifecycle() -> Result<()> {
        let storage = Storage::open_in_memory()?;

        // 1. Create sections
        let sec1 =
            storage.create_thread_section("Active Projects", Some("#ff0000"), Some("folder"))?;
        let _sec2 = storage.create_thread_section("Archived Work", None, None)?;

        let sections = storage.list_thread_sections()?;
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].name, "Active Projects");

        // 2. Rename section
        let renamed = storage.rename_thread_section(
            &sec1.id,
            "Urgent Projects",
            Some("#00ff00"),
            Some("fire"),
        )?;
        assert!(renamed);

        let sections = storage.list_thread_sections()?;
        assert_eq!(sections[0].name, "Urgent Projects");

        // 3. Move thread to section
        let meta = SessionMetadata {
            id: "thread-abc".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            model: "test-model".to_string(),
            cwd: "/tmp".to_string(),
            title: "Test Thread".to_string(),
            extra: std::collections::HashMap::new(),
        };
        storage.save_thread(&meta, "/tmp/rollout.jsonl", "workspace-write", "on-request")?;

        let moved = storage.move_thread_to_section("thread-abc", Some(&sec1.id))?;
        assert!(moved);

        let in_sec = storage.list_threads_by_section(Some(&sec1.id), 10)?;
        assert_eq!(in_sec.len(), 1);
        assert_eq!(in_sec[0].id, "thread-abc");
        assert_eq!(in_sec[0].section_id, Some(sec1.id.clone()));

        // 4. Delete section
        let deleted = storage.delete_thread_section(&sec1.id)?;
        assert!(deleted);

        let sections = storage.list_thread_sections()?;
        assert_eq!(sections.len(), 1);

        // Thread section_id should be cleared to None
        let thread = storage.get_thread("thread-abc")?.unwrap();
        assert_eq!(thread.section_id, None);

        Ok(())
    }

    #[test]
    fn test_revert_thread_and_lineage() -> Result<()> {
        let storage = Storage::open_in_memory()?;

        // Create base thread
        let meta = SessionMetadata {
            id: "root-thread".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            model: "test-model".to_string(),
            cwd: "/tmp".to_string(),
            title: "Root Thread".to_string(),
            extra: std::collections::HashMap::new(),
        };
        storage.save_thread(&meta, "/tmp/rollout.jsonl", "workspace-write", "on-request")?;

        // Add 3 turns
        storage.save_turn("turn_1", "root-thread", 1, "completed")?;
        storage.save_turn("turn_2", "root-thread", 2, "completed")?;
        storage.save_turn("turn_3", "root-thread", 3, "completed")?;

        let turns = storage.list_turns("root-thread")?;
        assert_eq!(turns.len(), 3);

        // Revert to turn 1 (should delete turn 2 and 3)
        let deleted_count = storage.revert_thread("root-thread", 1)?;
        assert_eq!(deleted_count, 2);

        let turns_after = storage.list_turns("root-thread")?;
        assert_eq!(turns_after.len(), 1);
        assert_eq!(turns_after[0].turn_index, 1);

        // Fork thread
        let forked = storage.fork_thread("root-thread", "Child Thread")?;
        let grandchild = storage.fork_thread(&forked.id, "Grandchild Thread")?;

        // Test lineage
        let lineage = storage.thread_lineage(&grandchild.id)?;
        assert_eq!(lineage.len(), 3);
        assert_eq!(lineage[0].id, grandchild.id);
        assert_eq!(lineage[1].id, forked.id);
        assert_eq!(lineage[2].id, "root-thread");

        Ok(())
    }

    #[test]
    fn test_checkpoint_snapshots_and_revert() -> Result<()> {
        let storage = Storage::open_in_memory()?;

        let meta = SessionMetadata {
            id: "checkpoint-thread".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            model: "test-model".to_string(),
            cwd: "/tmp".to_string(),
            title: "Checkpoint Thread".to_string(),
            extra: std::collections::HashMap::new(),
        };
        storage.save_thread(&meta, "/tmp/rollout.jsonl", "workspace-write", "on-request")?;

        // Add 3 turns
        storage.save_turn("turn_1", "checkpoint-thread", 1, "completed")?;
        storage.save_turn("turn_2", "checkpoint-thread", 2, "completed")?;

        // Create checkpoint 1
        let cp1 = storage.create_checkpoint(
            "checkpoint-thread",
            "Initial setup",
            Some("commit-sha-1234"),
            2,
            &["src/main.rs".to_string(), "Cargo.toml".to_string()],
            Some(&serde_json::json!({"step": "init"})),
        )?;

        assert_eq!(cp1.turn_index, 2);
        assert_eq!(cp1.modified_files.len(), 2);
        assert_eq!(cp1.git_commit.as_deref(), Some("commit-sha-1234"));

        // Add turn 3 and checkpoint 2
        storage.save_turn("turn_3", "checkpoint-thread", 3, "completed")?;
        let cp2 = storage.create_checkpoint(
            "checkpoint-thread",
            "Added features",
            Some("commit-sha-5678"),
            3,
            &["src/lib.rs".to_string()],
            None,
        )?;

        // List checkpoints
        let list = storage.list_checkpoints("checkpoint-thread")?;
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, cp2.id); // newest first
        assert_eq!(list[1].id, cp1.id);

        // Restore checkpoint 1 with conversation revert
        let restored = storage.restore_checkpoint(&cp1.id, true)?;
        assert_eq!(restored.id, cp1.id);
        assert_eq!(restored.turn_index, 2);

        // Verify turns after revert: turn 3 should be gone
        let turns = storage.list_turns("checkpoint-thread")?;
        assert_eq!(turns.len(), 2);
        assert_eq!(turns.last().unwrap().turn_index, 2);

        // Delete checkpoint
        let deleted = storage.delete_checkpoint(&cp2.id)?;
        assert!(deleted);
        let list_after = storage.list_checkpoints("checkpoint-thread")?;
        assert_eq!(list_after.len(), 1);

        Ok(())
    }

    #[test]
    fn test_save_item_redacts_secrets_at_rollout_boundary() -> Result<()> {
        let storage = Storage::open_in_memory()?;

        let meta = SessionMetadata {
            id: "redact-thread".to_string(),
            title: "Redaction Test".to_string(),
            model: "meta/llama-3.3-70b-instruct".to_string(),
            cwd: "/tmp".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            extra: std::collections::HashMap::new(),
        };
        storage.save_thread(&meta, "", "workspace-write", "on-request")?;

        storage.save_item(
            "item-secret-1",
            "redact-thread",
            None,
            "user",
            r#"my key is sk-abcdefghijklmnopqrstuvwx and api_key = "nvapi-AbCdEfGhIjKlMnOpQrStUvWx""#,
        )?;
        storage.save_item(
            "item-clean-1",
            "redact-thread",
            None,
            "assistant",
            "Plain text output without credentials.",
        )?;

        let items = storage.list_items("redact-thread")?;
        assert_eq!(items.len(), 2);

        let user_item = items
            .iter()
            .find(|(_, content, _)| content.contains("my key is"))
            .unwrap();
        assert!(
            !user_item.1.contains("sk-abcdefghijklmnopqrstuvwx"),
            "raw secret must not be persisted"
        );
        assert!(
            user_item.1.contains("[REDACTED_SECRET]"),
            "persisted item must carry the redaction marker"
        );

        // The bearer-style assignment value is redacted too.
        assert!(!user_item.1.contains("nvapi-AbCdEfGhIjKlMnOpQrStUvWx"));
        assert!(user_item.1.contains("api_key = \"[REDACTED_SECRET]\""));

        // Clean content round-trips unchanged.
        let assistant_item = items
            .iter()
            .find(|(_, content, _)| content.contains("Plain text"))
            .unwrap();
        assert_eq!(assistant_item.1, "Plain text output without credentials.");

        Ok(())
    }

    /// §5.5: retained facts — per-origin ordinal scoping and kind filtering.
    #[test]
    fn test_retained_facts_ordinals_scoped_per_origin() -> Result<()> {
        let storage = Storage::open_in_memory()?;

        // Same (thread, kind), two origins — each origin's ordinals start at 0.
        let f1 = storage.save_retained_fact("t1", "verified_qa", "engine", "Q1→A1", None)?;
        let f2 = storage.save_retained_fact("t1", "verified_qa", "engine", "Q2→A2", None)?;
        let g1 = storage.save_retained_fact("t1", "verified_qa", "cli", "Q1→A1-cli", None)?;

        assert_eq!(f1.ordinal, 0);
        assert_eq!(f2.ordinal, 1);
        assert_eq!(g1.ordinal, 0, "ordinals are scoped per origin");

        let facts = storage.list_retained_facts("t1", "verified_qa")?;
        assert_eq!(facts.len(), 3);
        // Ordering: origin ASC, then ordinal ASC.
        assert_eq!(
            facts
                .iter()
                .map(|f| (f.origin.as_str(), f.ordinal))
                .collect::<Vec<_>>(),
            vec![("cli", 0), ("engine", 0), ("engine", 1)]
        );

        // Kind filtering.
        assert!(
            storage
                .list_retained_facts("t1", "compaction_checkpoint")?
                .is_empty()
        );
        // Thread filtering.
        assert!(
            storage
                .list_retained_facts("other-thread", "verified_qa")?
                .is_empty()
        );

        Ok(())
    }

    /// §5.5: malformed checkpoints (empty model_hash) are RETURNED, not
    /// filtered — visible data beats silently discarded evidence.
    #[test]
    fn test_retained_facts_malformed_checkpoint_returned() -> Result<()> {
        let storage = Storage::open_in_memory()?;

        let good = storage.save_retained_fact(
            "t2",
            "compaction_checkpoint",
            "engine",
            r#"{"window_id":"w-1","turns":4}"#,
            Some("sha256:abc123"),
        )?;
        // Malformed: checkpoint without a model_hash.
        let bad = storage.save_retained_fact(
            "t2",
            "compaction_checkpoint",
            "engine",
            r#"{"window_id":"w-2","turns":2}"#,
            None,
        )?;

        assert!(good.model_hash.is_some());
        assert!(
            bad.model_hash.is_none(),
            "malformed checkpoint preserved as-is"
        );

        let facts = storage.list_retained_facts("t2", "compaction_checkpoint")?;
        assert_eq!(
            facts.len(),
            2,
            "malformed row must be returned, not filtered"
        );
        assert!(facts.iter().any(|f| f.model_hash.is_none()));
        assert!(
            facts
                .iter()
                .any(|f| f.model_hash == Some("sha256:abc123".to_string()))
        );

        Ok(())
    }

    /// §6.1: Storage rollout mirror — save_thread/save_item mirror to JSONL
    /// when enabled; replay round-trips; mirror is off by default.
    #[test]
    fn test_rollout_mirror_roundtrip() -> Result<()> {
        let storage = Storage::open_in_memory()?;
        assert!(storage.rollout_root().is_none(), "mirror is off by default");

        let dir = tempfile::tempdir()?;
        storage.set_rollout_root(dir.path().to_path_buf());
        assert_eq!(storage.rollout_root(), Some(dir.path().to_path_buf()));

        let meta = SessionMetadata {
            id: "mirror-thread".to_string(),
            title: "Mirror".to_string(),
            model: "test-model".to_string(),
            // Point at the workspace so the best-effort git identity is
            // exercisable (tempdirs are not repos).
            cwd: env!("CARGO_MANIFEST_DIR").to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            extra: std::collections::HashMap::new(),
        };
        storage.save_thread(&meta, "/tmp/rollout", "workspace-write", "on-request")?;
        storage.save_item("i-1", "mirror-thread", Some("t-1"), "user_message", "hello")?;
        storage.save_item("i-2", "mirror-thread", Some("t-1"), "assistant", "hi there")?;

        let items = storage.read_thread_rollout("mirror-thread")?;
        assert!(items.len() >= 3, "session_meta + 2 items: {items:?}");
        assert_eq!(items[0].kind, "session_meta");
        assert!(
            matches!(items[0].payload["git"], serde_json::Value::Object(_)),
            "git identity is embedded (best-effort)"
        );
        assert_eq!(items[1].payload["content"], "hello");
        assert_eq!(items[2].payload["content"], "hi there");

        // Ordinals are monotonic across the mirror.
        let ordinals: Vec<u64> = items.iter().map(|i| i.ordinal).collect();
        assert_eq!(ordinals, (0..ordinals.len() as u64).collect::<Vec<_>>());

        // The persisted files use the upstream naming shape.
        let mut found = false;
        for entry in std::fs::read_dir(dir.path())? {
            let name = entry?.file_name().to_string_lossy().to_string();
            if name.starts_with("rollout-") && name.contains("-mirror-thread.jsonl") {
                found = true;
            }
        }
        assert!(found, "rollout file with upstream naming must exist");

        Ok(())
    }

    /// §6.1: the 64 KiB persist truncation applies to mirrored item content.
    #[test]
    fn test_rollout_mirror_truncates_large_content() -> Result<()> {
        let storage = Storage::open_in_memory()?;
        let dir = tempfile::tempdir()?;
        storage.set_rollout_root(dir.path().to_path_buf());

        let meta = SessionMetadata {
            id: "big-thread".to_string(),
            title: "Big".to_string(),
            model: "test-model".to_string(),
            cwd: dir.path().display().to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            extra: std::collections::HashMap::new(),
        };
        storage.save_thread(&meta, "/tmp/rollout", "workspace-write", "on-request")?;

        let big = "x".repeat(64 * 1024 + 4096);
        storage.save_item("i-big", "big-thread", None, "tool_result", &big)?;

        let items = storage.read_thread_rollout("big-thread")?;
        assert_eq!(items.len(), 2, "session_meta + the item");
        assert_eq!(items[0].kind, "session_meta");
        let content = items[1].payload["content"].as_str().unwrap();
        assert!(
            content.contains("[Truncated for persist:"),
            "large content must be truncated at the mirror"
        );
        assert!(content.len() < 64 * 1024 + 200);

        Ok(())
    }

    #[test]
    fn test_claims_and_inbox_lifecycle() -> Result<()> {
        let storage = Storage::open_in_memory()?;
        let meta = SessionMetadata {
            id: "thread-claims-test".to_string(),
            title: "Claims Test".to_string(),
            model: "test-model".to_string(),
            cwd: "/tmp".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            extra: std::collections::HashMap::new(),
        };
        storage.save_thread(&meta, "/tmp/rollout", "workspace-write", "on-request")?;

        // Goal lifecycle
        let goal = storage.set_goal(&meta.id, "Implement crash recovery", Some(10000))?;
        assert_eq!(goal.objective, "Implement crash recovery");
        storage.update_goal_status(&meta.id, "paused")?;
        let retrieved = storage.get_goal(&meta.id)?.unwrap();
        assert_eq!(retrieved.status, "paused");
        storage.clear_goal(&meta.id)?;
        assert!(storage.get_goal(&meta.id)?.is_none());

        // Claims lifecycle
        let claim =
            storage.create_execution_claim(&meta.id, 1, 99999999, Some("Prompt preview"))?;
        assert_eq!(claim.status, "running");
        assert_eq!(claim.turn_index, 1);

        let active = storage.get_active_claim(&meta.id)?.unwrap();
        assert_eq!(active.claim_id, claim.claim_id);

        // PID 99999999 is dead, recover should transition it to crashed
        let recovered = storage.recover_crashed_claims()?;
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].status, "crashed");
        assert!(storage.get_active_claim(&meta.id)?.is_none());

        // Inbox lifecycle
        let item1 = storage.enqueue_inbox(&meta.id, "prompt", "Do step 1")?;
        assert_eq!(item1.status, "queued");
        let item2 = storage.enqueue_inbox(&meta.id, "steer", "Adjust direction")?;
        assert_eq!(item2.status, "queued");

        let dequeued = storage.dequeue_inbox(&meta.id)?.unwrap();
        assert_eq!(dequeued.id, item1.id);
        assert_eq!(dequeued.status, "queued"); // was queued at query time, updated to active

        let list = storage.list_inbox(&meta.id)?;
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].status, "active");
        assert_eq!(list[1].status, "queued");

        Ok(())
    }

    #[test]
    fn test_evidence_records_and_session_memories_persistence() -> Result<()> {
        let storage = Storage::open_in_memory()?;

        // Evidence Records
        let rec1 = StoredEvidenceRecord {
            id: "ev-1".to_string(),
            requirement_id: "REQ-01".to_string(),
            evidence_type: "TestOutput".to_string(),
            description: "cargo test passed 12 tests".to_string(),
            verified: true,
            turn_index: Some(3),
            timestamp_epoch_ms: 1000,
            details_json: Some(r#"{"exit_code": 0}"#.to_string()),
        };
        let rec2 = StoredEvidenceRecord {
            id: "ev-2".to_string(),
            requirement_id: "REQ-01".to_string(),
            evidence_type: "CompilerCheck".to_string(),
            description: "cargo check 0 errors".to_string(),
            verified: true,
            turn_index: Some(4),
            timestamp_epoch_ms: 2000,
            details_json: None,
        };
        storage.store_evidence_record(&rec1)?;
        storage.store_evidence_record(&rec2)?;

        let retrieved_ev = storage.query_evidence_by_requirement("REQ-01")?;
        assert_eq!(retrieved_ev.len(), 2);
        assert_eq!(retrieved_ev[0].id, "ev-1");
        assert_eq!(retrieved_ev[1].id, "ev-2");
        assert!(retrieved_ev[0].verified);

        // Session Memories
        let mem1 = StoredSessionMemory {
            id: "mem-1".to_string(),
            topic: "Rust Edition".to_string(),
            detail: "Always use Edition 2024 for all workspace crates.".to_string(),
            citation_count: 1,
            last_used_at_epoch_ms: None,
            created_at_epoch_ms: 5000,
        };
        let mem2 = StoredSessionMemory {
            id: "mem-2".to_string(),
            topic: "Sandbox Policy".to_string(),
            detail: "Fail closed when bubblewrap is unavailable.".to_string(),
            citation_count: 5,
            last_used_at_epoch_ms: Some(6000),
            created_at_epoch_ms: 4000,
        };
        storage.store_memory(&mem1)?;
        storage.store_memory(&mem2)?;

        let all_mem = storage.query_memories("")?;
        assert_eq!(all_mem.len(), 2);
        assert_eq!(
            all_mem[0].id, "mem-2",
            "mem-2 should come first due to higher citation count"
        );

        let search_hits = storage.query_memories("bubblewrap")?;
        assert_eq!(search_hits.len(), 1);
        assert_eq!(search_hits[0].id, "mem-2");

        let cited = storage.increment_memory_citation("mem-1")?;
        assert!(cited);
        let updated_mem1 = storage.get_memory_by_id("mem-1")?.unwrap();
        assert_eq!(updated_mem1.citation_count, 2);
        assert!(updated_mem1.last_used_at_epoch_ms.is_some());

        Ok(())
    }
}
