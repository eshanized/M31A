//! Machine-readable JSON report projector and JSON Schema generator (RPT-02, D-13).

use crate::report::model::{CompletionReport, ReportCandidate};

/// Projector for serializing CompletionReport and ReportCandidate to JSON and generating JSON Schema.
pub struct JsonReportProjector;

impl JsonReportProjector {
    /// Render a `CompletionReport` into formatted, indented JSON.
    pub fn render_report(report: &CompletionReport) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(report)
    }

    /// Render a `ReportCandidate` into formatted, indented JSON.
    pub fn render_candidate(candidate: &ReportCandidate) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(candidate)
    }

    /// Generate the JSON Schema for `CompletionReport` using `schemars`.
    pub fn generate_schema() -> serde_json::Value {
        let schema = schemars::schema_for!(CompletionReport);
        serde_json::to_value(&schema).unwrap_or_else(|_| serde_json::json!({}))
    }

    /// Generate formatted JSON Schema as string.
    pub fn generate_schema_json() -> Result<String, serde_json::Error> {
        let schema = schemars::schema_for!(CompletionReport);
        serde_json::to_string_pretty(&schema)
    }

    /// Validate a JSON string against `CompletionReport` schema by deserializing it.
    pub fn validate_json(json_str: &str) -> Result<CompletionReport, serde_json::Error> {
        serde_json::from_str::<CompletionReport>(json_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::MissionId;
    use crate::report::model::*;
    use chrono::Utc;

    #[test]
    fn test_json_projection_and_schema() {
        let report = CompletionReport {
            schema_version: 1,
            mission_id: MissionId::new(),
            objective: "Test JSON schema".to_string(),
            requirements_summary: vec![],
            plan_summary: PlanReportSummary {
                plan_id: "plan-1".to_string(),
                total_tasks: 0,
                completed_tasks: 0,
                failed_tasks: 0,
                duration_seconds: 0,
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
            final_status: CompletionStatus::Succeeded,
            created_at: Utc::now(),
            duration_seconds: 42,
        };

        let json = JsonReportProjector::render_report(&report).expect("renders json");
        assert!(json.contains("\"mission_id\":"));
        assert!(json.contains("\"final_status\": \"succeeded\""));

        let validated = JsonReportProjector::validate_json(&json).expect("validates");
        assert_eq!(validated.mission_id, report.mission_id);

        let schema = JsonReportProjector::generate_schema();
        assert!(schema.is_object());
        let schema_json = JsonReportProjector::generate_schema_json().expect("schema renders");
        assert!(schema_json.contains("CompletionReport"));
    }
}
