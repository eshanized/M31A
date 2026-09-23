//! SQLite repository implementation for Long-Horizon Engineering Memory.
//!
//! Reuses canonical SqlitePool without introducing any second database or duplicate storage.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;
use uuid::Uuid;

use crate::ids::{CheckId, MissionId, TaskId};
use crate::memory::types::{
    AssumptionStatus, DecisionStatus, EngineeringAssumption, EngineeringDecision,
    FailureDiagnosisRecord, FileHashRecord, MemoryScope, RepairStatus, ReviewFindingRecord,
    ReviewFindingSeverity, ReviewFindingStatus, VerificationRecord, VerificationValidity,
};

/// Trait defining canonical engineering memory queries and mutations.
#[async_trait]
pub trait EngineeringMemoryStore: Send + Sync {
    async fn save_decision(&self, decision: &EngineeringDecision) -> Result<(), sqlx::Error>;
    async fn get_decision(&self, id: &str) -> Result<Option<EngineeringDecision>, sqlx::Error>;
    async fn list_decisions(
        &self,
        scope: Option<MemoryScope>,
        mission_id: Option<MissionId>,
        status: Option<DecisionStatus>,
    ) -> Result<Vec<EngineeringDecision>, sqlx::Error>;
    async fn supersede_decision(
        &self,
        old_id: &str,
        new_decision: &EngineeringDecision,
    ) -> Result<(), sqlx::Error>;

    async fn save_assumption(&self, assumption: &EngineeringAssumption) -> Result<(), sqlx::Error>;
    async fn get_assumption(&self, id: Uuid) -> Result<Option<EngineeringAssumption>, sqlx::Error>;
    async fn update_assumption_status(
        &self,
        id: Uuid,
        status: AssumptionStatus,
        evidence: Option<&str>,
        invalidated_by: Option<&str>,
    ) -> Result<(), sqlx::Error>;
    async fn list_assumptions(
        &self,
        mission_id: MissionId,
        task_id: Option<TaskId>,
        status: Option<AssumptionStatus>,
    ) -> Result<Vec<EngineeringAssumption>, sqlx::Error>;

    async fn save_failure_diagnosis(
        &self,
        diagnosis: &FailureDiagnosisRecord,
    ) -> Result<(), sqlx::Error>;
    async fn get_failure_diagnosis(
        &self,
        id: Uuid,
    ) -> Result<Option<FailureDiagnosisRecord>, sqlx::Error>;
    async fn find_diagnosis_by_signature(
        &self,
        mission_id: MissionId,
        signature: &str,
    ) -> Result<Option<FailureDiagnosisRecord>, sqlx::Error>;
    async fn update_repair_status(&self, id: Uuid, status: RepairStatus)
    -> Result<(), sqlx::Error>;
    async fn increment_recurrence(&self, id: Uuid) -> Result<(), sqlx::Error>;
    async fn list_diagnoses(
        &self,
        mission_id: MissionId,
        task_id: Option<TaskId>,
    ) -> Result<Vec<FailureDiagnosisRecord>, sqlx::Error>;

    async fn save_review_finding(&self, finding: &ReviewFindingRecord) -> Result<(), sqlx::Error>;
    async fn get_review_finding(
        &self,
        id: Uuid,
    ) -> Result<Option<ReviewFindingRecord>, sqlx::Error>;
    async fn update_review_finding_status(
        &self,
        id: Uuid,
        status: ReviewFindingStatus,
        rationale: Option<&str>,
        resolved_by: Option<&str>,
    ) -> Result<(), sqlx::Error>;
    async fn list_review_findings(
        &self,
        mission_id: MissionId,
        task_id: Option<TaskId>,
        status: Option<ReviewFindingStatus>,
    ) -> Result<Vec<ReviewFindingRecord>, sqlx::Error>;

    async fn save_verification_record(
        &self,
        record: &VerificationRecord,
    ) -> Result<(), sqlx::Error>;
    async fn get_verification_record(
        &self,
        id: Uuid,
    ) -> Result<Option<VerificationRecord>, sqlx::Error>;
    async fn find_latest_verification_for_requirement(
        &self,
        mission_id: MissionId,
        requirement_key: &str,
    ) -> Result<Option<VerificationRecord>, sqlx::Error>;
    async fn invalidate_verification(
        &self,
        id: Uuid,
        validity: VerificationValidity,
    ) -> Result<(), sqlx::Error>;
    async fn list_verification_records(
        &self,
        mission_id: MissionId,
        task_id: Option<TaskId>,
    ) -> Result<Vec<VerificationRecord>, sqlx::Error>;
}

/// SQLite-backed engineering memory repository.
#[derive(Debug, Clone)]
pub struct SqliteEngineeringMemoryRepository {
    pool: SqlitePool,
}

impl SqliteEngineeringMemoryRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[async_trait]
impl EngineeringMemoryStore for SqliteEngineeringMemoryRepository {
    async fn save_decision(&self, decision: &EngineeringDecision) -> Result<(), sqlx::Error> {
        let alts_json = serde_json::to_string(&decision.alternatives_considered)
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
        let cons_json = serde_json::to_string(&decision.consequences)
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
        let reqs_json = serde_json::to_string(&decision.linked_requirements)
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
        let files_json = serde_json::to_string(&decision.linked_files)
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
        let symbols_json = serde_json::to_string(&decision.linked_symbols)
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;

        let mission_bytes = decision.mission_id.map(|m| m.as_bytes().to_vec());

        sqlx::query(
            r#"
            INSERT INTO engineering_decisions (
                id, scope, mission_id, title, context, decision, rationale,
                alternatives_json, consequences_json, status, superseded_by,
                linked_requirements_json, linked_files_json, linked_symbols_json,
                provenance_actor, provenance_reason, version, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                context = excluded.context,
                decision = excluded.decision,
                rationale = excluded.rationale,
                alternatives_json = excluded.alternatives_json,
                consequences_json = excluded.consequences_json,
                status = excluded.status,
                superseded_by = excluded.superseded_by,
                linked_requirements_json = excluded.linked_requirements_json,
                linked_files_json = excluded.linked_files_json,
                linked_symbols_json = excluded.linked_symbols_json,
                provenance_actor = excluded.provenance_actor,
                provenance_reason = excluded.provenance_reason,
                version = excluded.version,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&decision.id)
        .bind(decision.scope.as_str())
        .bind(mission_bytes)
        .bind(&decision.title)
        .bind(&decision.context)
        .bind(&decision.decision)
        .bind(&decision.rationale)
        .bind(alts_json)
        .bind(cons_json)
        .bind(decision.status.to_string())
        .bind(&decision.superseded_by)
        .bind(reqs_json)
        .bind(files_json)
        .bind(symbols_json)
        .bind(&decision.provenance_actor)
        .bind(&decision.provenance_reason)
        .bind(decision.version as i64)
        .bind(decision.created_at.to_rfc3339())
        .bind(decision.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_decision(&self, id: &str) -> Result<Option<EngineeringDecision>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, scope, mission_id, title, context, decision, rationale,
                   alternatives_json, consequences_json, status, superseded_by,
                   linked_requirements_json, linked_files_json, linked_symbols_json,
                   provenance_actor, provenance_reason, version, created_at, updated_at
            FROM engineering_decisions
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => {
                let id: String = r.get("id");
                let scope_str: String = r.get("scope");
                let scope = MemoryScope::from_str(&scope_str).map_err(sqlx::Error::Protocol)?;
                let m_bytes: Option<Vec<u8>> = r.get("mission_id");
                let mission_id = m_bytes.and_then(|b| {
                    if b.len() == 16 {
                        let mut arr = [0u8; 16];
                        arr.copy_from_slice(&b);
                        Some(MissionId::from_bytes(arr))
                    } else {
                        None
                    }
                });
                let title: String = r.get("title");
                let context: String = r.get("context");
                let decision: String = r.get("decision");
                let rationale: String = r.get("rationale");
                let alts_raw: String = r.get("alternatives_json");
                let alternatives_considered: Vec<String> =
                    serde_json::from_str(&alts_raw).unwrap_or_default();
                let cons_raw: String = r.get("consequences_json");
                let consequences: Vec<String> = serde_json::from_str(&cons_raw).unwrap_or_default();
                let status_str: String = r.get("status");
                let status =
                    DecisionStatus::from_str(&status_str).unwrap_or(DecisionStatus::Proposed);
                let superseded_by: Option<String> = r.get("superseded_by");
                let reqs_raw: String = r.get("linked_requirements_json");
                let linked_requirements: Vec<String> =
                    serde_json::from_str(&reqs_raw).unwrap_or_default();
                let files_raw: String = r.get("linked_files_json");
                let linked_files: Vec<String> =
                    serde_json::from_str(&files_raw).unwrap_or_default();
                let symbols_raw: String = r.get("linked_symbols_json");
                let linked_symbols: Vec<String> =
                    serde_json::from_str(&symbols_raw).unwrap_or_default();
                let provenance_actor: String = r.get("provenance_actor");
                let provenance_reason: Option<String> = r.get("provenance_reason");
                let version: i64 = r.get("version");
                let created_str: String = r.get("created_at");
                let updated_str: String = r.get("updated_at");

                let created_at = DateTime::parse_from_rfc3339(&created_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());
                let updated_at = DateTime::parse_from_rfc3339(&updated_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(Some(EngineeringDecision {
                    id,
                    scope,
                    mission_id,
                    title,
                    context,
                    decision,
                    rationale,
                    alternatives_considered,
                    consequences,
                    status,
                    superseded_by,
                    linked_requirements,
                    linked_files,
                    linked_symbols,
                    provenance_actor,
                    provenance_reason,
                    version: version as u32,
                    created_at,
                    updated_at,
                }))
            }
            None => Ok(None),
        }
    }

    async fn list_decisions(
        &self,
        scope: Option<MemoryScope>,
        mission_id: Option<MissionId>,
        status: Option<DecisionStatus>,
    ) -> Result<Vec<EngineeringDecision>, sqlx::Error> {
        let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            r#"
            SELECT id, scope, mission_id, title, context, decision, rationale,
                   alternatives_json, consequences_json, status, superseded_by,
                   linked_requirements_json, linked_files_json, linked_symbols_json,
                   provenance_actor, provenance_reason, version, created_at, updated_at
            FROM engineering_decisions
            WHERE 1=1
            "#,
        );

        if let Some(s) = scope {
            builder.push(" AND scope = ").push_bind(s.as_str());
        }
        if let Some(m) = mission_id {
            builder
                .push(" AND (mission_id = ")
                .push_bind(m.as_bytes().to_vec())
                .push(" OR scope = 'project')");
        }
        if let Some(st) = status {
            builder.push(" AND status = ").push_bind(st.to_string());
        }
        builder.push(" ORDER BY created_at ASC");

        let rows = builder.build().fetch_all(&self.pool).await?;
        let mut results = Vec::with_capacity(rows.len());

        for r in rows {
            let id: String = r.get("id");
            let scope_str: String = r.get("scope");
            let sc = MemoryScope::from_str(&scope_str).map_err(sqlx::Error::Protocol)?;
            let m_bytes: Option<Vec<u8>> = r.get("mission_id");
            let mid = m_bytes.and_then(|b| {
                if b.len() == 16 {
                    let mut arr = [0u8; 16];
                    arr.copy_from_slice(&b);
                    Some(MissionId::from_bytes(arr))
                } else {
                    None
                }
            });
            let title: String = r.get("title");
            let context: String = r.get("context");
            let decision: String = r.get("decision");
            let rationale: String = r.get("rationale");
            let alts_raw: String = r.get("alternatives_json");
            let alternatives_considered: Vec<String> =
                serde_json::from_str(&alts_raw).unwrap_or_default();
            let cons_raw: String = r.get("consequences_json");
            let consequences: Vec<String> = serde_json::from_str(&cons_raw).unwrap_or_default();
            let status_str: String = r.get("status");
            let st = DecisionStatus::from_str(&status_str).unwrap_or(DecisionStatus::Proposed);
            let superseded_by: Option<String> = r.get("superseded_by");
            let reqs_raw: String = r.get("linked_requirements_json");
            let linked_requirements: Vec<String> =
                serde_json::from_str(&reqs_raw).unwrap_or_default();
            let files_raw: String = r.get("linked_files_json");
            let linked_files: Vec<String> = serde_json::from_str(&files_raw).unwrap_or_default();
            let symbols_raw: String = r.get("linked_symbols_json");
            let linked_symbols: Vec<String> =
                serde_json::from_str(&symbols_raw).unwrap_or_default();
            let provenance_actor: String = r.get("provenance_actor");
            let provenance_reason: Option<String> = r.get("provenance_reason");
            let version: i64 = r.get("version");
            let created_str: String = r.get("created_at");
            let updated_str: String = r.get("updated_at");

            let created_at = DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let updated_at = DateTime::parse_from_rfc3339(&updated_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            results.push(EngineeringDecision {
                id,
                scope: sc,
                mission_id: mid,
                title,
                context,
                decision,
                rationale,
                alternatives_considered,
                consequences,
                status: st,
                superseded_by,
                linked_requirements,
                linked_files,
                linked_symbols,
                provenance_actor,
                provenance_reason,
                version: version as u32,
                created_at,
                updated_at,
            });
        }

        Ok(results)
    }

    async fn supersede_decision(
        &self,
        old_id: &str,
        new_decision: &EngineeringDecision,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        // 1. Insert new decision
        let alts_json = serde_json::to_string(&new_decision.alternatives_considered)
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
        let cons_json = serde_json::to_string(&new_decision.consequences)
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
        let reqs_json = serde_json::to_string(&new_decision.linked_requirements)
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
        let files_json = serde_json::to_string(&new_decision.linked_files)
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
        let symbols_json = serde_json::to_string(&new_decision.linked_symbols)
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
        let mission_bytes = new_decision.mission_id.map(|m| m.as_bytes().to_vec());

        sqlx::query(
            r#"
            INSERT INTO engineering_decisions (
                id, scope, mission_id, title, context, decision, rationale,
                alternatives_json, consequences_json, status, superseded_by,
                linked_requirements_json, linked_files_json, linked_symbols_json,
                provenance_actor, provenance_reason, version, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&new_decision.id)
        .bind(new_decision.scope.as_str())
        .bind(mission_bytes)
        .bind(&new_decision.title)
        .bind(&new_decision.context)
        .bind(&new_decision.decision)
        .bind(&new_decision.rationale)
        .bind(alts_json)
        .bind(cons_json)
        .bind(new_decision.status.to_string())
        .bind(&new_decision.superseded_by)
        .bind(reqs_json)
        .bind(files_json)
        .bind(symbols_json)
        .bind(&new_decision.provenance_actor)
        .bind(&new_decision.provenance_reason)
        .bind(new_decision.version as i64)
        .bind(new_decision.created_at.to_rfc3339())
        .bind(new_decision.updated_at.to_rfc3339())
        .execute(&mut *tx)
        .await?;

        // 2. Mark old decision as superseded
        let now_str = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE engineering_decisions
            SET status = 'superseded', superseded_by = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&new_decision.id)
        .bind(&now_str)
        .bind(old_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn save_assumption(&self, assumption: &EngineeringAssumption) -> Result<(), sqlx::Error> {
        let task_bytes = assumption.task_id.map(|t| t.as_bytes().to_vec());

        sqlx::query(
            r#"
            INSERT INTO engineering_assumptions (
                id, scope, mission_id, task_id, statement, status,
                evidence_summary, target_file, target_symbol, expected_hash,
                invalidated_by, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                evidence_summary = excluded.evidence_summary,
                invalidated_by = excluded.invalidated_by,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(assumption.id.as_bytes().as_slice())
        .bind(assumption.scope.as_str())
        .bind(assumption.mission_id.as_bytes().as_slice())
        .bind(task_bytes)
        .bind(&assumption.statement)
        .bind(assumption.status.as_str())
        .bind(&assumption.evidence_summary)
        .bind(&assumption.target_file)
        .bind(&assumption.target_symbol)
        .bind(&assumption.expected_hash)
        .bind(&assumption.invalidated_by)
        .bind(assumption.created_at.to_rfc3339())
        .bind(assumption.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_assumption(&self, id: Uuid) -> Result<Option<EngineeringAssumption>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, scope, mission_id, task_id, statement, status,
                   evidence_summary, target_file, target_symbol, expected_hash,
                   invalidated_by, created_at, updated_at
            FROM engineering_assumptions
            WHERE id = ?
            "#,
        )
        .bind(id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => {
                let id_bytes: Vec<u8> = r.get("id");
                let mut id_arr = [0u8; 16];
                id_arr.copy_from_slice(&id_bytes);
                let id = Uuid::from_bytes(id_arr);

                let scope_str: String = r.get("scope");
                let scope = MemoryScope::from_str(&scope_str).map_err(sqlx::Error::Protocol)?;

                let m_bytes: Vec<u8> = r.get("mission_id");
                let mut m_arr = [0u8; 16];
                m_arr.copy_from_slice(&m_bytes);
                let mission_id = MissionId::from_bytes(m_arr);

                let t_bytes: Option<Vec<u8>> = r.get("task_id");
                let task_id = t_bytes.and_then(|b| {
                    if b.len() == 16 {
                        let mut arr = [0u8; 16];
                        arr.copy_from_slice(&b);
                        Some(TaskId::from_bytes(arr))
                    } else {
                        None
                    }
                });

                let statement: String = r.get("statement");
                let status_str: String = r.get("status");
                let status =
                    AssumptionStatus::from_str(&status_str).unwrap_or(AssumptionStatus::Active);
                let evidence_summary: Option<String> = r.get("evidence_summary");
                let target_file: Option<String> = r.get("target_file");
                let target_symbol: Option<String> = r.get("target_symbol");
                let expected_hash: Option<String> = r.get("expected_hash");
                let invalidated_by: Option<String> = r.get("invalidated_by");
                let created_str: String = r.get("created_at");
                let updated_str: String = r.get("updated_at");

                let created_at = DateTime::parse_from_rfc3339(&created_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());
                let updated_at = DateTime::parse_from_rfc3339(&updated_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(Some(EngineeringAssumption {
                    id,
                    scope,
                    mission_id,
                    task_id,
                    statement,
                    status,
                    evidence_summary,
                    target_file,
                    target_symbol,
                    expected_hash,
                    invalidated_by,
                    created_at,
                    updated_at,
                }))
            }
            None => Ok(None),
        }
    }

    async fn update_assumption_status(
        &self,
        id: Uuid,
        status: AssumptionStatus,
        evidence: Option<&str>,
        invalidated_by: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let now_str = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE engineering_assumptions
            SET status = ?, evidence_summary = COALESCE(?, evidence_summary),
                invalidated_by = COALESCE(?, invalidated_by), updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(status.as_str())
        .bind(evidence)
        .bind(invalidated_by)
        .bind(now_str)
        .bind(id.as_bytes().as_slice())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn list_assumptions(
        &self,
        mission_id: MissionId,
        task_id: Option<TaskId>,
        status: Option<AssumptionStatus>,
    ) -> Result<Vec<EngineeringAssumption>, sqlx::Error> {
        let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            r#"
            SELECT id, scope, mission_id, task_id, statement, status,
                   evidence_summary, target_file, target_symbol, expected_hash,
                   invalidated_by, created_at, updated_at
            FROM engineering_assumptions
            WHERE mission_id = 
            "#,
        );
        builder.push_bind(mission_id.as_bytes().as_slice());

        if let Some(t) = task_id {
            builder
                .push(" AND task_id = ")
                .push_bind(t.as_bytes().to_vec());
        }
        if let Some(s) = status {
            builder.push(" AND status = ").push_bind(s.as_str());
        }
        builder.push(" ORDER BY created_at ASC");

        let rows = builder.build().fetch_all(&self.pool).await?;
        let mut list = Vec::with_capacity(rows.len());

        for r in rows {
            let id_bytes: Vec<u8> = r.get("id");
            let mut id_arr = [0u8; 16];
            id_arr.copy_from_slice(&id_bytes);
            let id = Uuid::from_bytes(id_arr);

            let scope_str: String = r.get("scope");
            let scope = MemoryScope::from_str(&scope_str).map_err(sqlx::Error::Protocol)?;

            let m_bytes: Vec<u8> = r.get("mission_id");
            let mut m_arr = [0u8; 16];
            m_arr.copy_from_slice(&m_bytes);
            let mid = MissionId::from_bytes(m_arr);

            let t_bytes: Option<Vec<u8>> = r.get("task_id");
            let tid = t_bytes.and_then(|b| {
                if b.len() == 16 {
                    let mut arr = [0u8; 16];
                    arr.copy_from_slice(&b);
                    Some(TaskId::from_bytes(arr))
                } else {
                    None
                }
            });

            let statement: String = r.get("statement");
            let status_str: String = r.get("status");
            let st = AssumptionStatus::from_str(&status_str).unwrap_or(AssumptionStatus::Active);
            let evidence_summary: Option<String> = r.get("evidence_summary");
            let target_file: Option<String> = r.get("target_file");
            let target_symbol: Option<String> = r.get("target_symbol");
            let expected_hash: Option<String> = r.get("expected_hash");
            let invalidated_by: Option<String> = r.get("invalidated_by");
            let created_str: String = r.get("created_at");
            let updated_str: String = r.get("updated_at");

            let created_at = DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let updated_at = DateTime::parse_from_rfc3339(&updated_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            list.push(EngineeringAssumption {
                id,
                scope,
                mission_id: mid,
                task_id: tid,
                statement,
                status: st,
                evidence_summary,
                target_file,
                target_symbol,
                expected_hash,
                invalidated_by,
                created_at,
                updated_at,
            });
        }

        Ok(list)
    }

    async fn save_failure_diagnosis(
        &self,
        diagnosis: &FailureDiagnosisRecord,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO failure_diagnoses (
                id, mission_id, task_id, failure_signature, failure_class,
                error_message, snapshot_hash, hypothesis, root_cause,
                recommended_action, repair_proposal_json, repair_status,
                recurrence_count, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                repair_status = excluded.repair_status,
                recurrence_count = excluded.recurrence_count,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(diagnosis.id.as_bytes().as_slice())
        .bind(diagnosis.mission_id.as_bytes().as_slice())
        .bind(diagnosis.task_id.as_bytes().as_slice())
        .bind(&diagnosis.failure_signature)
        .bind(&diagnosis.failure_class)
        .bind(&diagnosis.error_message)
        .bind(&diagnosis.snapshot_hash)
        .bind(&diagnosis.hypothesis)
        .bind(&diagnosis.root_cause)
        .bind(&diagnosis.recommended_action)
        .bind(&diagnosis.repair_proposal_json)
        .bind(diagnosis.repair_status.as_str())
        .bind(diagnosis.recurrence_count as i64)
        .bind(diagnosis.created_at.to_rfc3339())
        .bind(diagnosis.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_failure_diagnosis(
        &self,
        id: Uuid,
    ) -> Result<Option<FailureDiagnosisRecord>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, mission_id, task_id, failure_signature, failure_class,
                   error_message, snapshot_hash, hypothesis, root_cause,
                   recommended_action, repair_proposal_json, repair_status,
                   recurrence_count, created_at, updated_at
            FROM failure_diagnoses
            WHERE id = ?
            "#,
        )
        .bind(id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => {
                let id_bytes: Vec<u8> = r.get("id");
                let mut id_arr = [0u8; 16];
                id_arr.copy_from_slice(&id_bytes);
                let id = Uuid::from_bytes(id_arr);

                let m_bytes: Vec<u8> = r.get("mission_id");
                let mut m_arr = [0u8; 16];
                m_arr.copy_from_slice(&m_bytes);
                let mission_id = MissionId::from_bytes(m_arr);

                let t_bytes: Vec<u8> = r.get("task_id");
                let mut t_arr = [0u8; 16];
                t_arr.copy_from_slice(&t_bytes);
                let task_id = TaskId::from_bytes(t_arr);

                let failure_signature: String = r.get("failure_signature");
                let failure_class: String = r.get("failure_class");
                let error_message: String = r.get("error_message");
                let snapshot_hash: String = r.get("snapshot_hash");
                let hypothesis: String = r.get("hypothesis");
                let root_cause: String = r.get("root_cause");
                let recommended_action: String = r.get("recommended_action");
                let repair_proposal_json: Option<String> = r.get("repair_proposal_json");
                let repair_status_str: String = r.get("repair_status");
                let repair_status =
                    RepairStatus::from_str(&repair_status_str).unwrap_or(RepairStatus::Proposed);
                let recurrence_count: i64 = r.get("recurrence_count");
                let created_str: String = r.get("created_at");
                let updated_str: String = r.get("updated_at");

                let created_at = DateTime::parse_from_rfc3339(&created_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());
                let updated_at = DateTime::parse_from_rfc3339(&updated_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(Some(FailureDiagnosisRecord {
                    id,
                    mission_id,
                    task_id,
                    failure_signature,
                    failure_class,
                    error_message,
                    snapshot_hash,
                    hypothesis,
                    root_cause,
                    recommended_action,
                    repair_proposal_json,
                    repair_status,
                    recurrence_count: recurrence_count as u32,
                    created_at,
                    updated_at,
                }))
            }
            None => Ok(None),
        }
    }

    async fn find_diagnosis_by_signature(
        &self,
        mission_id: MissionId,
        signature: &str,
    ) -> Result<Option<FailureDiagnosisRecord>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, mission_id, task_id, failure_signature, failure_class,
                   error_message, snapshot_hash, hypothesis, root_cause,
                   recommended_action, repair_proposal_json, repair_status,
                   recurrence_count, created_at, updated_at
            FROM failure_diagnoses
            WHERE mission_id = ? AND failure_signature = ?
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .bind(signature)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => {
                let id_bytes: Vec<u8> = r.get("id");
                let mut id_arr = [0u8; 16];
                id_arr.copy_from_slice(&id_bytes);
                let id = Uuid::from_bytes(id_arr);

                let m_bytes: Vec<u8> = r.get("mission_id");
                let mut m_arr = [0u8; 16];
                m_arr.copy_from_slice(&m_bytes);
                let mid = MissionId::from_bytes(m_arr);

                let t_bytes: Vec<u8> = r.get("task_id");
                let mut t_arr = [0u8; 16];
                t_arr.copy_from_slice(&t_bytes);
                let task_id = TaskId::from_bytes(t_arr);

                let failure_signature: String = r.get("failure_signature");
                let failure_class: String = r.get("failure_class");
                let error_message: String = r.get("error_message");
                let snapshot_hash: String = r.get("snapshot_hash");
                let hypothesis: String = r.get("hypothesis");
                let root_cause: String = r.get("root_cause");
                let recommended_action: String = r.get("recommended_action");
                let repair_proposal_json: Option<String> = r.get("repair_proposal_json");
                let repair_status_str: String = r.get("repair_status");
                let repair_status =
                    RepairStatus::from_str(&repair_status_str).unwrap_or(RepairStatus::Proposed);
                let recurrence_count: i64 = r.get("recurrence_count");
                let created_str: String = r.get("created_at");
                let updated_str: String = r.get("updated_at");

                let created_at = DateTime::parse_from_rfc3339(&created_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());
                let updated_at = DateTime::parse_from_rfc3339(&updated_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(Some(FailureDiagnosisRecord {
                    id,
                    mission_id: mid,
                    task_id,
                    failure_signature,
                    failure_class,
                    error_message,
                    snapshot_hash,
                    hypothesis,
                    root_cause,
                    recommended_action,
                    repair_proposal_json,
                    repair_status,
                    recurrence_count: recurrence_count as u32,
                    created_at,
                    updated_at,
                }))
            }
            None => Ok(None),
        }
    }

    async fn update_repair_status(
        &self,
        id: Uuid,
        status: RepairStatus,
    ) -> Result<(), sqlx::Error> {
        let now_str = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE failure_diagnoses
            SET repair_status = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(status.as_str())
        .bind(now_str)
        .bind(id.as_bytes().as_slice())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn increment_recurrence(&self, id: Uuid) -> Result<(), sqlx::Error> {
        let now_str = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE failure_diagnoses
            SET recurrence_count = recurrence_count + 1, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(now_str)
        .bind(id.as_bytes().as_slice())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn list_diagnoses(
        &self,
        mission_id: MissionId,
        task_id: Option<TaskId>,
    ) -> Result<Vec<FailureDiagnosisRecord>, sqlx::Error> {
        let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            r#"
            SELECT id, mission_id, task_id, failure_signature, failure_class,
                   error_message, snapshot_hash, hypothesis, root_cause,
                   recommended_action, repair_proposal_json, repair_status,
                   recurrence_count, created_at, updated_at
            FROM failure_diagnoses
            WHERE mission_id = 
            "#,
        );
        builder.push_bind(mission_id.as_bytes().as_slice());

        if let Some(t) = task_id {
            builder
                .push(" AND task_id = ")
                .push_bind(t.as_bytes().to_vec());
        }
        builder.push(" ORDER BY created_at ASC");

        let rows = builder.build().fetch_all(&self.pool).await?;
        let mut list = Vec::with_capacity(rows.len());

        for r in rows {
            let id_bytes: Vec<u8> = r.get("id");
            let mut id_arr = [0u8; 16];
            id_arr.copy_from_slice(&id_bytes);
            let id = Uuid::from_bytes(id_arr);

            let m_bytes: Vec<u8> = r.get("mission_id");
            let mut m_arr = [0u8; 16];
            m_arr.copy_from_slice(&m_bytes);
            let mid = MissionId::from_bytes(m_arr);

            let t_bytes: Vec<u8> = r.get("task_id");
            let mut t_arr = [0u8; 16];
            t_arr.copy_from_slice(&t_bytes);
            let tid = TaskId::from_bytes(t_arr);

            let failure_signature: String = r.get("failure_signature");
            let failure_class: String = r.get("failure_class");
            let error_message: String = r.get("error_message");
            let snapshot_hash: String = r.get("snapshot_hash");
            let hypothesis: String = r.get("hypothesis");
            let root_cause: String = r.get("root_cause");
            let recommended_action: String = r.get("recommended_action");
            let repair_proposal_json: Option<String> = r.get("repair_proposal_json");
            let repair_status_str: String = r.get("repair_status");
            let repair_status =
                RepairStatus::from_str(&repair_status_str).unwrap_or(RepairStatus::Proposed);
            let recurrence_count: i64 = r.get("recurrence_count");
            let created_str: String = r.get("created_at");
            let updated_str: String = r.get("updated_at");

            let created_at = DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let updated_at = DateTime::parse_from_rfc3339(&updated_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            list.push(FailureDiagnosisRecord {
                id,
                mission_id: mid,
                task_id: tid,
                failure_signature,
                failure_class,
                error_message,
                snapshot_hash,
                hypothesis,
                root_cause,
                recommended_action,
                repair_proposal_json,
                repair_status,
                recurrence_count: recurrence_count as u32,
                created_at,
                updated_at,
            });
        }

        Ok(list)
    }

    async fn save_review_finding(&self, finding: &ReviewFindingRecord) -> Result<(), sqlx::Error> {
        let check_bytes = finding.check_id.map(|c| c.as_bytes().to_vec());
        let l_start = finding.line_start.map(|l| l as i64);
        let l_end = finding.line_end.map(|l| l as i64);
        let sev_str = match finding.severity {
            ReviewFindingSeverity::Info => "info",
            ReviewFindingSeverity::Warning => "warning",
            ReviewFindingSeverity::Error => "error",
            ReviewFindingSeverity::CriticalSecurity => "critical_security",
        };
        let res_at = finding.resolved_at.map(|dt| dt.to_rfc3339());

        sqlx::query(
            r#"
            INSERT INTO review_findings (
                id, mission_id, task_id, check_id, file_path,
                line_start, line_end, severity, description,
                recommendation, status, resolution_rationale,
                resolved_by, resolved_at, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                resolution_rationale = excluded.resolution_rationale,
                resolved_by = excluded.resolved_by,
                resolved_at = excluded.resolved_at
            "#,
        )
        .bind(finding.id.as_bytes().as_slice())
        .bind(finding.mission_id.as_bytes().as_slice())
        .bind(finding.task_id.as_bytes().as_slice())
        .bind(check_bytes)
        .bind(&finding.file_path)
        .bind(l_start)
        .bind(l_end)
        .bind(sev_str)
        .bind(&finding.description)
        .bind(&finding.recommendation)
        .bind(finding.status.as_str())
        .bind(&finding.resolution_rationale)
        .bind(&finding.resolved_by)
        .bind(res_at)
        .bind(finding.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_review_finding(
        &self,
        id: Uuid,
    ) -> Result<Option<ReviewFindingRecord>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, mission_id, task_id, check_id, file_path,
                   line_start, line_end, severity, description,
                   recommendation, status, resolution_rationale,
                   resolved_by, resolved_at, created_at
            FROM review_findings
            WHERE id = ?
            "#,
        )
        .bind(id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => {
                let id_bytes: Vec<u8> = r.get("id");
                let mut id_arr = [0u8; 16];
                id_arr.copy_from_slice(&id_bytes);
                let id = Uuid::from_bytes(id_arr);

                let m_bytes: Vec<u8> = r.get("mission_id");
                let mut m_arr = [0u8; 16];
                m_arr.copy_from_slice(&m_bytes);
                let mission_id = MissionId::from_bytes(m_arr);

                let t_bytes: Vec<u8> = r.get("task_id");
                let mut t_arr = [0u8; 16];
                t_arr.copy_from_slice(&t_bytes);
                let task_id = TaskId::from_bytes(t_arr);

                let c_bytes: Option<Vec<u8>> = r.get("check_id");
                let check_id = c_bytes.and_then(|b| {
                    if b.len() == 16 {
                        let mut arr = [0u8; 16];
                        arr.copy_from_slice(&b);
                        Some(CheckId::from_bytes(arr))
                    } else {
                        None
                    }
                });

                let file_path: String = r.get("file_path");
                let line_start: Option<i64> = r.get("line_start");
                let line_end: Option<i64> = r.get("line_end");
                let sev_str: String = r.get("severity");
                let severity = match sev_str.as_str() {
                    "info" => ReviewFindingSeverity::Info,
                    "warning" => ReviewFindingSeverity::Warning,
                    "critical_security" => ReviewFindingSeverity::CriticalSecurity,
                    _ => ReviewFindingSeverity::Error,
                };
                let description: String = r.get("description");
                let recommendation: String = r.get("recommendation");
                let status_str: String = r.get("status");
                let status =
                    ReviewFindingStatus::from_str(&status_str).unwrap_or(ReviewFindingStatus::Open);
                let resolution_rationale: Option<String> = r.get("resolution_rationale");
                let resolved_by: Option<String> = r.get("resolved_by");
                let res_str: Option<String> = r.get("resolved_at");
                let resolved_at = res_str.and_then(|s| {
                    DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&Utc))
                        .ok()
                });
                let created_str: String = r.get("created_at");
                let created_at = DateTime::parse_from_rfc3339(&created_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(Some(ReviewFindingRecord {
                    id,
                    mission_id,
                    task_id,
                    check_id,
                    file_path,
                    line_start: line_start.map(|l| l as usize),
                    line_end: line_end.map(|l| l as usize),
                    severity,
                    description,
                    recommendation,
                    status,
                    resolution_rationale,
                    resolved_by,
                    resolved_at,
                    created_at,
                }))
            }
            None => Ok(None),
        }
    }

    async fn update_review_finding_status(
        &self,
        id: Uuid,
        status: ReviewFindingStatus,
        rationale: Option<&str>,
        resolved_by: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let now_str = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE review_findings
            SET status = ?, resolution_rationale = COALESCE(?, resolution_rationale),
                resolved_by = COALESCE(?, resolved_by), resolved_at = ?
            WHERE id = ?
            "#,
        )
        .bind(status.as_str())
        .bind(rationale)
        .bind(resolved_by)
        .bind(now_str)
        .bind(id.as_bytes().as_slice())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn list_review_findings(
        &self,
        mission_id: MissionId,
        task_id: Option<TaskId>,
        status: Option<ReviewFindingStatus>,
    ) -> Result<Vec<ReviewFindingRecord>, sqlx::Error> {
        let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            r#"
            SELECT id, mission_id, task_id, check_id, file_path,
                   line_start, line_end, severity, description,
                   recommendation, status, resolution_rationale,
                   resolved_by, resolved_at, created_at
            FROM review_findings
            WHERE mission_id = 
            "#,
        );
        builder.push_bind(mission_id.as_bytes().as_slice());

        if let Some(t) = task_id {
            builder
                .push(" AND task_id = ")
                .push_bind(t.as_bytes().to_vec());
        }
        if let Some(s) = status {
            builder.push(" AND status = ").push_bind(s.as_str());
        }
        builder.push(" ORDER BY created_at ASC");

        let rows = builder.build().fetch_all(&self.pool).await?;
        let mut list = Vec::with_capacity(rows.len());

        for r in rows {
            let id_bytes: Vec<u8> = r.get("id");
            let mut id_arr = [0u8; 16];
            id_arr.copy_from_slice(&id_bytes);
            let id = Uuid::from_bytes(id_arr);

            let m_bytes: Vec<u8> = r.get("mission_id");
            let mut m_arr = [0u8; 16];
            m_arr.copy_from_slice(&m_bytes);
            let mid = MissionId::from_bytes(m_arr);

            let t_bytes: Vec<u8> = r.get("task_id");
            let mut t_arr = [0u8; 16];
            t_arr.copy_from_slice(&t_bytes);
            let tid = TaskId::from_bytes(t_arr);

            let c_bytes: Option<Vec<u8>> = r.get("check_id");
            let check_id = c_bytes.and_then(|b| {
                if b.len() == 16 {
                    let mut arr = [0u8; 16];
                    arr.copy_from_slice(&b);
                    Some(CheckId::from_bytes(arr))
                } else {
                    None
                }
            });

            let file_path: String = r.get("file_path");
            let line_start: Option<i64> = r.get("line_start");
            let line_end: Option<i64> = r.get("line_end");
            let sev_str: String = r.get("severity");
            let severity = match sev_str.as_str() {
                "info" => ReviewFindingSeverity::Info,
                "warning" => ReviewFindingSeverity::Warning,
                "critical_security" => ReviewFindingSeverity::CriticalSecurity,
                _ => ReviewFindingSeverity::Error,
            };
            let description: String = r.get("description");
            let recommendation: String = r.get("recommendation");
            let status_str: String = r.get("status");
            let st =
                ReviewFindingStatus::from_str(&status_str).unwrap_or(ReviewFindingStatus::Open);
            let resolution_rationale: Option<String> = r.get("resolution_rationale");
            let resolved_by: Option<String> = r.get("resolved_by");
            let res_str: Option<String> = r.get("resolved_at");
            let resolved_at = res_str.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            });
            let created_str: String = r.get("created_at");
            let created_at = DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            list.push(ReviewFindingRecord {
                id,
                mission_id: mid,
                task_id: tid,
                check_id,
                file_path,
                line_start: line_start.map(|l| l as usize),
                line_end: line_end.map(|l| l as usize),
                severity,
                description,
                recommendation,
                status: st,
                resolution_rationale,
                resolved_by,
                resolved_at,
                created_at,
            });
        }

        Ok(list)
    }

    async fn save_verification_record(
        &self,
        record: &VerificationRecord,
    ) -> Result<(), sqlx::Error> {
        let files_json = serde_json::to_string(&record.files_verified)
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
        let inact_at = record.invalidated_at.map(|dt| dt.to_rfc3339());

        sqlx::query(
            r#"
            INSERT INTO verification_records (
                id, mission_id, task_id, requirement_key, check_id,
                snapshot_hash, files_verified_json, passed,
                validity_status, invalidated_at, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                validity_status = excluded.validity_status,
                invalidated_at = excluded.invalidated_at
            "#,
        )
        .bind(record.id.as_bytes().as_slice())
        .bind(record.mission_id.as_bytes().as_slice())
        .bind(record.task_id.as_bytes().as_slice())
        .bind(&record.requirement_key)
        .bind(record.check_id.as_bytes().as_slice())
        .bind(&record.snapshot_hash)
        .bind(files_json)
        .bind(if record.passed { 1 } else { 0 })
        .bind(record.validity_status.as_str())
        .bind(inact_at)
        .bind(record.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_verification_record(
        &self,
        id: Uuid,
    ) -> Result<Option<VerificationRecord>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, mission_id, task_id, requirement_key, check_id,
                   snapshot_hash, files_verified_json, passed,
                   validity_status, invalidated_at, created_at
            FROM verification_records
            WHERE id = ?
            "#,
        )
        .bind(id.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => {
                let id_bytes: Vec<u8> = r.get("id");
                let mut id_arr = [0u8; 16];
                id_arr.copy_from_slice(&id_bytes);
                let id = Uuid::from_bytes(id_arr);

                let m_bytes: Vec<u8> = r.get("mission_id");
                let mut m_arr = [0u8; 16];
                m_arr.copy_from_slice(&m_bytes);
                let mission_id = MissionId::from_bytes(m_arr);

                let t_bytes: Vec<u8> = r.get("task_id");
                let mut t_arr = [0u8; 16];
                t_arr.copy_from_slice(&t_bytes);
                let task_id = TaskId::from_bytes(t_arr);

                let c_bytes: Vec<u8> = r.get("check_id");
                let mut c_arr = [0u8; 16];
                c_arr.copy_from_slice(&c_bytes);
                let check_id = CheckId::from_bytes(c_arr);

                let requirement_key: String = r.get("requirement_key");
                let snapshot_hash: String = r.get("snapshot_hash");
                let files_raw: String = r.get("files_verified_json");
                let files_verified: Vec<FileHashRecord> =
                    serde_json::from_str(&files_raw).unwrap_or_default();
                let passed_int: i64 = r.get("passed");
                let passed = passed_int == 1;
                let val_str: String = r.get("validity_status");
                let validity_status =
                    VerificationValidity::from_str(&val_str).unwrap_or(VerificationValidity::Valid);
                let inact_str: Option<String> = r.get("invalidated_at");
                let invalidated_at = inact_str.and_then(|s| {
                    DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&Utc))
                        .ok()
                });
                let created_str: String = r.get("created_at");
                let created_at = DateTime::parse_from_rfc3339(&created_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(Some(VerificationRecord {
                    id,
                    mission_id,
                    task_id,
                    requirement_key,
                    check_id,
                    snapshot_hash,
                    files_verified,
                    passed,
                    validity_status,
                    invalidated_at,
                    created_at,
                }))
            }
            None => Ok(None),
        }
    }

    async fn find_latest_verification_for_requirement(
        &self,
        mission_id: MissionId,
        requirement_key: &str,
    ) -> Result<Option<VerificationRecord>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT id, mission_id, task_id, requirement_key, check_id,
                   snapshot_hash, files_verified_json, passed,
                   validity_status, invalidated_at, created_at
            FROM verification_records
            WHERE mission_id = ? AND requirement_key = ?
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .bind(requirement_key)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => {
                let id_bytes: Vec<u8> = r.get("id");
                let mut id_arr = [0u8; 16];
                id_arr.copy_from_slice(&id_bytes);
                let id = Uuid::from_bytes(id_arr);

                let m_bytes: Vec<u8> = r.get("mission_id");
                let mut m_arr = [0u8; 16];
                m_arr.copy_from_slice(&m_bytes);
                let mid = MissionId::from_bytes(m_arr);

                let t_bytes: Vec<u8> = r.get("task_id");
                let mut t_arr = [0u8; 16];
                t_arr.copy_from_slice(&t_bytes);
                let task_id = TaskId::from_bytes(t_arr);

                let c_bytes: Vec<u8> = r.get("check_id");
                let mut c_arr = [0u8; 16];
                c_arr.copy_from_slice(&c_bytes);
                let check_id = CheckId::from_bytes(c_arr);

                let req_key: String = r.get("requirement_key");
                let snapshot_hash: String = r.get("snapshot_hash");
                let files_raw: String = r.get("files_verified_json");
                let files_verified: Vec<FileHashRecord> =
                    serde_json::from_str(&files_raw).unwrap_or_default();
                let passed_int: i64 = r.get("passed");
                let passed = passed_int == 1;
                let val_str: String = r.get("validity_status");
                let validity_status =
                    VerificationValidity::from_str(&val_str).unwrap_or(VerificationValidity::Valid);
                let inact_str: Option<String> = r.get("invalidated_at");
                let invalidated_at = inact_str.and_then(|s| {
                    DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&Utc))
                        .ok()
                });
                let created_str: String = r.get("created_at");
                let created_at = DateTime::parse_from_rfc3339(&created_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(Some(VerificationRecord {
                    id,
                    mission_id: mid,
                    task_id,
                    requirement_key: req_key,
                    check_id,
                    snapshot_hash,
                    files_verified,
                    passed,
                    validity_status,
                    invalidated_at,
                    created_at,
                }))
            }
            None => Ok(None),
        }
    }

    async fn invalidate_verification(
        &self,
        id: Uuid,
        validity: VerificationValidity,
    ) -> Result<(), sqlx::Error> {
        let now_str = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            UPDATE verification_records
            SET validity_status = ?, invalidated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(validity.as_str())
        .bind(now_str)
        .bind(id.as_bytes().as_slice())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn list_verification_records(
        &self,
        mission_id: MissionId,
        task_id: Option<TaskId>,
    ) -> Result<Vec<VerificationRecord>, sqlx::Error> {
        let mut builder = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            r#"
            SELECT id, mission_id, task_id, requirement_key, check_id,
                   snapshot_hash, files_verified_json, passed,
                   validity_status, invalidated_at, created_at
            FROM verification_records
            WHERE mission_id = 
            "#,
        );
        builder.push_bind(mission_id.as_bytes().as_slice());

        if let Some(t) = task_id {
            builder
                .push(" AND task_id = ")
                .push_bind(t.as_bytes().to_vec());
        }
        builder.push(" ORDER BY created_at ASC");

        let rows = builder.build().fetch_all(&self.pool).await?;
        let mut list = Vec::with_capacity(rows.len());

        for r in rows {
            let id_bytes: Vec<u8> = r.get("id");
            let mut id_arr = [0u8; 16];
            id_arr.copy_from_slice(&id_bytes);
            let id = Uuid::from_bytes(id_arr);

            let m_bytes: Vec<u8> = r.get("mission_id");
            let mut m_arr = [0u8; 16];
            m_arr.copy_from_slice(&m_bytes);
            let mid = MissionId::from_bytes(m_arr);

            let t_bytes: Vec<u8> = r.get("task_id");
            let mut t_arr = [0u8; 16];
            t_arr.copy_from_slice(&t_bytes);
            let tid = TaskId::from_bytes(t_arr);

            let c_bytes: Vec<u8> = r.get("check_id");
            let mut c_arr = [0u8; 16];
            c_arr.copy_from_slice(&c_bytes);
            let check_id = CheckId::from_bytes(c_arr);

            let req_key: String = r.get("requirement_key");
            let snapshot_hash: String = r.get("snapshot_hash");
            let files_raw: String = r.get("files_verified_json");
            let files_verified: Vec<FileHashRecord> =
                serde_json::from_str(&files_raw).unwrap_or_default();
            let passed_int: i64 = r.get("passed");
            let passed = passed_int == 1;
            let val_str: String = r.get("validity_status");
            let validity_status =
                VerificationValidity::from_str(&val_str).unwrap_or(VerificationValidity::Valid);
            let inact_str: Option<String> = r.get("invalidated_at");
            let invalidated_at = inact_str.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            });
            let created_str: String = r.get("created_at");
            let created_at = DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            list.push(VerificationRecord {
                id,
                mission_id: mid,
                task_id: tid,
                requirement_key: req_key,
                check_id,
                snapshot_hash,
                files_verified,
                passed,
                validity_status,
                invalidated_at,
                created_at,
            });
        }

        Ok(list)
    }
}
