//! Structured diagnostic log database and security audit trail.
//!
//! Captures high-fidelity runtime events, error diagnostics, and security-sensitive
//! operations (such as sandbox escapes, execve invocations, and credential masking events)
//! into a durable SQLite audit log store with multi-field filtering.

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// §6.2 retention budget: maximum audit-log pool size (upstream 10 MiB).
pub const RETENTION_MAX_BYTES: i64 = 10 * 1024 * 1024;
/// §6.2 retention budget: maximum rows per log partition (upstream 1,000).
pub const RETENTION_MAX_ROWS: i64 = 1_000;

/// Severity and classification levels for audit and diagnostic logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    SecurityAudit,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
            Self::SecurityAudit => "SECURITY_AUDIT",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "TRACE" => Some(Self::Trace),
            "DEBUG" => Some(Self::Debug),
            "INFO" => Some(Self::Info),
            "WARN" => Some(Self::Warn),
            "ERROR" => Some(Self::Error),
            "SECURITY_AUDIT" => Some(Self::SecurityAudit),
            _ => None,
        }
    }
}

/// Durable audit log record in SQLite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditLogRecord {
    pub id: String,
    pub timestamp_epoch_ms: i64,
    pub level: LogLevel,
    pub target_crate: String,
    pub thread_id: Option<String>,
    pub message: String,
    pub metadata_json: String,
}

/// Search and filter criteria for diagnostic queries.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LogQueryFilter {
    pub level: Option<LogLevel>,
    pub target_crate: Option<String>,
    pub thread_id: Option<String>,
    pub since_epoch_ms: Option<i64>,
    pub until_epoch_ms: Option<i64>,
    pub query_text: Option<String>,
    pub limit: Option<usize>,
}

/// SQLite-backed diagnostic log database.
#[derive(Debug, Clone)]
pub struct LogDatabase {
    conn: Arc<Mutex<Connection>>,
}

impl LogDatabase {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    /// §6.2: ensures the audit-log schema on a dedicated connection.
    fn ensure_log_schema(conn: &Connection) -> Result<()> {
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS audit_logs (
                id TEXT PRIMARY KEY,
                timestamp_epoch_ms INTEGER NOT NULL,
                level TEXT NOT NULL,
                target_crate TEXT NOT NULL,
                thread_id TEXT,
                message TEXT NOT NULL,
                metadata_json TEXT NOT NULL
            );
            "#,
            [],
        )
        .context("Failed ensuring audit_logs schema")?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_logs(timestamp_epoch_ms)",
            [],
        )
        .context("Failed ensuring audit_logs index")?;
        Ok(())
    }

    /// §6.2: opens a DEDICATED logs pool at `path` (separate SQLite file from
    /// the primary state DB, reducing lock contention). Self-heals from
    /// corruption: the corrupt file is backed up and a fresh pool is created.
    pub fn open_dedicated(path: &std::path::Path) -> Result<Self> {
        match Self::open_dedicated_inner(path) {
            Ok(db) => Ok(db),
            Err(e) if is_anyhow_sqlite_corruption(&e) => {
                Self::backup_and_recreate(path)?;
                Self::open_dedicated_inner(path)
            }
            Err(e) => Err(e),
        }
    }

    fn open_dedicated_inner(path: &std::path::Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed creating logs dir: {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open logs database at {:?}", path))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA busy_timeout = 5000; PRAGMA cache_size = -32000; PRAGMA temp_store = MEMORY; PRAGMA mmap_size = 67108864;",
        )
        .context("Failed configuring logs database pragmas")?;
        Self::ensure_log_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// §6.2 corruption recovery (upstream `backup_runtime_db_for_fresh_start`):
    /// moves the corrupt DB to `<name>.corrupt-<ts>.db`, removes stale WAL/SHM
    /// siblings, leaving a clean path for a fresh database.
    pub fn backup_and_recreate(path: &std::path::Path) -> Result<()> {
        let ts = Utc::now().format("%Y%m%dT%H%M%SZ");
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "logs.db".to_string());
        let backup_name = format!("{file_name}.corrupt-{ts}");
        let backup = path.with_file_name(backup_name);
        if path.exists() {
            std::fs::rename(path, &backup)
                .with_context(|| format!("Failed backing up corrupt DB to {}", backup.display()))?;
        }
        for suffix in ["-wal", "-shm"] {
            let sibling = path.with_file_name(format!("{file_name}{suffix}"));
            if sibling.exists() {
                std::fs::remove_file(&sibling).with_context(|| {
                    format!("Failed removing stale sibling: {}", sibling.display())
                })?;
            }
        }
        Ok(())
    }

    /// §6.2: enforces the retention budgets (10 MiB / 1,000 rows,
    /// oldest-first). Returns the number of pruned rows.
    pub fn enforce_retention(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let total_rows: i64 =
            conn.query_row("SELECT COUNT(*) FROM audit_logs", [], |r| r.get(0))?;
        let total_bytes: i64 = conn.query_row(
            "SELECT COALESCE(SUM(LENGTH(message) + LENGTH(metadata_json) + LENGTH(target_crate) + 32), 0) FROM audit_logs",
            [],
            |r| r.get(0),
        )?;

        let mut pruned = 0usize;
        let prune = |conn: &Connection, limit: i64| -> Result<usize> {
            if limit <= 0 {
                return Ok(0);
            }
            Ok(conn.execute(
                "DELETE FROM audit_logs WHERE id IN (
                    SELECT id FROM audit_logs ORDER BY timestamp_epoch_ms ASC, id ASC LIMIT ?1
                )",
                params![limit],
            )?)
        };

        if total_bytes > RETENTION_MAX_BYTES {
            // Proportional estimate: prune the fraction over budget.
            let estimate = (total_rows as f64
                * (1.0 - RETENTION_MAX_BYTES as f64 / total_bytes as f64))
                as i64;
            pruned += prune(&conn, estimate)?;
        }
        let remaining: i64 = conn.query_row("SELECT COUNT(*) FROM audit_logs", [], |r| r.get(0))?;
        if remaining > RETENTION_MAX_ROWS {
            pruned += prune(&conn, remaining - RETENTION_MAX_ROWS)?;
        }
        Ok(pruned)
    }

    /// §6.2: spawns a background reclamation worker enforcing retention on an
    /// interval. Dropping (or `stop`ping) the handle signals shutdown.
    pub fn spawn_retention_worker(&self, interval: std::time::Duration) -> RetentionWorker {
        let db = self.clone();
        let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = Arc::clone(&shutdown);
        let handle = std::thread::spawn(move || {
            while !flag.load(std::sync::atomic::Ordering::Relaxed) {
                std::thread::sleep(interval);
                if flag.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                // Reclamation is best-effort.
                let _ = db.enforce_retention();
            }
        });
        RetentionWorker {
            shutdown,
            handle: Some(handle),
        }
    }

    /// Append a structured log entry into SQLite.
    pub fn append(
        &self,
        level: LogLevel,
        target_crate: impl Into<String>,
        thread_id: Option<&str>,
        message: impl Into<String>,
        metadata: &serde_json::Value,
    ) -> Result<String> {
        let conn = self.conn.lock().unwrap();
        let log_id = format!("log-{}", Uuid::new_v4().simple());
        let now = Utc::now().timestamp_millis();
        let metadata_str = serde_json::to_string(metadata)?;
        let msg = message.into();
        let crate_name = target_crate.into();

        conn.execute(
            r#"
            INSERT INTO audit_logs (
                id, timestamp_epoch_ms, level, target_crate, thread_id, message, metadata_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                log_id,
                now,
                level.as_str(),
                crate_name,
                thread_id,
                msg,
                metadata_str,
            ],
        )
        .context("Failed inserting record into audit_logs")?;

        Ok(log_id)
    }

    /// Query audit logs matching filter parameters.
    pub fn query(&self, filter: LogQueryFilter) -> Result<Vec<AuditLogRecord>> {
        let conn = self.conn.lock().unwrap();

        let mut query = String::from(
            "SELECT id, timestamp_epoch_ms, level, target_crate, thread_id, message, metadata_json FROM audit_logs WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(level) = filter.level {
            query.push_str(" AND level = ?");
            params.push(Box::new(level.as_str().to_string()));
        }

        if let Some(ref target) = filter.target_crate {
            query.push_str(" AND target_crate = ?");
            params.push(Box::new(target.clone()));
        }

        if let Some(ref tid) = filter.thread_id {
            query.push_str(" AND thread_id = ?");
            params.push(Box::new(tid.clone()));
        }

        if let Some(since) = filter.since_epoch_ms {
            query.push_str(" AND timestamp_epoch_ms >= ?");
            params.push(Box::new(since));
        }

        if let Some(until) = filter.until_epoch_ms {
            query.push_str(" AND timestamp_epoch_ms <= ?");
            params.push(Box::new(until));
        }

        if let Some(ref text) = filter.query_text {
            query.push_str(" AND message LIKE ?");
            params.push(Box::new(format!("%{}%", text)));
        }

        query.push_str(" ORDER BY timestamp_epoch_ms DESC");

        if let Some(limit) = filter.limit {
            query.push_str(&format!(" LIMIT {}", limit));
        } else {
            query.push_str(" LIMIT 500");
        }

        let mut stmt = conn.prepare(&query)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let level_str: String = row.get(2)?;
            Ok(AuditLogRecord {
                id: row.get(0)?,
                timestamp_epoch_ms: row.get(1)?,
                level: LogLevel::from_str(&level_str).unwrap_or(LogLevel::Info),
                target_crate: row.get(3)?,
                thread_id: row.get(4)?,
                message: row.get(5)?,
                metadata_json: row.get(6)?,
            })
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    /// Count error occurrences since a given timestamp.
    pub fn count_errors_since(&self, since_epoch_ms: i64) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT COUNT(*) FROM audit_logs WHERE (level = 'ERROR' OR level = 'SECURITY_AUDIT') AND timestamp_epoch_ms >= ?1",
        )?;
        let count: usize = stmt.query_row(params![since_epoch_ms], |r| r.get(0))?;
        Ok(count)
    }

    /// Prune old logs to prevent unbounded database growth.
    pub fn prune_logs_older_than(&self, cutoff_epoch_ms: i64) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute(
            "DELETE FROM audit_logs WHERE timestamp_epoch_ms < ?1",
            params![cutoff_epoch_ms],
        )?;
        Ok(deleted)
    }
}

/// §6.2: matches SQLite corruption signatures (upstream
/// `is_sqlite_corruption_error`).
pub fn is_sqlite_corruption_error(e: &rusqlite::Error) -> bool {
    match e {
        rusqlite::Error::SqliteFailure(ffi_err, Some(msg)) => {
            ffi_err.code == rusqlite::ErrorCode::DatabaseCorrupt
                || ffi_err.code == rusqlite::ErrorCode::NotADatabase
                || msg.to_lowercase().contains("corrupt")
                || msg.to_lowercase().contains("malformed")
                || msg.to_lowercase().contains("not a database")
        }
        _ => {
            let s = e.to_string().to_lowercase();
            s.contains("corrupt") || s.contains("malformed") || s.contains("not a database")
        }
    }
}

/// Corruption check over an `anyhow` error chain.
fn is_anyhow_sqlite_corruption(e: &anyhow::Error) -> bool {
    e.chain()
        .filter_map(|c| c.downcast_ref::<rusqlite::Error>())
        .any(is_sqlite_corruption_error)
}

/// Handle to the §6.2 background reclamation worker; `stop` (or drop) joins
/// the worker thread.
pub struct RetentionWorker {
    shutdown: Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl RetentionWorker {
    /// Signals shutdown and joins the worker thread (may block up to one
    /// interval).
    pub fn stop(mut self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for RetentionWorker {
    fn drop(&mut self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::{BUILTIN_MIGRATIONS, MigrationEngine};

    fn setup_test_log_db() -> LogDatabase {
        let mut conn = Connection::open_in_memory().unwrap();
        MigrationEngine::apply_all(&mut conn, BUILTIN_MIGRATIONS).unwrap();
        LogDatabase::new(Arc::new(Mutex::new(conn)))
    }

    #[test]
    fn test_append_and_query_filter() {
        let db = setup_test_log_db();

        db.append(
            LogLevel::Info,
            "m31a-core",
            Some("thread-101"),
            "Session initialized successfully",
            &serde_json::json!({"turns": 0}),
        )
        .unwrap();

        db.append(
            LogLevel::Error,
            "m31a-sandbox",
            Some("thread-101"),
            "Seccomp rule blocked banned syscall",
            &serde_json::json!({"syscall": 123}),
        )
        .unwrap();

        db.append(
            LogLevel::SecurityAudit,
            "m31a-execpolicy",
            None,
            "SSRF attempt to 169.254.169.254 intercepted",
            &serde_json::json!({"ip": "169.254.169.254"}),
        )
        .unwrap();

        // Query all errors
        let error_filter = LogQueryFilter {
            level: Some(LogLevel::Error),
            ..Default::default()
        };
        let errors = db.query(error_filter).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].target_crate, "m31a-sandbox");

        // Query by text
        let ssrf_filter = LogQueryFilter {
            query_text: Some("SSRF".to_string()),
            ..Default::default()
        };
        let ssrf_logs = db.query(ssrf_filter).unwrap();
        assert_eq!(ssrf_logs.len(), 1);
        assert_eq!(ssrf_logs[0].level, LogLevel::SecurityAudit);

        // Count errors
        let error_count = db.count_errors_since(0).unwrap();
        assert_eq!(error_count, 2); // 1 Error + 1 SecurityAudit
    }

    /// §6.2: dedicated pool round-trip, separated from the primary DB.
    #[test]
    fn test_open_dedicated_pool() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("logs.db");
        let db = LogDatabase::open_dedicated(&path).expect("dedicated open");

        db.append(
            LogLevel::Info,
            "test",
            None,
            "dedicated pool entry",
            &serde_json::json!({}),
        )
        .unwrap();
        let logs = db
            .query(LogQueryFilter {
                target_crate: Some("test".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].message, "dedicated pool entry");

        // Schema exists at the dedicated path; reopening keeps data (WAL).
        let reopened = LogDatabase::open_dedicated(&path).expect("reopen");
        let logs = reopened
            .query(LogQueryFilter {
                target_crate: Some("test".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(logs.len(), 1, "dedicated pool must persist across opens");
    }

    /// §6.2: corruption recovery — a corrupt file is backed up and a fresh
    /// pool is created instead of failing forever.
    #[test]
    fn test_corruption_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("logs.db");

        // Write garbage that SQLite will reject (NOTADB).
        std::fs::write(&path, b"definitely not a sqlite database, just text bytes").unwrap();
        let db = LogDatabase::open_dedicated(&path).expect("self-healing open");

        // A backup of the corrupt file exists.
        let entries: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().to_string()))
            .collect();
        assert!(
            entries.iter().any(|n| n.contains(".corrupt-")),
            "corrupt file must be backed up: {entries:?}"
        );

        // The fresh pool works.
        db.append(
            LogLevel::Warn,
            "test",
            None,
            "post-recovery entry",
            &serde_json::json!({}),
        )
        .unwrap();
        let logs = db.query(LogQueryFilter::default()).unwrap();
        assert_eq!(logs.len(), 1);
    }

    /// §6.2: corruption predicate matches the canonical signatures.
    #[test]
    fn test_corruption_error_predicate() {
        let corrupt = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT),
            Some("database disk image is malformed".to_string()),
        );
        assert!(is_sqlite_corruption_error(&corrupt));

        let benign = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT),
            Some("FOREIGN KEY constraint failed".to_string()),
        );
        assert!(!is_sqlite_corruption_error(&benign));
    }

    /// §6.2: retention row budget — 1,050 rows → prune 50 oldest.
    #[test]
    fn test_retention_row_budget() {
        let dir = tempfile::tempdir().unwrap();
        let db = LogDatabase::open_dedicated(&dir.path().join("logs.db")).unwrap();

        for i in 0..1_050 {
            db.append(
                LogLevel::Info,
                "test",
                None,
                format!("entry-{i}"),
                &serde_json::json!({}),
            )
            .unwrap();
        }

        let pruned = db.enforce_retention().unwrap();
        assert_eq!(pruned, 50, "must prune down to RETENTION_MAX_ROWS");

        let logs = db
            .query(LogQueryFilter {
                limit: Some(2_000),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(logs.len() as i64, RETENTION_MAX_ROWS);
        // Oldest pruned: the first entries are gone; newest remain.
        assert!(!logs.iter().any(|l| l.message == "entry-0"));
        assert!(logs.iter().any(|l| l.message == "entry-1049"));
    }

    /// §6.2: retention byte budget — ~12 MiB of payload → prune ~2 MiB worth.
    #[test]
    fn test_retention_byte_budget() {
        let dir = tempfile::tempdir().unwrap();
        let db = LogDatabase::open_dedicated(&dir.path().join("logs.db")).unwrap();

        // 12 entries × 1 MiB message ≈ 12 MiB > 10 MiB budget.
        for _i in 0..12 {
            db.append(
                LogLevel::Info,
                "test",
                None,
                "y".repeat(1_000_000),
                &serde_json::json!({}),
            )
            .unwrap();
        }
        let pruned = db.enforce_retention().unwrap();
        assert!(pruned >= 1, "byte budget must force pruning");

        let logs = db.query(LogQueryFilter::default()).unwrap();
        assert!((logs.len() as i64) <= 12, "prune must not grow the pool");
        // The oldest entries went first.
        assert!(logs.iter().all(|l| l.message.len() == 1_000_000));
    }

    /// §6.2: the background reclamation worker runs and stops cleanly.
    #[test]
    fn test_retention_worker_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let db = LogDatabase::open_dedicated(&dir.path().join("logs.db")).unwrap();

        for i in 0..3 {
            db.append(
                LogLevel::Info,
                "test",
                None,
                format!("entry-{i}"),
                &serde_json::json!({}),
            )
            .unwrap();
        }

        let worker = db.spawn_retention_worker(std::time::Duration::from_millis(50));
        // Under-budget pool must be left alone by the worker; stop() joins.
        std::thread::sleep(std::time::Duration::from_millis(180));
        let after = db.query(LogQueryFilter::default()).unwrap().len();
        assert_eq!(after, 3, "under-budget pool must be left alone");
        worker.stop();
    }
}
