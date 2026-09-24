//! SQLite repository for Completion Reports metadata (RPT-01, D-13).

use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;

use crate::error::M31AError;
use crate::ids::{ArtifactId, MissionId};
use crate::report::model::CompletionReport;

/// Metadata record for a persisted completion report in SQLite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionReportMetadata {
    pub mission_id: MissionId,
    pub schema_version: u32,
    pub status: String,
    pub report_md_artifact_id: ArtifactId,
    pub report_json_artifact_id: ArtifactId,
    pub tasks_succeeded: usize,
    pub tasks_failed: usize,
    pub verification_passed: usize,
    pub token_usage_json: String,
    pub evidence_count: usize,
    pub created_at: DateTime<Utc>,
}

/// SQLite concrete repository for completion reports.
#[derive(Debug, Clone)]
pub struct SqliteReportRepository {
    pool: SqlitePool,
}

impl SqliteReportRepository {
    /// Create a new report repository with the given pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Record a completion report's metadata into SQLite.
    pub async fn record_report(
        &self,
        report: &CompletionReport,
        md_artifact_id: &ArtifactId,
        json_artifact_id: &ArtifactId,
    ) -> Result<(), M31AError> {
        let tasks_succeeded = report
            .tasks_executed
            .iter()
            .filter(|t| t.status.eq_ignore_ascii_case("succeeded"))
            .count() as i64;

        let tasks_failed = report
            .tasks_executed
            .iter()
            .filter(|t| t.status.eq_ignore_ascii_case("failed"))
            .count() as i64;

        let verification_passed = report
            .verifications
            .iter()
            .filter(|v| {
                v.status.eq_ignore_ascii_case("passed")
                    || v.status.eq_ignore_ascii_case("satisfied")
            })
            .count() as i64;

        let token_usage_json = serde_json::json!({
            "total_tokens": report.total_tokens_used(),
            "cost_usd": report.total_cost_usd(),
            "models": report.models_used,
        })
        .to_string();

        let evidence_count = report.total_evidence_count() as i64;
        let created_at_str = report.created_at.to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO completion_reports (
                mission_id, schema_version, status, report_md_artifact_id,
                report_json_artifact_id, tasks_succeeded, tasks_failed,
                verification_passed, token_usage_json, evidence_count, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(mission_id) DO UPDATE SET
                schema_version = excluded.schema_version,
                status = excluded.status,
                report_md_artifact_id = excluded.report_md_artifact_id,
                report_json_artifact_id = excluded.report_json_artifact_id,
                tasks_succeeded = excluded.tasks_succeeded,
                tasks_failed = excluded.tasks_failed,
                verification_passed = excluded.verification_passed,
                token_usage_json = excluded.token_usage_json,
                evidence_count = excluded.evidence_count,
                created_at = excluded.created_at
            "#,
        )
        .bind(report.mission_id.to_string())
        .bind(report.schema_version as i64)
        .bind(report.final_status.as_str())
        .bind(md_artifact_id.to_string())
        .bind(json_artifact_id.to_string())
        .bind(tasks_succeeded)
        .bind(tasks_failed)
        .bind(verification_passed)
        .bind(token_usage_json)
        .bind(evidence_count)
        .bind(created_at_str)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Retrieve completion report metadata for a given mission.
    pub async fn get_report_metadata(
        &self,
        mission_id: &MissionId,
    ) -> Result<Option<CompletionReportMetadata>, M31AError> {
        let row_opt = sqlx::query(
            r#"
            SELECT
                mission_id, schema_version, status, report_md_artifact_id,
                report_json_artifact_id, tasks_succeeded, tasks_failed,
                verification_passed, token_usage_json, evidence_count, created_at
            FROM completion_reports
            WHERE mission_id = ?
            "#,
        )
        .bind(mission_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        let row = match row_opt {
            Some(r) => r,
            None => return Ok(None),
        };

        let mission_id_str: String = row.get("mission_id");
        let parsed_mission_id = MissionId::from_str(&mission_id_str)
            .map_err(|e| M31AError::internal(format!("Invalid mission_id in DB: {e}")))?;

        let schema_version_i64: i64 = row.get("schema_version");
        let status: String = row.get("status");

        let md_artifact_str: String = row.get("report_md_artifact_id");
        let md_artifact_id = ArtifactId::from_str(&md_artifact_str).map_err(|e| {
            M31AError::internal(format!("Invalid report_md_artifact_id in DB: {e}"))
        })?;

        let json_artifact_str: String = row.get("report_json_artifact_id");
        let json_artifact_id = ArtifactId::from_str(&json_artifact_str).map_err(|e| {
            M31AError::internal(format!("Invalid report_json_artifact_id in DB: {e}"))
        })?;

        let tasks_succeeded_i64: i64 = row.get("tasks_succeeded");
        let tasks_failed_i64: i64 = row.get("tasks_failed");
        let verification_passed_i64: i64 = row.get("verification_passed");
        let token_usage_json: String = row.get("token_usage_json");
        let evidence_count_i64: i64 = row.get("evidence_count");
        let created_at_str: String = row.get("created_at");

        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| M31AError::internal(format!("Invalid created_at RFC3339 in DB: {e}")))?
            .with_timezone(&Utc);

        Ok(Some(CompletionReportMetadata {
            mission_id: parsed_mission_id,
            schema_version: schema_version_i64 as u32,
            status,
            report_md_artifact_id: md_artifact_id,
            report_json_artifact_id: json_artifact_id,
            tasks_succeeded: tasks_succeeded_i64 as usize,
            tasks_failed: tasks_failed_i64 as usize,
            verification_passed: verification_passed_i64 as usize,
            token_usage_json,
            evidence_count: evidence_count_i64 as usize,
            created_at,
        }))
    }
}
