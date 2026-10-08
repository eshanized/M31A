//! Acceptance Scenario F: Crash Recovery & Resume (CONTEXT_M31A.md §99, TST-01, CHK-02, CHK-05, D-15).
//!
//! Objectives:
//! 1. Set up isolated fixture repository and checkpoint state.
//! 2. Seed mission with Task 1 (Succeeded) and Task 2 (in-flight Running).
//! 3. Create durable Checkpoint 1 and simulate runtime crash interruption.
//! 4. Scanner discovers incomplete mission, verifies baseline, and classifies as `SafeToResume`.
//! 5. `SafeResumeEngine` preserves Task 1 without re-execution, reschedules Task 2, and completes mission.

use async_trait::async_trait;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;
use tempfile::tempdir;

use crate::checkpoint::crash_recovery::{CrashRecoveryClassification, StartupCrashRecoveryScanner};
use crate::checkpoint::manager::CheckpointManager;
use crate::checkpoint::manifest::CheckpointManifest;
use crate::checkpoint::resume::SafeResumeEngine;
use crate::eval::fixture::FixtureRepoBuilder;
use crate::eval::scenarios::{EvalScenario, ScenarioResult, ScenarioStatus};
use crate::git::trailers::CommitTrailers;
use crate::ids::{CheckpointId, MissionId, TaskId};
use crate::persistence::artifacts::fs_store::FsArtifactStore;
use crate::persistence::sqlite::schema::initialize_database;
use crate::repo::drift::RepositoryBaseline;
use crate::state_machine::TaskState;
use crate::state_machine::agent::AgentRole;

pub struct ScenarioF;

#[async_trait]
impl EvalScenario for ScenarioF {
    fn id(&self) -> &'static str {
        "f"
    }

    fn name(&self) -> &'static str {
        "Scenario F: Crash Recovery & Resume"
    }

    fn description(&self) -> &'static str {
        "Simulate runtime crash interruption mid-execution, scan, preserve completed work, and safely resume"
    }

    async fn run(&self) -> Result<ScenarioResult, String> {
        let start = Instant::now();

        // 1. Set up fixture repository
        let mut builder = FixtureRepoBuilder::new().map_err(|e| e.to_string())?;
        builder
            .with_file("src/task1.rs", "pub fn part_one() -> bool { true }\n")
            .with_commit("feat: initial commit for part one");

        let fixture = builder.build().map_err(|e| e.to_string())?;

        // 2. Set up isolated SQLite database and artifact store
        let temp_dir = tempdir().map_err(|e| e.to_string())?;
        let db_path = temp_dir.path().join("scenario_f_crash.db");
        let pool = initialize_database(&db_path)
            .await
            .map_err(|e| e.to_string())?;

        let artifacts_dir = temp_dir.path().join("artifacts");
        let artifact_store = Arc::new(FsArtifactStore::new(&artifacts_dir));

        let mission_id = MissionId::new();
        let task_1_id = TaskId::new();
        let task_2_id = TaskId::new();
        let now = chrono::Utc::now().to_rfc3339();

        // Insert mission and tasks into SQLite
        sqlx::query(
            "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(mission_id.as_bytes().as_slice())
        .bind("Multi-step feature with crash recovery")
        .bind("in_progress")
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

        sqlx::query(
            "INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(task_1_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind("Implement Part One")
        .bind("succeeded")
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

        sqlx::query(
            "INSERT INTO tasks (id, mission_id, title, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(task_2_id.as_bytes().as_slice())
        .bind(mission_id.as_bytes().as_slice())
        .bind("Implement Part Two")
        .bind("running") // in-flight when crash occurs
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

        // 3. Capture baseline for checkpoint manifest
        let baseline = RepositoryBaseline::capture(
            fixture.path(),
            mission_id,
            Some(task_1_id),
            Some("HEAD".to_string()),
        )
        .map_err(|e| e.to_string())?;
        baseline
            .save_to_db(&pool)
            .await
            .map_err(|e| e.to_string())?;

        // Create Checkpoint 1
        let checkpoint_id = CheckpointId::new();
        let mut task_states = BTreeMap::new();
        task_states.insert(task_1_id, TaskState::Succeeded);
        task_states.insert(task_2_id, TaskState::Running);

        let manifest = CheckpointManifest::new(
            checkpoint_id,
            mission_id,
            1,
            "execution",
            1,
            format!("{mission_id}-cp-1"),
            task_states,
            BTreeMap::new(),
            "policy-hash-scenario-f",
            vec![],
            vec![],
            "Checkpoint after task 1",
        );

        let staging_dir = temp_dir.path().join("staging");
        let cp_manager = CheckpointManager::new(pool.clone(), artifact_store.clone(), &staging_dir);
        cp_manager
            .create_checkpoint(&manifest, vec![])
            .await
            .map_err(|e| e.to_string())?;

        // ---------------------------------------------------------------------
        // 4. SIMULATE CRASH: Process is killed/aborted mid-execution
        // ---------------------------------------------------------------------

        // 5. STARTUP RECOVERY: New process launches scanner
        let scanner =
            StartupCrashRecoveryScanner::new(pool.clone(), artifact_store.clone(), fixture.path());
        let scan_result = scanner
            .scan_and_reconcile(mission_id)
            .await
            .map_err(|e| e.to_string())?;

        if scan_result.classification != CrashRecoveryClassification::SafeToResume {
            return Err(format!(
                "Expected SafeToResume classification, got {:?}",
                scan_result.classification
            ));
        }

        // 6. Safe Resume Engine preserves succeeded work and reschedules in-flight task
        let resume_engine =
            SafeResumeEngine::new(pool.clone(), artifact_store.clone(), fixture.path());
        let resume_report = resume_engine
            .resume_mission(mission_id, &manifest)
            .await
            .map_err(|e| e.to_string())?;

        let task1_preserved = resume_report.preserved_tasks.contains(&task_1_id);
        let task2_rescheduled = resume_report.rescheduled_tasks.contains(&task_2_id);

        if !task1_preserved || !task2_rescheduled {
            return Err(format!(
                "Task preservation failed: task1_preserved={}, task2_rescheduled={}",
                task1_preserved, task2_rescheduled
            ));
        }

        // 7. Complete Task 2 in the workspace and commit
        fixture
            .write_file("src/task2.rs", "pub fn part_two() -> bool { true }\n")
            .map_err(|e| e.to_string())?;
        fixture
            .run_git(&["add", "src/task2.rs"])
            .map_err(|e| e.to_string())?;

        let trailers = CommitTrailers::new(
            mission_id,
            task_2_id,
            AgentRole::implementer(),
            "eval-model",
        );

        let commit_msg = CommitTrailers::embed_trailers(
            "feat: complete task 2 after crash resumption",
            &trailers,
        )
        .map_err(|e| e.to_string())?;

        fixture
            .run_git(&["commit", "-m", &commit_msg])
            .map_err(|e| e.to_string())?;

        // Update task 2 to Succeeded in DB
        sqlx::query("UPDATE tasks SET status = 'succeeded' WHERE id = ?")
            .bind(task_2_id.as_bytes().as_slice())
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;

        let duration_ms = start.elapsed().as_millis() as u64;
        let verification_passed = task1_preserved && task2_rescheduled;

        Ok(ScenarioResult {
            scenario_id: "f".to_string(),
            name: self.name().to_string(),
            status: if verification_passed {
                ScenarioStatus::Passed
            } else {
                ScenarioStatus::Failed
            },
            duration_ms,
            tokens_used: 0,
            cost_usd: 0.0,
            verification_passed,
            replans_count: 0,
            retries_count: 0,
            files_modified: 2,
            details: format!(
                "Crash scanned -> classified as SafeToResume. Preserved task {}, rescheduled in-flight task {}.",
                task_1_id, task_2_id
            ),
        })
    }
}
