//! Durable budget ledger.
//!
//! The [`BudgetEnforcer`] is intentionally synchronous and in-memory, which
//! makes a restart forget consumption and re-admit over-consumed budgets
//! (fail-open). This ledger persists admission-critical counters per mission
//! on every settlement and hydrates a fresh enforcer on startup:
//!
//! ```text
//! before crash:  remaining budget = X
//! after restart: remaining budget = X
//! ```
//!
//! Only admission-critical consumption is persisted (tokens, cost, artifact
//! bytes, steps, calls, retries). Reserved amounts are transient by design:
//! a crash releases reservations (work never started cannot be charged), and
//! the pause/escalate paths already release before halting. Transient
//! presentation counters are deliberately not persisted.

use sqlx::SqlitePool;

use crate::budget::enforcer::BudgetEnforcer;
use crate::ids::MissionId;

/// Durable per-mission budget consumption ledger.
#[derive(Debug, Clone)]
pub struct BudgetLedger {
    pool: SqlitePool,
}

/// Hydrated ledger row: authoritative consumption plus estimated
/// provenance. `None` estimated columns predate the provenance split and
/// hydrate as zero (fully-authoritative legacy rows).
type LedgerRow = (i64, i64, i64, i64, i64, i64, Option<i64>, Option<i64>);

impl BudgetLedger {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Record current enforcer consumption for a mission (upsert).
    ///
    /// Persists BOTH authoritative and estimated consumption: a restart must
    /// restore the full accounted totals, or estimated usage would silently
    /// stop counting against admission after a crash.
    pub async fn record(
        &self,
        mission_id: MissionId,
        enforcer: &BudgetEnforcer,
    ) -> Result<(), sqlx::Error> {
        let snap = enforcer.snapshot();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO budget_ledger (
                mission_id, consumed_tokens, consumed_cost_microcents,
                consumed_artifact_bytes, steps_consumed, calls_consumed,
                retries_consumed, estimated_tokens, estimated_cost_microcents,
                updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(mission_id) DO UPDATE SET
                consumed_tokens = excluded.consumed_tokens,
                consumed_cost_microcents = excluded.consumed_cost_microcents,
                consumed_artifact_bytes = excluded.consumed_artifact_bytes,
                steps_consumed = excluded.steps_consumed,
                calls_consumed = excluded.calls_consumed,
                retries_consumed = excluded.retries_consumed,
                estimated_tokens = excluded.estimated_tokens,
                estimated_cost_microcents = excluded.estimated_cost_microcents,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .bind(snap.tokens_consumed as i64)
        .bind((snap.cost_consumed_usd * 100_000_000.0) as i64)
        .bind(snap.artifact_bytes_consumed as i64)
        .bind(snap.agent_steps_consumed as i64)
        .bind(snap.model_calls_consumed as i64)
        .bind(snap.retries_consumed as i64)
        .bind(snap.tokens_consumed_estimated as i64)
        .bind((snap.cost_consumed_estimated_usd * 100_000_000.0) as i64)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Record inside an existing transaction (atomic with sibling writes).
    ///
    /// Participates in the same logical transaction as task execution state:
    /// settlement and the task state transition commit together, so memory
    /// and database can never diverge into "consumed here, unused there".
    pub async fn record_tx(
        &self,
        mission_id: MissionId,
        enforcer: &BudgetEnforcer,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ) -> Result<(), sqlx::Error> {
        let snap = enforcer.snapshot();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO budget_ledger (
                mission_id, consumed_tokens, consumed_cost_microcents,
                consumed_artifact_bytes, steps_consumed, calls_consumed,
                retries_consumed, estimated_tokens, estimated_cost_microcents,
                updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(mission_id) DO UPDATE SET
                consumed_tokens = excluded.consumed_tokens,
                consumed_cost_microcents = excluded.consumed_cost_microcents,
                consumed_artifact_bytes = excluded.consumed_artifact_bytes,
                steps_consumed = excluded.steps_consumed,
                calls_consumed = excluded.calls_consumed,
                retries_consumed = excluded.retries_consumed,
                estimated_tokens = excluded.estimated_tokens,
                estimated_cost_microcents = excluded.estimated_cost_microcents,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .bind(snap.tokens_consumed as i64)
        .bind((snap.cost_consumed_usd * 100_000_000.0) as i64)
        .bind(snap.artifact_bytes_consumed as i64)
        .bind(snap.agent_steps_consumed as i64)
        .bind(snap.model_calls_consumed as i64)
        .bind(snap.retries_consumed as i64)
        .bind(snap.tokens_consumed_estimated as i64)
        .bind((snap.cost_consumed_estimated_usd * 100_000_000.0) as i64)
        .bind(&now)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    /// Hydrate a fresh enforcer from the durable ledger. Returns `true` when
    /// a ledger row existed (counters restored), `false` for missions that
    /// never recorded (nothing to restore). Must only target a fresh
    /// enforcer: restoring onto live counters would double-count.
    ///
    /// Restores estimated consumption into the estimated counters, preserving
    /// provenance: post-restart admission denies exactly as pre-crash.
    pub async fn hydrate(
        &self,
        mission_id: MissionId,
        enforcer: &BudgetEnforcer,
    ) -> Result<bool, sqlx::Error> {
        let row: Option<LedgerRow> = sqlx::query_as(
            r#"
            SELECT consumed_tokens, consumed_cost_microcents,
                    consumed_artifact_bytes, steps_consumed, calls_consumed,
                    retries_consumed, estimated_tokens, estimated_cost_microcents
            FROM budget_ledger WHERE mission_id = ?
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await?;
        match row {
            None => Ok(false),
            Some((tokens, cost_mc, artifact_bytes, steps, calls, retries, est_tok, est_cost)) => {
                enforcer.restore_consumed(
                    tokens.max(0) as u64,
                    cost_mc.max(0) as u64,
                    artifact_bytes.max(0) as u64,
                    steps.max(0) as usize,
                    calls.max(0) as usize,
                    retries.max(0) as usize,
                    est_tok.unwrap_or(0).max(0) as u64,
                    est_cost.unwrap_or(0).max(0) as u64,
                );
                Ok(true)
            }
        }
    }
}
