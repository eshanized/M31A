//! Canonical CompletionReport domain models and ReportCandidate (RPT-01, D-13).

use crate::ids::{AgentId, ArtifactId, MissionId, TaskId};
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Canonical final completion status of a mission (RPT-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CompletionStatus {
    Succeeded,
    Failed,
    Blocked,
    Cancelled,
}

impl CompletionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Blocked => "blocked",
            Self::Cancelled => "cancelled",
        }
    }
}

impl fmt::Display for CompletionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for CompletionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "succeeded" | "completed" | "success" => Ok(Self::Succeeded),
            "failed" | "failure" => Ok(Self::Failed),
            "blocked" => Ok(Self::Blocked),
            "cancelled" | "canceled" => Ok(Self::Cancelled),
            other => Err(format!("Unknown completion status: '{other}'")),
        }
    }
}

/// Requirement status and evidence in a completion report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RequirementReportItem {
    pub id: String,
    pub description: String,
    pub status: String,
    pub evidence_locator: Option<String>,
}

/// High-level plan execution summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlanReportSummary {
    pub plan_id: String,
    pub total_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub duration_seconds: u64,
}

/// Execution summary for a single task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TaskReportItem {
    pub task_id: TaskId,
    pub title: String,
    pub status: String,
    pub agent_id: Option<AgentId>,
    pub duration_seconds: u64,
    pub evidence_locator: Option<String>,
}

/// Agent attribution and activity summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AgentReportItem {
    pub agent_id: AgentId,
    pub role: String,
    pub model: String,
    pub steps_count: usize,
    pub total_tokens: u64,
}

/// Model usage, token consumption, and cost attribution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ModelReportItem {
    pub provider: String,
    pub model: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub estimated_cost_usd: f64,
}

/// File modification record with line counts and diff locator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FileChangeReportItem {
    pub path: String,
    pub lines_added: usize,
    pub lines_removed: usize,
    pub diff_artifact_locator: Option<String>,
}

/// Git repository state at completion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GitReportState {
    pub base_commit: String,
    pub final_commit: String,
    pub branch: String,
    pub commit_trailers: Vec<String>,
    pub clean_worktree: bool,
}

/// Verification check result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VerificationReportItem {
    pub tier: u8,
    pub check_id: String,
    pub status: String,
    pub evidence_locator: Option<String>,
}

/// Review verdict record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReviewReportItem {
    pub reviewer_role: String,
    pub verdict: String,
    pub comments: String,
    pub timestamp: DateTime<Utc>,
}

/// Failure classification and root cause diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FailureReportItem {
    pub classification: String,
    pub root_cause: String,
    pub recovered: bool,
}

/// Recovery, retry, or replan actions taken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RecoveryReportItem {
    pub strategy: String,
    pub count: u32,
    pub justification: String,
}

/// Policy evaluation and governance escalation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PolicyReportItem {
    pub tool: String,
    pub decision: String,
    pub actor: String,
    pub reason: String,
}

/// Content-addressed artifact produced during mission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactReportItem {
    pub id: ArtifactId,
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

/// Canonical 16-field CompletionReport domain model (RPT-01, D-13).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CompletionReport {
    pub schema_version: u32,
    pub mission_id: MissionId,
    pub objective: String,
    pub requirements_summary: Vec<RequirementReportItem>,
    pub plan_summary: PlanReportSummary,
    pub tasks_executed: Vec<TaskReportItem>,
    pub agents_used: Vec<AgentReportItem>,
    pub models_used: Vec<ModelReportItem>,
    pub files_changed: Vec<FileChangeReportItem>,
    pub git_state: GitReportState,
    pub verifications: Vec<VerificationReportItem>,
    pub review_verdicts: Vec<ReviewReportItem>,
    pub failures_encountered: Vec<FailureReportItem>,
    pub retries_and_replans: Vec<RecoveryReportItem>,
    pub policy_escalations: Vec<PolicyReportItem>,
    pub artifacts_produced: Vec<ArtifactReportItem>,
    pub final_status: CompletionStatus,
    pub created_at: DateTime<Utc>,
    pub duration_seconds: u64,
}

impl CompletionReport {
    /// Compute total tokens consumed across all models.
    pub fn total_tokens_used(&self) -> u64 {
        self.models_used.iter().map(|m| m.total_tokens).sum()
    }

    /// Compute total cost across all models.
    pub fn total_cost_usd(&self) -> f64 {
        self.models_used.iter().map(|m| m.estimated_cost_usd).sum()
    }

    /// Total count of durable evidence locators across all sections.
    pub fn total_evidence_count(&self) -> usize {
        let mut count = 0;
        for r in &self.requirements_summary {
            if r.evidence_locator.is_some() {
                count += 1;
            }
        }
        for t in &self.tasks_executed {
            if t.evidence_locator.is_some() {
                count += 1;
            }
        }
        for f in &self.files_changed {
            if f.diff_artifact_locator.is_some() {
                count += 1;
            }
        }
        for v in &self.verifications {
            if v.evidence_locator.is_some() {
                count += 1;
            }
        }
        count + self.artifacts_produced.len()
    }
}

/// Pre-gate candidate report used to break circular completion dependency (D-13).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ReportCandidate {
    pub schema_version: u32,
    pub mission_id: MissionId,
    pub objective: String,
    pub requirements_summary: Vec<RequirementReportItem>,
    pub plan_summary: PlanReportSummary,
    pub tasks_executed: Vec<TaskReportItem>,
    pub agents_used: Vec<AgentReportItem>,
    pub models_used: Vec<ModelReportItem>,
    pub files_changed: Vec<FileChangeReportItem>,
    pub git_state: GitReportState,
    pub verifications: Vec<VerificationReportItem>,
    pub review_verdicts: Vec<ReviewReportItem>,
    pub failures_encountered: Vec<FailureReportItem>,
    pub retries_and_replans: Vec<RecoveryReportItem>,
    pub policy_escalations: Vec<PolicyReportItem>,
    pub artifacts_produced: Vec<ArtifactReportItem>,
    pub provisional_status: Option<CompletionStatus>,
    pub candidate_timestamp: DateTime<Utc>,
    pub duration_seconds: u64,
}

impl ReportCandidate {
    /// Seal candidate into final CompletionReport once completion gate evaluation concludes.
    pub fn into_sealed(self, final_status: CompletionStatus) -> CompletionReport {
        CompletionReport {
            schema_version: self.schema_version,
            mission_id: self.mission_id,
            objective: self.objective,
            requirements_summary: self.requirements_summary,
            plan_summary: self.plan_summary,
            tasks_executed: self.tasks_executed,
            agents_used: self.agents_used,
            models_used: self.models_used,
            files_changed: self.files_changed,
            git_state: self.git_state,
            verifications: self.verifications,
            review_verdicts: self.review_verdicts,
            failures_encountered: self.failures_encountered,
            retries_and_replans: self.retries_and_replans,
            policy_escalations: self.policy_escalations,
            artifacts_produced: self.artifacts_produced,
            final_status,
            created_at: Utc::now(),
            duration_seconds: self.duration_seconds,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_report_16_fields_exist_and_roundtrip() {
        let report = CompletionReport {
            schema_version: 1,
            mission_id: MissionId::new(),
            objective: "Implement subsystem verification".to_string(),
            requirements_summary: vec![RequirementReportItem {
                id: "RPT-01".to_string(),
                description: "Completion report".to_string(),
                status: "Satisfied".to_string(),
                evidence_locator: Some(
                    "artifact://018f0000-0000-7000-8000-000000000001".to_string(),
                ),
            }],
            plan_summary: PlanReportSummary {
                plan_id: "plan-12".to_string(),
                total_tasks: 10,
                completed_tasks: 10,
                failed_tasks: 0,
                duration_seconds: 360,
            },
            tasks_executed: vec![TaskReportItem {
                task_id: TaskId::new(),
                title: "Task 1".to_string(),
                status: "Succeeded".to_string(),
                agent_id: Some(AgentId::new()),
                duration_seconds: 45,
                evidence_locator: None,
            }],
            agents_used: vec![AgentReportItem {
                agent_id: AgentId::new(),
                role: "Implementer".to_string(),
                model: "claude-3-5-sonnet".to_string(),
                steps_count: 8,
                total_tokens: 15000,
            }],
            models_used: vec![ModelReportItem {
                provider: "anthropic".to_string(),
                model: "claude-3-5-sonnet".to_string(),
                prompt_tokens: 10000,
                completion_tokens: 5000,
                total_tokens: 15000,
                estimated_cost_usd: 0.12,
            }],
            files_changed: vec![FileChangeReportItem {
                path: "src/report/model.rs".to_string(),
                lines_added: 200,
                lines_removed: 0,
                diff_artifact_locator: None,
            }],
            git_state: GitReportState {
                base_commit: "abcdef1".to_string(),
                final_commit: "abcdef2".to_string(),
                branch: "master".to_string(),
                commit_trailers: vec!["Signed-off-by: M31A".to_string()],
                clean_worktree: true,
            },
            verifications: vec![VerificationReportItem {
                tier: 2,
                check_id: "check-fmt".to_string(),
                status: "Passed".to_string(),
                evidence_locator: None,
            }],
            review_verdicts: vec![ReviewReportItem {
                reviewer_role: "Architect".to_string(),
                verdict: "Approved".to_string(),
                comments: "Looks solid".to_string(),
                timestamp: Utc::now(),
            }],
            failures_encountered: vec![],
            retries_and_replans: vec![],
            policy_escalations: vec![],
            artifacts_produced: vec![ArtifactReportItem {
                id: ArtifactId::new(),
                name: "REPORT.md".to_string(),
                path: "artifacts/REPORT.md".to_string(),
                size_bytes: 4096,
                sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                    .to_string(),
            }],
            final_status: CompletionStatus::Succeeded,
            created_at: Utc::now(),
            duration_seconds: 360,
        };

        let json = serde_json::to_string_pretty(&report).expect("serialization works");
        let deserialized: CompletionReport =
            serde_json::from_str(&json).expect("deserialization works");
        assert_eq!(report.mission_id, deserialized.mission_id);
        assert_eq!(report.final_status, deserialized.final_status);
        assert_eq!(report.total_evidence_count(), 2);
    }

    #[test]
    fn test_candidate_into_sealed() {
        let candidate = ReportCandidate {
            schema_version: 1,
            mission_id: MissionId::new(),
            objective: "Candidate test".to_string(),
            requirements_summary: vec![],
            plan_summary: PlanReportSummary {
                plan_id: "p1".to_string(),
                total_tasks: 1,
                completed_tasks: 1,
                failed_tasks: 0,
                duration_seconds: 10,
            },
            tasks_executed: vec![],
            agents_used: vec![],
            models_used: vec![],
            files_changed: vec![],
            git_state: GitReportState {
                base_commit: "a".to_string(),
                final_commit: "b".to_string(),
                branch: "main".to_string(),
                commit_trailers: vec![],
                clean_worktree: true,
            },
            verifications: vec![],
            review_verdicts: vec![],
            failures_encountered: vec![],
            retries_and_replans: vec![],
            policy_escalations: vec![],
            artifacts_produced: vec![],
            provisional_status: Some(CompletionStatus::Succeeded),
            candidate_timestamp: Utc::now(),
            duration_seconds: 10,
        };

        let sealed = candidate.into_sealed(CompletionStatus::Succeeded);
        assert_eq!(sealed.final_status, CompletionStatus::Succeeded);
        assert_eq!(sealed.objective, "Candidate test");
    }
}
