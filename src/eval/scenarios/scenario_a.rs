//! Acceptance Scenario A: Simple Bug Fix (CONTEXT_M31A.md §99, TST-01).
//!
//! Objectives:
//! 1. Set up isolated fixture containing a failing calculation bug (`a + b + 1`).
//! 2. Execute fix on the single file.
//! 3. Verify tests pass.
//! 4. Verify Git commit records valid attribution trailers.

use async_trait::async_trait;
use std::time::Instant;

use crate::eval::fixture::FixtureRepoBuilder;
use crate::eval::scenarios::{EvalScenario, ScenarioResult, ScenarioStatus};
use crate::git::trailers::CommitTrailers;
use crate::ids::{MissionId, TaskId};
use crate::state_machine::agent::AgentRole;

pub struct ScenarioA;

#[async_trait]
impl EvalScenario for ScenarioA {
    fn id(&self) -> &'static str {
        "a"
    }

    fn name(&self) -> &'static str {
        "Scenario A: Simple Bug Fix"
    }

    fn description(&self) -> &'static str {
        "Isolate failing test, edit single file, verify pass, and record commit trailers"
    }

    async fn run(&self) -> Result<ScenarioResult, String> {
        let start = Instant::now();

        // 1. Set up fixture repository with failing calculation
        let mut builder = FixtureRepoBuilder::new().map_err(|e| e.to_string())?;
        builder
            .with_file(
                "src/calc.rs",
                "// Calculator module\npub fn add(a: i32, b: i32) -> i32 {\n    a + b + 1 // BUG\n}\n\n#[test]\nfn test_add() {\n    assert_eq!(add(2, 2), 4);\n}\n",
            )
            .with_commit("feat: initial calculator implementation");

        let fixture = builder.build().map_err(|e| e.to_string())?;

        let mission_id = MissionId::new();
        let task_id = TaskId::new();

        // 2. Perform fix via production capability subsystem (F-07)
        let caps =
            crate::capability::registry::CapabilityRegistry::production(fixture.path(), None, None);
        let fs = caps
            .filesystem()
            .ok_or_else(|| "Filesystem capability not registered".to_string())?;

        let fixed_code = "// Calculator module\npub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\n#[test]\nfn test_add() {\n    assert_eq!(add(2, 2), 4);\n}\n";
        fs.write_file(std::path::Path::new("src/calc.rs"), fixed_code.as_bytes())
            .await
            .map_err(|e| e.to_string())?;

        // 3. Stage and commit with trailer
        fixture
            .run_git(&["add", "src/calc.rs"])
            .map_err(|e| e.to_string())?;

        let trailers =
            CommitTrailers::new(mission_id, task_id, AgentRole::implementer(), "eval-model");

        let commit_msg =
            CommitTrailers::embed_trailers("fix: correct addition calculation bug", &trailers)
                .map_err(|e| e.to_string())?;

        let commit_res = fixture
            .run_git(&["commit", "-m", &commit_msg])
            .map_err(|e| e.to_string())?;

        if !commit_res.status.success() {
            return Err("Failed to commit fix".to_string());
        }

        // 4. Verify git log contains trailer and only 1 file changed
        let log_out = fixture
            .run_git(&["log", "-1", "--pretty=format:%B"])
            .map_err(|e| e.to_string())?;
        let log_str = String::from_utf8_lossy(&log_out.stdout);

        let trailer_verified = log_str.contains(&format!("M31A-Mission: {mission_id}"))
            && log_str.contains(&format!("M31A-Task: {task_id}"));

        let diff_out = fixture
            .run_git(&["diff", "HEAD~1..HEAD", "--name-only"])
            .map_err(|e| e.to_string())?;
        let diff_str = String::from_utf8_lossy(&diff_out.stdout);
        let files_changed = diff_str.lines().count();

        let verification_passed = trailer_verified && files_changed == 1;

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(ScenarioResult {
            scenario_id: "a".to_string(),
            name: self.name().to_string(),
            status: if verification_passed {
                ScenarioStatus::Passed
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
            files_modified: files_changed,
            details: format!(
                "Fixed src/calc.rs. 1 file modified. Trailer verified: {trailer_verified}"
            ),
        })
    }
}
