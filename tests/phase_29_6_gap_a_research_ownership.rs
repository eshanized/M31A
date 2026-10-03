//! Regression tests for Phase 29.6 Gap A: Research Artifact Ownership.
//!
//! Proves:
//! 1. Researcher roles are read-only and cannot directly write protected artifacts.
//! 2. Compiler & hierarchy verification do not require disk-backed files from read-only researchers.
//! 3. Authorized downstream role materializes research artifacts to disk.
//! 4. Evidence survives the handoff and artifact provenance is preserved.
//! 5. Custom research dimensions are not silently dropped.
//! 6. Missing evidence fails safely without fabricating content.

use m31a::agent::registry::RoleRegistry;
use m31a::ids::{MissionId, TaskId};
use m31a::kernel::plan::VerificationStrategy;
use m31a::planning::requirements::{RequirementCategory, RequirementPriority};
use m31a::state_machine::agent::AgentRole;
use m31a::verification::hierarchy::VerificationHierarchyEngine;
use m31a::workflow::compiler::CompiledWorkflow;
use m31a::workflow::definition::{QualityGate, WorkflowStepDefinition};
use m31a::workflow::genesis::ProjectCharter;
use m31a::workflow::genesis::dimension_registry::{
    ResearchDimensionDefinition, ResearchDimensionRegistry,
};
use m31a::workflow::genesis::provenance::ResearchFinding;
use m31a::workflow::genesis::research_decision::ResearchDimension;
use m31a::workflow::genesis::researcher::ResearchOrchestrator;
use m31a::workflow::genesis::synthesis::ResearchSynthesizer;
use tempfile::tempdir;

#[test]
fn test_researcher_is_read_only_and_cannot_write() {
    let registry = RoleRegistry::global().read().expect("lock readable");
    let researcher_role = AgentRole::stack_researcher();
    let def = registry
        .resolve(&researcher_role)
        .expect("stack_researcher exists");

    assert!(def.read_only_verification);
    assert!(!def.write_tools_permitted);
    assert_eq!(def.profile.sandbox_policy, "read_only");
}

#[tokio::test]
async fn test_compiler_and_hierarchy_read_only_gate() {
    let researcher_role = AgentRole::stack_researcher();
    let step = WorkflowStepDefinition {
        key: "test_research_step".to_string(),
        name: "Test Research Step".to_string(),
        role: researcher_role.clone(),
        prompt_template: "test.prompt".to_string(),
        required_inputs: Vec::new(),
        expected_outputs: Vec::new(),
        required_capabilities: Vec::new(),
        quality_gate: QualityGate::default(),
        depends_on: Vec::new(),
        timeout_secs: 60,
        allows_parallelism: true,
        recovery_strategy: None,

        prompt_ref: None,
    };

    // Compiler should map read-only step to its default verification (empty artifact inspection)
    let strat = CompiledWorkflow::map_step_verification(&step);
    match &strat {
        VerificationStrategy::ArtifactInspection { paths } => {
            assert!(
                paths.is_empty(),
                "Read-only role must not expect disk paths"
            );
        }
        _ => panic!("Expected ArtifactInspection with empty paths"),
    }

    // Verification hierarchy must pass read-only task gate even if no files exist on disk
    let temp = tempdir().unwrap();
    let hierarchy = VerificationHierarchyEngine::for_workspace(temp.path(), None, None);
    let checks = hierarchy
        .execute_hierarchy_for_task(
            MissionId::new(),
            TaskId::new(),
            temp.path(),
            "dummy_hash",
            "stack_researcher",
            "ArtifactInspection",
        )
        .await;

    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].command_or_tool, "read_only_task_gate");
    assert_eq!(
        checks[0].status,
        m31a::verification::types::CheckStatus::Passed
    );
}

#[test]
fn test_authorized_downstream_role_materializes_research_artifacts() {
    let temp = tempdir().unwrap();
    let workspace = temp.path();

    let mut finding1 = ResearchFinding::new(
        ResearchDimension::stack(),
        "Stack Findings",
        "Rust 2024 edition selected for kernel.",
    );
    finding1.risks.push("Nightly feature requirement".into());
    finding1
        .recommendations
        .push("Use stable 1.85+ if possible".into());

    let mut finding2 = ResearchFinding::new(
        ResearchDimension::security(),
        "Security Findings",
        "Strict sandbox enforcement via seccomp.",
    );
    finding2.tradeoffs.push("Overhead of ptrace vs eBPF".into());

    let findings = vec![finding1, finding2];
    let charter = ProjectCharter::new("SecureApp", "Secure runtime application");
    let summary = ResearchSynthesizer::synthesize(&charter, &findings).unwrap();

    let written_paths = ResearchOrchestrator::materialize_research_artifacts(
        workspace,
        ".planning",
        &findings,
        Some(&summary),
    )
    .expect("materialization succeeds");

    assert_eq!(written_paths.len(), 3);
    assert!(workspace.join(".planning/research/STACK.md").exists());
    assert!(workspace.join(".planning/research/SECURITY.md").exists());
    assert!(workspace.join(".planning/research/SUMMARY.md").exists());

    // Verify STACK.md content
    let stack_content =
        std::fs::read_to_string(workspace.join(".planning/research/STACK.md")).unwrap();
    assert!(stack_content.contains("Rust 2024 edition"));
    assert!(stack_content.contains("Nightly feature requirement"));

    // Verify SUMMARY.md content
    let summary_content =
        std::fs::read_to_string(workspace.join(".planning/research/SUMMARY.md")).unwrap();
    assert!(summary_content.contains("SecureApp"));
    assert!(summary_content.contains("Executive Summary"));
}

#[test]
fn test_evidence_survives_handoff_and_provenance_preserved() {
    let temp = tempdir().unwrap();
    let workspace = temp.path();

    let mut finding = ResearchFinding::new(
        ResearchDimension::architecture(),
        "Architecture Findings",
        "Layered L0-L9 architecture verified.",
    );
    finding.risks.push("Cyclic dependencies".into());
    finding
        .tradeoffs
        .push("Monolith crate vs multi-crate".into());
    finding
        .recommendations
        .push("Enforce downward dependency rule".into());

    let findings = vec![finding];
    let charter = ProjectCharter::new("M31A Core", "Autonomous Agent");
    let summary = ResearchSynthesizer::synthesize(&charter, &findings).unwrap();

    ResearchOrchestrator::materialize_research_artifacts(
        workspace,
        ".planning",
        &findings,
        Some(&summary),
    )
    .unwrap();

    // Verify collect_findings preserves the evidence back from the materialized artifacts
    let collected = ResearchOrchestrator::collect_findings(
        workspace,
        ".planning",
        &[ResearchDimension::architecture()],
    )
    .unwrap();

    assert_eq!(collected.len(), 1);
    assert_eq!(collected[0].dimension, ResearchDimension::architecture());
    assert!(collected[0].summary.contains("Layered L0-L9 architecture"));
    assert_eq!(collected[0].risks, vec!["Cyclic dependencies"]);
    assert_eq!(
        collected[0].tradeoffs,
        vec!["Monolith crate vs multi-crate"]
    );
    assert_eq!(
        collected[0].recommendations,
        vec!["Enforce downward dependency rule"]
    );
}

#[test]
fn test_custom_research_dimensions_not_silently_dropped() {
    let temp = tempdir().unwrap();
    let workspace = temp.path();

    // Register a custom research dimension
    let custom_dim_id = "compliance_phase29_6";
    let custom_dim = ResearchDimension::new(custom_dim_id);
    let custom_def = ResearchDimensionDefinition {
        id: custom_dim.clone(),
        display_name: "Compliance".to_string(),
        description: "Regulatory and compliance constraints".to_string(),
        prompt_contract: "genesis.compliance".to_string(),
        agent_role: AgentRole::stack_researcher(),
        artifact_filename: "COMPLIANCE.md".to_string(),
        requirement_prefix: "COMP".to_string(),
        requirement_category: RequirementCategory::Security,
        requirement_priority: RequirementPriority::Must,
        rationale: "Compliance requirement".to_string(),
        full_set: false,
        brownfield_targeted: false,
        builtin: false,
    };

    {
        let mut reg = ResearchDimensionRegistry::global()
            .write()
            .expect("lock writable");
        reg.register(custom_def).unwrap();
    }

    let mut finding = ResearchFinding::new(
        custom_dim.clone(),
        "Compliance Findings",
        "GDPR and SOC2 compliance constraints analyzed.",
    );
    finding.risks.push("Data retention limits".into());

    let findings = vec![finding];
    let charter = ProjectCharter::new("CompliantApp", "FinTech Service");
    let summary = ResearchSynthesizer::synthesize(&charter, &findings).unwrap();

    let paths = ResearchOrchestrator::materialize_research_artifacts(
        workspace,
        ".planning",
        &findings,
        Some(&summary),
    )
    .unwrap();

    assert!(
        paths
            .iter()
            .any(|p| p.ends_with(".planning/research/COMPLIANCE.md"))
    );
    assert!(workspace.join(".planning/research/COMPLIANCE.md").exists());

    let collected =
        ResearchOrchestrator::collect_findings(workspace, ".planning", &[custom_dim]).unwrap();
    assert_eq!(collected.len(), 1);
    assert_eq!(
        collected[0].summary,
        "GDPR and SOC2 compliance constraints analyzed."
    );
    assert_eq!(collected[0].risks, vec!["Data retention limits"]);
}

#[test]
fn test_failed_artifact_synthesis_fails_safely_no_fabrication() {
    let temp = tempdir().unwrap();
    let workspace = temp.path();

    // With no files written on disk, collect_findings must return empty findings (no fabrication)
    let collected = ResearchOrchestrator::collect_findings(
        workspace,
        ".planning",
        &[ResearchDimension::stack(), ResearchDimension::security()],
    )
    .unwrap();

    assert!(
        collected.is_empty(),
        "Missing files must not fabricate findings"
    );

    // parse_finding on empty or header-only content returns None
    assert!(
        ResearchOrchestrator::parse_finding(&ResearchDimension::stack(), "stack", "").is_none()
    );
    assert!(
        ResearchOrchestrator::parse_finding(
            &ResearchDimension::stack(),
            "stack",
            "# Header Only\n## Subheader\n"
        )
        .is_none()
    );
}
