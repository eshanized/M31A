//! Integration tests for WorkflowEngine failure modes, boundary errors, gate halts,
//! cycle detection, and missing artifact blockage (Package 3).

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
use m31a::workflow::error::WorkflowError;
use m31a::workflow::repository::{SqliteWorkflowRepository, WorkflowRepository};
use m31a::workflow::state::{WorkflowRunState, WorkflowStepState};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

fn setup_failure_catalog() -> InMemoryPromptCatalog {
    let mut catalog = InMemoryPromptCatalog::new();

    let roles = [
        ("step_a", AgentRole::researcher()),
        ("step_b", AgentRole::integrator()),
        ("approval_step", AgentRole::reviewer()),
    ];

    for (id, role) in roles {
        let contract = PromptContract::new(
            id,
            1,
            role,
            format!("{} prompt", id),
            vec![PromptParameter {
                name: "missing_param".to_string(),
                description: "Input".to_string(),
                is_required: false,
                default_value: None,
            }],
            format!("Execute {}: {{{{ missing_param }}}}", id),
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
) -> Result<CompiledWorkflow, WorkflowError> {
    let def = WorkflowDefinition {
        id: id.to_string(),
        name: format!("Workflow {}", id),
        description: "Failure test workflow".to_string(),
        version: 1,
        steps,
        default_recovery_strategy: default_strategy,
    };
    def.validate()?;

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

    Ok(CompiledWorkflow {
        definition: def,
        topological_order,
        provenance,
    })
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
    let catalog = Arc::new(setup_failure_catalog());
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
async fn test_empty_workflow_rejected() {
    let err = build_compiled("empty_wf", vec![], RecoveryStrategy::Fail).unwrap_err();
    match err {
        WorkflowError::InvalidDefinition(msg) => {
            assert!(msg.contains("must contain at least one step"));
        }
        other => panic!("expected InvalidDefinition error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_cyclic_workflow_rejected() {
    let step_a = WorkflowStepDefinition {
        key: "step_a".to_string(),
        name: "Step A".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "step_a".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec!["step_b".to_string()], // Depends on B
        timeout_secs: 300,
        allows_parallelism: false,
        recovery_strategy: None,
    
        prompt_ref: None,};

    let step_b = WorkflowStepDefinition {
        key: "step_b".to_string(),
        name: "Step B".to_string(),
        role: AgentRole::integrator(),
        prompt_template: "step_b".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec!["step_a".to_string()], // Depends on A -> Cycle!
        timeout_secs: 300,
        allows_parallelism: false,
        recovery_strategy: None,
    
        prompt_ref: None,};

    let err = build_compiled("cycle_wf", vec![step_a, step_b], RecoveryStrategy::Fail).unwrap_err();
    match err {
        WorkflowError::CycleDetected { cycle } => {
            assert!(!cycle.is_empty());
        }
        other => panic!("expected CycleDetected error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_missing_required_upstream_artifact_blocks_step() {
    let dir = tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    tokio::fs::create_dir_all(&workspace).await.unwrap();
    let model = Arc::new(TestModelCaller::new());
    let (engine, repo, _) = setup_engine(&workspace, model).await;

    // step_a declares an output but does NOT write it to disk
    let step_a = WorkflowStepDefinition {
        key: "step_a".to_string(),
        name: "Step A".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "step_a".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![OutputBinding {
            artifact_name: "NON_EXISTENT.md".to_string(),
            relative_path: PathBuf::from("non_existent.md"),
            schema_type: "markdown".to_string(),
        }],
        required_capabilities: vec![],
        quality_gate: QualityGate {
            required_artifacts: vec!["NON_EXISTENT.md".to_string()],
            ..Default::default()
        },
        depends_on: vec![],
        timeout_secs: 300,
        allows_parallelism: false,
        recovery_strategy: Some(RecoveryStrategy::Fail),
    
        prompt_ref: None,};

    // step_b requires an artifact that step_a never produced
    let step_b = WorkflowStepDefinition {
        key: "step_b".to_string(),
        name: "Step B".to_string(),
        role: AgentRole::integrator(),
        prompt_template: "step_b".to_string(),
        required_inputs: vec![InputBinding {
            parameter_name: "missing_param".to_string(),
            source_step_key: "step_a".to_string(),
            artifact_name: "NON_EXISTENT.md".to_string(),
            is_optional: false, // Strict requirement
        }],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec!["step_a".to_string()],
        timeout_secs: 300,
        allows_parallelism: false,
        recovery_strategy: None,
    
        prompt_ref: None,};

    let compiled = build_compiled(
        "missing_art_wf",
        vec![step_a, step_b],
        RecoveryStrategy::Fail,
    )
    .unwrap();

    let handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(&workspace))
        .await
        .unwrap();

    assert_eq!(handle.status, WorkflowRunState::Failed);

    let step_a_run = repo
        .get_step_run_by_key(handle.run_id, "step_a")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(step_a_run.status, WorkflowStepState::Failed);

    let step_b_run = repo
        .get_step_run_by_key(handle.run_id, "step_b")
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        step_b_run.status,
        WorkflowStepState::Pending | WorkflowStepState::Blocked
    ));

    // Verify step_b is not completed
    let snapshot = engine.inspect_workflow(handle.run_id).await.unwrap();
    let step_b_snap = snapshot
        .step_runs
        .iter()
        .find(|s| s.step_key == "step_b")
        .unwrap();
    assert!(matches!(
        step_b_snap.status,
        WorkflowStepState::Pending | WorkflowStepState::Blocked
    ));
}

#[tokio::test]
async fn test_tier1_missing_artifact_gate_halts_workflow() {
    let dir = tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    tokio::fs::create_dir_all(&workspace).await.unwrap();
    let model = Arc::new(TestModelCaller::new());
    let (engine, repo, _) = setup_engine(&workspace, model).await;

    // Step expects "required_output.md", but nothing is written on disk
    let step = WorkflowStepDefinition {
        key: "step_a".to_string(),
        name: "Step A".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "step_a".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![OutputBinding {
            artifact_name: "required_output.md".to_string(),
            relative_path: PathBuf::from("required_output.md"),
            schema_type: "markdown".to_string(),
        }],
        required_capabilities: vec![],
        quality_gate: QualityGate {
            required_artifacts: vec!["required_output.md".to_string()],
            ..Default::default()
        },
        depends_on: vec![],
        timeout_secs: 300,
        allows_parallelism: false,
        recovery_strategy: Some(RecoveryStrategy::Fail),
    
        prompt_ref: None,};

    let compiled = build_compiled("t1_fail_wf", vec![step], RecoveryStrategy::Fail).unwrap();

    let handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(&workspace))
        .await
        .unwrap();

    assert_eq!(handle.status, WorkflowRunState::Failed);

    let step_run = repo
        .get_step_run_by_key(handle.run_id, "step_a")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(step_run.status, WorkflowStepState::Failed);
    assert!(step_run.halt_reason.is_some());
}

#[tokio::test]
async fn test_deny_human_approval_fails_workflow() {
    let dir = tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    tokio::fs::create_dir_all(&workspace).await.unwrap();
    let model = Arc::new(TestModelCaller::new());
    let (engine, repo, _) = setup_engine(&workspace, model).await;

    let step = WorkflowStepDefinition {
        key: "approval_step".to_string(),
        name: "Approval Step".to_string(),
        role: AgentRole::reviewer(),
        prompt_template: "approval_step".to_string(),
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
        recovery_strategy: Some(RecoveryStrategy::Fail),
    
        prompt_ref: None,};

    let compiled = build_compiled("deny_wf", vec![step], RecoveryStrategy::Fail).unwrap();

    let handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(&workspace))
        .await
        .unwrap();

    assert_eq!(handle.status, WorkflowRunState::AwaitingApproval);

    // Operator denies the step
    let state_after_deny = engine
        .deny_step(
            handle.run_id,
            "approval_step",
            "Artifact does not meet requirements",
            &compiled,
        )
        .await
        .unwrap();

    assert_eq!(state_after_deny, WorkflowRunState::Failed);

    let step_run = repo
        .get_step_run_by_key(handle.run_id, "approval_step")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(step_run.status, WorkflowStepState::Failed);
    assert!(
        step_run
            .halt_reason
            .unwrap()
            .contains("Artifact does not meet requirements")
    );
}
