//! Acceptance Scenario E: Unattended ASK Fail-Closed (CONTEXT_M31A.md §99, TST-01, SEC-06, D-15).
//!
//! Objectives:
//! 1. Set up isolated fixture repository.
//! 2. Dispatch an action requiring interactive user confirmation (`PolicyDecision::Ask`).
//! 3. Verify that under `AutonomyMode::Unattended`, the approval coordinator converts `Ask` into `Deny`.
//! 4. Confirm zero unapproved side effects occur.

use async_trait::async_trait;
use std::time::{Duration, Instant};

use crate::eval::fixture::FixtureRepoBuilder;
use crate::eval::scenarios::{EvalScenario, ScenarioResult, ScenarioStatus};
use crate::ids::{AgentId, MissionId, TaskId, ToolCallId};
use crate::policy::approval::{ApprovalAction, ApprovalCoordinator, ApprovalRequest};
use crate::state::intake::AutonomyMode;
use crate::tools::RiskClass;

pub struct ScenarioE;

#[async_trait]
impl EvalScenario for ScenarioE {
    fn id(&self) -> &'static str {
        "e"
    }

    fn name(&self) -> &'static str {
        "Scenario E: Unattended ASK Fail-Closed"
    }

    fn description(&self) -> &'static str {
        "Verify non-interactive/unattended mode converts interactive ASK into DENY fail-closed"
    }

    async fn run(&self) -> Result<ScenarioResult, String> {
        let start = Instant::now();

        // 1. Set up fixture repository
        let mut builder = FixtureRepoBuilder::new().map_err(|e| e.to_string())?;
        builder
            .with_file("config.toml", "[server]\nport = 8080\n")
            .with_commit("feat: initial config");

        let fixture = builder.build().map_err(|e| e.to_string())?;

        // 2. Initialize approval coordinator with no interactive terminal channel
        let coordinator = ApprovalCoordinator::new(None, None);

        // 3. Create approval request that would normally prompt the user
        let req = ApprovalRequest::new(
            MissionId::new(),
            Some(TaskId::new()),
            Some(AgentId::new()),
            ToolCallId::new(),
            "git_push",
            serde_json::json!({
                "remote": "origin",
                "branch": "main",
                "force": true
            }),
            vec!["repo:refs/heads/main".to_string()],
            RiskClass::HighRiskMutation,
            Some("rule-protected-branch-push".to_string()),
            "policy-hash-scenario-e",
            "Force push to main requires manual interactive confirmation",
        );

        // 4. Request approval in Unattended autonomy mode
        let action = coordinator
            .request_approval(req, AutonomyMode::Unattended, Duration::from_secs(5))
            .await
            .map_err(|e| e.to_string())?;

        let denied = matches!(action, ApprovalAction::Deny { .. });
        if !denied {
            return Err(format!(
                "Expected ApprovalAction::Deny in Unattended mode, got {:?}",
                action
            ));
        }

        // 5. Verify zero side effects occurred in fixture
        let status_out = fixture
            .run_git(&["status", "--porcelain"])
            .map_err(|e| e.to_string())?;
        let clean_worktree = status_out.stdout.is_empty();

        let verification_passed = denied && clean_worktree;
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(ScenarioResult {
            scenario_id: "e".to_string(),
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
            details: "Unattended mode converted interactive ASK directly to DENY fail-closed. Clean worktree.".to_string(),
        })
    }
}
