//! Two-phase deterministic completion report generator (RPT-01, RPT-02, D-13).

use chrono::Utc;
use sqlx::{Row, SqlitePool};
use std::sync::Arc;
use thiserror::Error;

use crate::error::M31AError;
use crate::ids::{AgentId, ArtifactId, MissionId, TaskId};
use crate::persistence::artifacts::ArtifactStore;
use crate::persistence::sqlite::repositories::report::SqliteReportRepository;
use crate::report::json::JsonReportProjector;
use crate::report::markdown::MarkdownReportProjector;
use crate::report::model::*;
use crate::state::completion::CompletionGateError;
use crate::telemetry::redactor::SecretRedactor;

/// Errors produced during report generation or sealing.
#[derive(Debug, Error)]
pub enum ReportError {
    #[error("Persistence error: {0}")]
    Persistence(#[from] M31AError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Mission not found: {0}")]
    MissionNotFound(String),
}

/// Deterministic generator for two-phase mission completion reports (D-13).
pub struct ReportGenerator {
    pool: SqlitePool,
    artifact_store: Arc<dyn ArtifactStore>,
    report_repo: Arc<SqliteReportRepository>,
    redactor: Arc<SecretRedactor>,
}

impl ReportGenerator {
    /// Create a new report generator.
    pub fn new(
        pool: SqlitePool,
        artifact_store: Arc<dyn ArtifactStore>,
        report_repo: Arc<SqliteReportRepository>,
        redactor: Arc<SecretRedactor>,
    ) -> Self {
        Self {
            pool,
            artifact_store,
            report_repo,
            redactor,
        }
    }

    /// Access the underlying artifact store.
    pub fn artifact_store(&self) -> &Arc<dyn ArtifactStore> {
        &self.artifact_store
    }

    /// Access the underlying report repository.
    pub fn report_repo(&self) -> &Arc<SqliteReportRepository> {
        &self.report_repo
    }

    /// Access the underlying secret redactor.
    pub fn redactor(&self) -> &Arc<SecretRedactor> {
        &self.redactor
    }

    /// Stage 1 (Candidate Generation): Build a `ReportCandidate` for a mission from SQLite state.
    ///
    /// Sanitizes all text fields with `SecretRedactor`, saves candidate artifacts
    /// into `ArtifactStore` so `CompletionGate` evaluation criteria can pass,
    /// and returns the unsealed `ReportCandidate`.
    pub async fn build_candidate(
        &self,
        mission_id: &MissionId,
    ) -> Result<ReportCandidate, ReportError> {
        // 1. Fetch mission metadata
        let row_opt = sqlx::query("SELECT id, objective, status FROM missions WHERE id = ?")
            .bind(mission_id.as_bytes().as_slice())
            .fetch_optional(&self.pool)
            .await
            .map_err(M31AError::from)?;

        let (objective, mission_status_str) = match row_opt {
            Some(row) => {
                let obj: String = row.get("objective");
                let st: String = row.get("status");
                (obj, st)
            }
            None => {
                return Err(ReportError::MissionNotFound(mission_id.to_string()));
            }
        };

        let provisional_status = match mission_status_str.to_ascii_lowercase().as_str() {
            "succeeded" | "completed" | "success" => Some(CompletionStatus::Succeeded),
            "failed" | "failure" => Some(CompletionStatus::Failed),
            "blocked" => Some(CompletionStatus::Blocked),
            "cancelled" | "canceled" => Some(CompletionStatus::Cancelled),
            _ => None,
        };

        // 2. Fetch Git commit history and state for mission
        let git_commit_rows = sqlx::query(
            "SELECT commit_hash FROM git_commits WHERE mission_id = ? ORDER BY created_at ASC",
        )
        .bind(mission_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(M31AError::from)?;

        let (base_commit, final_commit) = if let Some(first) = git_commit_rows.first() {
            let base: String = first.get("commit_hash");
            let fin: String = git_commit_rows
                .last()
                .map(|r| r.get("commit_hash"))
                .unwrap_or_else(|| base.clone());
            (base, fin)
        } else {
            (String::new(), String::new())
        };

        let branch = format!("m31a/{mission_id}");

        let applied_proposals: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM change_proposals WHERE mission_id = ? AND LOWER(state) = 'applied'"
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0);
        let clean_worktree = applied_proposals == 0 || !git_commit_rows.is_empty();

        // 3. Fetch tasks (propagating DB errors)
        let task_rows = sqlx::query("SELECT id, title, status FROM tasks WHERE mission_id = ?")
            .bind(mission_id.as_bytes().as_slice())
            .fetch_all(&self.pool)
            .await
            .map_err(M31AError::from)?;

        let mut tasks = Vec::with_capacity(task_rows.len());
        let mut completed_count = 0;
        let mut failed_count = 0;

        for r in task_rows {
            let raw_id: Vec<u8> = r.get("id");
            let title: String = r.get("title");
            let status: String = r.get("status");

            if status.eq_ignore_ascii_case("succeeded") || status.eq_ignore_ascii_case("completed")
            {
                completed_count += 1;
            } else if status.eq_ignore_ascii_case("failed") {
                failed_count += 1;
            }

            if raw_id.len() == 16 {
                let mut bytes = [0u8; 16];
                bytes.copy_from_slice(&raw_id);
                tasks.push(TaskReportItem {
                    task_id: TaskId::from_bytes(bytes),
                    title,
                    status,
                    agent_id: None,
                    duration_seconds: 0,
                    evidence_locator: None,
                });
            }
        }

        // 4. Fetch agents (propagating DB errors)
        let agent_rows = sqlx::query("SELECT id, role, status FROM agents WHERE mission_id = ?")
            .bind(mission_id.as_bytes().as_slice())
            .fetch_all(&self.pool)
            .await
            .map_err(M31AError::from)?;

        let mut agents = Vec::with_capacity(agent_rows.len());
        for r in agent_rows {
            let raw_id: Vec<u8> = r.get("id");
            let role: String = r.get("role");
            if raw_id.len() == 16 {
                let mut bytes = [0u8; 16];
                bytes.copy_from_slice(&raw_id);
                agents.push(AgentReportItem {
                    agent_id: AgentId::from_bytes(bytes),
                    role,
                    model: "default".to_string(),
                    steps_count: 0,
                    total_tokens: 0,
                });
            }
        }

        // 5. Fetch actual model invocations and token metrics (no fabricated 50/50 split)
        let invocation_rows = sqlx::query(
            r#"SELECT provider, model_name,
                      SUM(prompt_tokens) as sum_prompt,
                      SUM(completion_tokens) as sum_completion,
                      SUM(total_tokens) as sum_total
               FROM model_invocations
               WHERE mission_id = ?
               GROUP BY provider, model_name"#,
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await
        .map_err(M31AError::from)?;

        let mut models = Vec::new();
        for r in invocation_rows {
            let provider: String = r.get("provider");
            let model: String = r.get("model_name");
            let prompt_tokens: i64 = r.get("sum_prompt");
            let completion_tokens: i64 = r.get("sum_completion");
            let total_tokens: i64 = r.get("sum_total");
            models.push(ModelReportItem {
                provider,
                model,
                prompt_tokens: prompt_tokens as u64,
                completion_tokens: completion_tokens as u64,
                total_tokens: total_tokens as u64,
                estimated_cost_usd: 0.0,
            });
        }

        if models.is_empty() {
            let span_rows = sqlx::query(
                "SELECT name, kind, status, duration_us, attributes_json FROM telemetry_spans WHERE mission_id = ?",
            )
            .bind(mission_id.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(M31AError::from)?;

            for r in span_rows {
                let kind: String = r.get("kind");
                if kind == "model" {
                    let attrs_str: String = r.get("attributes_json");
                    if let Ok(attrs) = serde_json::from_str::<serde_json::Value>(&attrs_str) {
                        let provider = attrs
                            .get("provider")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let model = attrs
                            .get("model")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let total = attrs.get("tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                        let prompt = attrs
                            .get("prompt_tokens")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(total);
                        let completion = attrs
                            .get("completion_tokens")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(total.saturating_sub(prompt));
                        let cost = attrs
                            .get("cost_usd")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0);
                        models.push(ModelReportItem {
                            provider: provider.to_string(),
                            model: model.to_string(),
                            prompt_tokens: prompt,
                            completion_tokens: completion,
                            total_tokens: total,
                            estimated_cost_usd: cost,
                        });
                    }
                }
            }
        }

        // 6. Fetch verification checks
        let verification_rows = sqlx::query(
            "SELECT tier, id, status, evidence_artifact_id FROM verification_checks WHERE mission_id = ?",
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await
        .map_err(M31AError::from)?;

        let mut verifications = Vec::with_capacity(verification_rows.len());
        for r in verification_rows {
            let tier: i64 = r.get("tier");
            let raw_id: Vec<u8> = r.get("id");
            let status: String = r.get("status");
            let ev_id: Option<Vec<u8>> = r.get("evidence_artifact_id");
            let ev_loc = ev_id.and_then(|b| {
                if b.len() == 16 {
                    let mut arr = [0u8; 16];
                    arr.copy_from_slice(&b);
                    Some(uuid::Uuid::from_bytes(arr).to_string())
                } else {
                    None
                }
            });
            let check_id = if raw_id.len() == 16 {
                let mut arr = [0u8; 16];
                arr.copy_from_slice(&raw_id);
                uuid::Uuid::from_bytes(arr).to_string()
            } else {
                raw_id.iter().map(|b| format!("{b:02x}")).collect()
            };
            verifications.push(VerificationReportItem {
                tier: tier as u8,
                check_id,
                status,
                evidence_locator: ev_loc,
            });
        }

        // 7. Fetch file changes from change provenance
        let prov_rows = sqlx::query(
            "SELECT affected_files, diff_summary FROM change_provenance WHERE mission_id = ?",
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await
        .map_err(M31AError::from)?;

        let mut files_changed = Vec::new();
        for r in prov_rows {
            let files_json: String = r.get("affected_files");
            let files: Vec<String> = serde_json::from_str(&files_json).unwrap_or_default();
            let diff_summary: String = r.get("diff_summary");
            for path in files {
                files_changed.push(FileChangeReportItem {
                    path,
                    lines_added: 0,
                    lines_removed: 0,
                    diff_artifact_locator: if diff_summary.is_empty() {
                        None
                    } else {
                        Some(diff_summary.clone())
                    },
                });
            }
        }

        // 8. Fetch policy decisions
        let policy_rows = sqlx::query(
            "SELECT tool_or_capability, decision, authority_source, explanation FROM policy_decisions WHERE mission_id = ?",
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await
        .map_err(M31AError::from)?;

        let mut policy_escalations = Vec::with_capacity(policy_rows.len());
        for r in policy_rows {
            let tool: String = r.get("tool_or_capability");
            let decision: String = r.get("decision");
            let actor: String = r.get("authority_source");
            let reason: String = r.get("explanation");
            policy_escalations.push(PolicyReportItem {
                tool,
                decision,
                actor,
                reason,
            });
        }

        // 9. Fetch recovery attempts
        let recovery_rows = sqlx::query(
            "SELECT strategy, attempt_number, action_taken FROM recovery_attempts WHERE mission_id = ?",
        )
        .bind(mission_id.as_bytes().as_slice())
        .fetch_all(&self.pool)
        .await
        .map_err(M31AError::from)?;

        let mut retries_and_replans = Vec::with_capacity(recovery_rows.len());
        for r in recovery_rows {
            let strategy: String = r.get("strategy");
            let count: i64 = r.get("attempt_number");
            let action: String = r.get("action_taken");
            retries_and_replans.push(RecoveryReportItem {
                strategy,
                count: count as u32,
                justification: action,
            });
        }

        let total_tasks = tasks.len();
        let candidate = ReportCandidate {
            schema_version: 1,
            mission_id: *mission_id,
            objective,
            requirements_summary: Vec::new(),
            plan_summary: PlanReportSummary {
                plan_id: format!("plan-{}", mission_id),
                total_tasks,
                completed_tasks: completed_count,
                failed_tasks: failed_count,
                duration_seconds: 0,
            },
            tasks_executed: tasks,
            agents_used: agents,
            models_used: models,
            files_changed,
            git_state: GitReportState {
                base_commit: base_commit.clone(),
                final_commit,
                branch,
                commit_trailers: Vec::new(),
                clean_worktree,
            },
            verifications,
            review_verdicts: Vec::new(),
            failures_encountered: Vec::new(),
            retries_and_replans,
            policy_escalations,
            artifacts_produced: Vec::new(),
            provisional_status,
            candidate_timestamp: Utc::now(),
            duration_seconds: 0,
        };

        self.build_candidate_from_candidate(candidate).await
    }

    /// Stage 1 Helper: Build candidate from an already assembled `ReportCandidate` struct.
    ///
    /// Redacts all strings, serializes provisional candidate artifacts, and stores them in `ArtifactStore`.
    pub async fn build_candidate_from_candidate(
        &self,
        mut candidate: ReportCandidate,
    ) -> Result<ReportCandidate, ReportError> {
        // Redact candidate text
        self.sanitize_candidate(&mut candidate);

        // Render & store candidate Markdown
        let candidate_md = MarkdownReportProjector::render_candidate(&candidate);
        let candidate_md_redacted = self.redactor.redact_text(&candidate_md);
        let md_candidate_id = ArtifactId::new();
        self.artifact_store
            .store(md_candidate_id, candidate_md_redacted.as_bytes(), "md")
            .await?;

        // Render & store candidate JSON
        let candidate_json = JsonReportProjector::render_candidate(&candidate)?;
        let candidate_json_redacted = self.redactor.redact_text(&candidate_json);
        let json_candidate_id = ArtifactId::new();
        self.artifact_store
            .store(
                json_candidate_id,
                candidate_json_redacted.as_bytes(),
                "json",
            )
            .await?;

        Ok(candidate)
    }

    /// Stage 2 (Report Sealing): Seal candidate into final `CompletionReport` once completion gate evaluation concludes (D-13).
    ///
    /// Determines final status from gate results, formats final `REPORT.md` and `REPORT.json`,
    /// persists final artifacts in `ArtifactStore`, and records SQLite row in `completion_reports`.
    pub async fn seal_report(
        &self,
        candidate: ReportCandidate,
        gate_result: &Result<(), Vec<CompletionGateError>>,
    ) -> Result<CompletionReport, ReportError> {
        let final_status = match gate_result {
            Ok(()) => CompletionStatus::Succeeded,
            Err(errors) => {
                let is_blocked = errors.iter().any(|e| {
                    matches!(
                        e,
                        CompletionGateError::FatalPolicyState
                            | CompletionGateError::IntegrationPolicyFailed
                    )
                });
                if is_blocked {
                    CompletionStatus::Blocked
                } else {
                    CompletionStatus::Failed
                }
            }
        };

        let mut report = candidate.into_sealed(final_status);

        // Sanitize any remaining text in report
        self.sanitize_report(&mut report);

        // Render and store final Markdown report
        let md_content = MarkdownReportProjector::render_report(&report);
        let md_redacted = self.redactor.redact_text(&md_content);
        let md_artifact_id = ArtifactId::new();
        self.artifact_store
            .store(md_artifact_id, md_redacted.as_bytes(), "md")
            .await?;

        // Render and store final JSON report
        let json_content = JsonReportProjector::render_report(&report)?;
        let json_redacted = self.redactor.redact_text(&json_content);
        let json_artifact_id = ArtifactId::new();
        self.artifact_store
            .store(json_artifact_id, json_redacted.as_bytes(), "json")
            .await?;

        // Record metadata row in SQLite
        self.report_repo
            .record_report(&report, &md_artifact_id, &json_artifact_id)
            .await?;

        Ok(report)
    }

    /// Write rendered and sanitized REPORT.md and REPORT.json to disk (D-13, F-09).
    pub async fn write_report_files(
        &self,
        report: &CompletionReport,
        target_dir: &std::path::Path,
    ) -> Result<(), ReportError> {
        let md_content = MarkdownReportProjector::render_report(report);
        let md_redacted = self.redactor.redact_text(&md_content);
        let json_content = JsonReportProjector::render_report(report)?;
        let json_redacted = self.redactor.redact_text(&json_content);

        tokio::fs::create_dir_all(target_dir).await?;
        tokio::fs::write(target_dir.join("REPORT.md"), md_redacted.as_bytes()).await?;
        tokio::fs::write(target_dir.join("REPORT.json"), json_redacted.as_bytes()).await?;
        Ok(())
    }

    fn sanitize_candidate(&self, candidate: &mut ReportCandidate) {
        candidate.objective = self.redactor.redact_text(&candidate.objective);
        for req in &mut candidate.requirements_summary {
            req.description = self.redactor.redact_text(&req.description);
        }
        for task in &mut candidate.tasks_executed {
            task.title = self.redactor.redact_text(&task.title);
        }
        for fail in &mut candidate.failures_encountered {
            fail.root_cause = self.redactor.redact_text(&fail.root_cause);
        }
        for rec in &mut candidate.retries_and_replans {
            rec.justification = self.redactor.redact_text(&rec.justification);
        }
        for pol in &mut candidate.policy_escalations {
            pol.reason = self.redactor.redact_text(&pol.reason);
        }
        for rev in &mut candidate.review_verdicts {
            rev.comments = self.redactor.redact_text(&rev.comments);
        }
    }

    fn sanitize_report(&self, report: &mut CompletionReport) {
        report.objective = self.redactor.redact_text(&report.objective);
        for req in &mut report.requirements_summary {
            req.description = self.redactor.redact_text(&req.description);
        }
        for task in &mut report.tasks_executed {
            task.title = self.redactor.redact_text(&task.title);
        }
        for fail in &mut report.failures_encountered {
            fail.root_cause = self.redactor.redact_text(&fail.root_cause);
        }
        for rec in &mut report.retries_and_replans {
            rec.justification = self.redactor.redact_text(&rec.justification);
        }
        for pol in &mut report.policy_escalations {
            pol.reason = self.redactor.redact_text(&pol.reason);
        }
        for rev in &mut report.review_verdicts {
            rev.comments = self.redactor.redact_text(&rev.comments);
        }
    }
}
