//! Durable mutating-tool deduplication fence.
//!
//! Crash-between-action-and-persistence replays an already-committed mutation
//! as a brand-new side effect (at-least-once). This module provides the
//! durable identity that closes the gap where safe:
//!
//! ```text
//! same committed mutation (task + tool + canonical input hash)
//!   → recognized from the fence
//!   → no duplicate side effect
//! ```
//!
//! Identity is semantic, never timestamp-only. Outcomes distinguish committed
//! success (suppress duplicates) from failed attempts (legitimate retry still
//! proceeds). Recovery is never made impossible: only a recorded *success*
//! suppresses re-execution.
//!
//! Suppression applies exclusively to the idempotent file-mutation allowlist
//! ([`is_fenced_mutating_tool`]). Non-idempotent tools (process execution,
//! test runs, network) always execute; their outcomes are still recorded for
//! audit. Suppressing a repeated `cargo test` would break legitimate
//! verify-after-fix flows, so commands are fenced by record, not by skip.

use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use crate::ids::TaskId;

/// Default staleness threshold in seconds for considering an in-progress claim crashed/stale (5 minutes).
pub const DEFAULT_CLAIM_STALENESS_SECS: i64 = 300;

/// Error types occurring during mutation fence operations.
#[derive(Debug, thiserror::Error)]
pub enum DedupError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Durable persistence failed: {0}")]
    PersistenceFailed(String),
    #[error("Concurrent mutation in progress for task {task_id}, tool {tool_name}")]
    ConcurrentInProgress { task_id: TaskId, tool_name: String },
    #[error("Indeterminate mutation state: {0}")]
    Indeterminate(String),
}

/// Tools whose identical re-execution is byte-for-byte idempotent and hence
/// safe to suppress on a recorded prior success.
pub fn is_fenced_mutating_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "write_file" | "edit_file" | "apply_patch" | "fs.write" | "workspace_fs_write"
    )
}

/// Canonical JSON serialization: object keys sorted recursively so
/// semantically identical inputs hash identically regardless of key order.
pub fn canonical_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => serde_json::to_string(s).unwrap_or_default(),
        serde_json::Value::Array(items) => {
            let parts: Vec<String> = items.iter().map(canonical_json).collect();
            format!("[{}]", parts.join(","))
        }
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let parts: Vec<String> = keys
                .iter()
                .map(|k| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(k).unwrap_or_default(),
                        canonical_json(&map[*k])
                    )
                })
                .collect();
            format!("{{{}}}", parts.join(","))
        }
    }
}

/// Semantic fingerprint for a mutating tool action: task identity, tool name,
/// and canonical input hash. Stable across restarts for identical work.
pub fn fingerprint_mutation(
    task_id: &TaskId,
    tool_name: &str,
    params: &serde_json::Value,
) -> String {
    fingerprint_parts(&[
        task_id.as_bytes(),
        tool_name.as_bytes(),
        canonical_json(params).as_bytes(),
    ])
}

/// Fingerprint over arbitrary byte parts (e.g. Lane B proposal content).
pub fn fingerprint_parts(parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
        hasher.update([0u8]);
    }
    format!("{:x}", hasher.finalize())
}

/// SHA-256 hex of a byte slice (output/evidence hashing for audit + verify).
pub fn sha_hex(data: &[u8]) -> String {
    crate::kernel::hash::sha256_hex(data)
}

/// Fence verdict for an intended mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FenceVerdict {
    /// No recorded success: claim acquired ('in_progress'), execute normally.
    Proceed,
    /// Identical mutation already committed: skip the side effect.
    DuplicateSuppressed { output_hash: Option<String> },
    /// Concurrent identical mutation already in progress (not stale).
    ConcurrentInProgress,
    /// Indeterminate previous execution outcome requiring intervention or explicit retry.
    Indeterminate { details: String },
}

/// Durable deduplication fence backed by the `tool_mutation_fence` table.
///
/// Ensures exact-once execution for mutating tools through durable claim acquisition
/// and atomic outcome commits.
#[derive(Debug, Clone)]
pub struct MutationDedupFence {
    pool: SqlitePool,
}

impl MutationDedupFence {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Check for a prior committed success of this exact mutation without acquiring a claim.
    pub async fn check(
        &self,
        task_id: &TaskId,
        tool_name: &str,
        fingerprint: &str,
    ) -> FenceVerdict {
        let row: Option<(String, Option<String>)> = sqlx::query_as(
            "SELECT outcome, output_hash FROM tool_mutation_fence WHERE task_id = ? AND tool_name = ? AND fingerprint = ?",
        )
        .bind(task_id.as_bytes().as_slice())
        .bind(tool_name)
        .bind(fingerprint)
        .fetch_optional(&self.pool)
        .await
        .unwrap_or(None);
        match row {
            Some((outcome, output_hash)) if outcome == "success" => {
                FenceVerdict::DuplicateSuppressed { output_hash }
            }
            Some((outcome, _)) if outcome == "in_progress" => FenceVerdict::ConcurrentInProgress,
            Some((outcome, _)) if outcome == "indeterminate" => FenceVerdict::Indeterminate {
                details: "Prior outcome ended in indeterminate state".to_string(),
            },
            _ => FenceVerdict::Proceed,
        }
    }

    /// Acquire an explicit execution claim for a mutating tool action.
    ///
    /// Inserts `outcome = 'in_progress'` using the primary key `(task_id, tool_name, fingerprint)`.
    /// On conflict, inspects existing status:
    /// - `success`: returns `DuplicateSuppressed` with previous output hash.
    /// - `failure`: permits retry, resetting to `in_progress`.
    /// - `in_progress`: checks staleness against `stale_after_secs`. If stale, resets to `in_progress`;
    ///   if active, returns `ConcurrentInProgress` to reject concurrent mutations.
    /// - `indeterminate`: if stale, permits retry; if active, returns `Indeterminate`.
    pub async fn acquire_claim(
        &self,
        task_id: &TaskId,
        tool_name: &str,
        fingerprint: &str,
        stale_after_secs: Option<i64>,
    ) -> Result<FenceVerdict, DedupError> {
        let now = chrono::Utc::now();
        let now_str = now.to_rfc3339();

        let insert_res = sqlx::query(
            r#"
            INSERT INTO tool_mutation_fence
                (task_id, tool_name, fingerprint, outcome, output_hash, outcome_json, created_at, updated_at)
            VALUES (?, ?, ?, 'in_progress', NULL, NULL, ?, ?)
            ON CONFLICT(task_id, tool_name, fingerprint) DO NOTHING
            "#,
        )
        .bind(task_id.as_bytes().as_slice())
        .bind(tool_name)
        .bind(fingerprint)
        .bind(&now_str)
        .bind(&now_str)
        .execute(&self.pool)
        .await?;

        if insert_res.rows_affected() == 1 {
            return Ok(FenceVerdict::Proceed);
        }

        // Row already exists - inspect current status
        let row: Option<(String, Option<String>, Option<String>, String)> = sqlx::query_as(
            r#"
            SELECT outcome, output_hash, outcome_json, updated_at
            FROM tool_mutation_fence
            WHERE task_id = ? AND tool_name = ? AND fingerprint = ?
            "#,
        )
        .bind(task_id.as_bytes().as_slice())
        .bind(tool_name)
        .bind(fingerprint)
        .fetch_optional(&self.pool)
        .await?;

        let (outcome, output_hash, _outcome_json, updated_at_str) = match row {
            Some(r) => r,
            None => {
                // If deleted in a concurrent race, return proceed
                return Ok(FenceVerdict::Proceed);
            }
        };

        match outcome.as_str() {
            "success" => Ok(FenceVerdict::DuplicateSuppressed { output_hash }),
            "failure" => {
                // Legitimate retry: reset outcome to in_progress
                let update_res = sqlx::query(
                    r#"
                    UPDATE tool_mutation_fence
                    SET outcome = 'in_progress',
                        output_hash = NULL,
                        outcome_json = NULL,
                        updated_at = ?
                    WHERE task_id = ? AND tool_name = ? AND fingerprint = ? AND outcome = 'failure'
                    "#,
                )
                .bind(&now_str)
                .bind(task_id.as_bytes().as_slice())
                .bind(tool_name)
                .bind(fingerprint)
                .execute(&self.pool)
                .await?;

                if update_res.rows_affected() > 0 {
                    Ok(FenceVerdict::Proceed)
                } else {
                    // State changed concurrently; re-check
                    Box::pin(self.acquire_claim(task_id, tool_name, fingerprint, stale_after_secs))
                        .await
                }
            }
            "in_progress" => {
                let stale_threshold = stale_after_secs.unwrap_or(DEFAULT_CLAIM_STALENESS_SECS);
                let is_stale = chrono::DateTime::parse_from_rfc3339(&updated_at_str)
                    .map(|updated_at| {
                        now.signed_duration_since(updated_at.with_timezone(&chrono::Utc))
                            >= chrono::Duration::seconds(stale_threshold)
                    })
                    .unwrap_or(true);

                if is_stale {
                    // Stale in_progress claim (crashed worker) - recover claim
                    let update_res = sqlx::query(
                        r#"
                        UPDATE tool_mutation_fence
                        SET outcome = 'in_progress',
                            updated_at = ?
                        WHERE task_id = ? AND tool_name = ? AND fingerprint = ? AND outcome = 'in_progress' AND updated_at = ?
                        "#,
                    )
                    .bind(&now_str)
                    .bind(task_id.as_bytes().as_slice())
                    .bind(tool_name)
                    .bind(fingerprint)
                    .bind(&updated_at_str)
                    .execute(&self.pool)
                    .await?;

                    if update_res.rows_affected() > 0 {
                        Ok(FenceVerdict::Proceed)
                    } else {
                        // Concurrent update won
                        Ok(FenceVerdict::ConcurrentInProgress)
                    }
                } else {
                    Ok(FenceVerdict::ConcurrentInProgress)
                }
            }
            "indeterminate" => {
                let stale_threshold = stale_after_secs.unwrap_or(DEFAULT_CLAIM_STALENESS_SECS);
                let is_stale = chrono::DateTime::parse_from_rfc3339(&updated_at_str)
                    .map(|updated_at| {
                        now.signed_duration_since(updated_at.with_timezone(&chrono::Utc))
                            >= chrono::Duration::seconds(stale_threshold)
                    })
                    .unwrap_or(true);

                if is_stale {
                    // Reset indeterminate state on staleness to allow manual/retry recovery
                    let update_res = sqlx::query(
                        r#"
                        UPDATE tool_mutation_fence
                        SET outcome = 'in_progress',
                            updated_at = ?
                        WHERE task_id = ? AND tool_name = ? AND fingerprint = ? AND outcome = 'indeterminate'
                        "#,
                    )
                    .bind(&now_str)
                    .bind(task_id.as_bytes().as_slice())
                    .bind(tool_name)
                    .bind(fingerprint)
                    .execute(&self.pool)
                    .await?;

                    if update_res.rows_affected() > 0 {
                        Ok(FenceVerdict::Proceed)
                    } else {
                        Ok(FenceVerdict::Indeterminate {
                            details: "Concurrent state change on indeterminate claim".to_string(),
                        })
                    }
                } else {
                    Ok(FenceVerdict::Indeterminate {
                        details: format!(
                            "Identical '{}' mutation ended in indeterminate state at {}",
                            tool_name, updated_at_str
                        ),
                    })
                }
            }
            other => Ok(FenceVerdict::Indeterminate {
                details: format!("Unknown outcome '{}' in mutation fence", other),
            }),
        }
    }

    /// Commit the terminal outcome of a tool execution.
    ///
    /// Updates the row with `outcome = 'success'` or `'failure'`.
    /// If persistence fails, records `indeterminate` before returning an actionable error.
    pub async fn commit_outcome(
        &self,
        task_id: &TaskId,
        tool_name: &str,
        fingerprint: &str,
        success: bool,
        output_hash: Option<&str>,
        outcome_json: Option<&str>,
    ) -> Result<(), DedupError> {
        let now = chrono::Utc::now().to_rfc3339();
        let outcome = if success { "success" } else { "failure" };

        let update_res = sqlx::query(
            r#"
            UPDATE tool_mutation_fence
            SET outcome = ?,
                output_hash = ?,
                outcome_json = ?,
                updated_at = ?
            WHERE task_id = ? AND tool_name = ? AND fingerprint = ?
            "#,
        )
        .bind(outcome)
        .bind(output_hash)
        .bind(outcome_json)
        .bind(&now)
        .bind(task_id.as_bytes().as_slice())
        .bind(tool_name)
        .bind(fingerprint)
        .execute(&self.pool)
        .await;

        match update_res {
            Ok(result) => {
                if result.rows_affected() == 0 {
                    // Fallback upsert if no in_progress record exists
                    sqlx::query(
                        r#"
                        INSERT INTO tool_mutation_fence
                            (task_id, tool_name, fingerprint, outcome, output_hash, outcome_json, created_at, updated_at)
                        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                        ON CONFLICT(task_id, tool_name, fingerprint) DO UPDATE SET
                            outcome = excluded.outcome,
                            output_hash = excluded.output_hash,
                            outcome_json = excluded.outcome_json,
                            updated_at = excluded.updated_at
                        "#,
                    )
                    .bind(task_id.as_bytes().as_slice())
                    .bind(tool_name)
                    .bind(fingerprint)
                    .bind(outcome)
                    .bind(output_hash)
                    .bind(outcome_json)
                    .bind(&now)
                    .bind(&now)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| DedupError::PersistenceFailed(e.to_string()))?;
                }
                Ok(())
            }
            Err(err) => {
                // If terminal update fails, mark outcome as indeterminate to avoid silent retry
                let _ = sqlx::query(
                    r#"
                    UPDATE tool_mutation_fence
                    SET outcome = 'indeterminate',
                        updated_at = ?
                    WHERE task_id = ? AND tool_name = ? AND fingerprint = ?
                    "#,
                )
                .bind(&now)
                .bind(task_id.as_bytes().as_slice())
                .bind(tool_name)
                .bind(fingerprint)
                .execute(&self.pool)
                .await;

                Err(DedupError::PersistenceFailed(err.to_string()))
            }
        }
    }

    /// Record an execution outcome. Success suppresses future duplicates;
    /// failure explicitly permits legitimate retry.
    pub async fn record(
        &self,
        task_id: &TaskId,
        tool_name: &str,
        fingerprint: &str,
        success: bool,
        output_hash: Option<&str>,
        outcome_json: Option<&str>,
    ) {
        let _ = self
            .commit_outcome(
                task_id,
                tool_name,
                fingerprint,
                success,
                output_hash,
                outcome_json,
            )
            .await;
    }

    /// Fetch a recorded full outcome (Lane B replay without re-mutation).
    pub async fn recorded_outcome(
        &self,
        task_id: &TaskId,
        tool_name: &str,
        fingerprint: &str,
    ) -> Option<String> {
        sqlx::query_scalar(
            "SELECT outcome_json FROM tool_mutation_fence WHERE task_id = ? AND tool_name = ? AND fingerprint = ? AND outcome = 'success'",
        )
        .bind(task_id.as_bytes().as_slice())
        .bind(tool_name)
        .bind(fingerprint)
        .fetch_optional(&self.pool)
        .await
        .unwrap_or(None)
        .flatten()
    }
}
