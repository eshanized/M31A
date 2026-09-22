//! Change provenance and requirement traceability store.
//!
//! Enforces:
//! - Every mutation remains associated with: mission, requirement, task, change proposal.
//! - Answers:
//!   "Which changes implement this requirement?"
//!   "Which requirement authorized this change?"
//!   "What changed in this file and was it verified?"

use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use std::sync::RwLock;
use uuid::Uuid;

use crate::ids::{MissionId, TaskId};
use crate::kernel::change::{ChangeProposalId, ChangeProvenanceRecord};

/// Authoritative provenance store supporting dual SQLite persistence and memory cache.
pub struct ChangeProvenanceStore {
    cache: RwLock<HashMap<ChangeProposalId, ChangeProvenanceRecord>>,
}

impl Default for ChangeProvenanceStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ChangeProvenanceStore {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Record a completed change provenance record into memory cache and optional SQLite database.
    pub async fn record(
        &self,
        record: &ChangeProvenanceRecord,
        pool: Option<&SqlitePool>,
    ) -> Result<(), sqlx::Error> {
        // 1. In-memory cache update
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(record.proposal_id, record.clone());
        }

        // 2. Durable SQLite insertion if pool provided
        if let Some(p) = pool {
            let id = Uuid::now_v7();
            let req_keys_json = serde_json::to_string(&record.requirement_keys)
                .unwrap_or_else(|_| "[]".to_string());
            let affected_files_json =
                serde_json::to_string(&record.affected_files).unwrap_or_else(|_| "[]".to_string());
            let affected_symbols_json = serde_json::to_string(&record.affected_symbols)
                .unwrap_or_else(|_| "[]".to_string());

            sqlx::query(
                r#"
                INSERT INTO change_provenance (
                    id, proposal_id, task_id, mission_id, requirement_keys,
                    affected_files, affected_symbols, pre_mutation_hash, post_mutation_hash,
                    diff_summary, verification_passed, created_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(id.as_bytes().as_slice())
            .bind(record.proposal_id.as_bytes().as_slice())
            .bind(record.task_id.as_bytes().as_slice())
            .bind(record.mission_id.as_bytes().as_slice())
            .bind(req_keys_json)
            .bind(affected_files_json)
            .bind(affected_symbols_json)
            .bind(&record.pre_mutation_hash)
            .bind(&record.post_mutation_hash)
            .bind(&record.diff_summary)
            .bind(if record.verification_passed { 1 } else { 0 })
            .bind(record.timestamp.to_rfc3339())
            .execute(p)
            .await?;
        }

        Ok(())
    }

    /// Retrieve provenance record for a specific proposal ID.
    pub async fn get_by_proposal(
        &self,
        proposal_id: ChangeProposalId,
        pool: Option<&SqlitePool>,
    ) -> Result<Option<ChangeProvenanceRecord>, sqlx::Error> {
        if let Ok(cache) = self.cache.read()
            && let Some(r) = cache.get(&proposal_id)
        {
            return Ok(Some(r.clone()));
        }

        if let Some(p) = pool {
            let row = sqlx::query(
                r#"
                SELECT proposal_id, task_id, mission_id, requirement_keys, affected_files,
                       affected_symbols, pre_mutation_hash, post_mutation_hash,
                       diff_summary, verification_passed, created_at
                FROM change_provenance
                WHERE proposal_id = ?
                "#,
            )
            .bind(proposal_id.as_bytes().as_slice())
            .fetch_optional(p)
            .await?;

            if let Some(r) = row {
                return Ok(Some(Self::decode_row(&r)?));
            }
        }

        Ok(None)
    }

    /// List all changes that were authorized by a given requirement key.
    pub async fn list_for_requirement(
        &self,
        requirement_key: &str,
        pool: Option<&SqlitePool>,
    ) -> Result<Vec<ChangeProvenanceRecord>, sqlx::Error> {
        let mut results = Vec::new();

        if let Some(p) = pool {
            let rows = sqlx::query(
                r#"
                SELECT proposal_id, task_id, mission_id, requirement_keys, affected_files,
                       affected_symbols, pre_mutation_hash, post_mutation_hash,
                       diff_summary, verification_passed, created_at
                FROM change_provenance
                ORDER BY created_at ASC
                "#,
            )
            .fetch_all(p)
            .await?;

            for r in rows {
                let rec = Self::decode_row(&r)?;
                if rec.requirement_keys.iter().any(|k| k == requirement_key) {
                    results.push(rec);
                }
            }
        } else if let Ok(cache) = self.cache.read() {
            for rec in cache.values() {
                if rec.requirement_keys.iter().any(|k| k == requirement_key) {
                    results.push(rec.clone());
                }
            }
        }

        Ok(results)
    }

    /// List all changes that modified a specific file path.
    pub async fn list_for_file(
        &self,
        file_path: &str,
        pool: Option<&SqlitePool>,
    ) -> Result<Vec<ChangeProvenanceRecord>, sqlx::Error> {
        let mut results = Vec::new();
        let clean = file_path
            .trim()
            .trim_start_matches('@')
            .trim_start_matches("./");

        if let Some(p) = pool {
            let rows = sqlx::query(
                r#"
                SELECT proposal_id, task_id, mission_id, requirement_keys, affected_files,
                       affected_symbols, pre_mutation_hash, post_mutation_hash,
                       diff_summary, verification_passed, created_at
                FROM change_provenance
                ORDER BY created_at ASC
                "#,
            )
            .fetch_all(p)
            .await?;

            for r in rows {
                let rec = Self::decode_row(&r)?;
                if rec.affected_files.iter().any(|f| {
                    let f_clean = f.trim().trim_start_matches('@').trim_start_matches("./");
                    f_clean == clean
                }) {
                    results.push(rec);
                }
            }
        } else if let Ok(cache) = self.cache.read() {
            for rec in cache.values() {
                if rec.affected_files.iter().any(|f| {
                    let f_clean = f.trim().trim_start_matches('@').trim_start_matches("./");
                    f_clean == clean
                }) {
                    results.push(rec.clone());
                }
            }
        }

        Ok(results)
    }

    fn decode_row(r: &sqlx::sqlite::SqliteRow) -> Result<ChangeProvenanceRecord, sqlx::Error> {
        let prop_bytes: Vec<u8> = r.get("proposal_id");
        let prop_uuid =
            Uuid::from_slice(&prop_bytes).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;

        let task_bytes: Vec<u8> = r.get("task_id");
        let task_arr: [u8; 16] = task_bytes
            .as_slice()
            .try_into()
            .map_err(|_| sqlx::Error::Protocol("invalid task_id".to_string()))?;
        let task_id = TaskId::from_bytes(task_arr);

        let mission_bytes: Vec<u8> = r.get("mission_id");
        let mission_arr: [u8; 16] = mission_bytes
            .as_slice()
            .try_into()
            .map_err(|_| sqlx::Error::Protocol("invalid mission_id".to_string()))?;
        let mission_id = MissionId::from_bytes(mission_arr);

        let req_json: String = r.get("requirement_keys");
        let requirement_keys: Vec<String> = serde_json::from_str(&req_json).unwrap_or_default();

        let files_json: String = r.get("affected_files");
        let affected_files: Vec<String> = serde_json::from_str(&files_json).unwrap_or_default();

        let sym_json: String = r.get("affected_symbols");
        let affected_symbols: Vec<String> = serde_json::from_str(&sym_json).unwrap_or_default();

        let pre_mutation_hash: String = r.get("pre_mutation_hash");
        let post_mutation_hash: String = r.get("post_mutation_hash");
        let diff_summary: String = r.get("diff_summary");
        let ver_passed_int: i64 = r.get("verification_passed");

        let created_str: String = r.get("created_at");
        let timestamp = DateTime::parse_from_rfc3339(&created_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;

        Ok(ChangeProvenanceRecord {
            proposal_id: ChangeProposalId(prop_uuid),
            task_id,
            mission_id,
            requirement_keys,
            affected_files,
            affected_symbols,
            pre_mutation_hash,
            post_mutation_hash,
            diff_summary,
            verification_passed: ver_passed_int != 0,
            timestamp,
        })
    }
}
