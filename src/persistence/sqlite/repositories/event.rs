//! SQLite concrete implementation of EventRepository (D-10, D-11, D-16, EVT-02, EVT-03).

use crate::error::M31AError;
use crate::events::envelope::{EventCategory, EventEnvelope};
use crate::events::types::EventType;
use crate::ids::{EventId, MissionId, SessionId};
use crate::persistence::sqlite::repositories::EventRepository;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use std::str::FromStr;

/// SQLite repository for persistent event logs.
#[derive(Debug, Clone)]
pub struct SqliteEventRepository {
    pool: SqlitePool,
}

impl SqliteEventRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Retrieve the next sequence number for a mission or globally.
    pub async fn next_sequence(&self, mission_id: Option<MissionId>) -> Result<u64, M31AError> {
        let latest = self.latest_sequence(mission_id).await?;
        Ok(latest + 1)
    }

    /// List events for a mission strictly after a given sequence watermark.
    pub async fn list_since_sequence(
        &self,
        mission_id: MissionId,
        since: u64,
    ) -> Result<Vec<EventEnvelope>, M31AError> {
        let rows = sqlx::query(
            r#"
            SELECT id, sequence, mission_id, session_id, actor, category,
                   correlation_id, causation_id, event_type, payload, schema_version, created_at
            FROM event_log
            WHERE mission_id = ? AND sequence > ?
            ORDER BY sequence ASC
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .bind(since as i64)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_row_to_envelope).collect()
    }
}

#[async_trait]
impl EventRepository for SqliteEventRepository {
    async fn append(&self, envelope: &EventEnvelope) -> Result<(), M31AError> {
        // P1-04: event persistence is a security boundary. Callers must not
        // be trusted to have sanitized ToolRequested.arguments,
        // ToolCompleted.result, ToolFailed.error, etc. Scrub the serialized
        // payload here so no raw credential reaches the event log.
        let raw_payload_json = serde_json::to_string(&envelope.event_type)
            .map_err(|e| M31AError::validation(e.to_string()))?;
        let payload_json =
            crate::telemetry::redactor::SecretRedactor::new().redact_text(&raw_payload_json);
        let session_bytes = envelope.session_id.map(|s| *s.as_bytes());
        let causation_bytes = envelope.causation_id.map(|c| *c.as_bytes());

        sqlx::query(
            r#"
            INSERT INTO event_log (
                id, sequence, mission_id, session_id, actor, category,
                correlation_id, causation_id, event_type, payload, schema_version, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(envelope.id.as_bytes().as_slice())
        .bind(envelope.sequence as i64)
        .bind(
            envelope
                .mission_id
                .map(|m| *m.as_bytes())
                .as_ref()
                .map(|b| b.as_slice()),
        )
        .bind(session_bytes.as_ref().map(|b| b.as_slice()))
        .bind(&envelope.actor)
        .bind(envelope.category.to_string())
        .bind(envelope.correlation_id.as_deref())
        .bind(causation_bytes.as_ref().map(|b| b.as_slice()))
        .bind(envelope.event_type.name())
        .bind(&payload_json)
        .bind(envelope.schema_version as i64)
        .bind(envelope.timestamp.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn list_by_mission(
        &self,
        mission_id: MissionId,
    ) -> Result<Vec<EventEnvelope>, M31AError> {
        let rows = sqlx::query(
            r#"
            SELECT id, sequence, mission_id, session_id, actor, category,
                   correlation_id, causation_id, event_type, payload, schema_version, created_at
            FROM event_log
            WHERE mission_id = ?
            ORDER BY sequence ASC
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(map_row_to_envelope).collect()
    }

    async fn latest_sequence(&self, mission_id: Option<MissionId>) -> Result<u64, M31AError> {
        let row: (i64,) = match mission_id {
            Some(mid) => {
                sqlx::query_as(
                    "SELECT COALESCE(MAX(sequence), 0) FROM event_log WHERE mission_id = ?",
                )
                .bind(mid.as_bytes().as_slice())
                .fetch_one(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as("SELECT COALESCE(MAX(sequence), 0) FROM event_log")
                    .fetch_one(&self.pool)
                    .await?
            }
        };

        Ok(row.0 as u64)
    }
}

fn map_row_to_envelope(row: SqliteRow) -> Result<EventEnvelope, M31AError> {
    let id_raw: Vec<u8> = row.try_get("id")?;
    let id_bytes: [u8; 16] = id_raw
        .try_into()
        .map_err(|_| M31AError::persistence("corrupt event id BLOB length"))?;
    let id = EventId::from_bytes(id_bytes);

    let sequence_i64: i64 = row.try_get("sequence")?;
    let sequence = sequence_i64 as u64;

    let mission_id_raw: Option<Vec<u8>> = row.try_get("mission_id")?;
    let mission_id = mission_id_raw
        .and_then(|bytes| <[u8; 16]>::try_from(bytes).ok())
        .map(MissionId::from_bytes);

    let session_id_raw: Option<Vec<u8>> = row.try_get("session_id")?;
    let session_id = session_id_raw
        .and_then(|bytes| <[u8; 16]>::try_from(bytes).ok())
        .map(SessionId::from_bytes);

    let actor: String = row.try_get("actor")?;

    let category_str: String = row.try_get("category")?;
    let category = EventCategory::from_str(&category_str)?;

    let correlation_id: Option<String> = row.try_get("correlation_id")?;

    let causation_id_raw: Option<Vec<u8>> = row.try_get("causation_id")?;
    let causation_id = causation_id_raw
        .and_then(|bytes| <[u8; 16]>::try_from(bytes).ok())
        .map(EventId::from_bytes);

    let payload_str: String = row.try_get("payload")?;
    let event_type: EventType = serde_json::from_str(&payload_str)
        .map_err(|e| M31AError::persistence(format!("corrupt event payload JSON: {}", e)))?;

    let schema_version_i64: i64 = row.try_get("schema_version")?;
    let schema_version = schema_version_i64 as u32;

    let created_at_str: String = row.try_get("created_at")?;
    let timestamp = DateTime::parse_from_rfc3339(&created_at_str)
        .map_err(|e| M31AError::persistence(format!("invalid event created_at: {}", e)))?
        .with_timezone(&Utc);

    Ok(EventEnvelope {
        id,
        sequence,
        timestamp,
        mission_id,
        session_id,
        actor,
        event_type,
        category,
        correlation_id,
        causation_id,
        schema_version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::sqlite::schema::initialize_database;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_sqlite_event_repository_sequence_ordering() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let pool = initialize_database(&db_path).await.unwrap();
        let repo = SqliteEventRepository::new(pool);

        let mission_id = MissionId::new();

        assert_eq!(repo.latest_sequence(Some(mission_id)).await.unwrap(), 0);
        assert_eq!(repo.next_sequence(Some(mission_id)).await.unwrap(), 1);

        let mut env1 = EventEnvelope::new(
            1,
            Some(mission_id),
            None,
            "tester".to_string(),
            EventType::MissionStarted {
                mission_id,
                objective: "Obj".to_string(),
            },
        );
        env1.correlation_id = Some("corr-1".to_string());

        let mut env2 = EventEnvelope::new(
            2,
            Some(mission_id),
            None,
            "tester".to_string(),
            EventType::MissionCancelled {
                mission_id,
                reason: "User requested".to_string(),
            },
        );
        env2.causation_id = Some(env1.id);
        env2.correlation_id = Some("corr-1".to_string());

        repo.append(&env1).await.unwrap();
        repo.append(&env2).await.unwrap();

        assert_eq!(repo.latest_sequence(Some(mission_id)).await.unwrap(), 2);
        assert_eq!(repo.next_sequence(Some(mission_id)).await.unwrap(), 3);

        let events = repo.list_by_mission(mission_id).await.unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sequence, 1);
        assert_eq!(events[1].sequence, 2);
        assert_eq!(events[0].correlation_id.as_deref(), Some("corr-1"));
        assert_eq!(events[1].causation_id, Some(env1.id));

        // Test list_since_sequence
        let since_1 = repo.list_since_sequence(mission_id, 1).await.unwrap();
        assert_eq!(since_1.len(), 1);
        assert_eq!(since_1[0].sequence, 2);

        let since_2 = repo.list_since_sequence(mission_id, 2).await.unwrap();
        assert_eq!(since_2.len(), 0);
    }
}
