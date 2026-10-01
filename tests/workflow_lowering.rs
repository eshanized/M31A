//! Comprehensive unit and integration test suite for Workflow Lowering into Canonical Mission DAG (AD-003).
//!
//! Verifies:
//! - Structural lowering of declarative workflows into `CandidatePlan` and `Mission`.
//! - Fail-closed rejection of cycles, self-loops, missing endpoints, and empty graphs.
//! - Accurate mapping of roles to capability tags (`role:<role>`).
//! - Accurate mapping of verification strategies (ArtifactInspection, ReviewGate, Composite, Compilation).
//! - Resource estimate bounding and deterministic task key mapping.
//! - Idempotency, stability, and projection serialization (`plan.json`).

use m31a::ids::MissionId;
use m31a::kernel::plan::{CandidateTaskKey, VerificationStrategy};
use m31a::prompt::{InMemoryPromptCatalog, PromptContract};
use m31a::state_machine::agent::AgentRole;
use m31a::workflow::compiler::{CompiledWorkflow, WorkflowCompiler};
use m31a::workflow::definition::{
    OutputBinding, QualityGate, RecoveryStrategy, WorkflowDefinition, WorkflowStepDefinition,
};
use m31a::workflow::manifest::WorkflowManifest;
use m31a::workflow::provenance::WorkflowProvenance;
use std::collections::BTreeMap;
use tempfile::tempdir;

fn build_step(
    key: &str,
    name: &str,
    role: AgentRole,
    depends_on: Vec<&str>,
    outputs: Vec<(&str, &str)>,
    require_approval: bool,
) -> WorkflowStepDefinition {
    WorkflowStepDefinition {
        key: key.to_string(),
        name: name.to_string(),
        role,
        prompt_template: format!("{}_prompt", key),
        required_inputs: vec![],
        expected_outputs: outputs
            .into_iter()
            .map(|(art, path)| OutputBinding {
                artifact_name: art.to_string(),
                relative_path: std::path::PathBuf::from(path),
                schema_type: "markdown".to_string(),
            })
            .collect(),
        required_capabilities: vec![],
        quality_gate: QualityGate {
            require_human_approval: require_approval,
            ..Default::default()
        },
        depends_on: depends_on.into_iter().map(String::from).collect(),
        timeout_secs: 180,
        allows_parallelism: true,
        recovery_strategy: None,
    }
}

fn build_compiled(id: &str, steps: Vec<WorkflowStepDefinition>) -> CompiledWorkflow {
    let def = WorkflowDefinition {
        id: id.to_string(),
        name: format!("Workflow {}", id),
        description: "Test lowering workflow".to_string(),
        version: 1,
        steps,
        default_recovery_strategy: RecoveryStrategy::Fail,
    };
    let provenance = WorkflowProvenance::new(
        id,
        1,
        "test-content-hash",
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

#[test]
fn test_01_lowering_single_step_preserves_attributes() {
    let step = build_step(
        "step_init",
        "Initialize Project",
        AgentRole::researcher(),
        vec![],
        vec![],
        false,
    );
    let compiled = build_compiled("single_step", vec![step]);
    let mission_id = MissionId::new();

    let lowered = compiled
        .lower(mission_id)
        .expect("single step lowers cleanly");
    assert_eq!(lowered.mission_id, mission_id);
    assert_eq!(lowered.candidate_plan.tasks.len(), 1);

    let task = &lowered.candidate_plan.tasks[0];
    assert_eq!(task.id, CandidateTaskKey::new("step_init"));
    assert_eq!(task.objective, "Initialize Project");
    assert_eq!(task.role, AgentRole::researcher());
    assert!(task.depends_on.is_empty());
    assert_eq!(lowered.candidate_plan.objective, "Initialize Project");
}

#[test]
fn test_02_lowering_linear_pipeline_wires_dependencies() {
    let step1 = build_step(
        "step_1",
        "Phase 1",
        AgentRole::researcher(),
        vec![],
        vec![],
        false,
    );
    let step2 = build_step(
        "step_2",
        "Phase 2",
        AgentRole::implementer(),
        vec!["step_1"],
        vec![],
        false,
    );
    let step3 = build_step(
        "step_3",
        "Phase 3",
        AgentRole::reviewer(),
        vec!["step_2"],
        vec![],
        false,
    );
    let compiled = build_compiled("linear", vec![step1, step2, step3]);

    let lowered = compiled.lower_default().expect("linear lowers cleanly");
    assert_eq!(lowered.candidate_plan.tasks.len(), 3);
    assert_eq!(lowered.candidate_plan.tasks[0].depends_on, vec![]);
    assert_eq!(
        lowered.candidate_plan.tasks[1].depends_on,
        vec![CandidateTaskKey::new("step_1")]
    );
    assert_eq!(
        lowered.candidate_plan.tasks[2].depends_on,
        vec![CandidateTaskKey::new("step_2")]
    );
}

#[test]
fn test_03_lowering_diamond_dag_topology() {
    let step_a = build_step(
        "step_a",
        "Root",
        AgentRole::researcher(),
        vec![],
        vec![],
        false,
    );
    let step_b1 = build_step(
        "step_b1",
        "Branch 1",
        AgentRole::implementer(),
        vec!["step_a"],
        vec![],
        false,
    );
    let step_b2 = build_step(
        "step_b2",
        "Branch 2",
        AgentRole::implementer(),
        vec!["step_a"],
        vec![],
        false,
    );
    let step_c = build_step(
        "step_c",
        "Join",
        AgentRole::integrator(),
        vec!["step_b1", "step_b2"],
        vec![],
        false,
    );
    let compiled = build_compiled("diamond", vec![step_a, step_b1, step_b2, step_c]);

    let lowered = compiled.lower_default().expect("diamond lowers cleanly");
    assert_eq!(lowered.candidate_plan.tasks.len(), 4);
    let join_task = lowered
        .candidate_plan
        .tasks
        .iter()
        .find(|t| t.id == CandidateTaskKey::new("step_c"))
        .unwrap();
    assert_eq!(join_task.depends_on.len(), 2);
    assert!(
        join_task
            .depends_on
            .contains(&CandidateTaskKey::new("step_b1"))
    );
    assert!(
        join_task
            .depends_on
            .contains(&CandidateTaskKey::new("step_b2"))
    );
}

#[test]
fn test_04_lowering_empty_workflow_fails_closed() {
    let def = WorkflowDefinition {
        id: "empty".to_string(),
        name: "Empty".to_string(),
        description: "Empty workflow".to_string(),
        version: 1,
        steps: vec![],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };
    let provenance = WorkflowProvenance::new(
        "empty",
        1,
        "hash",
        1,
        BTreeMap::new(),
        None,
        chrono::Utc::now(),
    );
    let compiled = CompiledWorkflow {
        definition: def,
        provenance,
        topological_order: vec![],
    };

    let res = compiled.lower_default();
    assert!(res.is_err(), "Empty workflow lowering must fail closed");
}

#[test]
fn test_05_lowering_cyclic_workflow_fails_closed() {
    let step1 = build_step(
        "step_1",
        "Phase 1",
        AgentRole::implementer(),
        vec!["step_2"],
        vec![],
        false,
    );
    let step2 = build_step(
        "step_2",
        "Phase 2",
        AgentRole::implementer(),
        vec!["step_1"],
        vec![],
        false,
    );
    // Bypass validation at definition level to test lowering defense
    let def = WorkflowDefinition {
        id: "cyclic".to_string(),
        name: "Cyclic".to_string(),
        description: "Cyclic".to_string(),
        version: 1,
        steps: vec![step1, step2],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };
    let provenance = WorkflowProvenance::new(
        "cyclic",
        1,
        "hash",
        1,
        BTreeMap::new(),
        None,
        chrono::Utc::now(),
    );
    let compiled = CompiledWorkflow {
        definition: def,
        provenance,
        topological_order: vec!["step_1".to_string(), "step_2".to_string()],
    };

    let res = compiled.lower_default();
    assert!(
        res.is_err(),
        "Cyclic candidate plan must be rejected fail-closed"
    );
}

#[test]
fn test_06_lowering_self_loop_fails_closed() {
    let step = build_step(
        "step_self",
        "Self Loop",
        AgentRole::implementer(),
        vec!["step_self"],
        vec![],
        false,
    );
    let def = WorkflowDefinition {
        id: "self_loop".to_string(),
        name: "Self Loop".to_string(),
        description: "Self Loop".to_string(),
        version: 1,
        steps: vec![step],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };
    let provenance = WorkflowProvenance::new(
        "self_loop",
        1,
        "hash",
        1,
        BTreeMap::new(),
        None,
        chrono::Utc::now(),
    );
    let compiled = CompiledWorkflow {
        definition: def,
        provenance,
        topological_order: vec!["step_self".to_string()],
    };

    let res = compiled.lower_default();
    assert!(
        res.is_err(),
        "Self loop candidate plan must be rejected fail-closed"
    );
}

#[test]
fn test_07_lowering_missing_dependency_fails_closed() {
    let step = build_step(
        "step_1",
        "Missing Prereq",
        AgentRole::implementer(),
        vec!["ghost_step"],
        vec![],
        false,
    );
    let def = WorkflowDefinition {
        id: "missing_dep".to_string(),
        name: "Missing Dep".to_string(),
        description: "Missing Dep".to_string(),
        version: 1,
        steps: vec![step],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };
    let provenance = WorkflowProvenance::new(
        "missing_dep",
        1,
        "hash",
        1,
        BTreeMap::new(),
        None,
        chrono::Utc::now(),
    );
    let compiled = CompiledWorkflow {
        definition: def,
        provenance,
        topological_order: vec!["step_1".to_string()],
    };

    let res = compiled.lower_default();
    assert!(
        res.is_err(),
        "Missing prerequisite endpoint must fail closed"
    );
}

#[test]
fn test_08_lowering_duplicate_dependency_edge_fails_closed() {
    let step_a = build_step(
        "step_a",
        "Root",
        AgentRole::researcher(),
        vec![],
        vec![],
        false,
    );
    let step_b = build_step(
        "step_b",
        "Duplicate Dep",
        AgentRole::implementer(),
        vec!["step_a", "step_a"],
        vec![],
        false,
    );
    let def = WorkflowDefinition {
        id: "dup_dep".to_string(),
        name: "Dup Dep".to_string(),
        description: "Dup Dep".to_string(),
        version: 1,
        steps: vec![step_a, step_b],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };
    let provenance = WorkflowProvenance::new(
        "dup_dep",
        1,
        "hash",
        1,
        BTreeMap::new(),
        None,
        chrono::Utc::now(),
    );
    let compiled = CompiledWorkflow {
        definition: def,
        provenance,
        topological_order: vec!["step_a".to_string(), "step_b".to_string()],
    };

    let res = compiled.lower_default();
    assert!(res.is_err(), "Duplicate edge in plan must fail closed");
}

#[test]
fn test_09_lowering_disconnected_components_allowed() {
    let step_a = build_step(
        "step_a",
        "Component 1",
        AgentRole::researcher(),
        vec![],
        vec![],
        false,
    );
    let step_b = build_step(
        "step_b",
        "Component 2",
        AgentRole::implementer(),
        vec![],
        vec![],
        false,
    );
    let compiled = build_compiled("disconnected", vec![step_a, step_b]);

    let lowered = compiled
        .lower_default()
        .expect("disconnected components are valid DAG");
    assert_eq!(lowered.candidate_plan.tasks.len(), 2);
    assert_eq!(lowered.candidate_plan.tasks[0].depends_on, vec![]);
    assert_eq!(lowered.candidate_plan.tasks[1].depends_on, vec![]);
}

#[test]
fn test_10_lowering_role_capability_tags_generated() {
    let step = build_step(
        "step_res",
        "Research",
        AgentRole::researcher(),
        vec![],
        vec![],
        false,
    );
    let compiled = build_compiled("roles", vec![step]);

    let lowered = compiled.lower_default().unwrap();
    let task = &lowered.candidate_plan.tasks[0];
    assert!(
        task.capabilities.iter().any(|c| c.id == "role:researcher"),
        "Task capabilities must include role:researcher tag"
    );
}

#[test]
fn test_11_lowering_explicit_capabilities_preserved() {
    let mut step = build_step(
        "step_spec",
        "Specialized",
        AgentRole::implementer(),
        vec![],
        vec![],
        false,
    );
    step.required_capabilities
        .push(m31a::kernel::plan::CapabilityRequirement::new(
            "custom:gpu_accelerator",
            m31a::kernel::plan::CapabilityAccessMode::Read,
        ));
    let compiled = build_compiled("custom_caps", vec![step]);

    let lowered = compiled.lower_default().unwrap();
    let task = &lowered.candidate_plan.tasks[0];
    assert!(
        task.capabilities
            .iter()
            .any(|c| c.id == "custom:gpu_accelerator")
    );
    assert!(task.capabilities.iter().any(|c| c.id == "role:implementer"));
}

#[test]
fn test_12_lowering_verification_artifact_inspection() {
    let step = build_step(
        "step_art",
        "Produce Artifact",
        AgentRole::implementer(),
        vec![],
        vec![("charter", "docs/CHARTER.md")],
        false,
    );
    let compiled = build_compiled("artifact_strat", vec![step]);

    let lowered = compiled.lower_default().unwrap();
    let task = &lowered.candidate_plan.tasks[0];
    match &task.verification {
        VerificationStrategy::ArtifactInspection { paths } => {
            assert_eq!(paths, &vec!["docs/CHARTER.md".to_string()]);
        }
        other => panic!("Expected ArtifactInspection, got {:?}", other),
    }
}

#[test]
fn test_13_lowering_verification_human_approval_gate() {
    let step = build_step(
        "step_app",
        "Approve Plan",
        AgentRole::reviewer(),
        vec![],
        vec![],
        true,
    );
    let compiled = build_compiled("review_gate", vec![step]);

    let lowered = compiled.lower_default().unwrap();
    let task = &lowered.candidate_plan.tasks[0];
    match &task.verification {
        VerificationStrategy::ReviewGate { reviewer_role } => {
            assert_eq!(*reviewer_role, Some(AgentRole::reviewer()));
        }
        other => panic!("Expected ReviewGate, got {:?}", other),
    }
}

#[test]
fn test_14_lowering_verification_composite_strategy() {
    let step = build_step(
        "step_comp",
        "Produce and Approve",
        AgentRole::integrator(),
        vec![],
        vec![("summary", "SUMMARY.md")],
        true,
    );
    let compiled = build_compiled("composite_gate", vec![step]);

    let lowered = compiled.lower_default().unwrap();
    let task = &lowered.candidate_plan.tasks[0];
    match &task.verification {
        VerificationStrategy::Composite { strategies } => {
            assert_eq!(strategies.len(), 2);
            assert!(matches!(
                strategies[0],
                VerificationStrategy::ArtifactInspection { .. }
            ));
            assert!(matches!(
                strategies[1],
                VerificationStrategy::ReviewGate { .. }
            ));
        }
        other => panic!("Expected Composite strategy, got {:?}", other),
    }
}

#[test]
fn test_15_lowering_verification_analytical_roles() {
    let step_diag = build_step(
        "step_diag",
        "Diagnose",
        AgentRole::diagnostician(),
        vec![],
        vec![],
        false,
    );
    let step_rev = build_step(
        "step_rev",
        "Review",
        AgentRole::reviewer(),
        vec![],
        vec![],
        false,
    );
    let compiled = build_compiled("analytical", vec![step_diag, step_rev]);

    let lowered = compiled.lower_default().unwrap();
    assert_eq!(
        lowered.candidate_plan.tasks[0].verification,
        VerificationStrategy::Compilation
    );
    assert_eq!(
        lowered.candidate_plan.tasks[1].verification,
        VerificationStrategy::ReviewGate {
            reviewer_role: Some(AgentRole::reviewer()),
        }
    );
}

#[test]
fn test_16_lowering_resource_estimates_bounded() {
    let mut step = build_step(
        "step_est",
        "Estimate Step",
        AgentRole::researcher(),
        vec![],
        vec![],
        false,
    );
    step.timeout_secs = 600;
    let compiled = build_compiled("estimates", vec![step]);

    let lowered = compiled.lower_default().unwrap();
    let task = &lowered.candidate_plan.tasks[0];
    assert_eq!(task.estimates.max_steps, 10);
    assert_eq!(task.estimates.max_duration_secs, 600);
    assert_eq!(task.estimates.max_tokens, 50_000);
    assert_eq!(task.estimates.max_cost_usd, 1.0);
}

#[test]
fn test_17_lowering_determinism_identical_runs() {
    let step1 = build_step(
        "step_1",
        "Step 1",
        AgentRole::researcher(),
        vec![],
        vec![],
        false,
    );
    let step2 = build_step(
        "step_2",
        "Step 2",
        AgentRole::implementer(),
        vec!["step_1"],
        vec![],
        false,
    );
    let compiled = build_compiled("determinism", vec![step1, step2]);

    let mission_id = MissionId::new();
    let lowered1 = compiled.lower(mission_id).unwrap();
    let lowered2 = compiled.lower(mission_id).unwrap();

    assert_eq!(lowered1.mission_id, lowered2.mission_id);
    assert_eq!(
        lowered1.candidate_plan.plan_id,
        lowered2.candidate_plan.plan_id
    );
    assert_eq!(
        lowered1.candidate_plan.objective,
        lowered2.candidate_plan.objective
    );
    assert_eq!(lowered1.candidate_plan.tasks, lowered2.candidate_plan.tasks);
    assert_eq!(lowered1.step_task_keys, lowered2.step_task_keys);
}

#[test]
fn test_18_lowering_projection_serialization() {
    let step = build_step(
        "step_proj",
        "Projection",
        AgentRole::researcher(),
        vec![],
        vec![],
        false,
    );
    let compiled = build_compiled("proj", vec![step]);
    let lowered = compiled.lower_default().unwrap();

    let json = serde_json::to_string_pretty(&lowered.candidate_plan).unwrap();
    assert!(json.contains("step_proj"));
    assert!(json.contains("Projection"));

    let deserialized: m31a::kernel::plan::CandidatePlan = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.plan_id, lowered.candidate_plan.plan_id);
    assert_eq!(deserialized.tasks, lowered.candidate_plan.tasks);
}

#[test]
fn test_19_lowering_direct_workflow_compiler_api() {
    let dir = tempdir().unwrap();
    let toml_str = r#"
[workflow]
id = "manifest_test"
name = "Manifest Lowering Test"
version = 1
description = "Direct compiler lower test"

[[steps]]
key = "init"
name = "Init Step"
role = "researcher"
prompt = "discovery:1"
timeout_secs = 120
"#;

    let manifest = WorkflowManifest::from_toml_str(toml_str).expect("parse manifest");
    let mut catalog = InMemoryPromptCatalog::new();
    let contract = PromptContract::new(
        "discovery",
        1,
        AgentRole::researcher(),
        "discovery prompt",
        vec![],
        "execute discovery",
        Some("markdown".to_string()),
    )
    .unwrap();
    catalog.register(contract).unwrap();

    let compiler = WorkflowCompiler::new(&catalog);
    let mission_id = MissionId::new();
    let lowered = compiler
        .lower(&manifest, None, Some(dir.path()), mission_id)
        .expect("WorkflowCompiler::lower succeeds");

    assert_eq!(lowered.mission_id, mission_id);
    assert_eq!(lowered.candidate_plan.tasks.len(), 1);
    assert_eq!(
        lowered.candidate_plan.tasks[0].id,
        CandidateTaskKey::new("init")
    );
}

#[test]
fn test_20_lowering_step_task_key_mapping() {
    let step1 = build_step(
        "step_alpha",
        "Alpha",
        AgentRole::researcher(),
        vec![],
        vec![],
        false,
    );
    let step2 = build_step(
        "step_beta",
        "Beta",
        AgentRole::implementer(),
        vec!["step_alpha"],
        vec![],
        false,
    );
    let compiled = build_compiled("mapping", vec![step1, step2]);

    let lowered = compiled.lower_default().unwrap();
    assert_eq!(lowered.step_task_keys.len(), 2);
    assert_eq!(
        lowered.step_task_keys.get("step_alpha"),
        Some(&CandidateTaskKey::new("step_alpha"))
    );
    assert_eq!(
        lowered.step_task_keys.get("step_beta"),
        Some(&CandidateTaskKey::new("step_beta"))
    );
}
