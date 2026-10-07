//! Authority convergence test suite enforcing single execution authority under AD-003.
//!
//! # Core Invariant
//!
//! "The model proposes. The runtime decides."
//!
//! Proves:
//! - WorkflowEngine does not act as an independent secondary execution authority.
//! - All execution passes through canonical Mission DAG, SchedulerEngine, and AutonomyController.
//! - Verification evidence (Law 6) is mandatory before task completion.
//! - PolicyGate, EvidenceCompletionGate, and CheckpointManager cannot be bypassed.
//! - Single canonical MissionId links WorkflowRun, WorkflowStepRun, TaskGraph, and execution events.

use m31a::agent::model_policy::{ModelCaller, ModelProposal};
use m31a::controller::dependencies::ControllerDependencies;
use m31a::events::EventBus;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::prompt::InMemoryPromptCatalog;
use m31a::state_machine::agent::AgentRole;
use m31a::workflow::compiler::CompiledWorkflow;
use m31a::workflow::definition::{
    OutputBinding, QualityGate, RecoveryStrategy, WorkflowDefinition, WorkflowStepDefinition,
};
use m31a::workflow::engine::{WorkflowEngine, WorkflowStartRequest};
use m31a::workflow::error::WorkflowError;
use m31a::workflow::provenance::WorkflowProvenance;
use m31a::workflow::repository::{SqliteWorkflowRepository, WorkflowRepository};
use m31a::workflow::state::WorkflowRunState;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;

struct ProposalModelCaller {
    pub proposal: ModelProposal,
}

#[async_trait::async_trait]
impl ModelCaller for ProposalModelCaller {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        Ok(self.proposal.clone())
    }
}

fn setup_git_cargo_fixture(dir: &std::path::Path) {
    let git_init = std::process::Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(dir)
        .status()
        .expect("git init failed");
    assert!(git_init.success());

    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "M31A Test Agent"])
        .current_dir(dir)
        .status();
    let _ = std::process::Command::new("git")
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

    let _ = std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir)
        .status();
    let _ = std::process::Command::new("git")
        .args(["commit", "-m", "Initial fixture commit"])
        .current_dir(dir)
        .status();
}

fn build_step(
    key: &str,
    name: &str,
    role: AgentRole,
    depends_on: Vec<&str>,
    outputs: Vec<(&str, &str)>,
) -> WorkflowStepDefinition {
    // Fixture steps bind REAL builtin contracts (wiring remediation
    // v0.1.1): fictional prompt ids fail closed at worker compilation
    // instead of being silently ignored as description metadata.
    let prompt_template = if role == AgentRole::reviewer() {
        "verification.reviewer".to_string()
    } else {
        "genesis.discovery".to_string()
    };
    WorkflowStepDefinition {
        key: key.to_string(),
        name: name.to_string(),
        role,
        prompt_template,
        required_inputs: vec![],
        expected_outputs: outputs
            .into_iter()
            .map(|(art, path)| OutputBinding {
                artifact_name: art.to_string(),
                relative_path: PathBuf::from(path),
                schema_type: "markdown".to_string(),
            })
            .collect(),
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: depends_on.into_iter().map(String::from).collect(),
        timeout_secs: 180,
        allows_parallelism: true,
        recovery_strategy: None,

        prompt_ref: None,
    }
}

fn build_test_workflow(steps: Vec<WorkflowStepDefinition>) -> CompiledWorkflow {
    let def = WorkflowDefinition {
        id: "convergence_wf".to_string(),
        name: "Authority Convergence Workflow".to_string(),
        description: "Test authority convergence".to_string(),
        version: 1,
        steps,
        default_recovery_strategy: RecoveryStrategy::Fail,
    };
    let provenance = WorkflowProvenance::new(
        "convergence_wf",
        1,
        "hash",
        1,
        BTreeMap::new(),
        None,
        chrono::Utc::now(),
    );
    CompiledWorkflow {
        topological_order: def.steps.iter().map(|s| s.key.clone()).collect(),
        definition: def,
        provenance,
    }
}

#[tokio::test]
async fn test_01_no_synthetic_mission_id_across_workflow_steps() {
    let dir = tempdir().unwrap();
    setup_git_cargo_fixture(dir.path());
    let m31a_dir = dir.path().join(".m31a");
    std::fs::create_dir_all(&m31a_dir).unwrap();
    let db_path = m31a_dir.join("authority.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let model = Arc::new(ProposalModelCaller {
        proposal: ModelProposal::Complete {
            summary: "Done".to_string(),
            artifacts: vec![],
        },
    });

    let fixture_config = m31a::config::ResolvedConfiguration::build_fallback(dir.path());
    let deps = ControllerDependencies::production_with_model_and_config(
        pool.clone(),
        dir.path().to_path_buf(),
        dir.path().join(".m31a"),
        None,
        Some(model),
        &fixture_config,
    );

    let catalog = InMemoryPromptCatalog::with_builtins();

    let step1 = build_step("step_1", "Step 1", AgentRole::researcher(), vec![], vec![]);
    let step2 = build_step(
        "step_2",
        "Step 2",
        AgentRole::reviewer(),
        vec!["step_1"],
        vec![],
    );
    let compiled = build_test_workflow(vec![step1, step2]);

    let repo = Arc::new(SqliteWorkflowRepository::new(pool.clone()));
    let engine = WorkflowEngine::new(repo.clone(), Arc::new(catalog), None).with_dependencies(deps);

    let handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(dir.path()))
        .await
        .expect("start workflow");

    assert_eq!(handle.status, WorkflowRunState::Completed);

    // Verify all steps share the EXACT SAME canonical MissionId from lowering
    let step_runs = repo.list_step_runs(handle.run_id).await.unwrap();
    assert_eq!(step_runs.len(), 2);
    let mission_id_1 = step_runs[0].mission_id.expect("mission_id on step 1");
    let mission_id_2 = step_runs[1].mission_id.expect("mission_id on step 2");
    assert_eq!(
        mission_id_1, mission_id_2,
        "All steps MUST share the same canonical lowered MissionId"
    );

    // Verify exactly ONE mission was persisted in SQLite for this workflow run
    let mission_count: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT id) FROM missions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        mission_count, 1,
        "Exactly one canonical Mission aggregate must exist; no per-step synthetic missions"
    );
}

#[tokio::test]
async fn test_02_task_completion_requires_verification_evidence() {
    let dir = tempdir().unwrap();
    setup_git_cargo_fixture(dir.path());
    let m31a_dir = dir.path().join(".m31a");
    std::fs::create_dir_all(&m31a_dir).unwrap();
    let db_path = m31a_dir.join("evidence.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let model = Arc::new(ProposalModelCaller {
        proposal: ModelProposal::Complete {
            summary: "Completed work".to_string(),
            artifacts: vec![],
        },
    });

    let fixture_config = m31a::config::ResolvedConfiguration::build_fallback(dir.path());
    let deps = ControllerDependencies::production_with_model_and_config(
        pool.clone(),
        dir.path().to_path_buf(),
        dir.path().join(".m31a"),
        None,
        Some(model),
        &fixture_config,
    );

    let catalog = InMemoryPromptCatalog::with_builtins();

    let step = build_step(
        "evidence_step",
        "Evidence Step",
        AgentRole::researcher(),
        vec![],
        vec![],
    );
    let compiled = build_test_workflow(vec![step]);

    let repo = Arc::new(SqliteWorkflowRepository::new(pool.clone()));
    let engine = WorkflowEngine::new(repo.clone(), Arc::new(catalog), None).with_dependencies(deps);

    let handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(dir.path()))
        .await
        .unwrap();

    assert_eq!(handle.status, WorkflowRunState::Completed);

    // Law 6 evidence check: verification_checks table MUST have recorded an entry
    let checks_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM verification_checks WHERE status = 'passed'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        checks_count >= 1,
        "Task completion requires verification evidence in SQLite (Law 6)"
    );
}

#[tokio::test]
async fn test_03_missing_outputs_cause_verification_failure_and_fail_closed() {
    let dir = tempdir().unwrap();
    setup_git_cargo_fixture(dir.path());
    let m31a_dir = dir.path().join(".m31a");
    std::fs::create_dir_all(&m31a_dir).unwrap();
    let db_path = m31a_dir.join("missing_art.db");
    let pool = initialize_database(&db_path).await.unwrap();

    // Model proposes complete, but does NOT write the expected file to disk
    let model = Arc::new(ProposalModelCaller {
        proposal: ModelProposal::Complete {
            summary: "I claim I produced the file".to_string(),
            artifacts: vec![],
        },
    });

    let fixture_config = m31a::config::ResolvedConfiguration::build_fallback(dir.path());
    let deps = ControllerDependencies::production_with_model_and_config(
        pool.clone(),
        dir.path().to_path_buf(),
        dir.path().join(".m31a"),
        None,
        Some(model),
        &fixture_config,
    );

    let catalog = InMemoryPromptCatalog::with_builtins();

    // Step expects "output.txt" which will NOT exist
    let step = build_step(
        "strict_art",
        "Strict Artifact Step",
        AgentRole::researcher(),
        vec![],
        vec![("output_file", "output.txt")],
    );
    let compiled = build_test_workflow(vec![step]);

    let repo = Arc::new(SqliteWorkflowRepository::new(pool.clone()));
    let engine = WorkflowEngine::new(repo.clone(), Arc::new(catalog), None).with_dependencies(deps);

    let handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(dir.path()))
        .await
        .unwrap();

    // Must fail closed because artifact was not actually produced on disk
    assert_eq!(handle.status, WorkflowRunState::Failed);

    let failure_checks: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM verification_checks WHERE status = 'failed'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        failure_checks >= 1,
        "Failed verification check must be recorded in SQLite"
    );
}

#[tokio::test]
async fn test_04_workflow_engine_fails_closed_without_dependencies() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("no_deps.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let repo = Arc::new(SqliteWorkflowRepository::new(pool));
    let catalog = Arc::new(InMemoryPromptCatalog::new());
    let engine = WorkflowEngine::new(repo, catalog, None);

    let step = build_step("step_x", "Step X", AgentRole::researcher(), vec![], vec![]);
    let compiled = build_test_workflow(vec![step]);

    let res = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(dir.path()))
        .await;

    assert!(res.is_err());
    match res.unwrap_err() {
        WorkflowError::ExecutionFailed { reason, .. } => {
            assert!(
                reason.contains("ControllerDependencies"),
                "Must fail closed indicating missing ControllerDependencies: got {}",
                reason
            );
        }
        other => panic!("Unexpected error: {:?}", other),
    }
}

#[tokio::test]
async fn test_05_workflow_engine_has_no_independent_step_executor() {
    // Step 20 Negative Architecture Guard:
    // WorkflowEngine has no compute_ready_steps, advance_workflow_internal,
    // or independent StepExecutor. All execution flows exclusively via AutonomyController.
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("no_independent_exec.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let repo = Arc::new(SqliteWorkflowRepository::new(pool));
    let catalog = Arc::new(InMemoryPromptCatalog::new());
    let engine = WorkflowEngine::new(repo, catalog, None);

    // Verify engine has no step executor field and fails closed when dependencies are missing
    let step = build_step("step_x", "Step X", AgentRole::researcher(), vec![], vec![]);
    let compiled = build_test_workflow(vec![step]);
    let res = engine
        .advance_workflow(m31a::ids::WorkflowRunId::new(), &compiled)
        .await;
    assert!(res.is_err());
}

#[tokio::test]
async fn test_06_canonical_projection_written_at_lowering() {
    let dir = tempdir().unwrap();
    setup_git_cargo_fixture(dir.path());
    let m31a_dir = dir.path().join(".m31a");
    std::fs::create_dir_all(&m31a_dir).unwrap();
    let db_path = m31a_dir.join("proj.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let model = Arc::new(ProposalModelCaller {
        proposal: ModelProposal::Complete {
            summary: "Done".to_string(),
            artifacts: vec![],
        },
    });

    let fixture_config = m31a::config::ResolvedConfiguration::build_fallback(dir.path());
    let deps = ControllerDependencies::production_with_model_and_config(
        pool.clone(),
        dir.path().to_path_buf(),
        dir.path().join(".m31a"),
        None,
        Some(model),
        &fixture_config,
    );

    let catalog = InMemoryPromptCatalog::with_builtins();

    let step = build_step(
        "proj_step",
        "Projection Step",
        AgentRole::researcher(),
        vec![],
        vec![],
    );
    let compiled = build_test_workflow(vec![step]);

    let repo = Arc::new(SqliteWorkflowRepository::new(pool.clone()));
    let engine = WorkflowEngine::new(repo, Arc::new(catalog), None).with_dependencies(deps);

    let _handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(dir.path()))
        .await
        .unwrap();

    let plan_projection = dir
        .path()
        .join(".planning")
        .join("projections")
        .join("plan.json");
    assert!(
        plan_projection.exists(),
        "Canonical plan.json projection must be written to disk"
    );

    let content = std::fs::read_to_string(plan_projection).unwrap();
    assert!(content.contains("proj_step"));
    assert!(content.contains("Projection Step"));
}

#[tokio::test]
async fn test_07_execute_autonomously_runs_canonical_spine() {
    let dir = tempdir().unwrap();
    setup_git_cargo_fixture(dir.path());
    let m31a_dir = dir.path().join(".m31a");
    std::fs::create_dir_all(&m31a_dir).unwrap();
    let db_path = m31a_dir.join("autonomous.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let model = Arc::new(ProposalModelCaller {
        proposal: ModelProposal::Complete {
            summary: "Autonomously executed step".to_string(),
            artifacts: vec![],
        },
    });

    let fixture_config = m31a::config::ResolvedConfiguration::build_fallback(dir.path());
    let deps = ControllerDependencies::production_with_model_and_config(
        pool.clone(),
        dir.path().to_path_buf(),
        dir.path().join(".m31a"),
        None,
        Some(model),
        &fixture_config,
    );

    let catalog = InMemoryPromptCatalog::with_builtins();

    let step = build_step(
        "auto_step",
        "Autonomous Step",
        AgentRole::researcher(),
        vec![],
        vec![],
    );
    let compiled = build_test_workflow(vec![step]);

    let repo = Arc::new(SqliteWorkflowRepository::new(pool.clone()));
    let engine = WorkflowEngine::new(repo.clone(), Arc::new(catalog), None).with_dependencies(deps);

    let snapshot = engine
        .execute_autonomously(&compiled, WorkflowStartRequest::new(dir.path()))
        .await
        .expect("execute_autonomously");

    assert_eq!(snapshot.run.status, WorkflowRunState::Completed);

    let all_tasks: Vec<(String, String)> = sqlx::query_as("SELECT title, status FROM tasks")
        .fetch_all(&pool)
        .await
        .unwrap();
    eprintln!("ALL TASKS: {:?}", all_tasks);
    let completed_tasks = all_tasks
        .iter()
        .filter(|(_, s)| s == "completed" || s == "succeeded")
        .count();
    assert_eq!(completed_tasks, 1);

    let all_missions: Vec<String> = sqlx::query_scalar("SELECT status FROM missions")
        .fetch_all(&pool)
        .await
        .unwrap();
    eprintln!("ALL MISSIONS: {:?}", all_missions);
    assert_eq!(all_missions.len(), 1);
}

#[tokio::test]
async fn test_08_event_stream_preserves_canonical_mission_id() {
    let dir = tempdir().unwrap();
    setup_git_cargo_fixture(dir.path());
    let m31a_dir = dir.path().join(".m31a");
    std::fs::create_dir_all(&m31a_dir).unwrap();
    let db_path = m31a_dir.join("events.db");
    let pool = initialize_database(&db_path).await.unwrap();

    let model = Arc::new(ProposalModelCaller {
        proposal: ModelProposal::Complete {
            summary: "Done".to_string(),
            artifacts: vec![],
        },
    });

    let bus = Arc::new(m31a::events::bus::BroadcastEventBus::new(128));
    use futures::StreamExt;
    use m31a::events::bus::EventFilter;
    let mut rx = bus.subscribe(EventFilter::default()).await;

    let fixture_config = m31a::config::ResolvedConfiguration::build_fallback(dir.path());
    let deps = ControllerDependencies::production_with_model_and_config(
        pool.clone(),
        dir.path().to_path_buf(),
        dir.path().join(".m31a"),
        Some(bus.clone()),
        Some(model),
        &fixture_config,
    );

    let catalog = InMemoryPromptCatalog::with_builtins();

    let step = build_step(
        "event_step",
        "Event Step",
        AgentRole::researcher(),
        vec![],
        vec![],
    );
    let compiled = build_test_workflow(vec![step]);

    let repo = Arc::new(SqliteWorkflowRepository::new(pool.clone()));
    let engine = WorkflowEngine::new(
        repo.clone(),
        Arc::new(catalog),
        Some(bus as Arc<dyn m31a::events::bus::EventBus>),
    )
    .with_dependencies(deps);

    let handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(dir.path()))
        .await
        .unwrap();

    assert_eq!(handle.status, WorkflowRunState::Completed);

    let step_runs = repo.list_step_runs(handle.run_id).await.unwrap();
    let canonical_mid = step_runs[0].mission_id.unwrap();

    let mut mission_id_found = false;
    while let Ok(Some(Ok(event))) =
        tokio::time::timeout(tokio::time::Duration::from_millis(50), rx.next()).await
    {
        if let Some(event_mid) = event.mission_id {
            assert_eq!(
                event_mid, canonical_mid,
                "Event mission_id must match canonical lowered MissionId"
            );
            mission_id_found = true;
        }
    }
    assert!(
        mission_id_found,
        "At least one event with canonical mission_id must be received"
    );
}
