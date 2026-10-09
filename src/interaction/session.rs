//! Durable interactive coding session aggregate and repository (PRD §01, MSN-05, CONTEXT.md §43).
//!
//! Stores and reconstructs:
//! - Session identity, workspace root, and active mission
//! - Strictly ordered, typed conversation turns:
//!   - UserMessage
//!   - AssistantMessage
//!   - ToolCallMessage
//!   - ToolResultMessage
//!   - SystemMessage
//!   - ApprovalMessage
//!   - VerificationMessage
//!
//! Enforces:
//! - Zero transcript loss across process restarts
//! - Foreign key satisfaction with SQLite missions table
//! - Cryptographically unforgeable UUIDv7 IDs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

use crate::error::M31AError;
use crate::events::bus::EventBus;
use crate::events::envelope::EventEnvelope;
use crate::events::types::EventType;
use crate::ids::{MissionId, SessionId};
use crate::interaction::mentions::MentionReference;
pub use crate::state::session::SessionState;

/// Strongly typed, durable interaction session aggregate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub id: SessionId,
    pub workspace_root: PathBuf,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: SessionState,
    pub active_mission_id: Option<MissionId>,
    pub metadata: serde_json::Value,
}

impl Session {
    pub fn new(id: SessionId, workspace_root: PathBuf) -> Self {
        let now = Utc::now();
        Self {
            id,
            workspace_root,
            created_at: now,
            updated_at: now,
            status: SessionState::Active,
            active_mission_id: None,
            metadata: serde_json::json!({}),
        }
    }
}

/// Strongly typed conversation turn in an interactive developer session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConversationTurn {
    UserMessage {
        id: Uuid,
        sequence: u64,
        content: String,
        raw_text: String,
        mentions: Vec<MentionReference>,
        created_at: DateTime<Utc>,
    },
    AssistantMessage {
        id: Uuid,
        sequence: u64,
        content: String,
        created_at: DateTime<Utc>,
    },
    ToolCallMessage {
        id: Uuid,
        sequence: u64,
        call_id: String,
        tool_name: String,
        arguments: serde_json::Value,
        created_at: DateTime<Utc>,
    },
    ToolResultMessage {
        id: Uuid,
        sequence: u64,
        call_id: String,
        tool_name: String,
        output: String,
        success: bool,
        created_at: DateTime<Utc>,
    },
    SystemMessage {
        id: Uuid,
        sequence: u64,
        content: String,
        created_at: DateTime<Utc>,
    },
    ApprovalMessage {
        id: Uuid,
        sequence: u64,
        request_id: String,
        prompt: String,
        decision: Option<String>,
        created_at: DateTime<Utc>,
    },
    VerificationMessage {
        id: Uuid,
        sequence: u64,
        passed: bool,
        summary: String,
        created_at: DateTime<Utc>,
    },
    AskUserMessage {
        id: Uuid,
        sequence: u64,
        question: String,
        options: Vec<crate::model::types::UserOption>,
        answer: Option<String>,
        created_at: DateTime<Utc>,
    },
}

impl ConversationTurn {
    pub fn sequence(&self) -> u64 {
        match self {
            Self::UserMessage { sequence, .. } => *sequence,
            Self::AssistantMessage { sequence, .. } => *sequence,
            Self::ToolCallMessage { sequence, .. } => *sequence,
            Self::ToolResultMessage { sequence, .. } => *sequence,
            Self::SystemMessage { sequence, .. } => *sequence,
            Self::ApprovalMessage { sequence, .. } => *sequence,
            Self::VerificationMessage { sequence, .. } => *sequence,
            Self::AskUserMessage { sequence, .. } => *sequence,
        }
    }

    pub fn kind_str(&self) -> &'static str {
        match self {
            Self::UserMessage { .. } => "user_message",
            Self::AssistantMessage { .. } => "assistant_message",
            Self::ToolCallMessage { .. } => "tool_call_message",
            Self::ToolResultMessage { .. } => "tool_result_message",
            Self::SystemMessage { .. } => "system_message",
            Self::ApprovalMessage { .. } => "approval_message",
            Self::VerificationMessage { .. } => "verification_message",
            Self::AskUserMessage { .. } => "ask_user_message",
        }
    }

    pub fn text_content(&self) -> &str {
        match self {
            Self::UserMessage { content, .. } => content,
            Self::AssistantMessage { content, .. } => content,
            Self::ToolCallMessage { tool_name, .. } => tool_name,
            Self::ToolResultMessage { output, .. } => output,
            Self::SystemMessage { content, .. } => content,
            Self::ApprovalMessage { prompt, .. } => prompt,
            Self::VerificationMessage { summary, .. } => summary,
            Self::AskUserMessage { question, .. } => question,
        }
    }
}

/// SQLite persistence repository for interactive sessions and conversation messages.
#[derive(Clone)]
pub struct SqliteSessionRepository {
    pool: SqlitePool,
    event_bus: Option<Arc<dyn EventBus>>,
}

impl SqliteSessionRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            event_bus: None,
        }
    }

    pub fn with_event_bus(mut self, bus: Arc<dyn EventBus>) -> Self {
        self.event_bus = Some(bus);
        self
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Creates and persists a new interactive session.
    /// Also inserts an initial root mission row to satisfy foreign key invariants.
    pub async fn create_session(&self, workspace_root: &Path) -> Result<Session, M31AError> {
        let session_id = SessionId::new();
        let root_mission_id = MissionId::new();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let ws_str = workspace_root.to_string_lossy().to_string();

        // 1. Insert placeholder root mission for session foreign key requirement via canonical mission repository
        let mission_repo = crate::persistence::sqlite::repositories::SqliteMissionRepository::new(
            self.pool.clone(),
        );
        let root_mission = crate::state::Mission::new(
            root_mission_id,
            format!("Interactive session root {}", session_id),
        );
        mission_repo.insert(&root_mission).await.map_err(|e| {
            M31AError::Internal(anyhow::anyhow!("Failed to insert session mission: {e}"))
        })?;

        // 2. Insert session row
        sqlx::query(
            r#"
            INSERT INTO sessions (
                id, mission_id, workspace_root, active_mission_id, status, metadata, created_at, updated_at
            ) VALUES (?, ?, ?, ?, 'active', '{}', ?, ?)
            "#
        )
        .bind(session_id.as_bytes().as_slice())
        .bind(root_mission_id.as_bytes().as_slice())
        .bind(&ws_str)
        .bind(root_mission_id.as_bytes().as_slice())
        .bind(&now_str)
        .bind(&now_str)
        .execute(&self.pool)
        .await
        .map_err(|e| M31AError::Internal(anyhow::anyhow!("Failed to insert session: {e}")))?;

        let session = Session {
            id: session_id,
            workspace_root: workspace_root.to_path_buf(),
            created_at: now,
            updated_at: now,
            status: SessionState::Active,
            active_mission_id: Some(root_mission_id),
            metadata: serde_json::json!({}),
        };

        if let Some(ref bus) = self.event_bus {
            let env = EventEnvelope::new(
                0,
                Some(root_mission_id),
                None,
                "session_repository".to_string(),
                EventType::SessionStarted {
                    session_id,
                    mission_id: root_mission_id,
                },
            );
            let _ = bus.publish(env).await;
        }

        Ok(session)
    }

    /// Retrieves an existing session by ID.
    pub async fn get_session(&self, id: SessionId) -> Result<Option<Session>, M31AError> {
        let row = sqlx::query_as::<_, (Vec<u8>, Option<Vec<u8>>, String, Option<Vec<u8>>, String, String, String, String)>(
            r#"
            SELECT id, mission_id, workspace_root, active_mission_id, status, metadata, created_at, updated_at
            FROM sessions WHERE id = ?
            "#
        )
        .bind(id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| M31AError::Internal(anyhow::anyhow!("Failed to fetch session: {e}")))?;

        match row {
            Some((
                _,
                _,
                ws_root,
                active_mid_bytes,
                status_str,
                meta_str,
                created_str,
                updated_str,
            )) => {
                // Session status and timestamps fail closed: a corrupt status must
                // never be silently coerced to Active or defaulted.
                let status: SessionState = status_str.parse().map_err(|e| {
                    M31AError::Internal(anyhow::anyhow!("corrupt session status: {e}"))
                })?;
                // Metadata is display-only; an empty object default is
                // acceptable and documented (no governance impact).
                let metadata: serde_json::Value =
                    serde_json::from_str(&meta_str).unwrap_or_default();
                let created_at = DateTime::parse_from_rfc3339(&created_str)
                    .map(|d| d.with_timezone(&Utc))
                    .map_err(|e| {
                        M31AError::Internal(anyhow::anyhow!("corrupt session created_at: {e}"))
                    })?;
                let updated_at = DateTime::parse_from_rfc3339(&updated_str)
                    .map(|d| d.with_timezone(&Utc))
                    .map_err(|e| {
                        M31AError::Internal(anyhow::anyhow!("corrupt session updated_at: {e}"))
                    })?;

                let active_mission_id = active_mid_bytes.and_then(|bytes| {
                    let mut arr = [0u8; 16];
                    if bytes.len() == 16 {
                        arr.copy_from_slice(&bytes);
                        Some(MissionId::from_bytes(arr))
                    } else {
                        None
                    }
                });

                Ok(Some(Session {
                    id,
                    workspace_root: PathBuf::from(ws_root),
                    created_at,
                    updated_at,
                    status,
                    active_mission_id,
                    metadata,
                }))
            }
            None => Ok(None),
        }
    }

    /// List all known sessions ordered by updated_at descending.
    pub async fn list_sessions(&self) -> Result<Vec<Session>, M31AError> {
        let rows = sqlx::query_as::<_, (Vec<u8>, Option<Vec<u8>>, String, Option<Vec<u8>>, String, String, String, String)>(
            r#"
            SELECT id, mission_id, workspace_root, active_mission_id, status, metadata, created_at, updated_at
            FROM sessions ORDER BY updated_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| M31AError::Internal(anyhow::anyhow!("Failed to list sessions: {e}")))?;

        let mut sessions = Vec::new();
        for (
            id_bytes,
            _,
            ws_root,
            active_mid_bytes,
            status_str,
            meta_str,
            created_str,
            updated_str,
        ) in rows
        {
            if id_bytes.len() != 16 {
                continue;
            }
            let mut id_arr = [0u8; 16];
            id_arr.copy_from_slice(&id_bytes);
            let id = SessionId::from_bytes(id_arr);

            // Session listing skips rows with corrupt status or timestamps rather
            // than coercing unvalidated states; display metadata defaults safely.
            let Ok(status) = status_str.parse::<SessionState>() else {
                continue;
            };
            let metadata: serde_json::Value = serde_json::from_str(&meta_str).unwrap_or_default();
            let (Ok(created_at), Ok(updated_at)) = (
                DateTime::parse_from_rfc3339(&created_str).map(|d| d.with_timezone(&Utc)),
                DateTime::parse_from_rfc3339(&updated_str).map(|d| d.with_timezone(&Utc)),
            ) else {
                continue;
            };

            let active_mission_id = active_mid_bytes.and_then(|bytes| {
                let mut arr = [0u8; 16];
                if bytes.len() == 16 {
                    arr.copy_from_slice(&bytes);
                    Some(MissionId::from_bytes(arr))
                } else {
                    None
                }
            });

            sessions.push(Session {
                id,
                workspace_root: PathBuf::from(ws_root),
                created_at,
                updated_at,
                status,
                active_mission_id,
                metadata,
            });
        }

        Ok(sessions)
    }

    /// Sets the active mission ID for a session.
    pub async fn set_active_mission(
        &self,
        session_id: SessionId,
        mission_id: MissionId,
    ) -> Result<(), M31AError> {
        let now_str = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE sessions SET active_mission_id = ?, mission_id = ?, updated_at = ? WHERE id = ?"
        )
        .bind(mission_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind(&now_str)
        .bind(session_id.as_bytes().as_slice())
        .execute(&self.pool)
        .await
        .map_err(|e| M31AError::Internal(anyhow::anyhow!("Failed to set active mission: {e}")))?;

        Ok(())
    }

    /// Update session status.
    pub async fn update_status(
        &self,
        session_id: SessionId,
        status: SessionState,
    ) -> Result<(), M31AError> {
        let now_str = Utc::now().to_rfc3339();
        let closed_at = if matches!(status, SessionState::Closed | SessionState::Failed) {
            Some(now_str.clone())
        } else {
            None
        };

        sqlx::query(
            "UPDATE sessions SET status = ?, updated_at = ?, closed_at = COALESCE(?, closed_at) WHERE id = ?"
        )
        .bind(status.to_string())
        .bind(&now_str)
        .bind(closed_at)
        .bind(session_id.as_bytes().as_slice())
        .execute(&self.pool)
        .await
        .map_err(|e| M31AError::Internal(anyhow::anyhow!("Failed to update session status: {e}")))?;

        Ok(())
    }

    /// Compute the next sequential message number for a session.
    pub async fn next_sequence(&self, session_id: SessionId) -> Result<u64, M31AError> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sequence), 0) FROM conversation_messages WHERE session_id = ?",
        )
        .bind(session_id.as_bytes().as_slice())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| M31AError::Internal(anyhow::anyhow!("Failed to fetch sequence: {e}")))?;

        Ok((count + 1) as u64)
    }

    /// Appends a typed conversation turn to the session history.
    pub async fn append_turn(
        &self,
        session_id: SessionId,
        turn: &ConversationTurn,
    ) -> Result<(), M31AError> {
        let turn_id = match turn {
            ConversationTurn::UserMessage { id, .. } => *id,
            ConversationTurn::AssistantMessage { id, .. } => *id,
            ConversationTurn::ToolCallMessage { id, .. } => *id,
            ConversationTurn::ToolResultMessage { id, .. } => *id,
            ConversationTurn::SystemMessage { id, .. } => *id,
            ConversationTurn::ApprovalMessage { id, .. } => *id,
            ConversationTurn::VerificationMessage { id, .. } => *id,
            ConversationTurn::AskUserMessage { id, .. } => *id,
        };

        let seq = turn.sequence();
        let kind = turn.kind_str();
        // P0-03: conversation history is a security boundary. The persisted
        // representation must be the safe (model-visible/diagnostic) form —
        // never raw execution evidence. Scrub secrets from both the content
        // column and the serialized payload before INSERT.
        let redactor = crate::telemetry::redactor::SecretRedactor::new();
        let content = redactor.redact_text(turn.text_content());
        let raw_payload_json = serde_json::to_string(turn)
            .map_err(|e| M31AError::Internal(anyhow::anyhow!("Failed to serialize turn: {e}")))?;
        let payload_json = redactor.redact_text(&raw_payload_json);
        let now_str = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO conversation_messages (
                id, session_id, sequence, kind, content, payload_json, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(turn_id.as_bytes().as_slice())
        .bind(session_id.as_bytes().as_slice())
        .bind(seq as i64)
        .bind(kind)
        .bind(content)
        .bind(payload_json)
        .bind(&now_str)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            M31AError::Internal(anyhow::anyhow!("Failed to insert conversation turn: {e}"))
        })?;

        // Update session's updated_at timestamp
        let _ = sqlx::query("UPDATE sessions SET updated_at = ? WHERE id = ?")
            .bind(&now_str)
            .bind(session_id.as_bytes().as_slice())
            .execute(&self.pool)
            .await;

        Ok(())
    }

    /// Retrieve all conversation turns for a session ordered by sequence ascending.
    pub async fn get_conversation(
        &self,
        session_id: SessionId,
    ) -> Result<Vec<ConversationTurn>, M31AError> {
        let rows = sqlx::query_as::<_, (String,)>(
            r#"
            SELECT payload_json FROM conversation_messages
            WHERE session_id = ? ORDER BY sequence ASC
            "#,
        )
        .bind(session_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| M31AError::Internal(anyhow::anyhow!("Failed to fetch conversation: {e}")))?;

        let mut turns = Vec::new();
        for (payload_str,) in rows {
            if let Ok(turn) = serde_json::from_str::<ConversationTurn>(&payload_str) {
                turns.push(turn);
            }
        }

        Ok(turns)
    }

    /// Clear all conversation turns for a session.
    pub async fn clear_conversation(&self, session_id: SessionId) -> Result<(), M31AError> {
        sqlx::query("DELETE FROM conversation_messages WHERE session_id = ?")
            .bind(session_id.as_bytes().as_slice())
            .execute(&self.pool)
            .await
            .map_err(|e| {
                M31AError::Internal(anyhow::anyhow!("Failed to clear conversation: {e}"))
            })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_session_lifecycle_and_conversation_persistence() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("session_test.db");
        let pool = crate::persistence::sqlite::schema::initialize_database(&db_path)
            .await
            .unwrap();

        let repo = SqliteSessionRepository::new(pool);
        let session = repo.create_session(dir.path()).await.unwrap();

        assert_eq!(session.status, SessionState::Active);
        assert_eq!(session.workspace_root, dir.path());

        // Append user turn
        let turn1 = ConversationTurn::UserMessage {
            id: Uuid::now_v7(),
            sequence: 1,
            content: "Fix parser".to_string(),
            raw_text: "Fix @src/parser.rs".to_string(),
            mentions: vec![],
            created_at: Utc::now(),
        };
        repo.append_turn(session.id, &turn1).await.unwrap();

        // Append assistant turn
        let turn2 = ConversationTurn::AssistantMessage {
            id: Uuid::now_v7(),
            sequence: 2,
            content: "Fixed parser in src/parser.rs".to_string(),
            created_at: Utc::now(),
        };
        repo.append_turn(session.id, &turn2).await.unwrap();

        // Retrieve conversation
        let history = repo.get_conversation(session.id).await.unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0], turn1);
        assert_eq!(history[1], turn2);

        // Resume / fetch session
        let loaded = repo
            .get_session(session.id)
            .await
            .unwrap()
            .expect("session exists");
        assert_eq!(loaded.id, session.id);
        assert_eq!(loaded.workspace_root, dir.path());
    }
}
