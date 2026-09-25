//! Durable policy audit logging in SQLite (POL-05, D-04).

use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};

use crate::ids::{AgentId, MissionId, PolicyEvaluationId, TaskId};
use crate::kernel::seams::policy::PolicyDecisionContract;
use crate::policy::effective::PolicyDecisionRecord;

/// Hash a JSON argument payload deterministically using SHA-256.
pub fn hash_normalized_arguments(args: &serde_json::Value) -> String {
    let canonical_str = match serde_json::to_string(args) {
        Ok(s) => s,
        Err(_) => args.to_string(),
    };
    let mut hasher = Sha256::new();
    hasher.update(canonical_str.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Durable audit record retrieved from SQLite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyDecisionAuditRow {
    pub id: PolicyEvaluationId,
    pub mission_id: MissionId,
    pub task_id: Option<TaskId>,
    pub agent_id: Option<AgentId>,
    pub tool_call_id: Option<String>,
    pub tool_or_capability: String,
    pub matched_rule_id: Option<String>,
    pub matched_layer: String,
    pub precedence_rank: i64,
    pub decision: String,
    pub normalized_args_hash: String,
    pub resource_scope: String,
    pub authority_source: String,
    pub policy_hash: String,
    pub explanation: String,
    pub created_at: String,
}

/// Durable policy auditor committing pre-execution records to SQLite.
#[derive(Debug, Clone, Default)]
pub struct DurablePolicyAuditor;

impl DurablePolicyAuditor {
    pub fn new() -> Self {
        Self
    }

    /// Record a policy decision durably in SQLite before side effects execute in Stage 9 (POL-05).
    #[allow(clippy::too_many_arguments)]
    pub async fn record_decision(
        &self,
        pool: &SqlitePool,
        mission_id: MissionId,
        task_id: Option<TaskId>,
        agent_id: Option<AgentId>,
        tool_call_id: Option<&str>,
        tool_or_capability: &str,
        record: &PolicyDecisionContract,
        normalized_args: &serde_json::Value,
        resource_scope: &str,
    ) -> Result<PolicyEvaluationId, sqlx::Error> {
        let eval_id = PolicyEvaluationId::new();
        let args_hash = hash_normalized_arguments(normalized_args);
        let created_at = Utc::now().to_rfc3339();

        let rank = record.precedence_rank.unwrap_or(99) as i64;
        let layer_str = record.matched_layer.as_deref().unwrap_or("none");
        let decision_str = format!("{:?}", record.decision).to_lowercase();

        let task_bytes = task_id.map(|t| t.as_bytes().to_vec());
        let agent_bytes = agent_id.map(|a| a.as_bytes().to_vec());

        sqlx::query(
            r#"
            INSERT INTO policy_decisions (
                id,
                mission_id,
                task_id,
                agent_id,
                tool_call_id,
                tool_or_capability,
                matched_rule_id,
                matched_layer,
                precedence_rank,
                decision,
                normalized_args_hash,
                resource_scope,
                authority_source,
                policy_hash,
                explanation,
                created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(eval_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind(task_bytes)
        .bind(agent_bytes)
        .bind(tool_call_id)
        .bind(tool_or_capability)
        .bind(record.matched_rule_id.as_deref())
        .bind(layer_str)
        .bind(rank)
        .bind(decision_str)
        .bind(args_hash)
        .bind(resource_scope)
        .bind(&record.authority_source)
        .bind(&record.policy_version_or_hash)
        .bind(&record.explanation)
        .bind(created_at)
        .execute(pool)
        .await?;

        Ok(eval_id)
    }

    /// Record a domain policy decision record durably in SQLite.
    #[allow(clippy::too_many_arguments)]
    pub async fn record_domain_decision(
        &self,
        pool: &SqlitePool,
        mission_id: MissionId,
        task_id: Option<TaskId>,
        agent_id: Option<AgentId>,
        tool_call_id: Option<&str>,
        tool_or_capability: &str,
        record: &PolicyDecisionRecord,
        normalized_args: &serde_json::Value,
        resource_scope: &str,
    ) -> Result<PolicyEvaluationId, sqlx::Error> {
        let contract: PolicyDecisionContract = record.into();
        self.record_decision(
            pool,
            mission_id,
            task_id,
            agent_id,
            tool_call_id,
            tool_or_capability,
            &contract,
            normalized_args,
            resource_scope,
        )
        .await
    }

    /// Retrieve an audit record by its ID.
    pub async fn get_decision(
        &self,
        pool: &SqlitePool,
        id: PolicyEvaluationId,
    ) -> Result<Option<PolicyDecisionAuditRow>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT
                id, mission_id, task_id, agent_id, tool_call_id,
                tool_or_capability, matched_rule_id, matched_layer,
                precedence_rank, decision, normalized_args_hash,
                resource_scope, authority_source, policy_hash,
                explanation, created_at
            FROM policy_decisions
            WHERE id = ?
            "#,
        )
        .bind(id.as_bytes().as_slice())
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| {
            let id_raw: Vec<u8> = r.get("id");
            let mut id_bytes = [0u8; 16];
            id_bytes.copy_from_slice(&id_raw);

            let mission_raw: Vec<u8> = r.get("mission_id");
            let mut mission_bytes = [0u8; 16];
            mission_bytes.copy_from_slice(&mission_raw);

            let task_id = r.get::<Option<Vec<u8>>, _>("task_id").map(|t| {
                let mut b = [0u8; 16];
                b.copy_from_slice(&t);
                TaskId::from_bytes(b)
            });

            let agent_id = r.get::<Option<Vec<u8>>, _>("agent_id").map(|a| {
                let mut b = [0u8; 16];
                b.copy_from_slice(&a);
                AgentId::from_bytes(b)
            });

            PolicyDecisionAuditRow {
                id: PolicyEvaluationId::from_bytes(id_bytes),
                mission_id: MissionId::from_bytes(mission_bytes),
                task_id,
                agent_id,
                tool_call_id: r.get("tool_call_id"),
                tool_or_capability: r.get("tool_or_capability"),
                matched_rule_id: r.get("matched_rule_id"),
                matched_layer: r.get("matched_layer"),
                precedence_rank: r.get("precedence_rank"),
                decision: r.get("decision"),
                normalized_args_hash: r.get("normalized_args_hash"),
                resource_scope: r.get("resource_scope"),
                authority_source: r.get("authority_source"),
                policy_hash: r.get("policy_hash"),
                explanation: r.get("explanation"),
                created_at: r.get("created_at"),
            }
        }))
    }

    /// List all audit records for a mission sorted by (created_at, id).
    pub async fn list_decisions_for_mission(
        &self,
        pool: &SqlitePool,
        mission_id: MissionId,
    ) -> Result<Vec<PolicyDecisionAuditRow>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, mission_id, task_id, agent_id, tool_call_id,
                tool_or_capability, matched_rule_id, matched_layer,
                precedence_rank, decision, normalized_args_hash,
                resource_scope, authority_source, policy_hash,
                explanation, created_at
            FROM policy_decisions
            WHERE mission_id = ?
            ORDER BY created_at ASC, id ASC
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_all(pool)
        .await?;

        let mut results = Vec::new();
        for r in rows {
            let id_raw: Vec<u8> = r.get("id");
            let mut id_bytes = [0u8; 16];
            id_bytes.copy_from_slice(&id_raw);

            let mission_raw: Vec<u8> = r.get("mission_id");
            let mut mission_bytes = [0u8; 16];
            mission_bytes.copy_from_slice(&mission_raw);

            let task_id = r.get::<Option<Vec<u8>>, _>("task_id").map(|t| {
                let mut b = [0u8; 16];
                b.copy_from_slice(&t);
                TaskId::from_bytes(b)
            });

            let agent_id = r.get::<Option<Vec<u8>>, _>("agent_id").map(|a| {
                let mut b = [0u8; 16];
                b.copy_from_slice(&a);
                AgentId::from_bytes(b)
            });

            results.push(PolicyDecisionAuditRow {
                id: PolicyEvaluationId::from_bytes(id_bytes),
                mission_id: MissionId::from_bytes(mission_bytes),
                task_id,
                agent_id,
                tool_call_id: r.get("tool_call_id"),
                tool_or_capability: r.get("tool_or_capability"),
                matched_rule_id: r.get("matched_rule_id"),
                matched_layer: r.get("matched_layer"),
                precedence_rank: r.get("precedence_rank"),
                decision: r.get("decision"),
                normalized_args_hash: r.get("normalized_args_hash"),
                resource_scope: r.get("resource_scope"),
                authority_source: r.get("authority_source"),
                policy_hash: r.get("policy_hash"),
                explanation: r.get("explanation"),
                created_at: r.get("created_at"),
            });
        }

        Ok(results)
    }
}
