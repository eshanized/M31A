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
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Fence verdict for an intended mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FenceVerdict {
    /// No recorded success: execute normally.
    Proceed,
    /// Identical mutation already committed: skip the side effect.
    DuplicateSuppressed { output_hash: Option<String> },
}

/// Durable deduplication fence backed by the `tool_mutation_fence` table.
///
/// Fence failures never fail the primary flow: check errors resolve to
/// `Proceed`, record errors are swallowed by callers after logging. A broken
/// fence degrades to at-least-once execution, never to a halt.
#[derive(Debug, Clone)]
pub struct MutationDedupFence {
    pool: SqlitePool,
}

impl MutationDedupFence {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Check for a prior committed success of this exact mutation.
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
            _ => FenceVerdict::Proceed,
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
        let now = chrono::Utc::now().to_rfc3339();
        let outcome = if success { "success" } else { "failure" };
        let _ = sqlx::query(
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
