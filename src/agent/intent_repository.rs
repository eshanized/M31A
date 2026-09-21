//! Durable persistence for Intent State.
//!
//! Provides `SqliteIntentRepository` for storing, loading, and versioning
//! `IntentState` in the session database.
//!
//! Design:
//! - `session_intent_state`: single mutable row per session (current state).
//! - `intent_state_versions`: append-only audit log of every version.
//! - `intent_decision_records`: dedicated decision lifecycle table.

use chrono::Utc;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::agent::intent::{
    IntentDecision, IntentState, deserialize_intent_state, serialize_intent_state,
};
use crate::ids::SessionId;

/// Persistent repository for intent state, backed by SQLite.
pub struct SqliteIntentRepository {
    pool: SqlitePool,
}

impl SqliteIntentRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Upsert the current intent state for a session.
    ///
    /// Atomically:
    /// 1. Writes/replaces `session_intent_state` (current state).
    /// 2. Appends to `intent_state_versions` (audit trail).
    pub async fn save(&self, state: &IntentState) -> Result<(), sqlx::Error> {
        let session_bytes = parse_session_id(&state.session_id)?;
        let intent_json = serialize_intent_state(state)
            .map_err(|e| sqlx::Error::Protocol(format!("Intent serialization error: {e}")))?;

        let now = Utc::now().to_rfc3339();
        let created_at = state.created_at.to_rfc3339();

        // Upsert current state
        sqlx::query(
            r#"
            INSERT INTO session_intent_state
                (session_id, version, raw_prompt, intent_json, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(session_id) DO UPDATE SET
                version = excluded.version,
                raw_prompt = excluded.raw_prompt,
                intent_json = excluded.intent_json,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(session_bytes.as_slice())
        .bind(state.version as i64)
        .bind(&state.raw_prompt)
        .bind(&intent_json)
        .bind(&created_at)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        // Append to audit log
        let version_id = Uuid::now_v7();
        sqlx::query(
            r#"
            INSERT INTO intent_state_versions
                (id, session_id, version, intent_json, change_summary, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(version_id.as_bytes().as_slice())
        .bind(session_bytes.as_slice())
        .bind(state.version as i64)
        .bind(&intent_json)
        .bind::<Option<String>>(None)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Load the current intent state for a session.
    pub async fn load(&self, session_id: SessionId) -> Result<Option<IntentState>, sqlx::Error> {
        let session_bytes = session_id.as_bytes().to_vec();

        let row = sqlx::query("SELECT intent_json FROM session_intent_state WHERE session_id = ?")
            .bind(session_bytes.as_slice())
            .fetch_optional(&self.pool)
            .await?;

        match row {
            None => Ok(None),
            Some(row) => {
                let json: String = row.get("intent_json");
                let state = deserialize_intent_state(&json).map_err(|e| {
                    sqlx::Error::Protocol(format!("Intent deserialization error: {e}"))
                })?;
                Ok(Some(state))
            }
        }
    }

    /// Persist a decision record to the dedicated decision table.
    pub async fn record_decision(
        &self,
        session_id: SessionId,
        decision: &IntentDecision,
    ) -> Result<(), sqlx::Error> {
        let session_bytes = session_id.as_bytes().to_vec();
        let decision_id = Uuid::now_v7();
        let options_json = serde_json::to_string(&decision.options_considered)
            .unwrap_or_else(|_| "[]".to_string());
        let now = Utc::now().to_rfc3339();

        let (resolution_type, resolution_json) = if let Some(res) = &decision.resolution {
            let json = serde_json::to_string(res).unwrap_or_default();
            let rtype = match res {
                crate::agent::intent::DecisionResolution::UserSelected { .. } => "user_selected",
                crate::agent::intent::DecisionResolution::RepositoryDerived { .. } => {
                    "repository_derived"
                }
                crate::agent::intent::DecisionResolution::PolicyForced { .. } => "policy_forced",
                crate::agent::intent::DecisionResolution::ModelInferred { .. } => "model_inferred",
            };
            (Some(rtype.to_string()), Some(json))
        } else {
            (None, None)
        };

        let resolved_at = if decision.resolution.is_some() {
            Some(now.clone())
        } else {
            None
        };

        sqlx::query(
            r#"
            INSERT OR IGNORE INTO intent_decision_records
                (id, session_id, decision_id, question, options_json,
                 consequence_rationale, resolution_type, resolution_json,
                 created_at, resolved_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(decision_id.as_bytes().as_slice())
        .bind(session_bytes.as_slice())
        .bind(&decision.id)
        .bind(&decision.question)
        .bind(&options_json)
        .bind(&decision.consequence_rationale)
        .bind(resolution_type)
        .bind(resolution_json)
        .bind(&now)
        .bind(resolved_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// List all intent state version numbers for a session (for audit).
    pub async fn list_versions(&self, session_id: SessionId) -> Result<Vec<i64>, sqlx::Error> {
        let session_bytes = session_id.as_bytes().to_vec();
        let rows = sqlx::query(
            "SELECT version FROM intent_state_versions WHERE session_id = ? ORDER BY version ASC",
        )
        .bind(session_bytes.as_slice())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| r.get::<i64, _>("version")).collect())
    }
}

fn parse_session_id(session_str: &str) -> Result<Vec<u8>, sqlx::Error> {
    // SessionId may be formatted as UUID; try to parse as bytes
    if let Ok(uuid) = uuid::Uuid::parse_str(session_str) {
        return Ok(uuid.as_bytes().to_vec());
    }
    // Fallback: use raw bytes of the string
    Ok(session_str.as_bytes().to_vec())
}
