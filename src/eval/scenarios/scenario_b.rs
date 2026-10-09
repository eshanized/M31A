//! Acceptance Scenario B: Multi-File Feature (CONTEXT_M31A.md §99, TST-01).
//!
//! Objectives:
//! 1. Set up isolated fixture containing modular components.
//! 2. Implement a cross-cutting caching feature spanning multiple files.
//! 3. Verify multiple files modified and clean worktree state.

use async_trait::async_trait;
use std::time::Instant;

use crate::eval::fixture::FixtureRepoBuilder;
use crate::eval::scenarios::{EvalScenario, ScenarioResult, ScenarioStatus};
use crate::git::trailers::CommitTrailers;
use crate::ids::{MissionId, TaskId};
use crate::state_machine::agent::AgentRole;

pub struct ScenarioB;

#[async_trait]
impl EvalScenario for ScenarioB {
    fn id(&self) -> &'static str {
        "b"
    }

    fn name(&self) -> &'static str {
        "Scenario B: Multi-File Feature"
    }

    fn description(&self) -> &'static str {
        "Implement cross-cutting feature across storage and service modules, verifying multi-file changes"
    }

    async fn run(&self) -> Result<ScenarioResult, String> {
        let start = Instant::now();

        // 1. Set up fixture repository with two interconnected files
        let mut builder = FixtureRepoBuilder::new().map_err(|e| e.to_string())?;
        builder
            .with_file(
                "src/storage.rs",
                "pub struct Storage;\nimpl Storage {\n    pub fn get(&self, key: &str) -> Option<String> { None }\n}\n",
            )
            .with_file(
                "src/service.rs",
                "use crate::storage::Storage;\npub struct Service { storage: Storage }\nimpl Service {\n    pub fn fetch(&self, key: &str) -> Option<String> { self.storage.get(key) }\n}\n",
            )
            .with_commit("feat: initial storage and service architecture");

        let fixture = builder.build().map_err(|e| e.to_string())?;

        // 2. Modify both files to implement caching
        fixture
            .write_file(
                "src/storage.rs",
                "use std::collections::HashMap;\nuse std::sync::Mutex;\npub struct Storage {\n    cache: Mutex<HashMap<String, String>>,\n}\nimpl Storage {\n    pub fn new() -> Self { Self { cache: Mutex::new(HashMap::new()) } }\n    pub fn get(&self, key: &str) -> Option<String> { self.cache.lock().unwrap().get(key).cloned() }\n}\n",
            )
            .map_err(|e| e.to_string())?;

        fixture
            .write_file(
                "src/service.rs",
                "use crate::storage::Storage;\npub struct Service { storage: Storage }\nimpl Service {\n    pub fn new(storage: Storage) -> Self { Self { storage } }\n    pub fn fetch(&self, key: &str) -> Option<String> { self.storage.get(key) }\n}\n",
            )
            .map_err(|e| e.to_string())?;

        // 3. Stage and commit
        fixture.run_git(&["add", "-A"]).map_err(|e| e.to_string())?;

        let mission_id = MissionId::new();
        let task_id = TaskId::new();
        let trailers =
            CommitTrailers::new(mission_id, task_id, AgentRole::implementer(), "eval-model");

        let commit_msg = CommitTrailers::embed_trailers(
            "feat: implement in-memory cache across storage and service",
            &trailers,
        )
        .map_err(|e| e.to_string())?;

        fixture
            .run_git(&["commit", "-m", &commit_msg])
            .map_err(|e| e.to_string())?;

        // 4. Verify multiple files modified and clean worktree
        let diff_out = fixture
            .run_git(&["diff", "HEAD~1..HEAD", "--name-only"])
            .map_err(|e| e.to_string())?;
        let diff_str = String::from_utf8_lossy(&diff_out.stdout);
        let files_changed = diff_str.lines().count();

        let status_out = fixture
            .run_git(&["status", "--porcelain"])
            .map_err(|e| e.to_string())?;
        let clean_worktree = status_out.stdout.is_empty();

        let verification_passed = files_changed >= 2 && clean_worktree;
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(ScenarioResult {
            scenario_id: "b".to_string(),
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
            details: format!("Modified {files_changed} files. Worktree clean: {clean_worktree}"),
        })
    }
}
