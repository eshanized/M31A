//! SQLite connection pool setup
//!
//! Per D-03, sqlx for SQLite with compile-time checked queries and migration support.
//! Per RESEARCH.md Pitfall 3, set appropriate busy_timeout for concurrent access.

use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};
use std::path::Path;
use std::time::Duration;

/// Creates a SQLite connection pool with WAL mode and busy_timeout.
///
/// - WAL mode enables better concurrent read performance
/// - busy_timeout prevents SQLITE_BUSY errors under concurrent access
/// - max_connections(5) limits pool size per RESEARCH.md
pub async fn create_pool(db_path: &Path) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(
            crate::config::canonical::DEFAULT_PROCESS_TIMEOUT_SECS,
        ))
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_create_pool() {
        let _dir = tempdir().unwrap(); // Keep TempDir alive
        let db_path = _dir.path().join("test.db");

        let pool = create_pool(&db_path).await.unwrap();

        // Verify pool works by running a simple query
        let row: (i64,) = sqlx::query_as("SELECT 1").fetch_one(&pool).await.unwrap();
        assert_eq!(row.0, 1);

        // Verify WAL mode is enabled
        let row: (String,) = sqlx::query_as("PRAGMA journal_mode")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.0.to_lowercase(), "wal");

        // Verify busy_timeout is set (may be 0 if not yet applied to this connection)
        let row: (i64,) = sqlx::query_as("PRAGMA busy_timeout")
            .fetch_one(&pool)
            .await
            .unwrap();
        // busy_timeout is per-connection; accept any positive value since pool sets it
        assert!(row.0 > 0, "busy_timeout should be positive, got {}", row.0);
    }
}
