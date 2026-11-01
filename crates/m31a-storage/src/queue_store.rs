//! Persistent SQLite background task queue store.
//!
//! Provides durable priority queueing, optimistic status transitions (`Pending` -> `Processing`
//! -> `Completed`/`Failed`), automatic retry tracking, and dead-letter queuing for
//! asynchronous agent tasks.

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Lifecycle status of a queued task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    DeadLetter,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::DeadLetter => "dead_letter",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "processing" => Some(Self::Processing),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "dead_letter" => Some(Self::DeadLetter),
            _ => None,
        }
    }
}

/// Durable task record stored in SQLite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueTaskRecord {
    pub id: String,
    pub thread_id: String,
    pub priority: i32,
    pub task_kind: String,
    pub payload_json: String,
    pub status: TaskStatus,
    pub retries: u32,
    pub max_retries: u32,
    pub error_message: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// SQLite-backed persistent queue store.
#[derive(Debug, Clone)]
pub struct QueueStore {
    conn: Arc<Mutex<Connection>>,
}

impl QueueStore {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// Enqueue a new task into the SQLite task queue.
    pub fn enqueue(
        &self,
        thread_id: &str,
        priority: i32,
        task_kind: &str,
        payload: &serde_json::Value,
        max_retries: u32,
    ) -> Result<String> {
        let conn = self.conn.lock().unwrap();
        let task_id = format!("task-{}", Uuid::new_v4().simple());
        let now = Utc::now().timestamp_millis();
        let payload_str = serde_json::to_string(payload)?;

        conn.execute(
            r#"
            INSERT INTO task_queue (
                id, thread_id, priority, task_kind, payload_json,
                status, retries, max_retries, error_message, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?10)
            "#,
            params![
                task_id,
                thread_id,
                priority,
                task_kind,
                payload_str,
                TaskStatus::Pending.as_str(),
                0,
                max_retries,
                now,
                now,
            ],
        )
        .context("Failed enqueuing task into SQLite task_queue")?;

        Ok(task_id)
    }

    /// Atomically claim and dequeue the next pending task with the highest priority.
    pub fn dequeue_next(&self) -> Result<Option<QueueTaskRecord>> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp_millis();

        // Find highest priority, earliest created pending task
        let mut stmt = conn.prepare(
            r#"
            SELECT id, thread_id, priority, task_kind, payload_json,
                   status, retries, max_retries, error_message, created_at, updated_at
            FROM task_queue
            WHERE status = 'pending'
            ORDER BY priority DESC, created_at ASC
            LIMIT 1
            "#,
        )?;

        let mut row_iter = stmt.query_map([], |row| {
            let status_str: String = row.get(5)?;
            Ok(QueueTaskRecord {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                priority: row.get(2)?,
                task_kind: row.get(3)?,
                payload_json: row.get(4)?,
                status: TaskStatus::from_str(&status_str).unwrap_or(TaskStatus::Pending),
                retries: row.get(6)?,
                max_retries: row.get(7)?,
                error_message: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;

        if let Some(task_res) = row_iter.next() {
            let mut task = task_res?;
            // Mark as processing
            conn.execute(
                "UPDATE task_queue SET status = 'processing', updated_at = ?1 WHERE id = ?2",
                params![now, task.id],
            )?;
            task.status = TaskStatus::Processing;
            task.updated_at = now;
            Ok(Some(task))
        } else {
            Ok(None)
        }
    }

    /// Mark a task as successfully completed.
    pub fn mark_completed(&self, task_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp_millis();
        conn.execute(
            "UPDATE task_queue SET status = 'completed', updated_at = ?1 WHERE id = ?2",
            params![now, task_id],
        )?;
        Ok(())
    }

    /// Record failure on a task. Automatically retries if under `max_retries`, or moves to `DeadLetter`.
    pub fn fail_or_retry(&self, task_id: &str, error_msg: &str) -> Result<TaskStatus> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp_millis();

        let mut stmt = conn.prepare("SELECT retries, max_retries FROM task_queue WHERE id = ?1")?;
        let (retries, max_retries): (u32, u32) = stmt
            .query_row(params![task_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .context("Task not found")?;

        let new_retries = retries + 1;
        let new_status = if new_retries >= max_retries {
            TaskStatus::DeadLetter
        } else {
            TaskStatus::Pending // reset to pending for retry
        };

        conn.execute(
            "UPDATE task_queue SET status = ?1, retries = ?2, error_message = ?3, updated_at = ?4 WHERE id = ?5",
            params![new_status.as_str(), new_retries, error_msg, now, task_id],
        )?;

        Ok(new_status)
    }

    /// List all tasks matching a specific lifecycle status.
    pub fn list_by_status(&self, status: TaskStatus) -> Result<Vec<QueueTaskRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, thread_id, priority, task_kind, payload_json,
                   status, retries, max_retries, error_message, created_at, updated_at
            FROM task_queue
            WHERE status = ?1
            ORDER BY priority DESC, created_at ASC
            "#,
        )?;

        let rows = stmt.query_map(params![status.as_str()], |row| {
            let status_str: String = row.get(5)?;
            Ok(QueueTaskRecord {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                priority: row.get(2)?,
                task_kind: row.get(3)?,
                payload_json: row.get(4)?,
                status: TaskStatus::from_str(&status_str).unwrap_or(TaskStatus::Pending),
                retries: row.get(6)?,
                max_retries: row.get(7)?,
                error_message: row.get(8)?,
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

    /// Purge completed tasks older than a given epoch millisecond timestamp.
    pub fn purge_completed_older_than(&self, cutoff_epoch_ms: i64) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute(
            "DELETE FROM task_queue WHERE status = 'completed' AND updated_at < ?1",
            params![cutoff_epoch_ms],
        )?;
        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::{BUILTIN_MIGRATIONS, MigrationEngine};

    fn setup_test_queue() -> QueueStore {
        let mut conn = Connection::open_in_memory().unwrap();
        MigrationEngine::apply_all(&mut conn, BUILTIN_MIGRATIONS).unwrap();
        QueueStore::new(Arc::new(Mutex::new(conn)))
    }

    #[test]
    fn test_enqueue_dequeue_lifecycle_with_priority() {
        let queue = setup_test_queue();

        // Enqueue low priority task
        let task_low = queue
            .enqueue(
                "thread-1",
                10,
                "format_code",
                &serde_json::json!({"file": "main.rs"}),
                2,
            )
            .unwrap();

        // Enqueue high priority task
        let task_high = queue
            .enqueue(
                "thread-1",
                100,
                "urgent_compile",
                &serde_json::json!({"mode": "release"}),
                2,
            )
            .unwrap();

        // First dequeue must return high priority task
        let first = queue.dequeue_next().unwrap().expect("Expected task");
        assert_eq!(first.id, task_high);
        assert_eq!(first.priority, 100);
        assert_eq!(first.status, TaskStatus::Processing);

        // Mark completed
        queue.mark_completed(&first.id).unwrap();

        // Second dequeue returns low priority task
        let second = queue.dequeue_next().unwrap().expect("Expected low task");
        assert_eq!(second.id, task_low);
        assert_eq!(second.priority, 10);
    }

    #[test]
    fn test_task_retry_and_dead_letter() {
        let queue = setup_test_queue();

        let tid = queue
            .enqueue("thread-2", 0, "test_job", &serde_json::json!({}), 2) // max 2 retries
            .unwrap();

        let task1 = queue.dequeue_next().unwrap().unwrap();
        assert_eq!(task1.id, tid);

        // Fail first attempt -> retried (status reset to pending)
        let s1 = queue.fail_or_retry(&tid, "Network timeout 1").unwrap();
        assert_eq!(s1, TaskStatus::Pending);

        // Dequeue again
        let task2 = queue.dequeue_next().unwrap().unwrap();
        assert_eq!(task2.retries, 1);

        // Fail second attempt -> max_retries reached -> DeadLetter
        let s2 = queue.fail_or_retry(&tid, "Network timeout 2").unwrap();
        assert_eq!(s2, TaskStatus::DeadLetter);

        // No more pending tasks
        assert!(queue.dequeue_next().unwrap().is_none());

        let dead_letters = queue.list_by_status(TaskStatus::DeadLetter).unwrap();
        assert_eq!(dead_letters.len(), 1);
        assert_eq!(
            dead_letters[0].error_message.as_deref(),
            Some("Network timeout 2")
        );
    }
}
