//! SQLite repository for Durable Budget Grants (BST-02, D-05).

use sqlx::{Row, SqlitePool};

use crate::budget::grants::BudgetGrant;
use crate::error::M31AError;
use crate::ids::MissionId;
use crate::state::budget::ResourceBudget;

/// SQLite concrete repository for budget grants.
#[derive(Debug, Clone)]
pub struct SqliteBudgetRepository {
    pool: SqlitePool,
}

impl SqliteBudgetRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Record an explicit budget grant to SQLite.
    pub async fn record_grant(&self, grant: &BudgetGrant) -> Result<(), M31AError> {
        let limits_json = serde_json::to_string(&grant.limits).map_err(|e| {
            M31AError::validation(format!("Failed to serialize budget limits: {}", e))
        })?;
        let granted_at_str = grant.granted_at.to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO budget_grants (
                grant_id, mission_id, actor, reason, limits_json, granted_at
            ) VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&grant.grant_id)
        .bind(grant.mission_id.to_string())
        .bind(&grant.actor)
        .bind(&grant.reason)
        .bind(limits_json)
        .bind(granted_at_str)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Retrieve all recorded budget grants for a mission, ordered chronologically.
    pub async fn get_grants(&self, mission_id: &MissionId) -> Result<Vec<BudgetGrant>, M31AError> {
        let rows = sqlx::query(
            r#"
            SELECT grant_id, mission_id, actor, reason, limits_json, granted_at
            FROM budget_grants
            WHERE mission_id = ?
            ORDER BY granted_at ASC
            "#,
        )
        .bind(mission_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut grants = Vec::with_capacity(rows.len());
        for row in rows {
            let grant_id: String = row.get("grant_id");
            let m_str: String = row.get("mission_id");
            let m_id: MissionId = m_str.parse().map_err(|e| {
                M31AError::persistence(format!("Invalid mission_id in budget_grants: {}", e))
            })?;
            let actor: String = row.get("actor");
            let reason: String = row.get("reason");
            let limits_json: String = row.get("limits_json");
            // Corrupt limits fail closed with typed corruption error.
            // Defaulting to `unbounded()` would silently grant unlimited
            // budget — a fail-open security hazard. The writer always
            // serializes a valid ResourceBudget.
            let limits: ResourceBudget = serde_json::from_str(&limits_json)
                .map_err(|e| M31AError::persistence(format!("corrupt budget limits_json: {e}")))?;
            let granted_at_str: String = row.get("granted_at");
            let granted_at = chrono::DateTime::parse_from_rfc3339(&granted_at_str)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| M31AError::persistence(format!("corrupt budget granted_at: {e}")))?;

            grants.push(BudgetGrant {
                grant_id,
                mission_id: m_id,
                actor,
                reason,
                limits,
                granted_at,
            });
        }

        Ok(grants)
    }

    /// Retrieve the most recent budget grant for a mission.
    pub async fn get_latest_grant(
        &self,
        mission_id: &MissionId,
    ) -> Result<Option<BudgetGrant>, M31AError> {
        let grants = self.get_grants(mission_id).await?;
        Ok(grants.into_iter().last())
    }
}
