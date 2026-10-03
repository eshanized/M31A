//! Comprehensive domain test suite for workflow kernel (Package 1).
//!
//! Tests:
//! - WorkflowRunId & WorkflowStepRunId contracts
//! - WorkflowDefinition & StepDefinition validation
//! - Cycle detection & topological ordering
//! - Artifact path security & confinement
//! - State machines & transition invariants

use m31a::ids::{WorkflowRunId, WorkflowStepRunId};
use m31a::kernel::plan::{CapabilityAccessMode, CapabilityRequirement};
use m31a::state_machine::agent::AgentRole;
use m31a::workflow::definition::{
    InputBinding, OutputBinding, QualityGate, RecoveryStrategy, WorkflowDefinition,
    WorkflowStepDefinition, validate_artifact_path,
};
use m31a::workflow::error::WorkflowError;
use m31a::workflow::state::{
    WorkflowArtifact, WorkflowArtifactStatus, WorkflowMode, WorkflowRun, WorkflowRunState,
    WorkflowStepRun, WorkflowStepState,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::str::FromStr;

fn helper_step(
    key: &str,
    deps: Vec<&str>,
    outputs: Vec<(&str, &str)>,
    inputs: Vec<(&str, &str, &str)>,
) -> WorkflowStepDefinition {
    WorkflowStepDefinition {
        key: key.to_string(),
        name: format!("Step {}", key),
        role: AgentRole::researcher(),
        prompt_template: format!("prompts/{}", key),
        required_inputs: inputs
            .into_iter()
            .map(|(param, src_step, art)| InputBinding {
                parameter_name: param.to_string(),
                source_step_key: src_step.to_string(),
                artifact_name: art.to_string(),
                is_optional: false,
            })
            .collect(),
        expected_outputs: outputs
            .into_iter()
            .map(|(art, rel_path)| OutputBinding {
                artifact_name: art.to_string(),
                relative_path: PathBuf::from(rel_path),
                schema_type: "markdown".to_string(),
            })
            .collect(),
        required_capabilities: vec![CapabilityRequirement::new(
            "fs.read",
            CapabilityAccessMode::Read,
        )],
        quality_gate: QualityGate::default(),
        depends_on: deps.into_iter().map(String::from).collect(),
        timeout_secs: 300,
        allows_parallelism: true,
        recovery_strategy: None,

        prompt_ref: None,
    }
}

#[test]
fn test_workflow_identifiers_contract() {
    let run_id_1 = WorkflowRunId::new();
    let run_id_2 = WorkflowRunId::new();
    assert_ne!(run_id_1, run_id_2);

    // Display & parsing
    let display_str = run_id_1.to_string();
    let parsed_id = WorkflowRunId::from_str(&display_str).expect("parse display str");
    assert_eq!(run_id_1, parsed_id);

    // Serde JSON roundtrip
    let json = serde_json::to_string(&run_id_1).expect("serialize json");
    let deserialized: WorkflowRunId = serde_json::from_str(&json).expect("deserialize json");
    assert_eq!(run_id_1, deserialized);

    // Byte conversion roundtrip
    let bytes = *run_id_1.as_bytes();
    let from_bytes_id = WorkflowRunId::from_bytes(bytes);
    assert_eq!(run_id_1, from_bytes_id);

    // Hash set uniqueness
    let mut set = HashSet::new();
    set.insert(run_id_1);
    set.insert(run_id_2);
    assert_eq!(set.len(), 2);

    // Step run ID
    let step_id_1 = WorkflowStepRunId::new();
    let step_id_2 = WorkflowStepRunId::new();
    assert_ne!(step_id_1, step_id_2);
    let step_json = serde_json::to_string(&step_id_1).expect("serialize step json");
    let step_deser: WorkflowStepRunId = serde_json::from_str(&step_json).expect("deserialize step");
    assert_eq!(step_id_1, step_deser);
    assert_eq!(
        step_id_1,
        WorkflowStepRunId::from_bytes(*step_id_1.as_bytes())
    );
}

#[test]
fn test_workflow_definition_serialization_roundtrip() {
    let def = WorkflowDefinition {
        id: "genesis-greenfield".to_string(),
        name: "Genesis Greenfield Workflow".to_string(),
        description: "Autonomous project genesis from scratch".to_string(),
        version: 1,
        steps: vec![
            helper_step(
                "discovery",
                vec![],
                vec![("DISCOVERY.md", "DISCOVERY.md")],
                vec![],
            ),
            helper_step(
                "charter",
                vec!["discovery"],
                vec![("PROJECT.md", "PROJECT.md")],
                vec![("discovery_doc", "discovery", "DISCOVERY.md")],
            ),
        ],
        default_recovery_strategy: RecoveryStrategy::Retry { max_retries: 2 },
    };

    // Validate
    assert!(def.validate().is_ok());

    // JSON roundtrip
    let json_str = serde_json::to_string_pretty(&def).expect("json serialize");
    let deserialized: WorkflowDefinition =
        serde_json::from_str(&json_str).expect("json deserialize");
    assert_eq!(def, deserialized);

    // TOML roundtrip
    let toml_str = toml::to_string(&def).expect("toml serialize");
    let deserialized_toml: WorkflowDefinition =
        toml::from_str(&toml_str).expect("toml deserialize");
    assert_eq!(def, deserialized_toml);
}

#[test]
fn test_validation_rejects_empty_id_or_name() {
    let mut def = WorkflowDefinition {
        id: "   ".to_string(),
        name: "Name".to_string(),
        description: "Desc".to_string(),
        version: 1,
        steps: vec![helper_step("step1", vec![], vec![], vec![])],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };
    assert!(matches!(
        def.validate(),
        Err(WorkflowError::InvalidDefinition(msg)) if msg.contains("id cannot be empty")
    ));

    def.id = "valid-id".to_string();
    def.name = "".to_string();
    assert!(matches!(
        def.validate(),
        Err(WorkflowError::InvalidDefinition(msg)) if msg.contains("name cannot be empty")
    ));

    def.name = "Valid Name".to_string();
    def.version = 0;
    assert!(matches!(
        def.validate(),
        Err(WorkflowError::InvalidDefinition(msg)) if msg.contains("version must be at least 1")
    ));
}

#[test]
fn test_validation_rejects_empty_steps() {
    let def = WorkflowDefinition {
        id: "test".to_string(),
        name: "Test".to_string(),
        description: "Desc".to_string(),
        version: 1,
        steps: vec![],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };
    assert!(matches!(
        def.validate(),
        Err(WorkflowError::InvalidDefinition(msg)) if msg.contains("at least one step")
    ));
}

#[test]
fn test_validation_rejects_duplicate_steps() {
    let def = WorkflowDefinition {
        id: "test-dup".to_string(),
        name: "Test Dup".to_string(),
        description: "Desc".to_string(),
        version: 1,
        steps: vec![
            helper_step("step_a", vec![], vec![], vec![]),
            helper_step("step_a", vec![], vec![], vec![]),
        ],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };
    assert!(matches!(
        def.validate(),
        Err(WorkflowError::DuplicateStep(key)) if key == "step_a"
    ));
}

#[test]
fn test_validation_rejects_missing_dependency() {
    let def = WorkflowDefinition {
        id: "test-missing-dep".to_string(),
        name: "Test Missing Dep".to_string(),
        description: "Desc".to_string(),
        version: 1,
        steps: vec![helper_step(
            "step_b",
            vec!["non_existent_step"],
            vec![],
            vec![],
        )],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };
    assert!(matches!(
        def.validate(),
        Err(WorkflowError::MissingDependency { step, prerequisite })
            if step == "step_b" && prerequisite == "non_existent_step"
    ));
}

#[test]
fn test_validation_rejects_self_dependency() {
    let def = WorkflowDefinition {
        id: "test-self-dep".to_string(),
        name: "Test Self Dep".to_string(),
        description: "Desc".to_string(),
        version: 1,
        steps: vec![helper_step("step_loop", vec!["step_loop"], vec![], vec![])],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };
    assert!(matches!(
        def.validate(),
        Err(WorkflowError::InvalidDefinition(msg)) if msg.contains("cannot depend on itself")
    ));
}

#[test]
fn test_validation_rejects_cyclic_dependencies() {
    // 2-node cycle: A -> B -> A
    let def_2 = WorkflowDefinition {
        id: "test-cycle-2".to_string(),
        name: "Test Cycle 2".to_string(),
        description: "Desc".to_string(),
        version: 1,
        steps: vec![
            helper_step("step_a", vec!["step_b"], vec![], vec![]),
            helper_step("step_b", vec!["step_a"], vec![], vec![]),
        ],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };
    assert!(matches!(
        def_2.validate(),
        Err(WorkflowError::CycleDetected { .. })
    ));

    // 3-node cycle: A -> B -> C -> A
    let def_3 = WorkflowDefinition {
        id: "test-cycle-3".to_string(),
        name: "Test Cycle 3".to_string(),
        description: "Desc".to_string(),
        version: 1,
        steps: vec![
            helper_step("step_a", vec!["step_c"], vec![], vec![]),
            helper_step("step_b", vec!["step_a"], vec![], vec![]),
            helper_step("step_c", vec!["step_b"], vec![], vec![]),
        ],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };
    let err = def_3.validate().expect_err("cycle should fail");
    match err {
        WorkflowError::CycleDetected { cycle } => {
            assert_eq!(cycle.len(), 3);
            assert!(cycle.contains(&"step_a".to_string()));
            assert!(cycle.contains(&"step_b".to_string()));
            assert!(cycle.contains(&"step_c".to_string()));
        }
        other => panic!("expected CycleDetected, got {:?}", other),
    }
}

#[test]
fn test_topological_order_linear_and_diamond() {
    // Linear
    let linear = WorkflowDefinition {
        id: "linear".to_string(),
        name: "Linear".to_string(),
        description: "Linear".to_string(),
        version: 1,
        steps: vec![
            helper_step("c", vec!["b"], vec![], vec![]),
            helper_step("a", vec![], vec![], vec![]),
            helper_step("b", vec!["a"], vec![], vec![]),
        ],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };
    assert_eq!(linear.topological_order().unwrap(), vec!["a", "b", "c"]);

    // Diamond: root -> left & right -> sink
    let diamond = WorkflowDefinition {
        id: "diamond".to_string(),
        name: "Diamond".to_string(),
        description: "Diamond".to_string(),
        version: 1,
        steps: vec![
            helper_step("sink", vec!["left", "right"], vec![], vec![]),
            helper_step("right", vec!["root"], vec![], vec![]),
            helper_step("left", vec!["root"], vec![], vec![]),
            helper_step("root", vec![], vec![], vec![]),
        ],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };
    assert_eq!(
        diamond.topological_order().unwrap(),
        vec!["root", "left", "right", "sink"]
    );
}

#[test]
fn test_input_binding_validation() {
    // Input references source step not in depends_on
    let def_missing_dep = WorkflowDefinition {
        id: "binding-fail".to_string(),
        name: "Binding Fail".to_string(),
        description: "Desc".to_string(),
        version: 1,
        steps: vec![
            helper_step("producer", vec![], vec![("data.json", "data.json")], vec![]),
            helper_step(
                "consumer",
                vec![], // forgot to add "producer" to depends_on
                vec![],
                vec![("input_data", "producer", "data.json")],
            ),
        ],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };
    assert!(matches!(
        def_missing_dep.validate(),
        Err(WorkflowError::InvalidBinding { reason, .. }) if reason.contains("must be declared in depends_on")
    ));

    // Input references artifact that producer does not declare
    let def_missing_artifact = WorkflowDefinition {
        id: "artifact-fail".to_string(),
        name: "Artifact Fail".to_string(),
        description: "Desc".to_string(),
        version: 1,
        steps: vec![
            helper_step(
                "producer",
                vec![],
                vec![("actual.json", "actual.json")],
                vec![],
            ),
            helper_step(
                "consumer",
                vec!["producer"],
                vec![],
                vec![("input_data", "producer", "missing.json")],
            ),
        ],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };
    assert!(matches!(
        def_missing_artifact.validate(),
        Err(WorkflowError::InvalidBinding { reason, .. }) if reason.contains("does not declare expected output artifact")
    ));
}

#[test]
fn test_artifact_path_security_comprehensive() {
    let ws = Path::new("/workspace/project");

    // 1. Valid paths
    assert!(validate_artifact_path(Path::new("valid/project/file.md"), Some(ws)).is_ok());
    assert!(validate_artifact_path(Path::new("PROJECT.md"), Some(ws)).is_ok());
    assert!(validate_artifact_path(Path::new(".planning/REQUIREMENTS.md"), Some(ws)).is_ok());
    assert!(validate_artifact_path(Path::new("research/STACK.md"), Some(ws)).is_ok());

    // 2. Absolute path attack
    assert!(matches!(
        validate_artifact_path(Path::new("/etc/passwd"), Some(ws)),
        Err(WorkflowError::InvalidArtifactPath { reason, .. }) if reason.contains("must be relative")
    ));

    // 3. Parent traversal attack
    assert!(matches!(
        validate_artifact_path(Path::new("../etc/passwd"), Some(ws)),
        Err(WorkflowError::InvalidArtifactPath { reason, .. }) if reason.contains("path traversal")
    ));
    assert!(matches!(
        validate_artifact_path(Path::new("../../secret"), Some(ws)),
        Err(WorkflowError::InvalidArtifactPath { reason, .. }) if reason.contains("path traversal")
    ));
    assert!(matches!(
        validate_artifact_path(Path::new("docs/../../secret"), Some(ws)),
        Err(WorkflowError::InvalidArtifactPath { reason, .. }) if reason.contains("path traversal")
    ));

    // 4. Forbidden internal runtime directories
    assert!(matches!(
        validate_artifact_path(Path::new(".m31a/m31a.db"), Some(ws)),
        Err(WorkflowError::InvalidArtifactPath { reason, .. }) if reason.contains(".m31a")
    ));
    assert!(matches!(
        validate_artifact_path(Path::new("nested/.m31a/config"), Some(ws)),
        Err(WorkflowError::InvalidArtifactPath { reason, .. }) if reason.contains(".m31a")
    ));
    assert!(matches!(
        validate_artifact_path(Path::new(".git/HEAD"), Some(ws)),
        Err(WorkflowError::InvalidArtifactPath { reason, .. }) if reason.contains(".git")
    ));

    // 5. Empty path
    assert!(matches!(
        validate_artifact_path(Path::new("   "), Some(ws)),
        Err(WorkflowError::InvalidArtifactPath { reason, .. }) if reason.contains("cannot be empty")
    ));
}

#[test]
fn test_workflow_run_state_machine_legal_transitions() {
    let mut run = WorkflowRun::new("genesis", 1, PathBuf::from("."), WorkflowMode::Standard);
    assert_eq!(run.status, WorkflowRunState::Pending);

    // Pending -> Running
    assert!(run.transition_to(WorkflowRunState::Running, None).is_ok());
    assert_eq!(run.status, WorkflowRunState::Running);

    // Running -> AwaitingInput
    assert!(
        run.transition_to(WorkflowRunState::AwaitingInput, None)
            .is_ok()
    );
    // AwaitingInput -> Running
    assert!(run.transition_to(WorkflowRunState::Running, None).is_ok());

    // Running -> AwaitingApproval
    assert!(
        run.transition_to(WorkflowRunState::AwaitingApproval, None)
            .is_ok()
    );
    // AwaitingApproval -> Blocked
    assert!(run.transition_to(WorkflowRunState::Blocked, None).is_ok());
    // Blocked -> Running
    assert!(run.transition_to(WorkflowRunState::Running, None).is_ok());

    // Running -> Failed
    assert!(
        run.transition_to(WorkflowRunState::Failed, Some("Step timeout".into()))
            .is_ok()
    );
    assert_eq!(run.error_summary.as_deref(), Some("Step timeout"));

    // Failed -> Running (recovery)
    assert!(run.transition_to(WorkflowRunState::Running, None).is_ok());

    // Running -> Completed
    assert!(run.transition_to(WorkflowRunState::Completed, None).is_ok());
    assert!(run.completed_at.is_some());

    // Completed -> Superseded
    assert!(
        run.transition_to(WorkflowRunState::Superseded, None)
            .is_ok()
    );

    // Superseded is terminal: cannot transition anywhere
    assert!(run.transition_to(WorkflowRunState::Running, None).is_err());
    assert!(run.transition_to(WorkflowRunState::Pending, None).is_err());
}

#[test]
fn test_workflow_step_state_machine_legal_and_terminal() {
    let run_id = WorkflowRunId::new();
    let mut step = WorkflowStepRun::new(run_id, "research");
    assert_eq!(step.status, WorkflowStepState::Pending);
    assert_eq!(step.attempt_count, 1);

    // Pending -> Running
    assert!(step.transition_to(WorkflowStepState::Running, None).is_ok());

    // Running -> Failed
    assert!(
        step.transition_to(WorkflowStepState::Failed, Some("Network error".into()))
            .is_ok()
    );
    assert_eq!(step.attempt_count, 1);

    // Retry Failed -> Running increments attempt count
    assert!(step.transition_to(WorkflowStepState::Running, None).is_ok());
    assert_eq!(step.attempt_count, 2);

    // Running -> AwaitingApproval
    assert!(
        step.transition_to(WorkflowStepState::AwaitingApproval, None)
            .is_ok()
    );

    // AwaitingApproval -> Completed
    assert!(
        step.transition_to(WorkflowStepState::Completed, None)
            .is_ok()
    );
    assert!(step.completed_at.is_some());

    // Completed is terminal
    assert!(
        step.transition_to(WorkflowStepState::Running, None)
            .is_err()
    );
    assert!(step.transition_to(WorkflowStepState::Failed, None).is_err());
}

#[test]
fn test_workflow_artifact_model() {
    let run_id = WorkflowRunId::new();
    let step_id = WorkflowStepRunId::new();
    let mut artifact = WorkflowArtifact::new(
        run_id,
        step_id,
        "SUMMARY.md",
        PathBuf::from("research/SUMMARY.md"),
        "sha256:abcd1234",
        1,
    );

    assert_eq!(artifact.status, WorkflowArtifactStatus::Valid);
    assert_eq!(artifact.name, "SUMMARY.md");
    assert_eq!(artifact.version, 1);

    artifact.mark_superseded();
    assert_eq!(artifact.status, WorkflowArtifactStatus::Superseded);
}
