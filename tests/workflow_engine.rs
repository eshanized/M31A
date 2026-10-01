//! Comprehensive integration test suite for WorkflowEngine Runtime (Package 3).

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
use m31a::workflow::state::{
    WorkflowArtifactStatus, WorkflowMode, WorkflowRunState, WorkflowStepState,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

/// Helper to set up a test prompt catalog with standard roles.
fn setup_test_catalog() -> InMemoryPromptCatalog {
    let mut catalog = InMemoryPromptCatalog::new();

    let roles = [
        ("discovery", AgentRole::researcher()),
        ("charter", AgentRole::integrator()),
        ("research_stack", AgentRole::researcher()),
        ("research_arch", AgentRole::researcher()),
        ("synthesis", AgentRole::integrator()),
        ("review", AgentRole::reviewer()),
        ("approval_step", AgentRole::reviewer()),
        ("retry_step", AgentRole::implementer()),
        ("input_step", AgentRole::researcher()),
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
                    description: "Default input".to_string(),
                    is_required: false,
                    default_value: Some("default".to_string()),
                },
                PromptParameter {
                    name: "operator_input".to_string(),
                    description: "Default operator input".to_string(),
                    is_required: false,
                    default_value: Some("default".to_string()),
                },
            ],
            format!("Execute role for {}: {{{{ input_a }}}}", id),
            Some("markdown".to_string()),
        )
        .unwrap();
        catalog.register(contract).unwrap();
    }

    catalog
}

/// Helper to create a compiled workflow from steps.
fn build_compiled_workflow(
    id: &str,
    steps: Vec<WorkflowStepDefinition>,
    default_strategy: RecoveryStrategy,
) -> CompiledWorkflow {
    let def = WorkflowDefinition {
        id: id.to_string(),
        name: format!("Workflow {}", id),
        description: "Test workflow description".to_string(),
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
        provenance,
        topological_order,
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
    fail_all: bool,
}

impl TestModelCaller {
    fn new() -> Self {
        Self::default()
    }

    fn failing() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            fail_all: true,
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl ModelCaller for TestModelCaller {
    async fn call_model(&self, context: &str) -> Result<ModelProposal, String> {
        self.calls.lock().unwrap().push(context.to_string());
        if self.fail_all {
            return Err("Model execution failure".to_string());
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
    let catalog = Arc::new(setup_test_catalog());
    let mut deps = runtime.dependencies().clone();
    let hierarchy = Arc::new(VerificationHierarchyEngine {
        tier1_deterministic: Arc::new(DeterministicRunner::new()),
        tier2_compiler: Arc::new(CompilerRunner::with_command("true")),
        tier3_tests: Arc::new(TestRunner::new().with_command("true")),
        tier4_static_analysis: Arc::new(StaticAnalysisRunner::with_command("true")),
        tier5_diff_invariants: Arc::new(DiffInvariantsRunner::new()),
        tier6_reviewer: None,
    });
    deps.verifier = Arc::new(
        EvidenceCompletionGate::new(
            runtime.pool().clone(),
            runtime.artifact_store().clone(),
            workspace.to_path_buf(),
        )
        .with_hierarchy_engine(hierarchy),
    );
    let repo = Arc::new(SqliteWorkflowRepository::new(runtime.pool().clone()));
    let engine = WorkflowEngine::new(repo.clone(), catalog, Some(runtime.event_bus().clone()))
        .with_dependencies(deps);
    (engine, repo, runtime)
}

#[tokio::test]
async fn test_workflow_start_and_initial_wave() {
    let dir = tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    tokio::fs::create_dir_all(&workspace).await.unwrap();
    let model = Arc::new(TestModelCaller::new());
    let (engine, repo, _) = setup_engine(&workspace, model).await;

    let step_a = WorkflowStepDefinition {
        key: "step_a".to_string(),
        name: "Step A".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "discovery".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 300,
        allows_parallelism: true,
        recovery_strategy: None,
    };

    let step_b = WorkflowStepDefinition {
        key: "step_b".to_string(),
        name: "Step B".to_string(),
        role: AgentRole::integrator(),
        prompt_template: "charter".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec!["step_a".to_string()],
        timeout_secs: 300,
        allows_parallelism: true,
        recovery_strategy: None,
    };

    let compiled =
        build_compiled_workflow("test_init", vec![step_a, step_b], RecoveryStrategy::Fail);

    // Compute waves
    let waves = engine.compute_execution_waves(&compiled).unwrap();
    assert_eq!(waves.len(), 2);
    assert_eq!(waves[0], vec!["step_a"]);
    assert_eq!(waves[1], vec!["step_b"]);

    // Start workflow
    let handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(&workspace))
        .await
        .unwrap();

    assert_eq!(handle.definition_id, "test_init");
    // Since step_a and step_b have no outputs or gates, both complete sequentially
    assert_eq!(handle.status, WorkflowRunState::Completed);

    let run = repo.get_run(handle.run_id).await.unwrap().unwrap();
    assert_eq!(run.status, WorkflowRunState::Completed);
}

#[tokio::test]
async fn test_linear_workflow_with_artifact_handoff() {
    let dir = tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    tokio::fs::create_dir_all(&workspace).await.unwrap();
    let model = Arc::new(TestModelCaller::new());
    let (engine, repo, _) = setup_engine(&workspace, model.clone()).await;

    let step_a = WorkflowStepDefinition {
        key: "discovery".to_string(),
        name: "Discovery Step".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "discovery".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![OutputBinding {
            artifact_name: "DISCOVERY.md".to_string(),
            relative_path: PathBuf::from("DISCOVERY.md"),
            schema_type: "markdown".to_string(),
        }],
        required_capabilities: vec![],
        quality_gate: QualityGate {
            schema: None,
            required_artifacts: vec!["DISCOVERY.md".to_string()],
            require_human_approval: false,
            max_ambiguity_percent: None,
        },
        depends_on: vec![],
        timeout_secs: 300,
        allows_parallelism: true,
        recovery_strategy: None,
    };

    let step_b = WorkflowStepDefinition {
        key: "charter".to_string(),
        name: "Charter Step".to_string(),
        role: AgentRole::integrator(),
        prompt_template: "charter".to_string(),
        required_inputs: vec![InputBinding {
            parameter_name: "input_a".to_string(),
            source_step_key: "discovery".to_string(),
            artifact_name: "DISCOVERY.md".to_string(),
            is_optional: false,
        }],
        expected_outputs: vec![OutputBinding {
            artifact_name: "PROJECT.md".to_string(),
            relative_path: PathBuf::from("PROJECT.md"),
            schema_type: "markdown".to_string(),
        }],
        required_capabilities: vec![],
        quality_gate: QualityGate {
            schema: None,
            required_artifacts: vec!["PROJECT.md".to_string()],
            require_human_approval: false,
            max_ambiguity_percent: None,
        },
        depends_on: vec!["discovery".to_string()],
        timeout_secs: 300,
        allows_parallelism: true,
        recovery_strategy: None,
    };

    // Pre-create output files in workspace so gates find them
    tokio::fs::write(
        workspace.join("DISCOVERY.md"),
        "# Discovery Notes\nVerified requirements",
    )
    .await
    .unwrap();
    tokio::fs::write(
        workspace.join("PROJECT.md"),
        "# Project Charter\nCharter verified",
    )
    .await
    .unwrap();

    let compiled = build_compiled_workflow(
        "linear_handoff",
        vec![step_a, step_b],
        RecoveryStrategy::Fail,
    );

    let handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(&workspace))
        .await
        .unwrap();

    assert_eq!(handle.status, WorkflowRunState::Completed);

    // Verify artifacts were durably stored and have content hashes
    let artifacts = repo.list_artifacts(handle.run_id).await.unwrap();
    assert_eq!(artifacts.len(), 2);
    let disc_art = artifacts
        .iter()
        .find(|a| a.name == "DISCOVERY.md")
        .expect("DISCOVERY.md artifact");
    let proj_art = artifacts
        .iter()
        .find(|a| a.name == "PROJECT.md")
        .expect("PROJECT.md artifact");

    assert_eq!(disc_art.status, WorkflowArtifactStatus::Valid);
    assert_eq!(proj_art.status, WorkflowArtifactStatus::Valid);
    assert!(!disc_art.content_hash.is_empty());

    // Verify model was called for each step
    let calls = model.calls();
    assert_eq!(calls.len(), 2);
}

#[tokio::test]
async fn test_parallel_sibling_steps_in_diamond_dag() {
    let dir = tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    tokio::fs::create_dir_all(&workspace).await.unwrap();
    let model = Arc::new(TestModelCaller::new());
    let (engine, repo, _) = setup_engine(&workspace, model).await;

    // Diamond DAG:
    //      discovery
    //       /     \
    // research_a  research_b
    //       \     /
    //      synthesis

    let discovery = WorkflowStepDefinition {
        key: "discovery".to_string(),
        name: "Discovery".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "discovery".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 300,
        allows_parallelism: true,
        recovery_strategy: None,
    };

    let research_a = WorkflowStepDefinition {
        key: "research_a".to_string(),
        name: "Research Stack".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "research_stack".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec!["discovery".to_string()],
        timeout_secs: 300,
        allows_parallelism: true,
        recovery_strategy: None,
    };

    let research_b = WorkflowStepDefinition {
        key: "research_b".to_string(),
        name: "Research Arch".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "research_arch".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec!["discovery".to_string()],
        timeout_secs: 300,
        allows_parallelism: true,
        recovery_strategy: None,
    };

    let synthesis = WorkflowStepDefinition {
        key: "synthesis".to_string(),
        name: "Synthesis".to_string(),
        role: AgentRole::integrator(),
        prompt_template: "synthesis".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec!["research_a".to_string(), "research_b".to_string()],
        timeout_secs: 300,
        allows_parallelism: true,
        recovery_strategy: None,
    };

    let compiled = build_compiled_workflow(
        "diamond_workflow",
        vec![discovery, research_a, research_b, synthesis],
        RecoveryStrategy::Fail,
    );

    // Verify waves
    let waves = engine.compute_execution_waves(&compiled).unwrap();
    assert_eq!(waves.len(), 3);
    assert_eq!(waves[0], vec!["discovery"]);
    assert_eq!(waves[1], vec!["research_a", "research_b"]);
    assert_eq!(waves[2], vec!["synthesis"]);

    let handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(&workspace))
        .await
        .unwrap();

    assert_eq!(handle.status, WorkflowRunState::Completed);

    // Verify all 4 step runs reached Completed status
    let step_runs = repo.list_step_runs(handle.run_id).await.unwrap();
    assert_eq!(step_runs.len(), 4);
    for s in &step_runs {
        assert_eq!(s.status, WorkflowStepState::Completed);
    }
}

#[tokio::test]
async fn test_quality_gate_failure_halts_workflow() {
    let dir = tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    tokio::fs::create_dir_all(&workspace).await.unwrap();
    let model = Arc::new(TestModelCaller::new());
    let (engine, repo, _) = setup_engine(&workspace, model).await;

    // Step demands "MANDATORY.md" but file is never written
    let step = WorkflowStepDefinition {
        key: "discovery".to_string(),
        name: "Discovery".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "discovery".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![OutputBinding {
            artifact_name: "MANDATORY.md".to_string(),
            relative_path: PathBuf::from("MANDATORY.md"),
            schema_type: "markdown".to_string(),
        }],
        required_capabilities: vec![],
        quality_gate: QualityGate {
            schema: None,
            required_artifacts: vec!["MANDATORY.md".to_string()],
            require_human_approval: false,
            max_ambiguity_percent: None,
        },
        depends_on: vec![],
        timeout_secs: 300,
        allows_parallelism: false,
        recovery_strategy: Some(RecoveryStrategy::Fail),
    };

    let compiled = build_compiled_workflow("qg_fail_wf", vec![step], RecoveryStrategy::Fail);

    let handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(&workspace))
        .await
        .unwrap();

    assert_eq!(handle.status, WorkflowRunState::Failed);

    let step_run = repo
        .get_step_run_by_key(handle.run_id, "discovery")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(step_run.status, WorkflowStepState::Failed);
    assert!(step_run.halt_reason.is_some());
}

#[tokio::test]
async fn test_human_approval_gate_and_continuation() {
    let dir = tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    tokio::fs::create_dir_all(&workspace).await.unwrap();
    let model = Arc::new(TestModelCaller::new());
    let (engine, repo, _) = setup_engine(&workspace, model).await;

    let step_a = WorkflowStepDefinition {
        key: "approval_step".to_string(),
        name: "Step with Human Sign-off".to_string(),
        role: AgentRole::reviewer(),
        prompt_template: "approval_step".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate {
            schema: None,
            required_artifacts: vec![],
            require_human_approval: true, // Gate demands human approval!
            max_ambiguity_percent: None,
        },
        depends_on: vec![],
        timeout_secs: 300,
        allows_parallelism: false,
        recovery_strategy: None,
    };

    let step_b = WorkflowStepDefinition {
        key: "downstream".to_string(),
        name: "Downstream step".to_string(),
        role: AgentRole::integrator(),
        prompt_template: "charter".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec!["approval_step".to_string()],
        timeout_secs: 300,
        allows_parallelism: false,
        recovery_strategy: None,
    };

    let compiled =
        build_compiled_workflow("approval_wf", vec![step_a, step_b], RecoveryStrategy::Fail);

    let handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(&workspace))
        .await
        .unwrap();

    // Must pause at AwaitingApproval
    assert_eq!(handle.status, WorkflowRunState::AwaitingApproval);

    let step_a_run = repo
        .get_step_run_by_key(handle.run_id, "approval_step")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(step_a_run.status, WorkflowStepState::AwaitingApproval);

    let step_b_run = repo
        .get_step_run_by_key(handle.run_id, "downstream")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(step_b_run.status, WorkflowStepState::Pending);

    // Operator approves the step!
    let next_state = engine
        .approve_step(handle.run_id, "approval_step", &compiled)
        .await
        .unwrap();

    // Downstream step executes and workflow completes
    assert_eq!(next_state, WorkflowRunState::Completed);

    let final_run = repo.get_run(handle.run_id).await.unwrap().unwrap();
    assert_eq!(final_run.status, WorkflowRunState::Completed);

    let step_a_done = repo
        .get_step_run_by_key(handle.run_id, "approval_step")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(step_a_done.status, WorkflowStepState::Completed);

    let step_b_done = repo
        .get_step_run_by_key(handle.run_id, "downstream")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(step_b_done.status, WorkflowStepState::Completed);
}

#[tokio::test]
async fn test_retry_recovery_strategy_and_exhaustion() {
    let dir = tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    tokio::fs::create_dir_all(&workspace).await.unwrap();
    let model = Arc::new(TestModelCaller::failing());
    let (engine, repo, _) = setup_engine(&workspace, model).await;

    let step = WorkflowStepDefinition {
        key: "retry_step".to_string(),
        name: "Retrying Step".to_string(),
        role: AgentRole::implementer(),
        prompt_template: "retry_step".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 300,
        allows_parallelism: false,
        recovery_strategy: Some(RecoveryStrategy::Retry { max_retries: 2 }),
    };

    let compiled = build_compiled_workflow("retry_wf", vec![step], RecoveryStrategy::Fail);

    let handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(&workspace))
        .await
        .unwrap();

    // Since max_retries = 2 and step failed, retries were exhausted
    let step_run = repo
        .get_step_run_by_key(handle.run_id, "retry_step")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(step_run.status, WorkflowStepState::Failed);
    assert_eq!(step_run.attempt_count, 2);
    assert!(step_run.halt_reason.is_some());
}

#[tokio::test]
async fn test_cancellation_and_pause_resume() {
    let dir = tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    tokio::fs::create_dir_all(&workspace).await.unwrap();
    let model = Arc::new(TestModelCaller::new());
    let (engine, repo, _) = setup_engine(&workspace, model).await;

    let step = WorkflowStepDefinition {
        key: "discovery".to_string(),
        name: "Discovery".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "discovery".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 300,
        allows_parallelism: false,
        recovery_strategy: None,
    };

    let compiled = build_compiled_workflow("cancel_wf", vec![step], RecoveryStrategy::Fail);

    // Test explicit cancellation
    let mut run = m31a::workflow::state::WorkflowRun::new(
        "cancel_wf",
        1,
        workspace.clone(),
        WorkflowMode::Standard,
    );
    run.transition_to(WorkflowRunState::Running, None).unwrap();
    repo.create_run(&run).await.unwrap();

    let step_run = m31a::workflow::state::WorkflowStepRun::new(run.id, "discovery");
    repo.create_step_run(&step_run).await.unwrap();

    // Pause workflow
    engine
        .pause_workflow(run.id, "operator requested pause")
        .await
        .unwrap();
    let paused_run = repo.get_run(run.id).await.unwrap().unwrap();
    assert_eq!(paused_run.status, WorkflowRunState::Blocked);

    // Resume workflow
    let resumed_state = engine.resume_workflow(run.id, &compiled).await.unwrap();
    assert_eq!(resumed_state, WorkflowRunState::Completed);

    // Test cancellation on another run
    let mut run2 =
        m31a::workflow::state::WorkflowRun::new("cancel_wf", 1, workspace, WorkflowMode::Standard);
    run2.transition_to(WorkflowRunState::Running, None).unwrap();
    repo.create_run(&run2).await.unwrap();

    let step_run2 = m31a::workflow::state::WorkflowStepRun::new(run2.id, "discovery");
    repo.create_step_run(&step_run2).await.unwrap();

    engine.cancel_workflow(run2.id, "user abort").await.unwrap();
    let cancelled_run = repo.get_run(run2.id).await.unwrap().unwrap();
    assert_eq!(cancelled_run.status, WorkflowRunState::Cancelled);

    let cancelled_step = repo
        .get_step_run_by_key(run2.id, "discovery")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cancelled_step.status, WorkflowStepState::Cancelled);
}
