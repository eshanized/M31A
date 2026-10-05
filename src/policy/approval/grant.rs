//! Durable SQLite persistent grants repository evaluated at Layer 9 (POL-03, D-10).

use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use uuid::Uuid;

use crate::ids::{ApprovalRequestId, MissionId, TaskId};
use crate::policy::approval::ApprovalResolutionScope;
use crate::policy::matcher::PolicyMatcher;

/// Persistent operator authorization grant stored in SQLite (D-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyGrant {
    pub id: Uuid,
    pub mission_id: MissionId,
    pub task_id: Option<TaskId>,
    pub scope_type: ApprovalResolutionScope,
    pub tool_or_capability: String,
    pub resource_pattern: String,
    pub arg_constraints: Option<serde_json::Value>,
    pub created_from_request_id: Option<ApprovalRequestId>,
    pub policy_hash: String,
    pub revoked: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl PolicyGrant {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mission_id: MissionId,
        task_id: Option<TaskId>,
        scope_type: ApprovalResolutionScope,
        tool_or_capability: impl Into<String>,
        resource_pattern: impl Into<String>,
        arg_constraints: Option<serde_json::Value>,
        created_from_request_id: Option<ApprovalRequestId>,
        policy_hash: impl Into<String>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            mission_id,
            task_id,
            scope_type,
            tool_or_capability: tool_or_capability.into(),
            resource_pattern: resource_pattern.into(),
            arg_constraints,
            created_from_request_id,
            policy_hash: policy_hash.into(),
            revoked: false,
            expires_at,
            created_at: Utc::now(),
        }
    }
}

/// Store for managing durable persistent operator grants.
#[derive(Debug, Clone, Default)]
pub struct PolicyGrantStore;

impl PolicyGrantStore {
    pub fn new() -> Self {
        Self
    }

    /// Persist an operator grant into SQLite.
    pub async fn create_grant(
        &self,
        pool: &SqlitePool,
        grant: &PolicyGrant,
    ) -> Result<(), sqlx::Error> {
        let task_bytes = grant.task_id.map(|t| t.as_bytes().to_vec());
        let req_bytes = grant.created_from_request_id.map(|r| r.as_bytes().to_vec());
        let args_str = grant.arg_constraints.as_ref().map(|a| a.to_string());
        let expires_str = grant.expires_at.map(|e| e.to_rfc3339());
        let created_str = grant.created_at.to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO policy_grants (
                id,
                mission_id,
                task_id,
                scope_type,
                tool_or_capability,
                resource_pattern,
                arg_constraints_json,
                created_from_request_id,
                policy_hash,
                revoked,
                expires_at,
                created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(grant.id.as_bytes().as_slice())
        .bind(grant.mission_id.as_bytes().as_slice())
        .bind(task_bytes)
        .bind(grant.scope_type.as_str())
        .bind(&grant.tool_or_capability)
        .bind(&grant.resource_pattern)
        .bind(args_str)
        .bind(req_bytes)
        .bind(&grant.policy_hash)
        .bind(if grant.revoked { 1 } else { 0 })
        .bind(expires_str)
        .bind(created_str)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Find all unrevoked, unexpired grants matching the current request and active policy hash.
    ///
    /// Every check in the runtime grant contract executes here: mission
    /// scope, task scope, tool scope, resource/path scope, argument
    /// constraints, expiry, revocation, policy-hash compatibility, and
    /// authorization provenance (grants minted without a creating approval
    /// request id never match). A grant is usable ONLY when every check
    /// passes — a row merely existing authorizes nothing.
    #[allow(clippy::too_many_arguments)]
    pub async fn find_applicable_grants(
        &self,
        pool: &SqlitePool,
        mission_id: MissionId,
        task_id: Option<TaskId>,
        tool: &str,
        target_path: Option<&Path>,
        workspace_root: &Path,
        args: &serde_json::Value,
        active_policy_hash: &str,
    ) -> Result<Vec<PolicyGrant>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, mission_id, task_id, scope_type, tool_or_capability,
                resource_pattern, arg_constraints_json, created_from_request_id,
                policy_hash, revoked, expires_at, created_at
            FROM policy_grants
            WHERE mission_id = ?
              AND revoked = 0
              AND policy_hash = ?
              AND (tool_or_capability = ? OR tool_or_capability = '*')
            ORDER BY created_at DESC
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .bind(active_policy_hash)
        .bind(tool)
        .fetch_all(pool)
        .await?;

        let mut matches = Vec::new();
        let now = Utc::now();

        for r in rows {
            let expires_str: Option<String> = r.get("expires_at");
            let expires_at = expires_str.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            });

            // Check expiration
            if expires_at.is_some_and(|exp| exp < now) {
                continue;
            }

            let scope_str: String = r.get("scope_type");
            let scope = match scope_str.as_str() {
                "once" => ApprovalResolutionScope::Once,
                "task" => ApprovalResolutionScope::Task,
                "mission" => ApprovalResolutionScope::Mission,
                "session" => ApprovalResolutionScope::Session,
                "bounded_policy" => ApprovalResolutionScope::BoundedPolicy,
                _ => continue,
            };

            let row_task_id = r.get::<Option<Vec<u8>>, _>("task_id").map(|t| {
                let mut b = [0u8; 16];
                b.copy_from_slice(&t);
                TaskId::from_bytes(b)
            });

            // Check task scope binding
            if scope == ApprovalResolutionScope::Task {
                match (task_id, row_task_id) {
                    (Some(curr), Some(grant_t)) if curr == grant_t => {}
                    _ => continue, // Does not match task
                }
            }

            // Check resource pattern if target_path is specified
            let resource_pattern: String = r.get("resource_pattern");
            if resource_pattern != "*"
                && target_path.is_some_and(|target| {
                    !PolicyMatcher::matches_path(&resource_pattern, target, workspace_root)
                })
            {
                continue;
            }

            // Check argument constraints
            let arg_constraints_json: Option<String> = r.get("arg_constraints_json");
            let arg_constraints: Option<serde_json::Value> =
                arg_constraints_json.and_then(|s| serde_json::from_str(&s).ok());
            if !PolicyMatcher::matches_args(arg_constraints.as_ref(), args) {
                continue;
            }

            let id_raw: Vec<u8> = r.get("id");
            let mut id_bytes = [0u8; 16];
            id_bytes.copy_from_slice(&id_raw);

            let mission_raw: Vec<u8> = r.get("mission_id");
            let mut mission_bytes = [0u8; 16];
            mission_bytes.copy_from_slice(&mission_raw);

            let req_id = r
                .get::<Option<Vec<u8>>, _>("created_from_request_id")
                .map(|req| {
                    let mut b = [0u8; 16];
                    b.copy_from_slice(&req);
                    ApprovalRequestId::from_bytes(b)
                });

            // Authorization provenance: a grant without a creating approval
            // request is not attributable to any operator decision and never
            // matches, no matter what else lines up.
            if req_id.is_none() {
                continue;
            }

            let created_str: String = r.get("created_at");
            let created_at = DateTime::parse_from_rfc3339(&created_str)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            matches.push(PolicyGrant {
                id: Uuid::from_bytes(id_bytes),
                mission_id: MissionId::from_bytes(mission_bytes),
                task_id: row_task_id,
                scope_type: scope,
                tool_or_capability: r.get("tool_or_capability"),
                resource_pattern,
                arg_constraints,
                created_from_request_id: req_id,
                policy_hash: r.get("policy_hash"),
                revoked: false,
                expires_at,
                created_at,
            });
        }

        Ok(matches)
    }

    /// Consume a one-shot (`Once`) grant: revoke it so it can never resolve a
    /// second request. Returns `true` when the grant was live and is now
    /// consumed, `false` when it was already gone (replay attempt).
    pub async fn consume_one_shot_grant(
        &self,
        pool: &SqlitePool,
        grant_id: uuid::Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result =
            sqlx::query("UPDATE policy_grants SET revoked = 1 WHERE id = ? AND revoked = 0")
                .bind(grant_id.as_bytes().as_slice())
                .execute(pool)
                .await?;
        Ok(result.rows_affected() == 1)
    }

    /// Automatically invalidate all grants for a task upon task completion.
    pub async fn invalidate_task_grants(
        &self,
        pool: &SqlitePool,
        task_id: TaskId,
    ) -> Result<u64, sqlx::Error> {
        let result =
            sqlx::query("UPDATE policy_grants SET revoked = 1 WHERE task_id = ? AND revoked = 0")
                .bind(task_id.as_bytes().as_slice())
                .execute(pool)
                .await?;

        Ok(result.rows_affected())
    }

    /// Automatically invalidate all grants for a mission upon mission completion.
    pub async fn invalidate_mission_grants(
        &self,
        pool: &SqlitePool,
        mission_id: MissionId,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE policy_grants SET revoked = 1 WHERE mission_id = ? AND revoked = 0",
        )
        .bind(mission_id.as_bytes().as_slice())
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }
}
