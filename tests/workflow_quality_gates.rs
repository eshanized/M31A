//! Integration test suite for Tier-1 and Tier-2 Quality Gate evaluation (Package 2).

use m31a::ids::{WorkflowRunId, WorkflowStepRunId};
use m31a::prompt::PromptContract;
use m31a::state_machine::agent::AgentRole;
use m31a::workflow::definition::{
    InputBinding, OutputBinding, QualityGate, WorkflowDefinition, WorkflowStepDefinition,
};
use m31a::workflow::quality_gate::{
    QualityGateContext, QualityGateEvaluator, QualityGateSeverity, QualityGateTier,
    Tier1QualityGateEvaluator, Tier2QualityGateEvaluator,
};
use m31a::workflow::state::WorkflowArtifact;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn test_tier1_required_artifact_success() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create artifact on disk
    let art_path = root.join("DISCOVERY.md");
    std::fs::write(&art_path, "# Discovery\nProject charter content").unwrap();

    let gate = QualityGate {
        schema: Some("markdown".to_string()),
        required_artifacts: vec!["DISCOVERY.md".to_string()],
        require_human_approval: false,
        max_ambiguity_percent: Some(10),
    };

    let context = QualityGateContext {
        step_key: "discovery",
        workspace_root: root,
        step_definition: None,
        workflow_definition: None,
        produced_artifacts: &[],
        upstream_artifacts: &[],
        prompt_contract: None,
    };

    let evaluator = Tier1QualityGateEvaluator::new();
    let result = evaluator.evaluate(&gate, &context).unwrap();
    assert!(result.is_passed);
    assert_eq!(result.tier, QualityGateTier::Tier1SyntaxAndArtifacts);
    assert!(result.violations.is_empty());
}

#[test]
fn test_tier1_missing_artifact_failure() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let gate = QualityGate {
        schema: None,
        required_artifacts: vec!["MISSING.md".to_string()],
        require_human_approval: false,
        max_ambiguity_percent: None,
    };

    let context = QualityGateContext {
        step_key: "discovery",
        workspace_root: root,
        step_definition: None,
        workflow_definition: None,
        produced_artifacts: &[],
        upstream_artifacts: &[],
        prompt_contract: None,
    };

    let evaluator = Tier1QualityGateEvaluator::new();
    let result = evaluator.evaluate(&gate, &context).unwrap();
    assert!(!result.is_passed);
    assert_eq!(result.violations.len(), 1);
    assert_eq!(result.violations[0].code, "QG-T1-01");
    assert_eq!(result.violations[0].severity, QualityGateSeverity::Error);
}

#[test]
fn test_tier1_empty_artifact_failure() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create empty artifact on disk (0 bytes)
    let empty_path = root.join("EMPTY.md");
    std::fs::write(&empty_path, b"").unwrap();

    let gate = QualityGate {
        schema: None,
        required_artifacts: vec!["EMPTY.md".to_string()],
        require_human_approval: false,
        max_ambiguity_percent: None,
    };

    let context = QualityGateContext {
        step_key: "discovery",
        workspace_root: root,
        step_definition: None,
        workflow_definition: None,
        produced_artifacts: &[],
        upstream_artifacts: &[],
        prompt_contract: None,
    };

    let evaluator = Tier1QualityGateEvaluator::new();
    let result = evaluator.evaluate(&gate, &context).unwrap();
    assert!(!result.is_passed);
    assert!(result.violations.iter().any(|v| v.code == "QG-T1-03"));
}

#[test]
fn test_tier1_invalid_hash_in_produced_artifact_record() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let run_id = WorkflowRunId::new();
    let step_id = WorkflowStepRunId::new();

    // Invalid hash (not 64 hex characters)
    let bad_art = WorkflowArtifact::new(
        run_id,
        step_id,
        "REPORT.md",
        PathBuf::from("REPORT.md"),
        "invalid_short_hash",
        1,
    );

    let gate = QualityGate {
        schema: None,
        required_artifacts: vec!["REPORT.md".to_string()],
        require_human_approval: false,
        max_ambiguity_percent: None,
    };

    let context = QualityGateContext {
        step_key: "report",
        workspace_root: root,
        step_definition: None,
        workflow_definition: None,
        produced_artifacts: &[bad_art],
        upstream_artifacts: &[],
        prompt_contract: None,
    };

    let evaluator = Tier1QualityGateEvaluator::new();
    let result = evaluator.evaluate(&gate, &context).unwrap();
    assert!(!result.is_passed);
    assert!(result.violations.iter().any(|v| v.code == "QG-T1-05"));
}

#[test]
fn test_tier2_relational_consistency_success() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let step_a = WorkflowStepDefinition {
        key: "step_a".to_string(),
        name: "Step A".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "genesis.discovery.v1".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![OutputBinding {
            artifact_name: "A.md".to_string(),
            relative_path: PathBuf::from("docs/A.md"),
            schema_type: "markdown".to_string(),
        }],
        required_capabilities: vec![],
        quality_gate: QualityGate {
            schema: None,
            required_artifacts: vec!["A.md".to_string()],
            require_human_approval: false,
            max_ambiguity_percent: None,
        },
        depends_on: vec![],
        timeout_secs: 600,
        allows_parallelism: false,
        recovery_strategy: None,
    };

    let step_b = WorkflowStepDefinition {
        key: "step_b".to_string(),
        name: "Step B".to_string(),
        role: AgentRole::architect(),
        prompt_template: "genesis.architecture.v1".to_string(),
        required_inputs: vec![InputBinding {
            parameter_name: "input_a".to_string(),
            source_step_key: "step_a".to_string(),
            artifact_name: "A.md".to_string(),
            is_optional: false,
        }],
        expected_outputs: vec![OutputBinding {
            artifact_name: "B.md".to_string(),
            relative_path: PathBuf::from("docs/B.md"),
            schema_type: "markdown".to_string(),
        }],
        required_capabilities: vec![],
        quality_gate: QualityGate {
            schema: None,
            required_artifacts: vec!["B.md".to_string()],
            require_human_approval: false,
            max_ambiguity_percent: None,
        },
        depends_on: vec!["step_a".to_string()],
        timeout_secs: 600,
        allows_parallelism: false,
        recovery_strategy: None,
    };

    let wf = WorkflowDefinition {
        id: "wf".to_string(),
        name: "WF".to_string(),
        description: "".to_string(),
        version: 1,
        steps: vec![step_a.clone(), step_b.clone()],
        default_recovery_strategy: m31a::workflow::definition::RecoveryStrategy::Fail,
    };

    let prompt_b = PromptContract::new(
        "genesis.architecture",
        1,
        AgentRole::architect(),
        "Architecture",
        vec![],
        "Body",
        None,
    )
    .unwrap();

    let context = QualityGateContext {
        step_key: "step_b",
        workspace_root: root,
        step_definition: Some(&step_b),
        workflow_definition: Some(&wf),
        produced_artifacts: &[],
        upstream_artifacts: &[],
        prompt_contract: Some(&prompt_b),
    };

    let evaluator = Tier2QualityGateEvaluator::new();
    let result = evaluator.evaluate(&step_b.quality_gate, &context).unwrap();
    assert!(result.is_passed);
    assert_eq!(result.tier, QualityGateTier::Tier2RelationalConsistency);
    assert!(result.violations.is_empty());
}

#[test]
fn test_tier2_catches_undeclared_gate_artifact() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Step declares NO expected outputs, but quality gate demands "NON_EXISTENT.md"
    let step = WorkflowStepDefinition {
        key: "step".to_string(),
        name: "Step".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "test.v1".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate {
            schema: None,
            required_artifacts: vec!["NON_EXISTENT.md".to_string()],
            require_human_approval: false,
            max_ambiguity_percent: None,
        },
        depends_on: vec![],
        timeout_secs: 600,
        allows_parallelism: false,
        recovery_strategy: None,
    };

    let context = QualityGateContext {
        step_key: "step",
        workspace_root: root,
        step_definition: Some(&step),
        workflow_definition: None,
        produced_artifacts: &[],
        upstream_artifacts: &[],
        prompt_contract: None,
    };

    let evaluator = Tier2QualityGateEvaluator::new();
    let result = evaluator.evaluate(&step.quality_gate, &context).unwrap();
    assert!(!result.is_passed);
    assert_eq!(result.violations[0].code, "QG-T2-01");
}

#[test]
fn test_tier2_catches_role_mismatch() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let step = WorkflowStepDefinition {
        key: "step".to_string(),
        name: "Step".to_string(),
        role: AgentRole::architect(), // Step wants Architect
        prompt_template: "test.v1".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 600,
        allows_parallelism: false,
        recovery_strategy: None,
    };

    let prompt = PromptContract::new(
        "test.prompt",
        1,
        AgentRole::implementer(), // Prompt is Implementer -> Mismatch!
        "Implementer prompt",
        vec![],
        "Body",
        None,
    )
    .unwrap();

    let context = QualityGateContext {
        step_key: "step",
        workspace_root: root,
        step_definition: Some(&step),
        workflow_definition: None,
        produced_artifacts: &[],
        upstream_artifacts: &[],
        prompt_contract: Some(&prompt),
    };

    let evaluator = Tier2QualityGateEvaluator::new();
    let result = evaluator.evaluate(&step.quality_gate, &context).unwrap();
    assert!(!result.is_passed);
    assert_eq!(result.violations[0].code, "QG-T2-05");
    assert!(result.violations[0].message.contains("role mismatch"));
}
