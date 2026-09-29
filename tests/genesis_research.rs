//! Test suite for Project Genesis research decisions, dimension dispatching, and workflow compilation (Package 4).

use m31a::state_machine::agent::AgentRole;
use m31a::workflow::genesis::{
    GenesisMode, GenesisOptions, ProjectCharter, ResearchDecision, ResearchDimension,
    ResearchOrchestrator, WorkflowTier, WorkspaceEnvironment, evaluate_research_decision,
};
use tempfile::tempdir;

#[test]
fn test_evaluate_research_decision_disabled_in_config() {
    let dir = tempdir().unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    let charter = ProjectCharter::new("Test App", "Overview of test app");
    let options = GenesisOptions {
        enable_research: false,
        ..Default::default()
    };

    let decision = evaluate_research_decision(&charter, &env, &options);

    assert!(!decision.execute_research);
    assert!(decision.selected_dimensions.is_empty());
    assert!(
        decision
            .skip_reason
            .as_deref()
            .unwrap_or("")
            .contains("disabled in configuration")
    );
}

#[test]
fn test_evaluate_research_decision_greenfield_triggers_all_six_dimensions() {
    let dir = tempdir().unwrap();
    let env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    assert!(env.is_greenfield());

    let charter = ProjectCharter::new("New System", "Build a high-performance database");
    let options = GenesisOptions::default();

    let decision = evaluate_research_decision(&charter, &env, &options);

    assert!(decision.execute_research);
    assert_eq!(decision.selected_dimensions.len(), 6);
    assert_eq!(
        decision.selected_dimensions,
        vec![
            ResearchDimension::stack(),
            ResearchDimension::features(),
            ResearchDimension::architecture(),
            ResearchDimension::pitfalls(),
            ResearchDimension::security(),
            ResearchDimension::deployment(),
        ]
    );
    assert!(
        decision
            .trigger_reason
            .as_deref()
            .unwrap_or("")
            .contains("Greenfield")
    );
}

#[test]
fn test_evaluate_research_decision_brownfield_bug_fix_skips() {
    let dir = tempdir().unwrap();
    // Simulate brownfield environment with multiple files
    let mut env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    env.detected_mode = GenesisMode::Brownfield;
    env.file_count = 25;

    let mut charter = ProjectCharter::new("Bug Fix", "Fix typo and null check in parser");
    charter.workflow_tier = WorkflowTier::Tiny;
    let options = GenesisOptions::default();

    let decision = evaluate_research_decision(&charter, &env, &options);

    assert!(!decision.execute_research);
    assert!(
        decision
            .skip_reason
            .as_deref()
            .unwrap_or("")
            .contains("Tiny/Medium")
    );
}

#[test]
fn test_evaluate_research_decision_brownfield_subsystem_triggers_targeted_dimensions() {
    let dir = tempdir().unwrap();
    let mut env = WorkspaceEnvironment::probe(dir.path()).unwrap();
    env.detected_mode = GenesisMode::Brownfield;
    env.file_count = 50;

    let charter = ProjectCharter::new(
        "Auth Upgrade",
        "Add passkeys WebAuthn authentication subsystem to existing platform",
    );
    let options = GenesisOptions::default();

    let decision = evaluate_research_decision(&charter, &env, &options);

    assert!(decision.execute_research);
    assert_eq!(decision.selected_dimensions.len(), 4);
    assert_eq!(
        decision.selected_dimensions,
        vec![
            ResearchDimension::stack(),
            ResearchDimension::architecture(),
            ResearchDimension::pitfalls(),
            ResearchDimension::security(),
        ]
    );
}

#[test]
fn test_build_workflow_definition_produces_valid_dag_with_parallel_dimensions() {
    let charter = ProjectCharter::new("Mercurial Server", "Self-hosted Mercurial hosting platform");
    let decision = ResearchDecision::execute(
        m31a::workflow::genesis::ResearchDimensionRegistry::global()
            .read()
            .expect("dimension registry readable")
            .full_set(),
        "Greenfield full research",
    );
    let options = GenesisOptions::default();

    let workflow_def =
        ResearchOrchestrator::build_workflow_definition(&charter, &decision, &options).unwrap();

    // 6 dimensions + 1 synthesis = 7 steps
    assert_eq!(workflow_def.steps.len(), 7);

    // Verify 6 dimension steps
    let dim_steps = &workflow_def.steps[0..6];
    for step in dim_steps {
        assert!(step.allows_parallelism);
        assert!(step.depends_on.is_empty());
        assert!(!step.expected_outputs.is_empty());
        assert!(step.required_inputs.is_empty());
    }

    // Verify roles
    assert_eq!(dim_steps[0].role, AgentRole::stack_researcher());
    assert_eq!(dim_steps[1].role, AgentRole::features_researcher());
    assert_eq!(dim_steps[2].role, AgentRole::architecture_researcher());
    assert_eq!(dim_steps[3].role, AgentRole::pitfalls_researcher());
    assert_eq!(dim_steps[4].role, AgentRole::security_researcher());
    assert_eq!(dim_steps[5].role, AgentRole::deployment_researcher());

    // Verify synthesis step
    let synthesis_step = &workflow_def.steps[6];
    assert_eq!(synthesis_step.key, "genesis_research_synthesis");
    assert_eq!(synthesis_step.role, AgentRole::synthesizer());
    assert!(!synthesis_step.allows_parallelism);
    assert_eq!(synthesis_step.depends_on.len(), 6);
    assert_eq!(synthesis_step.required_inputs.len(), 6);
    assert_eq!(synthesis_step.expected_outputs.len(), 1);
    assert_eq!(
        synthesis_step.expected_outputs[0].artifact_name,
        "SUMMARY.md"
    );

    // Semantic validation must succeed
    assert!(workflow_def.validate().is_ok());
}

#[test]
fn test_compile_research_workflow_produces_executable_plan() {
    let charter = ProjectCharter::new("Mercurial Server", "Self-hosted Mercurial hosting platform");
    let decision = ResearchDecision::execute(
        vec![ResearchDimension::stack(), ResearchDimension::security()],
        "Targeted research",
    );
    let options = GenesisOptions::default();

    let compiled =
        ResearchOrchestrator::compile_research_workflow(&charter, &decision, &options).unwrap();

    assert_eq!(compiled.definition.steps.len(), 3); // 2 dims + 1 synthesis
    assert_eq!(compiled.topological_order.len(), 3);
    assert_eq!(compiled.topological_order[2], "genesis_research_synthesis");
}
