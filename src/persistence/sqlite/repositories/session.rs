//! SQLite concrete implementation of SessionRepository (MSN-05, PST-01).

use crate::error::M31AError;
use crate::ids::{MissionId, SessionId};
use crate::persistence::sqlite::repositories::SessionRepository;
use crate::state::session::{Session, SessionState};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use std::str::FromStr;

/// SQLite repository for Session aggregates.
#[derive(Debug, Clone)]
pub struct SqliteSessionRepository {
    pool: SqlitePool,
}

impl SqliteSessionRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SessionRepository for SqliteSessionRepository {
    async fn insert(&self, session: &Session) -> Result<(), M31AError> {
        let metadata_json = serde_json::to_string(&session.metadata)
            .map_err(|e| M31AError::validation(e.to_string()))?;
        let closed_at_str = session.closed_at.map(|t| t.to_rfc3339());

        sqlx::query(
            r#"
            INSERT INTO sessions (
                id, mission_id, status, metadata, created_at, updated_at, closed_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                metadata = excluded.metadata,
                updated_at = excluded.updated_at,
                closed_at = excluded.closed_at
            "#,
        )
        .bind(session.id.as_bytes().as_slice())
        .bind(session.mission_id.as_bytes().as_slice())
        .bind(session.status.to_string())
        .bind(&metadata_json)
        .bind(session.created_at.to_rfc3339())
        .bind(session.updated_at.to_rfc3339())
        .bind(closed_at_str)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get(&self, id: SessionId) -> Result<Option<Session>, M31AError> {
        let maybe_row = sqlx::query(
            r#"
            SELECT id, mission_id, status, metadata, created_at, updated_at, closed_at
            FROM sessions WHERE id = ?
            "#,
        )
        .bind(id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await?;

        match maybe_row {
            Some(row) => Ok(Some(map_row_to_session(row)?)),
            None => Ok(None),
        }
    }

    async fn list_by_mission(&self, mission_id: MissionId) -> Result<Vec<Session>, M31AError> {
        let rows = sqlx::query(
            r#"
            SELECT id, mission_id, status, metadata, created_at, updated_at, closed_at
            FROM sessions WHERE mission_id = ? ORDER BY created_at ASC
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_row_to_session).collect()
    }

    async fn close(&self, id: SessionId) -> Result<(), M31AError> {
        let now = Utc::now().to_rfc3339();
        let rows_affected = sqlx::query(
            "UPDATE sessions SET status = ?, closed_at = ?, updated_at = ? WHERE id = ?",
        )
        .bind(SessionState::Closed.to_string())
        .bind(&now)
        .bind(&now)
        .bind(id.as_bytes().as_slice())
        .execute(&self.pool)
        .await?
        .rows_affected();

        if rows_affected == 0 {
            return Err(M31AError::not_found(format!("Session {}", id)));
        }
        Ok(())
    }
}

fn map_row_to_session(row: SqliteRow) -> Result<Session, M31AError> {
    let id_raw: Vec<u8> = row.try_get("id")?;
    let id_bytes: [u8; 16] = id_raw
        .try_into()
        .map_err(|_| M31AError::persistence("corrupt session id BLOB length"))?;
    let id = SessionId::from_bytes(id_bytes);

    let mission_id_raw: Vec<u8> = row.try_get("mission_id")?;
    let mission_id_bytes: [u8; 16] = mission_id_raw
        .try_into()
        .map_err(|_| M31AError::persistence("corrupt mission_id BLOB length in session"))?;
    let mission_id = MissionId::from_bytes(mission_id_bytes);

    let status_str: String = row.try_get("status")?;
    let status = SessionState::from_str(&status_str)?;

    let metadata_str: String = row.try_get("metadata")?;
    let metadata: serde_json::Value = serde_json::from_str(&metadata_str)
        .map_err(|e| M31AError::persistence(format!("corrupt session metadata: {e}")))?;

    let created_at_str: String = row.try_get("created_at")?;
    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map_err(|e| M31AError::persistence(format!("invalid created_at: {}", e)))?
        .with_timezone(&Utc);

    let updated_at_str: String = row.try_get("updated_at")?;
    let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
        .map_err(|e| M31AError::persistence(format!("invalid updated_at: {}", e)))?
        .with_timezone(&Utc);

    let closed_at_opt: Option<String> = row.try_get("closed_at")?;
    let closed_at = closed_at_opt
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    Ok(Session {
        id,
        mission_id,
        status,
        metadata,
        created_at,
        updated_at,
        closed_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::sqlite::repositories::SqliteMissionRepository;
    use crate::persistence::sqlite::schema::initialize_database;
    use crate::state::mission::Mission;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_sqlite_session_repository_lifecycle() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let pool = initialize_database(&db_path).await.unwrap();

        let mission_repo = SqliteMissionRepository::new(pool.clone());
        let session_repo = SqliteSessionRepository::new(pool);

        let mission_id = MissionId::new();
        let mission = Mission::new(mission_id, "Test Mission".to_string());
        mission_repo.insert(&mission).await.unwrap();

        let session_id = SessionId::new();
        let mut session = Session::new(session_id, mission_id);
        session.metadata = serde_json::json!({"client": "tui"});

        // Insert
        session_repo.insert(&session).await.unwrap();

        // Get
        let retrieved = session_repo
            .get(session_id)
            .await
            .unwrap()
            .expect("Session exists");
        assert_eq!(retrieved.id, session_id);
        assert_eq!(retrieved.mission_id, mission_id);
        assert_eq!(retrieved.status, SessionState::Active);
        assert_eq!(retrieved.metadata["client"], "tui");
        assert!(retrieved.closed_at.is_none());

        // List by mission
        let list = session_repo.list_by_mission(mission_id).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, session_id);

        // Close
        session_repo.close(session_id).await.unwrap();
        let closed = session_repo.get(session_id).await.unwrap().unwrap();
        assert_eq!(closed.status, SessionState::Closed);
        assert!(closed.closed_at.is_some());
    }
}
