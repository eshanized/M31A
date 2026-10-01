//! End-to-end canonical execution spine verification test for Phase 12 (AD-003).
//!
//! # Invariant
//!
//! "The model proposes. The runtime decides."
//!
//! Validates the full converged pipeline:
//! Declarative Manifest -> Workflow Compiler / Lowering -> Canonical Mission DAG ->
//! SchedulerEngine -> AutonomyController -> VerificationEngine (Law 6) ->
//! CheckpointManager -> Mission Completion.

use m31a::agent::model_policy::{ModelCaller, ModelProposal};
use m31a::controller::dependencies::ControllerDependencies;
use m31a::events::EventBus;
use m31a::events::bus::BroadcastEventBus;
use m31a::ids::MissionId;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::prompt::{InMemoryPromptCatalog, PromptContract};
use m31a::state_machine::agent::AgentRole;
use m31a::workflow::compiler::WorkflowCompiler;
use m31a::workflow::engine::{WorkflowEngine, WorkflowStartRequest};
use m31a::workflow::manifest::WorkflowManifest;
use m31a::workflow::repository::SqliteWorkflowRepository;
use m31a::workflow::state::WorkflowRunState;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use tempfile::tempdir;

/// Initialize minimal valid git and cargo repository fixture.
fn setup_git_cargo_fixture(dir: &Path) {
    let git_init = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(dir)
        .status()
        .expect("git init failed");
    assert!(git_init.success());

    let _ = Command::new("git")
        .args(["config", "user.name", "M31A Test Agent"])
        .current_dir(dir)
        .status();
    let _ = Command::new("git")
        .args(["config", "user.email", "agent@m31a.local"])
        .current_dir(dir)
        .status();

    std::fs::write(dir.join(".gitignore"), "/target\n.m31a\n").unwrap();

    let cargo_toml = r#"[package]
name = "fixture_package"
version = "0.1.0"
edition = "2021"

[dependencies]
"#;
    std::fs::write(dir.join("Cargo.toml"), cargo_toml).unwrap();

    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("src/lib.rs"),
        "pub fn placeholder() -> bool { true }\n",
    )
    .unwrap();

    let _ = Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir)
        .status();
    let _ = Command::new("git")
        .args(["commit", "-m", "Initial fixture commit"])
        .current_dir(dir)
        .status();
}

struct CompletingModelCaller {
    pub summary: String,
}

#[async_trait::async_trait]
impl ModelCaller for CompletingModelCaller {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        Ok(ModelProposal::Complete {
            summary: self.summary.clone(),
            artifacts: vec![],
        })
    }
}

#[tokio::test]
async fn test_canonical_e2e_spine_execution() {
    let dir = tempdir().unwrap();
    setup_git_cargo_fixture(dir.path());

    let m31a_dir = dir.path().join(".m31a");
    std::fs::create_dir_all(&m31a_dir).unwrap();
    let db_path = m31a_dir.join("e2e.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let model = Arc::new(CompletingModelCaller {
        summary: "E2E canonical execution verified".to_string(),
    });

    let bus = Arc::new(BroadcastEventBus::new(128));

    let deps = ControllerDependencies::production_with_model_and_config(
        pool.clone(),
        dir.path().to_path_buf(),
        dir.path().join(".m31a"),
        Some(bus.clone()),
        Some(model),
        None,
    );

    // 1. Declarative TOML manifest definition
    let toml_str = r#"
manifest_version = 1

[workflow]
id = "e2e_canonical_wf"
name = "Canonical E2E Workflow"
description = "Verifies full end-to-end spine under AD-003"
version = 1
default_recovery_strategy = { type = "fail" }

[[steps]]
key = "discovery"
name = "Canonical Discovery"
role = "researcher"
prompt = "discovery_prompt:1"
timeout_secs = 60
allows_parallelism = false
depends_on = []
"#;

    // 2. Parse declarative manifest
    let manifest = WorkflowManifest::from_toml_str(toml_str).expect("parse manifest");

    // 3. Register prompt contract in catalog
    let mut catalog = InMemoryPromptCatalog::new();
    let prompt_contract = PromptContract::new(
        "discovery_prompt",
        1,
        AgentRole::researcher(),
        "Discovery contract",
        vec![],
        "Perform repository discovery",
        Some("markdown".to_string()),
    )
    .unwrap();
    catalog.register(prompt_contract).unwrap();
    let compiler = WorkflowCompiler::new(&catalog);

    // 4. Lower manifest directly through WorkflowCompiler
    let mission_id = MissionId::new();
    let lowered = compiler
        .lower(&manifest, None, None, mission_id)
        .expect("lower manifest");

    assert_eq!(lowered.mission_id, mission_id);
    assert_eq!(lowered.candidate_plan.tasks.len(), 1);
    assert_eq!(lowered.candidate_plan.tasks[0].id.as_str(), "discovery");
    assert_eq!(
        lowered.candidate_plan.tasks[0].role,
        AgentRole::researcher()
    );

    // 5. Compile workflow definition for engine execution
    let compiled = compiler
        .compile(&manifest, None, None)
        .expect("compile manifest");
    let catalog = Arc::new(catalog);

    // 6. Execute through converged WorkflowEngine -> AutonomyController spine
    let repo = Arc::new(SqliteWorkflowRepository::new(pool.clone()));
    let engine = WorkflowEngine::new(repo.clone(), catalog, Some(bus as Arc<dyn EventBus>))
        .with_dependencies(deps.clone());

    let snapshot = engine
        .execute_autonomously(&compiled, WorkflowStartRequest::new(dir.path()))
        .await
        .expect("execute_autonomously failed");

    // 7. Verification of Single Execution Authority Outcomes:
    assert_eq!(
        snapshot.run.status,
        WorkflowRunState::Completed,
        "Workflow run must complete successfully"
    );

    // Canonical projection verified on disk
    let plan_projection = dir
        .path()
        .join(".planning")
        .join("projections")
        .join("plan.json");
    assert!(
        plan_projection.exists(),
        "Canonical plan.json must be written to .planning/projections/"
    );

    // SchedulerEngine: task graph materialized and marked completed
    let completed_tasks: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tasks WHERE status = 'succeeded' OR status = 'completed'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        completed_tasks, 1,
        "Task must be completed in SchedulerEngine"
    );

    // VerificationEngine: Law 6 empirical check recorded in SQLite
    let checks_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM verification_checks WHERE status = 'passed'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        checks_count >= 1,
        "Verification check must be recorded in SQLite"
    );

    // CheckpointManager: checkpoint created in SQLite
    let checkpoints_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM checkpoints")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        checkpoints_count >= 1,
        "Checkpoint must be sealed in SQLite"
    );

    // Mission Aggregate: exactly 1 canonical mission aggregate
    let mission_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM missions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(mission_count, 1, "Exactly 1 canonical mission must exist");
}
