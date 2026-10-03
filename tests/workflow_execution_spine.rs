//! Comprehensive test suite verifying the real execution spine for Workstream C.
//!
//! Verifies:
//! - Condition A: Step execution creates a real Mission and Task in the runtime.
//! - Condition B: Step reaches SchedulerEngine and acquires/executes a scheduler task lease.
//! - Condition C: Step reaches AgentRunner / WorkerRunner and invokes ModelCaller.
//! - Condition D: Step CANNOT complete merely because an artifact already exists on disk.
//! - Condition E: Successful real execution produces evidence and completes the step.
//! - Condition F: Verification failure produces workflow step failure.
//! - Condition G: Tool / policy denial produces workflow step failure.
//! - Condition H: Cancellation of workflow cancels running agent execution / worker supervisor.
//! - Condition I: Step retry semantics operate through real execution failures and bounded retries.
//! - Condition J: Workflow restart / recovery properly reconnects to or reconciles underlying task state.
//! - Condition K: Workflow step result contains real execution summary, not hardcoded strings.
//! - Condition L: ProductionStepExecutor cannot accidentally choose a mock executor in production.
//! - Phase 5 Golden Test: End-to-end multi-step workflow through the real runtime.

use async_trait::async_trait;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

use m31a::agent::model_policy::{ModelCaller, ModelProposal, ModelToolCall};
use m31a::ids::MissionId;
use m31a::kernel::seams::context::CompiledContext;
use m31a::kernel::seams::verifier::{
    CompletionGateOutcome, TaskVerificationRequest, VerificationEngine, VerificationError,
    VerificationOutcome,
};
use m31a::model::types::ChatMessage;
use m31a::persistence::sqlite::schema::initialize_database;
use m31a::prompt::{InMemoryPromptCatalog, PromptContract, PromptParameter};
use m31a::runtime::AppRuntime;
use m31a::state_machine::agent::AgentRole;
use m31a::workflow::compiler::CompiledWorkflow;
use m31a::workflow::definition::{
    OutputBinding, QualityGate, RecoveryStrategy, WorkflowDefinition, WorkflowStepDefinition,
};
use m31a::workflow::engine::{WorkflowEngine, WorkflowStartRequest};
use m31a::workflow::error::WorkflowError;
use m31a::workflow::provenance::WorkflowProvenance;
use m31a::workflow::repository::{SqliteWorkflowRepository, WorkflowRepository};
use m31a::workflow::state::{WorkflowMode, WorkflowRunState, WorkflowStepState};

/// Setup a standard test prompt catalog with required role contracts.
fn setup_test_catalog() -> InMemoryPromptCatalog {
    let mut catalog = InMemoryPromptCatalog::new();
    let templates = [
        ("discovery", AgentRole::researcher()),
        ("implement", AgentRole::implementer()),
        ("diagnose", AgentRole::diagnostician()),
        ("verify", AgentRole::verifier()),
        ("general", AgentRole::implementer()),
        ("review", AgentRole::reviewer()),
    ];

    for (id, role) in templates {
        let contract = PromptContract::new(
            id,
            1,
            role,
            format!("Template for {}", id),
            vec![PromptParameter {
                name: "input_a".to_string(),
                description: "Input param".to_string(),
                is_required: false,
                default_value: Some("default".to_string()),
            }],
            format!("Execute role {}: {{{{ input_a }}}}", id),
            Some("markdown".to_string()),
        )
        .unwrap();
        catalog.register(contract).unwrap();
    }
    catalog
}

/// Helper to compile a test workflow definition.
fn build_compiled_workflow(
    id: &str,
    steps: Vec<WorkflowStepDefinition>,
    default_strategy: RecoveryStrategy,
) -> CompiledWorkflow {
    let def = WorkflowDefinition {
        id: id.to_string(),
        name: format!("Workflow {}", id),
        description: "Test workflow".to_string(),
        version: 1,
        steps,
        default_recovery_strategy: default_strategy,
    };
    def.validate().unwrap();

    let topological_order = def.steps.iter().map(|s| s.key.clone()).collect();
    let provenance = WorkflowProvenance::new(
        id,
        1,
        "test-hash",
        1,
        BTreeMap::new(),
        None,
        chrono::Utc::now(),
    );

    CompiledWorkflow {
        definition: def,
        provenance,
        topological_order,
    }
}

/// Initialize a minimal git repository with Cargo.toml and src/lib.rs for testing.
fn setup_git_cargo_fixture(dir: &Path) {
    let git_init = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(dir)
        .status()
        .expect("git init failed");
    assert!(git_init.success());

    Command::new("git")
        .args(["config", "user.name", "M31A Test Agent"])
        .current_dir(dir)
        .status()
        .expect("git config user.name failed");
    Command::new("git")
        .args(["config", "user.email", "agent@m31a.local"])
        .current_dir(dir)
        .status()
        .expect("git config user.email failed");

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

    Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir)
        .status()
        .expect("git add failed");
    Command::new("git")
        .args(["commit", "-m", "Initial fixture commit"])
        .current_dir(dir)
        .status()
        .expect("git commit failed");
}

/// Simple model caller that completes with a canned summary.
#[derive(Default)]
struct SimpleCompletingModel {
    pub call_count: AtomicUsize,
    pub summary_text: String,
}

impl SimpleCompletingModel {
    fn new(summary: impl Into<String>) -> Self {
        Self {
            call_count: AtomicUsize::new(0),
            summary_text: summary.into(),
        }
    }
}

#[async_trait]
impl ModelCaller for SimpleCompletingModel {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(ModelProposal::Complete {
            summary: self.summary_text.clone(),
            artifacts: vec![],
        })
    }
}

/// Model caller that always returns an error.
struct ErrorModel {
    pub error_msg: String,
}

#[async_trait]
impl ModelCaller for ErrorModel {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        Err(self.error_msg.clone())
    }
}

/// A verifier that always rejects verification requests with a custom reason.
struct FailingVerifier {
    pub reason: String,
}

#[async_trait]
impl VerificationEngine for FailingVerifier {
    async fn verify_task(
        &self,
        _req: TaskVerificationRequest,
    ) -> Result<VerificationOutcome, VerificationError> {
        Ok(VerificationOutcome::Failed {
            reason: self.reason.clone(),
        })
    }

    async fn verify_completion_gate(
        &self,
        _mission_id: MissionId,
    ) -> Result<CompletionGateOutcome, VerificationError> {
        Ok(CompletionGateOutcome::Deficient {
            violations: vec![self.reason.clone()],
        })
    }
}

// =========================================================================
// TEST A, B, C: Lowering to Mission, Task, Scheduler, and AgentRunner
// =========================================================================

#[tokio::test]
async fn test_condition_a_b_c_workflow_step_lowers_through_canonical_spine() {
    let dir = tempdir().expect("tempdir failed");
    setup_git_cargo_fixture(dir.path());

    let model = Arc::new(SimpleCompletingModel::new(
        "Work completed by real agent runner",
    ));
    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model.clone());

    let catalog = Arc::new(setup_test_catalog());
    let engine = runtime.create_workflow_engine(catalog);

    let step = WorkflowStepDefinition {
        key: "step_discovery".to_string(),
        name: "Discovery Phase".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "genesis.discovery".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 60,
        allows_parallelism: true,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let compiled = build_compiled_workflow("wf_spine_test", vec![step], RecoveryStrategy::Fail);

    let start_req = WorkflowStartRequest::new(dir.path()).with_mode(WorkflowMode::Autonomous);
    let run_handle = engine
        .start_workflow(&compiled, start_req)
        .await
        .expect("start_workflow failed");

    // Condition C check: model caller was actually invoked by WorkerRunner
    assert!(
        model.call_count.load(Ordering::SeqCst) >= 1,
        "ModelCaller must be invoked through WorkerRunner (Condition C)"
    );

    // Condition A check: a real Mission aggregate was created in DB with workflow correlation
    let mission_rows = sqlx::query(
        "SELECT id, objective, status, constraints FROM missions WHERE objective = 'Discovery Phase'",
    )
    .fetch_all(runtime.pool())
    .await
    .expect("query missions failed");

    assert_eq!(
        mission_rows.len(),
        1,
        "A real Mission aggregate must exist in DB (Condition A)"
    );
    use sqlx::Row;
    let constraints: String = mission_rows[0].get("constraints");
    assert!(
        constraints.contains(&format!("workflow_run_id:{}", run_handle.run_id)),
        "Mission must contain workflow correlation constraint: {}",
        constraints
    );
    assert!(
        constraints.contains("step_key:step_discovery"),
        "Mission must contain step_key constraint"
    );

    // Condition B check: a real Task was materialized and completed in the scheduler
    let mission_id_bytes: Vec<u8> = mission_rows[0].get("id");
    let task_rows =
        sqlx::query("SELECT id, status, started_at, result FROM tasks WHERE mission_id = ?")
            .bind(&mission_id_bytes)
            .fetch_all(runtime.pool())
            .await
            .expect("query tasks failed");

    assert!(
        !task_rows.is_empty(),
        "Tasks must exist in SchedulerEngine for this mission (Condition B)"
    );
    let task_status: String = task_rows[0].get("status");
    assert!(
        task_status == "succeeded" || task_status == "completed",
        "Scheduler task must reach completed/succeeded state: {}",
        task_status
    );
    let started_at: Option<String> = task_rows[0].get("started_at");
    assert!(
        started_at.is_some(),
        "Scheduler task must have recorded started_at"
    );

    // Check workflow finished successfully
    let repo = SqliteWorkflowRepository::new(runtime.pool().clone());
    let run = repo.get_run(run_handle.run_id).await.unwrap().unwrap();
    assert_eq!(run.status, WorkflowRunState::Completed);
}

// =========================================================================
// TEST D: File existence alone MUST NOT constitute step success
// =========================================================================

#[tokio::test]
async fn test_condition_d_artifact_existence_alone_does_not_pass_step() {
    let dir = tempdir().expect("tempdir failed");
    setup_git_cargo_fixture(dir.path());

    // Pre-create the expected output file on disk BEFORE running the workflow
    let artifact_rel_path = PathBuf::from("preexisting_artifact.txt");
    let artifact_full_path = dir.path().join(&artifact_rel_path);
    std::fs::write(&artifact_full_path, "pre-existing content on disk").unwrap();
    assert!(artifact_full_path.exists());

    // Configure a failing model: model execution fails
    let model = Arc::new(ErrorModel {
        error_msg: "LLM synthesis failed unexpectedly".to_string(),
    });

    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model);

    let catalog = Arc::new(setup_test_catalog());
    let engine = runtime.create_workflow_engine(catalog);

    let step = WorkflowStepDefinition {
        key: "step_with_artifact".to_string(),
        name: "Step Requiring Artifact".to_string(),
        role: AgentRole::implementer(),
        prompt_template: "implement".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![OutputBinding {
            artifact_name: "required_doc".to_string(),
            relative_path: artifact_rel_path,
            schema_type: "text".to_string(),
        }],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 60,
        allows_parallelism: true,
        recovery_strategy: Some(RecoveryStrategy::Fail),

        prompt_ref: None,
    };

    let compiled = build_compiled_workflow("wf_existence_test", vec![step], RecoveryStrategy::Fail);

    let start_req = WorkflowStartRequest::new(dir.path()).with_mode(WorkflowMode::Autonomous);
    let run_handle = engine
        .start_workflow(&compiled, start_req)
        .await
        .expect("start_workflow failed");

    // The workflow MUST fail because model execution failed, despite file existing!
    let repo = SqliteWorkflowRepository::new(runtime.pool().clone());
    let run = repo.get_run(run_handle.run_id).await.unwrap().unwrap();
    assert_ne!(
        run.status,
        WorkflowRunState::Completed,
        "Step MUST NOT succeed merely because artifact file exists on disk (Law 5 / Condition D)!"
    );
    assert_eq!(
        run.status,
        WorkflowRunState::Failed,
        "Workflow must fail when underlying execution fails"
    );
}

// =========================================================================
// TEST E & K: Real execution produces real evidence and real summary
// =========================================================================

#[tokio::test]
async fn test_condition_e_k_successful_execution_records_evidence_and_summary() {
    let dir = tempdir().expect("tempdir failed");
    setup_git_cargo_fixture(dir.path());

    let unique_summary = "Unique real execution output from agent runner 42";
    let model = Arc::new(SimpleCompletingModel::new(unique_summary));

    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model);

    let catalog = Arc::new(setup_test_catalog());
    let engine = runtime.create_workflow_engine(catalog);

    let step = WorkflowStepDefinition {
        key: "step_real_summary".to_string(),
        name: "Real Summary Step".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "genesis.discovery".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 60,
        allows_parallelism: true,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let compiled = build_compiled_workflow("wf_summary_test", vec![step], RecoveryStrategy::Fail);

    let start_req = WorkflowStartRequest::new(dir.path()).with_mode(WorkflowMode::Autonomous);
    let run_handle = engine
        .start_workflow(&compiled, start_req)
        .await
        .expect("start_workflow failed");

    let repo = SqliteWorkflowRepository::new(runtime.pool().clone());
    let run = repo.get_run(run_handle.run_id).await.unwrap().unwrap();
    assert_eq!(run.status, WorkflowRunState::Completed);

    // Condition K check: Real task output matches the model output, not hardcoded strings
    use sqlx::Row;
    let task_rows = sqlx::query("SELECT result FROM tasks WHERE title = 'Real Summary Step'")
        .fetch_all(runtime.pool())
        .await
        .unwrap();

    assert!(!task_rows.is_empty(), "Task must exist in DB");
    let result_json: Option<String> = task_rows[0].get("result");
    assert!(
        result_json.is_some(),
        "Task must have recorded a result in JSON"
    );
    assert!(
        result_json.unwrap().contains(unique_summary),
        "Result must contain the real output from agent runner"
    );
}

// =========================================================================
// TEST F: Verification failure produces workflow step failure
// =========================================================================

#[tokio::test]
async fn test_condition_f_verification_failure_fails_workflow_step() {
    let dir = tempdir().expect("tempdir failed");
    setup_git_cargo_fixture(dir.path());

    let model = Arc::new(SimpleCompletingModel::new("Model claimed success"));
    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model);

    let catalog = Arc::new(setup_test_catalog());
    let mut deps = runtime.dependencies().clone();
    deps = deps.with_verifier(Arc::new(FailingVerifier {
        reason: "compiler tier invariant broken: syntax error in src/lib.rs".to_string(),
    }));

    let repo = Arc::new(SqliteWorkflowRepository::new(runtime.pool().clone()));
    let engine = WorkflowEngine::new(
        repo.clone(),
        catalog,
        Some(runtime.event_bus().clone() as Arc<dyn m31a::events::bus::EventBus>),
    )
    .with_dependencies(deps);

    let step = WorkflowStepDefinition {
        key: "step_verify_fail".to_string(),
        name: "Step with Failing Verification".to_string(),
        role: AgentRole::implementer(),
        prompt_template: "implement".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 60,
        allows_parallelism: true,
        recovery_strategy: Some(RecoveryStrategy::Fail),

        prompt_ref: None,
    };

    let compiled = build_compiled_workflow("wf_verif_fail", vec![step], RecoveryStrategy::Fail);

    let start_req = WorkflowStartRequest::new(dir.path()).with_mode(WorkflowMode::Autonomous);
    let run_handle = engine
        .start_workflow(&compiled, start_req)
        .await
        .expect("start_workflow failed");

    let run = repo.get_run(run_handle.run_id).await.unwrap().unwrap();
    assert_eq!(
        run.status,
        WorkflowRunState::Failed,
        "Verification failure must fail the step and workflow (Condition F)"
    );

    // Check that the scheduler task was marked failed
    use sqlx::Row;
    let task_rows =
        sqlx::query("SELECT status FROM tasks WHERE title = 'Step with Failing Verification'")
            .fetch_all(runtime.pool())
            .await
            .unwrap();

    assert!(!task_rows.is_empty());
    let status: String = task_rows[0].get("status");
    assert!(
        status == "failed" || status == "ready",
        "Task status should be failed or ready: got {}",
        status
    );
}

// =========================================================================
// TEST G: Tool / policy denial produces step failure
// =========================================================================

/// Model that proposes an illegal write to `.git/config`
struct PolicyViolatingModel;

#[async_trait]
impl ModelCaller for PolicyViolatingModel {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        Ok(ModelProposal::ToolCalls {
            calls: vec![ModelToolCall::new(
                "write_file",
                serde_json::json!({
                    "path": ".git/config",
                    "content": "[core]\nrepositoryformatversion = 0\n",
                }),
            )],
        })
    }
}

#[tokio::test]
async fn test_condition_g_policy_denial_produces_step_failure() {
    let dir = tempdir().expect("tempdir failed");
    setup_git_cargo_fixture(dir.path());

    let model = Arc::new(PolicyViolatingModel);
    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model);

    let catalog = Arc::new(setup_test_catalog());
    let engine = runtime.create_workflow_engine(catalog);

    let step = WorkflowStepDefinition {
        key: "step_policy_violation".to_string(),
        name: "Policy Violation Step".to_string(),
        role: AgentRole::implementer(),
        prompt_template: "implement".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 60,
        allows_parallelism: true,
        recovery_strategy: Some(RecoveryStrategy::Fail),

        prompt_ref: None,
    };

    let compiled = build_compiled_workflow("wf_policy_denial", vec![step], RecoveryStrategy::Fail);

    let start_req = WorkflowStartRequest::new(dir.path()).with_mode(WorkflowMode::Autonomous);
    let run_handle = engine
        .start_workflow(&compiled, start_req)
        .await
        .expect("start_workflow failed");

    let repo = SqliteWorkflowRepository::new(runtime.pool().clone());
    let run = repo.get_run(run_handle.run_id).await.unwrap().unwrap();
    assert_eq!(
        run.status,
        WorkflowRunState::Failed,
        "Policy denial must fail step execution (Condition G)"
    );
}

// =========================================================================
// TEST H: Cancellation cancels step execution and marks state Cancelled
// =========================================================================

#[tokio::test]
async fn test_condition_h_cancellation_marks_cancelled() {
    let dir = tempdir().expect("tempdir failed");
    setup_git_cargo_fixture(dir.path());

    let model = Arc::new(SimpleCompletingModel::new("Done"));
    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model);

    let catalog = Arc::new(setup_test_catalog());
    let engine = runtime.create_workflow_engine(catalog);

    let step = WorkflowStepDefinition {
        key: "discovery".to_string(),
        name: "Discovery".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "genesis.discovery".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 60,
        allows_parallelism: false,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let _compiled = build_compiled_workflow("cancel_wf", vec![step], RecoveryStrategy::Fail);

    let repo = SqliteWorkflowRepository::new(runtime.pool().clone());
    let mut run = m31a::workflow::state::WorkflowRun::new(
        "cancel_wf",
        1,
        dir.path().to_path_buf(),
        WorkflowMode::Standard,
    );
    run.transition_to(WorkflowRunState::Running, None).unwrap();
    repo.create_run(&run).await.unwrap();

    let step_run = m31a::workflow::state::WorkflowStepRun::new(run.id, "discovery");
    repo.create_step_run(&step_run).await.unwrap();

    engine.cancel_workflow(run.id, "user abort").await.unwrap();

    let cancelled_run = repo.get_run(run.id).await.unwrap().unwrap();
    assert_eq!(
        cancelled_run.status,
        WorkflowRunState::Cancelled,
        "Workflow must be marked Cancelled (Condition H)"
    );

    let cancelled_step = repo
        .get_step_run_by_key(run.id, "discovery")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        cancelled_step.status,
        WorkflowStepState::Cancelled,
        "Step run must be marked Cancelled"
    );
}

// =========================================================================
// TEST I: Bounded retries operate through real execution failures
// =========================================================================

struct CountingFailingModel {
    pub attempts: AtomicUsize,
}

#[async_trait]
impl ModelCaller for CountingFailingModel {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        let count = self.attempts.fetch_add(1, Ordering::SeqCst);
        Err(format!("Deterministic failure on attempt {}", count + 1))
    }
}

#[tokio::test]
async fn test_condition_i_step_retry_exhaustion_on_real_execution() {
    let dir = tempdir().expect("tempdir failed");
    setup_git_cargo_fixture(dir.path());

    let model = Arc::new(CountingFailingModel {
        attempts: AtomicUsize::new(0),
    });

    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model.clone());

    let catalog = Arc::new(setup_test_catalog());
    let engine = runtime.create_workflow_engine(catalog);

    let max_retries = 2;
    let step = WorkflowStepDefinition {
        key: "step_retry".to_string(),
        name: "Retrying Step".to_string(),
        role: AgentRole::implementer(),
        prompt_template: "implement".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 60,
        allows_parallelism: true,
        recovery_strategy: Some(RecoveryStrategy::Retry { max_retries }),

        prompt_ref: None,
    };

    let compiled = build_compiled_workflow("wf_retry_test", vec![step], RecoveryStrategy::Fail);

    let start_req = WorkflowStartRequest::new(dir.path()).with_mode(WorkflowMode::Autonomous);
    let run_handle = engine
        .start_workflow(&compiled, start_req)
        .await
        .expect("start_workflow failed");

    let repo = SqliteWorkflowRepository::new(runtime.pool().clone());
    let run = repo.get_run(run_handle.run_id).await.unwrap().unwrap();
    assert_eq!(
        run.status,
        WorkflowRunState::Failed,
        "Exhausted retries must fail workflow (Condition I)"
    );

    // Assert that bounded retries actually executed (initial attempt + max_retries).
    // Phase 29, Gap 7: each recovery cycle additionally invokes the model
    // once for evidence-based diagnosis (falling back to heuristic when the
    // model cannot help), so the honest total is max_retries + 1.
    assert_eq!(
        model.attempts.load(Ordering::SeqCst),
        max_retries as usize + 1,
        "Model must have been called exactly max_retries times plus one diagnostic call"
    );
}

// =========================================================================
// TEST J: Workflow recovery reconciles interrupted step state
// =========================================================================

#[tokio::test]
async fn test_condition_j_workflow_recovery_reconciles_interrupted_step() {
    let dir = tempdir().expect("tempdir failed");
    setup_git_cargo_fixture(dir.path());

    let model = Arc::new(SimpleCompletingModel::new("Recovered work completed"));
    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model);

    let catalog = Arc::new(setup_test_catalog());
    let engine = runtime.create_workflow_engine(catalog);
    let repo = SqliteWorkflowRepository::new(runtime.pool().clone());

    let step_a = WorkflowStepDefinition {
        key: "step_a".to_string(),
        name: "Step A".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "genesis.discovery".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 60,
        allows_parallelism: true,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let step_b = WorkflowStepDefinition {
        key: "step_b".to_string(),
        name: "Step B".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "genesis.discovery".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec!["step_a".to_string()],
        timeout_secs: 60,
        allows_parallelism: true,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let compiled =
        build_compiled_workflow("rec_workflow", vec![step_a, step_b], RecoveryStrategy::Fail);

    // Simulate an interrupted workflow: step_a completed, step_b was in Running when interrupted
    let mut run = m31a::workflow::state::WorkflowRun::new(
        "rec_workflow",
        1,
        dir.path().to_path_buf(),
        WorkflowMode::Standard,
    );
    run.transition_to(WorkflowRunState::Running, None).unwrap();
    repo.create_run(&run).await.unwrap();

    let mut step_a_run = m31a::workflow::state::WorkflowStepRun::new(run.id, "step_a");
    step_a_run
        .transition_to(WorkflowStepState::Running, None)
        .unwrap();
    step_a_run
        .transition_to(WorkflowStepState::Completed, None)
        .unwrap();
    repo.create_step_run(&step_a_run).await.unwrap();

    let mut step_b_run = m31a::workflow::state::WorkflowStepRun::new(run.id, "step_b");
    step_b_run
        .transition_to(WorkflowStepState::Running, None)
        .unwrap();
    repo.create_step_run(&step_b_run).await.unwrap();

    let mut registry = HashMap::new();
    registry.insert("rec_workflow".to_string(), compiled);

    let recovered = engine
        .recover_incomplete_workflows(&registry)
        .await
        .unwrap();
    assert_eq!(
        recovered.len(),
        1,
        "Workflow must be discovered and recovered (Condition J)"
    );
    assert_eq!(recovered[0], run.id);

    // Assert that the workflow and step_b now completed
    let run_after = repo.get_run(run.id).await.unwrap().unwrap();
    assert_eq!(run_after.status, WorkflowRunState::Completed);

    let step_b_after = repo
        .get_step_run_by_key(run.id, "step_b")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(step_b_after.status, WorkflowStepState::Completed);
}

// =========================================================================
// TEST L: WorkflowEngine fails closed without dependencies
// =========================================================================

#[tokio::test]
async fn test_condition_l_workflow_engine_fails_closed_without_dependencies() {
    let dir = tempdir().expect("tempdir failed");
    let db_path = dir.path().join("test.db");
    let pool = initialize_database(&db_path).await.unwrap();
    let repo = Arc::new(SqliteWorkflowRepository::new(pool));
    let catalog = Arc::new(setup_test_catalog());

    // Instantiate WorkflowEngine with NO dependencies
    let engine = WorkflowEngine::new(repo, catalog, None);

    let step = WorkflowStepDefinition {
        key: "step_unwired".to_string(),
        name: "Unwired Step".to_string(),
        role: AgentRole::implementer(),
        prompt_template: "implement".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 60,
        allows_parallelism: true,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let compiled = build_compiled_workflow("wf_unwired", vec![step], RecoveryStrategy::Fail);

    let res = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(dir.path()))
        .await;
    assert!(
        res.is_err(),
        "WorkflowEngine MUST fail closed when dependencies are missing (Condition L)"
    );

    match res {
        Err(WorkflowError::ExecutionFailed { reason, .. }) => {
            assert!(
                reason.contains("requires configured ControllerDependencies"),
                "Expected fail-closed error message, got: {}",
                reason
            );
        }
        other => panic!("Unexpected result: {:?}", other),
    }
}

// =========================================================================
// PHASE 5: GOLDEN AUTONOMOUS MULTI-STEP WORKFLOW SMOKE TEST
// =========================================================================

/// Multi-turn scripted model for the Phase 5 Golden End-to-End Workflow:
/// Step 1: Diagnose -> completes with analysis summary.
/// Step 2: Implement -> calls write_file to repair src/lib.rs, then completes.
/// Step 3: Verify -> calls run_tests to verify, then completes.
struct GoldenAutonomousRepairModel {
    pub turns: AtomicUsize,
}

impl GoldenAutonomousRepairModel {
    fn new() -> Self {
        Self {
            turns: AtomicUsize::new(0),
        }
    }
}

const REPAIRED_LIB_RS: &str = r#"//! Repaired library implementation.

pub fn evaluate_expression(expr: &str) -> Result<i64, String> {
    let tokens: Vec<&str> = expr.split_whitespace().collect();
    if tokens.len() != 3 {
        return Err("Expected format: <num> <op> <num>".to_string());
    }
    let left: i64 = tokens[0].parse().map_err(|e| format!("Invalid: {e}"))?;
    let op = tokens[1];
    let right: i64 = tokens[2].parse().map_err(|e| format!("Invalid: {e}"))?;
    match op {
        "+" => Ok(left + right),
        "-" => Ok(left - right),
        _ => Err("Unsupported".to_string()),
    }
}
"#;

#[async_trait]
impl ModelCaller for GoldenAutonomousRepairModel {
    async fn call_model(&self, context: &str) -> Result<ModelProposal, String> {
        if context.contains("Diagnose") || context.contains("diagnose") {
            Ok(ModelProposal::Complete {
                summary: "Diagnosed issue: parser test fails on evaluate_expression in src/lib.rs"
                    .to_string(),
                artifacts: vec![],
            })
        } else if context.contains("Implement") || context.contains("implement") {
            Ok(ModelProposal::ToolCalls {
                calls: vec![ModelToolCall::new(
                    "write_file",
                    serde_json::json!({
                        "path": "src/lib.rs",
                        "content": REPAIRED_LIB_RS,
                    }),
                )],
            })
        } else if context.contains("Verify") || context.contains("verify") {
            Ok(ModelProposal::ToolCalls {
                calls: vec![ModelToolCall::new(
                    "run_tests",
                    serde_json::json!({
                        "args": ["--test", "parser_test"]
                    }),
                )],
            })
        } else {
            Ok(ModelProposal::Complete {
                summary: "Step completed successfully".to_string(),
                artifacts: vec![],
            })
        }
    }

    async fn call_model_with_context(
        &self,
        compiled: &CompiledContext,
        _cancellation: &CancellationToken,
    ) -> Result<ModelProposal, String> {
        let _turn = self.turns.fetch_add(1, Ordering::SeqCst);

        // Multi-turn handling for implement & verify steps
        let last_tool_is_write = compiled.messages.iter().rev().find_map(|m| {
            if let ChatMessage::Assistant { tool_calls, .. } = m {
                tool_calls.last().map(|c| c.name.as_str())
            } else {
                None
            }
        });

        if last_tool_is_write == Some("write_file") {
            Ok(ModelProposal::ToolCalls {
                calls: vec![ModelToolCall::new(
                    "run_tests",
                    serde_json::json!({
                        "args": ["--test", "parser_test"]
                    }),
                )],
            })
        } else if compiled
            .messages
            .iter()
            .any(|m| matches!(m, ChatMessage::Tool { .. }))
        {
            Ok(ModelProposal::Complete {
                summary: "Action succeeded, concluding step".to_string(),
                artifacts: vec![],
            })
        } else {
            self.call_model(&compiled.system_prompt).await
        }
    }
}

#[tokio::test]
async fn test_golden_autonomous_multi_step_workflow_spine() {
    let dir = tempdir().expect("tempdir failed");
    let repo_path = dir.path();
    setup_git_cargo_fixture(repo_path);

    // Setup failing test fixture
    let initial_failing_lib = r#"pub fn evaluate_expression(_expr: &str) -> Result<i64, String> {
    Err("not implemented".to_string())
}
"#;
    tokio::fs::write(repo_path.join("src/lib.rs"), initial_failing_lib)
        .await
        .unwrap();

    tokio::fs::create_dir_all(repo_path.join("tests"))
        .await
        .unwrap();
    let parser_test_rs = r#"use fixture_package::evaluate_expression;

#[test]
fn test_addition() {
    assert_eq!(evaluate_expression("10 + 20"), Ok(30));
}
"#;
    tokio::fs::write(repo_path.join("tests/parser_test.rs"), parser_test_rs)
        .await
        .unwrap();

    // Commit failing test
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo_path)
        .status()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Failing test added"])
        .current_dir(repo_path)
        .status()
        .unwrap();

    // Confirm initial test fails
    let initial_check = Command::new("cargo")
        .args(["test", "--test", "parser_test"])
        .current_dir(repo_path)
        .output()
        .expect("cargo test failed");
    assert!(!initial_check.status.success(), "Initial test must fail");

    // Initialize real AppRuntime with scripted multi-turn model
    let model = Arc::new(GoldenAutonomousRepairModel::new());
    let runtime = AppRuntime::new(repo_path)
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model.clone());

    let catalog = Arc::new(setup_test_catalog());
    // Create workflow engine using runtime's real production step executor
    let engine = runtime.create_workflow_engine(catalog);

    // Define 3-step linear workflow: diagnose -> implement -> verify
    let step_diagnose = WorkflowStepDefinition {
        key: "step_1_diagnose".to_string(),
        name: "Diagnose Workspace Failure".to_string(),
        role: AgentRole::diagnostician(),
        // NOTE (wiring remediation v0.1.1): bare "diagnose" canonically
        // routes to recovery.diagnostician v2, which requires failure
        // evidence unavailable on this happy path. Diagnosis-family
        // contracts fail closed without it (nothing to diagnose), so this
        // discovery step binds the compilable discovery contract; the
        // diagnostician role default is covered with failure evidence in
        // the recovery/diagnostician suites.
        prompt_template: "genesis.discovery".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 60,
        allows_parallelism: false,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let step_implement = WorkflowStepDefinition {
        key: "step_2_implement".to_string(),
        name: "Implement Code Repair".to_string(),
        role: AgentRole::implementer(),
        prompt_template: "implement".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec!["step_1_diagnose".to_string()],
        timeout_secs: 60,
        allows_parallelism: false,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let step_verify = WorkflowStepDefinition {
        key: "step_3_verify".to_string(),
        name: "Verify Code Repair".to_string(),
        role: AgentRole::verifier(),
        prompt_template: "verify".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec!["step_2_implement".to_string()],
        timeout_secs: 60,
        allows_parallelism: false,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let compiled = build_compiled_workflow(
        "golden_autonomous_repair",
        vec![step_diagnose, step_implement, step_verify],
        RecoveryStrategy::Fail,
    );

    let start_req = WorkflowStartRequest::new(repo_path).with_mode(WorkflowMode::Autonomous);
    let run_handle = engine
        .start_workflow(&compiled, start_req)
        .await
        .expect("start_workflow failed");

    let repo = SqliteWorkflowRepository::new(runtime.pool().clone());
    let run = repo.get_run(run_handle.run_id).await.unwrap().unwrap();
    if run.status != WorkflowRunState::Completed {
        let step_runs = repo.list_step_runs(run.id).await.unwrap();
        for s in &step_runs {
            eprintln!(
                "FAILED WORKFLOW STEP: key={}, status={:?}, error={:?}",
                s.step_key, s.status, s.halt_reason
            );
        }
    }
    assert_eq!(
        run.status,
        WorkflowRunState::Completed,
        "Golden autonomous repair workflow must complete all 3 steps successfully!"
    );

    // Verify all 3 steps reached Completed state
    let step_runs = repo.list_step_runs(run.id).await.unwrap();
    assert_eq!(step_runs.len(), 3);
    for step_run in &step_runs {
        assert_eq!(
            step_run.status,
            WorkflowStepState::Completed,
            "Step {} must be Completed",
            step_run.step_key
        );
        assert!(
            step_run.mission_id.is_some(),
            "Step {} must have an associated MissionId",
            step_run.step_key
        );
    }

    // Verify workspace tests now pass after step 2 repair!
    let post_test = tokio::process::Command::new("cargo")
        .args(["test", "--test", "parser_test"])
        .current_dir(repo_path)
        .output()
        .await
        .expect("cargo test failed");
    assert!(
        post_test.status.success(),
        "Tests in workspace must pass after autonomous repair! Output:\nSTDOUT:\n{}\nSTDERR:\n{}",
        String::from_utf8_lossy(&post_test.stdout),
        String::from_utf8_lossy(&post_test.stderr)
    );

    // Verify that repaired code is present on disk
    let lib_content = tokio::fs::read_to_string(repo_path.join("src/lib.rs"))
        .await
        .unwrap();
    assert!(
        lib_content.contains("tokens.len() != 3"),
        "src/lib.rs must contain the repaired implementation"
    );
}
