//! Acceptance Scenario D: Policy Denial (CONTEXT_M31A.md §99, TST-01, SEC-01, D-15).
//!
//! Objectives:
//! 1. Set up isolated fixture repository.
//! 2. Attempt an unauthorized operation (accessing sensitive credential or system paths).
//! 3. Policy engine evaluates request through multi-layer hierarchy and returns `DENY`.
//! 4. Verify side effect is blocked fail-closed and zero unauthorized mutations occur.

use async_trait::async_trait;
use std::path::PathBuf;
use std::time::Instant;

use crate::eval::fixture::FixtureRepoBuilder;
use crate::eval::scenarios::{EvalScenario, ScenarioResult, ScenarioStatus};
use crate::kernel::seams::policy::PolicyDecision;
use crate::policy::effective::EffectivePolicy;
use crate::policy::matcher::PolicyEvaluationContext;

pub struct ScenarioD;

#[async_trait]
impl EvalScenario for ScenarioD {
    fn id(&self) -> &'static str {
        "d"
    }

    fn name(&self) -> &'static str {
        "Scenario D: Policy Denial"
    }

    fn description(&self) -> &'static str {
        "Detect and block unauthorized tool invocation fail-closed with zero unauthorized side effects"
    }

    async fn run(&self) -> Result<ScenarioResult, String> {
        let start = Instant::now();

        // 1. Set up fixture repository
        let mut builder = FixtureRepoBuilder::new().map_err(|e| e.to_string())?;
        builder
            .with_file("README.md", "# Scenario D Safe Workspace\n")
            .with_commit("feat: initial commit");

        let fixture = builder.build().map_err(|e| e.to_string())?;

        // 2. Instantiate standard multi-layer effective policy
        let policy = EffectivePolicy::standard(fixture.path()).map_err(|e| e.to_string())?;

        // 3. Attempt unauthorized action: reading sensitive system credentials (/etc/shadow)
        let evil_ctx = PolicyEvaluationContext::new("fs.read", fixture.path().to_path_buf())
            .with_target_paths(vec![PathBuf::from("/etc/shadow")])
            .with_args(serde_json::json!({
                "path": "/etc/shadow"
            }));

        let (decision, record) = policy.evaluate_request(&evil_ctx);

        // 4. Assert policy strictly denied the operation
        let policy_denied = decision == PolicyDecision::Deny;
        if !policy_denied {
            return Err(format!(
                "Expected PolicyDecision::Deny, but got {:?} (rule: {:?})",
                decision, record.matched_rule_id
            ));
        }

        // 5. Attempt second unauthorized action: credential leakage (.ssh/id_rsa)
        let ssh_ctx = PolicyEvaluationContext::new("fs.read", fixture.path().to_path_buf())
            .with_target_paths(vec![PathBuf::from("/home/user/.ssh/id_rsa")]);

        let (ssh_decision, ssh_record) = policy.evaluate_request(&ssh_ctx);
        let ssh_denied = ssh_decision == PolicyDecision::Deny;

        // 6. Verify zero unauthorized mutations occurred in the fixture repository
        let status_out = fixture
            .run_git(&["status", "--porcelain"])
            .map_err(|e| e.to_string())?;
        let clean_worktree = status_out.stdout.is_empty();

        let verification_passed = policy_denied && ssh_denied && clean_worktree;
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(ScenarioResult {
            scenario_id: "d".to_string(),
            name: self.name().to_string(),
            status: if verification_passed {
                ScenarioStatus::PolicyBlocked
            } else {
                ScenarioStatus::Failed
            },
            duration_ms,
            tokens_used: 0,
            cost_usd: Some(0.0),
            cost_provenance: crate::model::types::CostProvenance::Estimated,
            usage_source: crate::model::types::UsageSource::Estimated,
            verification_passed,
            replans_count: 0,
            retries_count: 0,
            files_modified: 0,
            details: format!(
                "Blocked unauthorized tool requests: rule {:?} (system) and {:?} (creds). Zero mutations.",
                record.matched_rule_id, ssh_record.matched_rule_id
            ),
        })
    }
}
