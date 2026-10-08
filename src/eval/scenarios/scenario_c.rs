//! Acceptance Scenario C: Failure and DAG Replan (CONTEXT_M31A.md §99, TST-01, D-08, D-15).
//!
//! Objectives:
//! 1. Set up isolated fixture containing a compilation failure.
//! 2. Detect failure and classify as `Compilation`.
//! 3. Trigger `DifferentialReplanEngine` to dynamically update the DAG to revision N+1.
//! 4. Durably persist replan in SQLite `recovery_attempts`.
//! 5. Apply corrective fix to fixture, commit with trailers, and verify pass.

use async_trait::async_trait;
use std::time::Instant;
use tempfile::tempdir;

use crate::dag::materializer::TaskGraphMaterializer;
use crate::eval::fixture::FixtureRepoBuilder;
use crate::eval::scenarios::{EvalScenario, ScenarioResult, ScenarioStatus};
use crate::git::trailers::CommitTrailers;
use crate::ids::MissionId;
use crate::kernel::plan::{
    CandidatePlan, CandidateTask, CandidateTaskKey, CapabilityAccessMode, CapabilityRequirement,
    ResourceEstimate, VerificationStrategy,
};
use crate::persistence::sqlite::schema::initialize_database;
use crate::recovery::FailureClassifier;
use crate::recovery::classifier::FailureClassification;
use crate::recovery::replan::{DifferentialReplanEngine, DifferentialReplanRequest};
use crate::state_machine::agent::AgentRole;

pub struct ScenarioC;

fn make_candidate_task(id: &str, objective: &str, deps: Vec<&str>) -> CandidateTask {
    let mut t = CandidateTask::new(
        id,
        objective.to_string(),
        AgentRole::implementer(),
        VerificationStrategy::Compilation,
        ResourceEstimate::default(),
    );
    for d in deps {
        t.depends_on.push(CandidateTaskKey::new(d));
    }
    t.capabilities.push(CapabilityRequirement::new(
        "fs.local",
        CapabilityAccessMode::ReadWrite,
    ));
    t
}

#[async_trait]
impl EvalScenario for ScenarioC {
    fn id(&self) -> &'static str {
        "c"
    }

    fn name(&self) -> &'static str {
        "Scenario C: Failure and DAG Replan"
    }

    fn description(&self) -> &'static str {
        "Detect failure, classify as Compilation, trigger differential replan, and recover successfully"
    }

    async fn run(&self) -> Result<ScenarioResult, String> {
        let start = Instant::now();

        // 1. Set up fixture repository with type mismatch compilation error
        let mut builder = FixtureRepoBuilder::new().map_err(|e| e.to_string())?;
        builder
            .with_file(
                "src/lib.rs",
                "pub fn get_value() -> i32 {\n    \"string_mismatch\" // Error: type mismatch\n}\n",
            )
            .with_commit("feat: initial library implementation");

        let fixture = builder.build().map_err(|e| e.to_string())?;

        // 2. Set up isolated SQLite database
        let temp_db_dir = tempdir().map_err(|e| e.to_string())?;
        let db_path = temp_db_dir.path().join("scenario_c_replan.db");
        let pool = initialize_database(&db_path)
            .await
            .map_err(|e| e.to_string())?;

        let mission_id = MissionId::new();
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO missions (id, objective, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(mission_id.as_bytes().as_slice())
        .bind("Fix compilation error via replan")
        .bind("in_progress")
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

        // 3. Initial Plan (v1): Task 1 (implement feature)
        let plan_v1 = CandidatePlan::new(
            "plan-v1",
            "Initial Plan",
            vec![make_candidate_task(
                "task_1",
                "Implement get_value returning i32",
                vec![],
            )],
        );

        let materializer = TaskGraphMaterializer::new(pool.clone());
        let graph_v1 = materializer
            .materialize(mission_id, &plan_v1)
            .await
            .map_err(|e| e.to_string())?;
        assert_eq!(graph_v1.revision, 1);

        // 4. Detect compilation error and classify deterministically
        let compiler_error = "error[E0308]: mismatched types: expected `i32`, found `&str`";
        let failure_class = FailureClassifier::classify_deterministic(None, compiler_error);
        if failure_class != FailureClassification::Compilation {
            return Err(format!(
                "Expected Compilation classification, got {:?}",
                failure_class
            ));
        }

        // 5. Trigger differential replan: plan_v2 replaces task_1 with corrective task_2
        let plan_v2 = CandidatePlan::new(
            "plan-v2",
            "Corrective Plan after Compilation Error",
            vec![
                make_candidate_task("task_1", "Initial attempt (failed)", vec![]),
                make_candidate_task("task_2", "Fix return type in get_value", vec!["task_1"]),
            ],
        );

        let failed_task_id = *graph_v1
            .candidate_to_task
            .get(&CandidateTaskKey::new("task_1"))
            .ok_or_else(|| "task_1 not in candidate map".to_string())?;

        let replan_engine = DifferentialReplanEngine::new(pool.clone());
        let replan_request = DifferentialReplanRequest {
            mission_id,
            failed_task_id: Some(failed_task_id),
            failure_class,
            diagnosis_or_reason: "Type mismatch error in src/lib.rs".to_string(),
            candidate_plan: plan_v2,
            trigger: None,
        };

        let replan_outcome = replan_engine
            .execute_replan(&graph_v1, replan_request)
            .await
            .map_err(|e| e.to_string())?;

        if replan_outcome.revision != 2 {
            return Err(format!(
                "Expected graph revision 2, got {}",
                replan_outcome.revision
            ));
        }

        // 6. Verify replan is recorded in SQLite recovery_attempts table
        let count_row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM recovery_attempts WHERE mission_id = ?")
                .bind(mission_id.as_bytes().as_slice())
                .fetch_one(&pool)
                .await
                .map_err(|e| e.to_string())?;

        if count_row.0 < 1 {
            return Err("Replan was not persisted in recovery_attempts".to_string());
        }

        // 7. Apply corrective fix to fixture repository
        let fixed_code = "pub fn get_value() -> i32 {\n    42\n}\n\n#[test]\nfn test_get_value() {\n    assert_eq!(get_value(), 42);\n}\n";
        fixture
            .write_file("src/lib.rs", fixed_code)
            .map_err(|e| e.to_string())?;

        fixture
            .run_git(&["add", "src/lib.rs"])
            .map_err(|e| e.to_string())?;

        let trailers = CommitTrailers::new(
            mission_id,
            failed_task_id,
            AgentRole::implementer(),
            "eval-model",
        );

        let commit_msg =
            CommitTrailers::embed_trailers("fix: resolve type mismatch returning i32", &trailers)
                .map_err(|e| e.to_string())?;

        fixture
            .run_git(&["commit", "-m", &commit_msg])
            .map_err(|e| e.to_string())?;

        let diff_out = fixture
            .run_git(&["diff", "HEAD~1..HEAD", "--name-only"])
            .map_err(|e| e.to_string())?;
        let diff_str = String::from_utf8_lossy(&diff_out.stdout);
        let files_changed = diff_str.lines().count();

        let verification_passed =
            replan_outcome.revision == 2 && count_row.0 >= 1 && files_changed == 1;
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(ScenarioResult {
            scenario_id: "c".to_string(),
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
            replans_count: 1,
            retries_count: 0,
            files_modified: files_changed,
            details: format!(
                "Compilation error classified -> DAG replanned to rev 2 (attempt_id: {}). Corrective fix applied.",
                replan_outcome.recovery_attempt_id
            ),
        })
    }
}
