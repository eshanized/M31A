//! Integration tests for WorkflowEngine crash recovery, resumption, cascading invalidation,
//! and stale-attempt protection (Package 3).

use m31a::agent::model_policy::{ModelCaller, ModelProposal};
use m31a::prompt::{InMemoryPromptCatalog, PromptContract, PromptParameter};
use m31a::runtime::AppRuntime;
use m31a::state_machine::agent::AgentRole;
use m31a::verification::gate::EvidenceCompletionGate;
use m31a::verification::hierarchy::VerificationHierarchyEngine;
use m31a::verification::runners::compiler::CompilerRunner;
use m31a::verification::runners::deterministic::DeterministicRunner;
use m31a::verification::runners::diff_invariants::DiffInvariantsRunner;
use m31a::verification::runners::static_analysis::StaticAnalysisRunner;
use m31a::verification::runners::tests::TestRunner;
use m31a::workflow::compiler::CompiledWorkflow;
use m31a::workflow::definition::{
    InputBinding, OutputBinding, QualityGate, RecoveryStrategy, WorkflowDefinition,
    WorkflowStepDefinition,
};
use m31a::workflow::engine::{WorkflowEngine, WorkflowStartRequest};
use m31a::workflow::repository::{SqliteWorkflowRepository, WorkflowRepository};
use m31a::workflow::state::{WorkflowArtifactStatus, WorkflowRunState, WorkflowStepState};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

fn setup_recovery_catalog() -> InMemoryPromptCatalog {
    let mut catalog = InMemoryPromptCatalog::new();

    let roles = [
        ("step_a", AgentRole::researcher()),
        ("step_b", AgentRole::integrator()),
        ("step_c", AgentRole::reviewer()),
        ("approval_step", AgentRole::reviewer()),
        ("ask_operator_step", AgentRole::researcher()),
        ("downstream", AgentRole::integrator()),
    ];

    for (id, role) in roles {
        let contract = PromptContract::new(
            id,
            1,
            role,
            format!("{} prompt description", id),
            vec![
                PromptParameter {
                    name: "input_a".to_string(),
                    description: "Input".to_string(),
                    is_required: false,
                    default_value: Some("default".to_string()),
                },
                PromptParameter {
                    name: "operator_input".to_string(),
                    description: "Operator override".to_string(),
                    is_required: false,
                    default_value: Some("default".to_string()),
                },
            ],
            format!("Execute {}: {{{{ input_a }}}}", id),
            Some("markdown".to_string()),
        )
        .unwrap();
        catalog.register(contract).unwrap();
    }

    catalog
}

fn build_compiled(
    id: &str,
    steps: Vec<WorkflowStepDefinition>,
    default_strategy: RecoveryStrategy,
) -> CompiledWorkflow {
    let def = WorkflowDefinition {
        id: id.to_string(),
        name: format!("Workflow {}", id),
        description: "Recovery test workflow".to_string(),
        version: 1,
        steps,
        default_recovery_strategy: default_strategy,
    };
    def.validate().unwrap();

    let topological_order = def.steps.iter().map(|s| s.key.clone()).collect();
    let provenance = m31a::workflow::provenance::WorkflowProvenance::new(
        id,
        1,
        "hash",
        1,
        BTreeMap::new(),
        None,
        chrono::Utc::now(),
    );

    CompiledWorkflow {
        definition: def,
        topological_order,
        provenance,
    }
}

fn setup_git_cargo_fixture(dir: &Path) {
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

#[derive(Default)]
struct TestModelCaller {
    calls: Mutex<Vec<String>>,
    fail_ask_operator: AtomicBool,
}

impl TestModelCaller {
    fn new() -> Self {
        Self::default()
    }

    fn with_fail_ask_operator() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            fail_ask_operator: AtomicBool::new(true),
        }
    }
}

#[async_trait::async_trait]
impl ModelCaller for TestModelCaller {
    async fn call_model(&self, context: &str) -> Result<ModelProposal, String> {
        self.calls.lock().unwrap().push(context.to_string());
        if self.fail_ask_operator.load(Ordering::SeqCst)
            && (context.contains("ask_operator_step") || context.contains("Operator Input Step"))
        {
            return Err("operator input needed".to_string());
        }
        Ok(ModelProposal::Complete {
            summary: "Completed successfully".to_string(),
            artifacts: vec![],
        })
    }
}

async fn setup_engine(
    workspace: &Path,
    model: Arc<dyn ModelCaller>,
) -> (
    WorkflowEngine,
    Arc<SqliteWorkflowRepository>,
    Arc<AppRuntime>,
) {
    setup_git_cargo_fixture(workspace);
    let runtime = Arc::new(
        AppRuntime::new(workspace)
            .await
            .expect("AppRuntime::new failed")
            .with_model_caller(model),
    );
    let catalog = Arc::new(setup_recovery_catalog());
    let mut deps = runtime.dependencies().clone();
    let hierarchy = Arc::new(VerificationHierarchyEngine {
        tier1_deterministic: Arc::new(DeterministicRunner::new()),
        tier2_compiler: Arc::new(CompilerRunner::with_command("true")),
        tier3_tests: Arc::new(TestRunner::new().with_command("true")),
        tier4_static_analysis: Arc::new(StaticAnalysisRunner::with_command("true")),
        tier5_diff_invariants: Arc::new(DiffInvariantsRunner::new()),
        tier6_reviewer: None,
    });
    deps = deps.with_verifier(Arc::new(
        EvidenceCompletionGate::new(
            runtime.pool().clone(),
            runtime.artifact_store().clone(),
            workspace.to_path_buf(),
        )
        .with_hierarchy_engine(hierarchy),
    ));
    let repo = Arc::new(SqliteWorkflowRepository::new(runtime.pool().clone()));
    let engine = WorkflowEngine::new(repo.clone(), catalog, Some(runtime.event_bus().clone()))
        .with_dependencies(deps);
    (engine, repo, runtime)
}

#[tokio::test]
async fn test_recover_incomplete_workflows_startup() {
    let dir = tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    tokio::fs::create_dir_all(&workspace).await.unwrap();
    let model = Arc::new(TestModelCaller::new());
    let (engine, repo, _) = setup_engine(&workspace, model).await;

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
        timeout_secs: 300,
        allows_parallelism: true,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let step_b = WorkflowStepDefinition {
        key: "step_b".to_string(),
        name: "Step B".to_string(),
        role: AgentRole::integrator(),
        prompt_template: "genesis.charter".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec!["step_a".to_string()],
        timeout_secs: 300,
        allows_parallelism: true,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let compiled = build_compiled("rec_startup", vec![step_a, step_b], RecoveryStrategy::Fail);

    // Simulate an incomplete workflow by inserting a run in Running state
    // with step_a Completed and step_b in Running (process died during step_b)
    let mut run = m31a::workflow::state::WorkflowRun::new(
        "rec_startup",
        1,
        workspace.clone(),
        m31a::workflow::state::WorkflowMode::Standard,
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
    registry.insert("rec_startup".to_string(), compiled.clone());

    let recovered = engine
        .recover_incomplete_workflows(&registry)
        .await
        .unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0], run.id);

    // Step B was reset from Running to Blocked during recovery, and then advance_workflow ran it
    let run_after = repo.get_run(run.id).await.unwrap().unwrap();
    assert_eq!(run_after.status, WorkflowRunState::Completed);

    let step_b_after = repo
        .get_step_run_by_key(run.id, "step_b")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(step_b_after.status, WorkflowStepState::Completed);
}

#[tokio::test]
async fn test_recovery_idempotency() {
    let dir = tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    tokio::fs::create_dir_all(&workspace).await.unwrap();
    let model = Arc::new(TestModelCaller::new());
    let (engine, repo, _) = setup_engine(&workspace, model).await;

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
        timeout_secs: 300,
        allows_parallelism: true,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let compiled = build_compiled("idempotent_rec", vec![step_a], RecoveryStrategy::Fail);

    let mut registry = HashMap::new();
    registry.insert("idempotent_rec".to_string(), compiled.clone());

    // Initial clean run
    let handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(&workspace))
        .await
        .unwrap();

    assert_eq!(handle.status, WorkflowRunState::Completed);

    // Running recovery now should find no incomplete runs
    let rec1 = engine
        .recover_incomplete_workflows(&registry)
        .await
        .unwrap();
    assert!(rec1.is_empty());

    // Running recovery a second time should also find nothing and succeed cleanly
    let rec2 = engine
        .recover_incomplete_workflows(&registry)
        .await
        .unwrap();
    assert!(rec2.is_empty());

    let final_run = repo.get_run(handle.run_id).await.unwrap().unwrap();
    assert_eq!(final_run.status, WorkflowRunState::Completed);
}

#[tokio::test]
async fn test_resume_from_awaiting_approval_after_restart() {
    let dir = tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    tokio::fs::create_dir_all(&workspace).await.unwrap();
    let model = Arc::new(TestModelCaller::new());
    let (engine, repo, runtime) = setup_engine(&workspace, model).await;

    let gate_step = WorkflowStepDefinition {
        key: "approval_step".to_string(),
        name: "Human Gate".to_string(),
        role: AgentRole::reviewer(),
        prompt_template: "verification.reviewer".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate {
            require_human_approval: true,
            ..Default::default()
        },
        depends_on: vec![],
        timeout_secs: 300,
        allows_parallelism: false,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let downstream = WorkflowStepDefinition {
        key: "downstream".to_string(),
        name: "Downstream".to_string(),
        role: AgentRole::integrator(),
        prompt_template: "genesis.charter".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec!["approval_step".to_string()],
        timeout_secs: 300,
        allows_parallelism: false,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let compiled = build_compiled(
        "approval_resume",
        vec![gate_step, downstream],
        RecoveryStrategy::Fail,
    );

    let handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(&workspace))
        .await
        .unwrap();

    assert_eq!(handle.status, WorkflowRunState::AwaitingApproval);

    // Simulate system restart with a new engine instance sharing the same repo/dependencies
    let catalog = Arc::new(setup_recovery_catalog());
    let new_engine = WorkflowEngine::new(repo.clone(), catalog, Some(runtime.event_bus().clone()))
        .with_dependencies(runtime.dependencies().clone());

    let mut registry = HashMap::new();
    registry.insert("approval_resume".to_string(), compiled.clone());

    // Recover incomplete workflows
    let recovered = new_engine
        .recover_incomplete_workflows(&registry)
        .await
        .unwrap();
    assert_eq!(recovered.len(), 1);

    // Workflow must still be in AwaitingApproval (recovery does not auto-bypass human approval)
    let run = repo.get_run(handle.run_id).await.unwrap().unwrap();
    assert_eq!(run.status, WorkflowRunState::AwaitingApproval);

    // Now operator approves
    let state_after_approval = new_engine
        .approve_step(handle.run_id, "approval_step", &compiled)
        .await
        .unwrap();

    assert_eq!(state_after_approval, WorkflowRunState::Completed);
}

#[tokio::test]
async fn test_resume_from_awaiting_input_after_restart() {
    let dir = tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    tokio::fs::create_dir_all(&workspace).await.unwrap();
    let model = Arc::new(TestModelCaller::with_fail_ask_operator());
    let (engine, repo, runtime) = setup_engine(&workspace, model.clone()).await;

    let step = WorkflowStepDefinition {
        key: "ask_operator_step".to_string(),
        name: "Operator Input Step".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "genesis.discovery".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 300,
        allows_parallelism: false,
        recovery_strategy: Some(RecoveryStrategy::AskOperator),

        prompt_ref: None,
    };

    let compiled = build_compiled("input_resume", vec![step], RecoveryStrategy::Fail);

    let handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(&workspace))
        .await
        .unwrap();

    assert_eq!(handle.status, WorkflowRunState::AwaitingInput);

    // Simulate restart with new engine
    let catalog = Arc::new(setup_recovery_catalog());
    let new_engine = WorkflowEngine::new(repo.clone(), catalog, Some(runtime.event_bus().clone()))
        .with_dependencies(runtime.dependencies().clone());

    let mut registry = HashMap::new();
    registry.insert("input_resume".to_string(), compiled.clone());

    let recovered = new_engine
        .recover_incomplete_workflows(&registry)
        .await
        .unwrap();
    assert_eq!(recovered.len(), 1);

    // Operator supplies the missing input
    model.fail_ask_operator.store(false, Ordering::SeqCst);
    let mut params = BTreeMap::new();
    params.insert("operator_input".to_string(), "configured_value".to_string());
    let state_after_input = new_engine
        .provide_input(handle.run_id, "ask_operator_step", params, &compiled)
        .await
        .unwrap();

    assert_eq!(state_after_input, WorkflowRunState::Completed);
}

#[tokio::test]
async fn test_cascading_invalidation() {
    let dir = tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    tokio::fs::create_dir_all(&workspace).await.unwrap();
    let model = Arc::new(TestModelCaller::new());
    let (engine, repo, _) = setup_engine(&workspace, model).await;

    // Prepare files on disk so artifacts are created
    tokio::fs::write(workspace.join("a.txt"), "Output A")
        .await
        .unwrap();
    tokio::fs::write(workspace.join("b.txt"), "Output B")
        .await
        .unwrap();
    tokio::fs::write(workspace.join("c.txt"), "Output C")
        .await
        .unwrap();

    let step_a = WorkflowStepDefinition {
        key: "step_a".to_string(),
        name: "Step A".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "genesis.discovery".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![OutputBinding {
            artifact_name: "A.txt".to_string(),
            relative_path: PathBuf::from("a.txt"),
            schema_type: "text".to_string(),
        }],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 300,
        allows_parallelism: false,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let step_b = WorkflowStepDefinition {
        key: "step_b".to_string(),
        name: "Step B".to_string(),
        role: AgentRole::integrator(),
        prompt_template: "genesis.charter".to_string(),
        required_inputs: vec![InputBinding {
            parameter_name: "input_a".to_string(),
            source_step_key: "step_a".to_string(),
            artifact_name: "A.txt".to_string(),
            is_optional: false,
        }],
        expected_outputs: vec![OutputBinding {
            artifact_name: "B.txt".to_string(),
            relative_path: PathBuf::from("b.txt"),
            schema_type: "text".to_string(),
        }],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec!["step_a".to_string()],
        timeout_secs: 300,
        allows_parallelism: false,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let step_c = WorkflowStepDefinition {
        key: "step_c".to_string(),
        name: "Step C".to_string(),
        role: AgentRole::reviewer(),
        prompt_template: "verification.reviewer".to_string(),
        required_inputs: vec![InputBinding {
            parameter_name: "input_a".to_string(),
            source_step_key: "step_b".to_string(),
            artifact_name: "B.txt".to_string(),
            is_optional: false,
        }],
        expected_outputs: vec![OutputBinding {
            artifact_name: "C.txt".to_string(),
            relative_path: PathBuf::from("c.txt"),
            schema_type: "text".to_string(),
        }],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec!["step_b".to_string()],
        timeout_secs: 300,
        allows_parallelism: false,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let compiled = build_compiled(
        "cascade_wf",
        vec![step_a, step_b, step_c],
        RecoveryStrategy::Fail,
    );

    // Run to completion
    let handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(&workspace))
        .await
        .unwrap();

    assert_eq!(handle.status, WorkflowRunState::Completed);

    // Verify all 3 artifacts exist and are Valid
    let artifacts = repo.list_artifacts(handle.run_id).await.unwrap();
    assert_eq!(artifacts.len(), 3);
    for art in &artifacts {
        assert_eq!(art.status, WorkflowArtifactStatus::Valid);
    }

    // Now invalidate downstream of step_a
    let invalidated = engine
        .invalidate_downstream_steps(handle.run_id, "step_a", &compiled)
        .await
        .unwrap();

    assert_eq!(
        invalidated,
        vec!["step_b".to_string(), "step_c".to_string()]
    );

    // Check step runs: step_b and step_c should be Blocked
    let b_run = repo
        .get_step_run_by_key(handle.run_id, "step_b")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(b_run.status, WorkflowStepState::Blocked);

    let c_run = repo
        .get_step_run_by_key(handle.run_id, "step_c")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(c_run.status, WorkflowStepState::Blocked);

    let a_run = repo
        .get_step_run_by_key(handle.run_id, "step_a")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(a_run.status, WorkflowStepState::Completed);

    // Check artifacts: A is Valid, B and C are Superseded
    let artifacts_after = repo.list_artifacts(handle.run_id).await.unwrap();
    for art in artifacts_after {
        if art.name == "A.txt" {
            assert_eq!(art.status, WorkflowArtifactStatus::Valid);
        } else {
            assert_eq!(art.status, WorkflowArtifactStatus::Superseded);
        }
    }
}

#[tokio::test]
async fn test_stale_execution_rejection() {
    let dir = tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    tokio::fs::create_dir_all(&workspace).await.unwrap();
    let model = Arc::new(TestModelCaller::new());
    let (engine, repo, _) = setup_engine(&workspace, model).await;

    let step = WorkflowStepDefinition {
        key: "step_a".to_string(),
        name: "Step A".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "genesis.discovery".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 300,
        allows_parallelism: false,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let compiled = build_compiled("stale_test", vec![step], RecoveryStrategy::Fail);

    let handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(&workspace))
        .await
        .unwrap();

    // Now artificially increment attempt count on the step to simulate advancement
    let mut step_run = repo
        .get_step_run_by_key(handle.run_id, "step_a")
        .await
        .unwrap()
        .unwrap();
    step_run.attempt_count = 3;
    repo.update_step_run(&step_run).await.unwrap();

    // Validating attempt 3 passes
    assert!(
        engine
            .validate_attempt_freshness(handle.run_id, "step_a", 3)
            .await
            .is_ok()
    );

    // Validating attempt 1 or 2 returns StaleExecution error
    let stale_err = engine
        .validate_attempt_freshness(handle.run_id, "step_a", 1)
        .await
        .unwrap_err();
    match stale_err {
        m31a::workflow::error::WorkflowError::StaleExecution {
            step_key,
            expected_attempt,
            received_attempt,
        } => {
            assert_eq!(step_key, "step_a");
            assert_eq!(expected_attempt, 3);
            assert_eq!(received_attempt, 1);
        }
        other => panic!("expected StaleExecution error, got {:?}", other),
    }
}
