//! Golden End-to-End Test Suite for Project Genesis Discovery & Research (Package 4).
//!
//! Verifies complete execution path:
//! Raw concept -> Environment probe -> Socratic interview -> ProjectCharter (.planning/PROJECT.md)
//! -> Research decision (6 dimensions) -> Compiled WorkflowDefinition -> Parallel execution
//! via WorkflowEngine -> Synthesis into .planning/research/SUMMARY.md -> SQLite run completion.

use m31a::agent::model_policy::{ModelCaller, ModelProposal};
use m31a::prompt::InMemoryPromptCatalog;
use m31a::runtime::AppRuntime;
use m31a::verification::gate::EvidenceCompletionGate;
use m31a::verification::hierarchy::VerificationHierarchyEngine;
use m31a::verification::runners::compiler::CompilerRunner;
use m31a::verification::runners::deterministic::DeterministicRunner;
use m31a::verification::runners::diff_invariants::DiffInvariantsRunner;
use m31a::verification::runners::static_analysis::StaticAnalysisRunner;
use m31a::verification::runners::tests::TestRunner;
use m31a::workflow::engine::{WorkflowEngine, WorkflowStartRequest};
use m31a::workflow::genesis::{
    ConvergenceReason, DiscoverySession, GenesisController, GenesisOptions, GenesisRequest,
    ResearchDimension, ResearchOrchestrator, ResearchSynthesizer,
};
use m31a::workflow::repository::{SqliteWorkflowRepository, WorkflowRepository};
use m31a::workflow::state::WorkflowRunState;
use std::fs::{create_dir_all, write};
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;

struct CompletingModelCaller {
    pub summary: String,
}

#[async_trait::async_trait]
impl ModelCaller for CompletingModelCaller {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        Ok(ModelProposal::Complete {
            summary: self.summary.clone(),
            artifacts: vec![],
        })
    }
}

#[tokio::test]
async fn test_golden_greenfield_genesis_discovery_and_research() {
    // 1. Setup workspace & isolated directory
    let dir = tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    create_dir_all(&workspace).unwrap();

    // 2. Intake
    let prompt = "Build a self-hosted Mercurial code hosting platform in Rust with SSH and HTTP wire protocols";
    let req = GenesisRequest::new(prompt, &workspace);
    let options = GenesisOptions::default();

    // 3. Deterministic environment probe
    let env = GenesisController::probe(&workspace).unwrap();
    assert!(env.is_greenfield());
    assert_eq!(env.file_count, 0);

    // 4. Socratic discovery interview
    let mut session = DiscoverySession::new(req, env.clone());

    // Turn 1: Problem & Personas
    let turn1 = session.next_turn().unwrap().unwrap();
    assert_eq!(turn1.turn_number, 1);
    session
        .submit_response("Teams self-hosting Mercurial repositories with auditability requirements")
        .unwrap();

    // Turn 2: Boundaries & Non-Goals
    let turn2 = session.next_turn().unwrap().unwrap();
    assert_eq!(turn2.turn_number, 2);
    session
        .submit_response("Strictly standalone single binary, no external Redis/Postgres in v1")
        .unwrap();

    // Turn 3: Technical Preferences
    let turn3 = session.next_turn().unwrap().unwrap();
    assert_eq!(turn3.turn_number, 3);
    session
        .submit_response("Rust 2024 edition, Axum for HTTP, russh for SSH, SQLite storage")
        .unwrap();

    // Finalize discovery via operator confirmation
    let converged = session.submit_response("/skip").unwrap();
    assert!(converged);
    assert_eq!(
        session.convergence_reason,
        Some(ConvergenceReason::OperatorOverride)
    );

    // Synthesize and persist PROJECT.md charter
    let charter = GenesisController::run_discovery(&mut session, &workspace, ".planning").unwrap();
    assert!(charter.confirmed_by_user);
    assert!(workspace.join(".planning/PROJECT.md").exists());

    let charter_md = std::fs::read_to_string(workspace.join(".planning/PROJECT.md")).unwrap();
    assert!(charter_md.contains("# Project Charter:"));
    assert!(charter_md.contains("Mercurial"));

    // 5. Research decision gate
    let decision = GenesisController::decide_research(&charter, &env, &options);
    assert!(decision.execute_research);
    assert_eq!(decision.selected_dimensions.len(), 6);
    assert!(
        decision
            .selected_dimensions
            .contains(&ResearchDimension::stack())
    );
    assert!(
        decision
            .selected_dimensions
            .contains(&ResearchDimension::features())
    );
    assert!(
        decision
            .selected_dimensions
            .contains(&ResearchDimension::architecture())
    );
    assert!(
        decision
            .selected_dimensions
            .contains(&ResearchDimension::pitfalls())
    );
    assert!(
        decision
            .selected_dimensions
            .contains(&ResearchDimension::security())
    );
    assert!(
        decision
            .selected_dimensions
            .contains(&ResearchDimension::deployment())
    );

    // 6. Compile research workflow definition
    let compiled =
        ResearchOrchestrator::compile_research_workflow(&charter, &decision, &options).unwrap();
    assert_eq!(compiled.definition.steps.len(), 7); // 6 parallel dims + 1 synthesis

    // 7. Prepare research output artifacts on disk
    let research_dir = workspace.join(".planning/research");
    create_dir_all(&research_dir).unwrap();

    for dim in &decision.selected_dimensions {
        let registry = m31a::workflow::genesis::ResearchDimensionRegistry::global()
            .read()
            .expect("dimension registry readable");
        let def = registry
            .resolve(dim)
            .expect("selected dimension is registered");
        let filename = def.artifact_filename.clone();
        let dim_id = def.id.as_str().to_string();
        drop(registry);
        let artifact_path = PathBuf::from(format!(".planning/research/{}", filename));
        let content = format!(
            "# Research Dimension: {}\n\nDetailed findings and recommendations for {}.",
            dim_id, charter.project_name
        );
        write(workspace.join(&artifact_path), content).unwrap();
    }

    let summary_path = PathBuf::from(".planning/research/SUMMARY.md");
    let summary_obj = ResearchSynthesizer::synthesize(&charter, &[]).unwrap();
    write(workspace.join(&summary_path), summary_obj.to_markdown()).unwrap();

    // 8. Execute research workflow via canonical WorkflowEngine
    let model = Arc::new(CompletingModelCaller {
        summary: "Genesis research step completed".to_string(),
    });
    let runtime = AppRuntime::new(&workspace)
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model.clone());

    let catalog = Arc::new(InMemoryPromptCatalog::with_builtins());
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
            workspace.clone(),
        )
        .with_hierarchy_engine(hierarchy),
    );

    let repo = Arc::new(SqliteWorkflowRepository::new(runtime.pool().clone()));
    let engine = WorkflowEngine::new(repo.clone(), catalog, Some(runtime.event_bus().clone()))
        .with_dependencies(deps);
    let start_req = WorkflowStartRequest::new(&workspace)
        .with_parameter("project_charter", charter.to_markdown());
    let handle = engine.start_workflow(&compiled, start_req).await.unwrap();
    assert_eq!(handle.status, WorkflowRunState::Completed);

    let run = repo.get_run(handle.run_id).await.unwrap().unwrap();
    assert_eq!(run.status, WorkflowRunState::Completed);

    // 9. Verify durable artifacts & assertions
    assert!(workspace.join(".planning/PROJECT.md").exists());
    assert!(workspace.join(".planning/research/STACK.md").exists());
    assert!(workspace.join(".planning/research/FEATURES.md").exists());
    assert!(
        workspace
            .join(".planning/research/ARCHITECTURE.md")
            .exists()
    );
    assert!(workspace.join(".planning/research/PITFALLS.md").exists());
    assert!(workspace.join(".planning/research/SECURITY.md").exists());
    assert!(workspace.join(".planning/research/DEPLOYMENT.md").exists());
    assert!(workspace.join(".planning/research/SUMMARY.md").exists());

    let summary_content =
        std::fs::read_to_string(workspace.join(".planning/research/SUMMARY.md")).unwrap();
    assert!(summary_content.contains("# Research Summary:"));
    assert!(summary_content.contains("## Executive Summary"));
    assert!(summary_content.contains("## Consensus Recommendations"));
    assert!(summary_content.contains("## Tradeoff Analysis"));
}
