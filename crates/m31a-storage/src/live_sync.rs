//! Live Thread Metadata Synchronizer and Debounced Storage Flush.
//!
//! Provides an asynchronous, in-memory buffer for real-time thread items, turns, and
//! metadata updates, debouncing SQLite disk writes during rapid streaming and batch tool runs
//! while guaranteeing durable atomic flushes on demand or upon shutdown.

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{Connection, params};
use std::sync::{Arc, Mutex};

use crate::db::TurnRecord;
use crate::transcript_export::TranscriptItemRecord;

/// Pending thread metadata update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingThreadMetaUpdate {
    pub thread_id: String,
    pub title: String,
    pub updated_at: i64,
}

#[derive(Debug, Default)]
struct SyncBuffer {
    items: Vec<TranscriptItemRecord>,
    turns: Vec<TurnRecord>,
    threads: Vec<PendingThreadMetaUpdate>,
}

/// Thread-safe live metadata synchronizer with debounced flushing.
#[derive(Debug, Clone)]
pub struct LiveSyncManager {
    conn: Arc<Mutex<Connection>>,
    buffer: Arc<Mutex<SyncBuffer>>,
    debounce_threshold: usize,
}

impl LiveSyncManager {
    /// Create a new synchronizer with a configurable debounce item threshold.
    pub fn new(conn: Arc<Mutex<Connection>>, debounce_threshold: usize) -> Self {
        Self {
            conn,
            buffer: Arc::new(Mutex::new(SyncBuffer::default())),
            debounce_threshold,
        }
    }

    /// Record a streaming or completed transcript item into the debounce buffer.
    pub fn record_item(
        &self,
        item_id: &str,
        thread_id: &str,
        turn_id: Option<&str>,
        item_type: &str,
        content: &str,
    ) {
        let mut buf = self.buffer.lock().unwrap();
        buf.items.push(TranscriptItemRecord {
            id: item_id.to_string(),
            thread_id: thread_id.to_string(),
            turn_id: turn_id.map(|s| s.to_string()),
            item_type: item_type.to_string(),
            content: content.to_string(),
            created_at: Utc::now().timestamp(),
        });
    }

    /// Record or update a turn status in the debounce buffer.
    pub fn record_turn(&self, turn_id: &str, thread_id: &str, turn_index: usize, status: &str) {
        let mut buf = self.buffer.lock().unwrap();
        // If turn already in buffer, update it; otherwise append
        if let Some(existing) = buf.turns.iter_mut().find(|t| t.id == turn_id) {
            existing.status = status.to_string();
        } else {
            buf.turns.push(TurnRecord {
                id: turn_id.to_string(),
                thread_id: thread_id.to_string(),
                turn_index,
                status: status.to_string(),
                created_at: Utc::now().timestamp(),
            });
        }
    }

    /// Record a thread title update in the debounce buffer.
    pub fn update_thread_title(&self, thread_id: &str, title: &str) {
        let mut buf = self.buffer.lock().unwrap();
        if let Some(existing) = buf.threads.iter_mut().find(|t| t.thread_id == thread_id) {
            existing.title = title.to_string();
            existing.updated_at = Utc::now().timestamp();
        } else {
            buf.threads.push(PendingThreadMetaUpdate {
                thread_id: thread_id.to_string(),
                title: title.to_string(),
                updated_at: Utc::now().timestamp(),
            });
        }
    }

    /// Returns true if unpersisted updates are pending in the buffer.
    pub fn is_dirty(&self) -> bool {
        let buf = self.buffer.lock().unwrap();
        !buf.items.is_empty() || !buf.turns.is_empty() || !buf.threads.is_empty()
    }

    /// Number of pending mutations in the buffer.
    pub fn pending_count(&self) -> usize {
        let buf = self.buffer.lock().unwrap();
        buf.items.len() + buf.turns.len() + buf.threads.len()
    }

    /// Flush pending items if the buffer size exceeds the debounce threshold.
    pub fn flush_if_threshold(&self) -> Result<Option<usize>> {
        if self.pending_count() >= self.debounce_threshold {
            let flushed = self.flush()?;
            Ok(Some(flushed))
        } else {
            Ok(None)
        }
    }

    /// Atomically flush all buffered items, turns, and thread updates to SQLite.
    pub fn flush(&self) -> Result<usize> {
        let (items, turns, threads) = {
            let mut buf = self.buffer.lock().unwrap();
            if buf.items.is_empty() && buf.turns.is_empty() && buf.threads.is_empty() {
                return Ok(0);
            }
            let items = std::mem::take(&mut buf.items);
            let turns = std::mem::take(&mut buf.turns);
            let threads = std::mem::take(&mut buf.threads);
            (items, turns, threads)
        };

        let total_count = items.len() + turns.len() + threads.len();
        let mut conn = self.conn.lock().unwrap();
        let tx = conn
            .transaction()
            .context("Failed to begin live-sync transaction")?;

        // 1. Flush turns
        {
            let mut stmt = tx.prepare(
                r#"
                INSERT INTO thread_turns (id, thread_id, turn_index, status, created_at)
                VALUES (?1, ?2, ?3, ?4, ?5)
                ON CONFLICT(id) DO UPDATE SET
                    status = excluded.status
                "#,
            )?;
            for turn in turns {
                stmt.execute(params![
                    turn.id,
                    turn.thread_id,
                    turn.turn_index as i64,
                    turn.status,
                    turn.created_at,
                ])?;
            }
        }

        // 2. Flush items
        {
            let mut stmt = tx.prepare(
                r#"
                INSERT INTO thread_items (id, thread_id, turn_id, item_type, content, created_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(id) DO UPDATE SET
                    content = excluded.content
                "#,
            )?;
            for item in items {
                stmt.execute(params![
                    item.id,
                    item.thread_id,
                    item.turn_id,
                    item.item_type,
                    item.content,
                    item.created_at,
                ])?;
            }
        }

        // 3. Flush thread title updates
        {
            let mut stmt = tx.prepare(
                r#"
                UPDATE threads
                SET title = ?1, updated_at = ?2
                WHERE id = ?3
                "#,
            )?;
            for t in threads {
                stmt.execute(params![t.title, t.updated_at, t.thread_id])?;
            }
        }

        tx.commit()
            .context("Failed to commit live-sync transaction")?;
        Ok(total_count)
    }

    /// Shutdown the synchronizer and guarantee all buffered records are written to disk.
    pub fn shutdown_and_flush(&self) -> Result<usize> {
        self.flush()
    }
}

#[cfg(test)]
mod tests {
    use crate::db::Storage;
    use chrono::Utc;
    use m31a_core::types::SessionMetadata;

    #[test]
    fn test_live_sync_buffering_and_batch_flush() {
        let storage = Storage::open_in_memory().unwrap();
        let meta = SessionMetadata {
            id: "sync-thread-1".to_string(),
            title: "Original Thread Title".to_string(),
            model: "test-model".to_string(),
            cwd: "/tmp".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            extra: std::collections::HashMap::new(),
        };
        let thread_id = meta.id.clone();

        storage
            .save_thread(&meta, "/tmp/rollout", "read-only", "on-failure")
            .unwrap();

        let sync = storage.live_sync(10);
        assert!(!sync.is_dirty());
        assert_eq!(sync.pending_count(), 0);

        // Record a turn and items into the buffer
        let turn_id = "turn-sync-1";
        sync.record_turn(turn_id, &thread_id, 0, "in_progress");
        sync.record_item(
            "item-sync-1",
            &thread_id,
            Some(turn_id),
            "user",
            "Hello agent",
        );
        sync.record_item(
            "item-sync-2",
            &thread_id,
            Some(turn_id),
            "thought",
            "Processing...",
        );
        sync.update_thread_title(&thread_id, "Updated Thread Title");

        assert!(sync.is_dirty());
        assert_eq!(sync.pending_count(), 4);

        // Verify items NOT yet written to storage before flush
        let items_before = storage.list_items(&thread_id).unwrap();
        assert!(items_before.is_empty());

        // Perform flush
        let flushed = sync.flush().unwrap();
        assert_eq!(flushed, 4);
        assert!(!sync.is_dirty());
        assert_eq!(sync.pending_count(), 0);

        // Verify items ARE in storage now
        let items_after = storage.list_items(&thread_id).unwrap();
        assert_eq!(items_after.len(), 2);

        // Verify turn is saved
        let turns = storage.list_turns(&thread_id).unwrap();
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].status, "in_progress");

        // Verify title was updated
        let thread_record = storage.get_thread(&thread_id).unwrap().unwrap();
        assert_eq!(thread_record.title, "Updated Thread Title");
    }

    #[test]
    fn test_live_sync_threshold_trigger() {
        let storage = Storage::open_in_memory().unwrap();
        let meta = SessionMetadata {
            id: "sync-thread-2".to_string(),
            title: "Threshold Test".to_string(),
            model: "test-model".to_string(),
            cwd: "/tmp".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            extra: std::collections::HashMap::new(),
        };
        let thread_id = meta.id.clone();
        storage
            .save_thread(&meta, "/tmp/rollout", "read-only", "on-failure")
            .unwrap();

        let sync = storage.live_sync(3); // threshold 3

        sync.record_item("item-1", &thread_id, None, "system", "msg 1");
        assert_eq!(sync.flush_if_threshold().unwrap(), None);

        sync.record_item("item-2", &thread_id, None, "system", "msg 2");
        assert_eq!(sync.flush_if_threshold().unwrap(), None);

        sync.record_item("item-3", &thread_id, None, "system", "msg 3");
        // Reached threshold 3 -> flushes
        assert_eq!(sync.flush_if_threshold().unwrap(), Some(3));
        assert!(!sync.is_dirty());

        let items = storage.list_items(&thread_id).unwrap();
        assert_eq!(items.len(), 3);
    }
}
