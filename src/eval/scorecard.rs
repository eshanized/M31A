//! Evaluation Scorecard Generator producing JSON and Markdown reports (D-15, TST-06).
//!
//! Provides structured aggregation of scenario outcomes:
//! - Machine-readable JSON schema validation (`schemars`).
//! - Human-readable Markdown summary tables with metrics (pass rate, duration, tokens, cost, replans).

use chrono::Utc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::eval::scenarios::{ScenarioResult, ScenarioStatus};

/// Summary metrics aggregated across all evaluated scenarios.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EvalSummary {
    pub total_scenarios: usize,
    pub passed: usize,
    pub policy_blocked: usize,
    pub failed: usize,
    pub harness_errors: usize,
    #[serde(default)]
    pub timed_out: usize,
    pub total_duration_ms: u64,
    pub total_tokens: u64,
    pub total_cost_usd: f64,
    pub pass_rate: f64,
}

/// Comprehensive evaluation scorecard containing aggregated metrics and individual scenario results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EvalScorecard {
    pub generated_at: String,
    pub summary: EvalSummary,
    pub results: Vec<ScenarioResult>,
}

impl EvalScorecard {
    /// Construct a new scorecard from a collection of scenario results.
    pub fn from_results(results: Vec<ScenarioResult>) -> Self {
        let total_scenarios = results.len();
        let mut passed = 0;
        let mut policy_blocked = 0;
        let mut failed = 0;
        let mut harness_errors = 0;
        let mut timed_out = 0;
        let mut total_duration_ms = 0;
        let mut total_tokens = 0;
        let mut total_cost_usd = 0.0;

        for r in &results {
            total_duration_ms += r.duration_ms;
            total_tokens += r.tokens_used;
            if let Some(c) = r.cost_usd {
                total_cost_usd += c;
            }

            match r.status {
                ScenarioStatus::Passed => passed += 1,
                ScenarioStatus::PolicyBlocked => policy_blocked += 1,
                ScenarioStatus::Failed => failed += 1,
                ScenarioStatus::HarnessError => harness_errors += 1,
                ScenarioStatus::TimedOut => timed_out += 1,
            }
        }

        let successful_outcomes = passed + policy_blocked;
        let pass_rate = if total_scenarios > 0 {
            (successful_outcomes as f64 / total_scenarios as f64) * 100.0
        } else {
            0.0
        };

        let summary = EvalSummary {
            total_scenarios,
            passed,
            policy_blocked,
            failed,
            harness_errors,
            timed_out,
            total_duration_ms,
            total_tokens,
            total_cost_usd,
            pass_rate,
        };

        Self {
            generated_at: Utc::now().to_rfc3339(),
            summary,
            results,
        }
    }

    /// Render scorecard to formatted JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Render scorecard to a clean Markdown summary table.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str("# M31A Autonomous Evaluation Scorecard\n\n");
        md.push_str(&format!("*Generated at: {}*\n\n", self.generated_at));

        md.push_str("## Overall Summary\n\n");
        md.push_str(&format!(
            "- **Total Scenarios:** {}\n",
            self.summary.total_scenarios
        ));
        md.push_str(&format!("- **Passed:** {}\n", self.summary.passed));
        md.push_str(&format!(
            "- **Policy Blocked (Fail-Closed):** {}\n",
            self.summary.policy_blocked
        ));
        md.push_str(&format!("- **Failed:** {}\n", self.summary.failed));
        md.push_str(&format!(
            "- **Harness Errors:** {}\n",
            self.summary.harness_errors
        ));
        md.push_str(&format!(
            "- **Effective Pass Rate:** {:.1}%\n",
            self.summary.pass_rate
        ));
        md.push_str(&format!(
            "- **Total Wall-Clock Time:** {:.2}s\n",
            self.summary.total_duration_ms as f64 / 1000.0
        ));
        md.push_str(&format!(
            "- **Total Tokens Used:** {}\n",
            self.summary.total_tokens
        ));
        md.push_str(&format!(
            "- **Estimated Cost:** ${:.4} USD\n\n",
            self.summary.total_cost_usd
        ));

        md.push_str("## Scenario Results\n\n");
        md.push_str("| ID | Scenario | Status | Duration | Tokens | Cost ($) | Files | Replans | Details |\n");
        md.push_str("|:---|:---|:---|:---|:---|:---|:---|:---|:---|\n");

        for r in &self.results {
            let status_badge = match r.status {
                ScenarioStatus::Passed => "✅ PASSED",
                ScenarioStatus::PolicyBlocked => "🛡️ BLOCKED",
                ScenarioStatus::Failed => "❌ FAILED",
                ScenarioStatus::HarnessError => "⚠️ ERROR",
                ScenarioStatus::TimedOut => "⏱️ TIMED OUT",
            };

            let cost_str = match r.cost_usd {
                Some(cost) => format!("${:.3}", cost),
                None => "unknown".to_string(),
            };

            md.push_str(&format!(
                "| `{}` | {} | {} | {}ms | {} | {} | {} | {} | {} |\n",
                r.scenario_id,
                r.name,
                status_badge,
                r.duration_ms,
                r.tokens_used,
                cost_str,
                r.files_modified,
                r.replans_count,
                r.details
            ));
        }

        md
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::types::{CostProvenance, UsageSource};

    #[test]
    fn test_scorecard_aggregation_and_formatting() {
        let results = vec![
            ScenarioResult {
                scenario_id: "a".to_string(),
                name: "Scenario A".to_string(),
                status: ScenarioStatus::Passed,
                duration_ms: 200,
                tokens_used: 1000,
                cost_usd: Some(0.005),
                cost_provenance: CostProvenance::Estimated,
                usage_source: UsageSource::Estimated,
                verification_passed: true,
                replans_count: 0,
                retries_count: 0,
                files_modified: 1,
                details: "Passed smoothly".to_string(),
            },
            ScenarioResult {
                scenario_id: "d".to_string(),
                name: "Scenario D".to_string(),
                status: ScenarioStatus::PolicyBlocked,
                duration_ms: 150,
                tokens_used: 800,
                cost_usd: Some(0.003),
                cost_provenance: CostProvenance::Estimated,
                usage_source: UsageSource::Estimated,
                verification_passed: true,
                replans_count: 0,
                retries_count: 0,
                files_modified: 0,
                details: "Blocked unauthorized access".to_string(),
            },
        ];

        let scorecard = EvalScorecard::from_results(results);
        assert_eq!(scorecard.summary.total_scenarios, 2);
        assert_eq!(scorecard.summary.passed, 1);
        assert_eq!(scorecard.summary.policy_blocked, 1);
        assert_eq!(scorecard.summary.pass_rate, 100.0);

        let json = scorecard.to_json().unwrap();
        assert!(json.contains("\"total_scenarios\": 2"));

        let md = scorecard.to_markdown();
        assert!(md.contains("# M31A Autonomous Evaluation Scorecard"));
        assert!(md.contains("✅ PASSED"));
        assert!(md.contains("🛡️ BLOCKED"));
    }
}
