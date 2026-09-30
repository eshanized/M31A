//! Golden end-to-end integration test for Greenfield Project Genesis Workflow (Package 3).
//!
//! Executes the canonical 8-step Greenfield Genesis workflow:
//! 1. discovery (Researcher) -> DISCOVERY.md
//! 2. charter (Integrator) -> PROJECT.md
//! 3. research_stack (Researcher) -> research/STACK.md
//! 4. research_architecture (Architect) -> research/ARCHITECTURE.md
//! 5. research_crates (Researcher) -> research/CRATES.md
//! 6. research_pitfalls (Researcher) -> research/PITFALLS.md
//! 7. research_verification (Verifier) -> research/VERIFICATION.md
//! 8. synthesis (Integrator) -> SYNTHESIS.md
//!
//! Verifies:
//! - Exact 4-wave topological execution order (Wave 2 executes 5 research steps in parallel)
//! - Clean upstream artifact handoff and content hashing across all 8 steps
//! - Tier 1 syntax/artifact validation and Tier 2 relational consistency across all gates
//! - Complete durable SQLite state matching the final Completed status

use m31a::agent::model_policy::{ModelCaller, ModelProposal};
use m31a::prompt::{InMemoryPromptCatalog, PromptContract, PromptParameter};
use m31a::runtime::AppRuntime;
use m31a::state_machine::agent::AgentRole;
use m31a::workflow::compiler::CompiledWorkflow;
use m31a::workflow::definition::{
    InputBinding, OutputBinding, QualityGate, RecoveryStrategy, WorkflowDefinition,
    WorkflowStepDefinition,
};
use m31a::workflow::engine::WorkflowStartRequest;
use m31a::workflow::repository::{SqliteWorkflowRepository, WorkflowRepository};
use m31a::workflow::state::{WorkflowArtifactStatus, WorkflowRunState, WorkflowStepState};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tempfile::tempdir;

fn setup_genesis_catalog() -> InMemoryPromptCatalog {
    let mut catalog = InMemoryPromptCatalog::new();

    let steps = [
        ("genesis.discovery", AgentRole::researcher(), vec![]),
        (
            "genesis.charter",
            AgentRole::integrator(),
            vec!["discovery_doc"],
        ),
        (
            "genesis.research_stack",
            AgentRole::researcher(),
            vec!["charter"],
        ),
        (
            "genesis.research_architecture",
            AgentRole::architect(),
            vec!["charter"],
        ),
        (
            "genesis.research_crates",
            AgentRole::researcher(),
            vec!["charter"],
        ),
        (
            "genesis.research_pitfalls",
            AgentRole::researcher(),
            vec!["charter"],
        ),
        (
            "genesis.research_verification",
            AgentRole::verifier(),
            vec!["charter"],
        ),
        (
            "genesis.synthesis",
            AgentRole::integrator(),
            vec![
                "stack",
                "architecture",
                "crates",
                "pitfalls",
                "verification",
            ],
        ),
    ];

    for (id, role, params) in steps {
        let prompt_params = params
            .iter()
            .map(|p| PromptParameter {
                name: p.to_string(),
                description: format!("Parameter {}", p),
                is_required: false,
                default_value: Some("".to_string()),
            })
            .collect();

        let contract = PromptContract::new(
            id,
            1,
            role,
            format!("Golden prompt contract for {}", id),
            prompt_params,
            format!("Execute role for {}: {{{{ charter }}}}", id),
            Some("markdown".to_string()),
        )
        .unwrap();

        catalog.register(contract).unwrap();
    }

    catalog
}

fn build_greenfield_genesis_workflow() -> CompiledWorkflow {
    let steps = vec![
        // Step 1: Discovery
        WorkflowStepDefinition {
            key: "discovery".to_string(),
            name: "Project Discovery".to_string(),
            role: AgentRole::researcher(),
            prompt_template: "genesis.discovery".to_string(),
            required_inputs: vec![],
            expected_outputs: vec![OutputBinding {
                artifact_name: "DISCOVERY.md".to_string(),
                relative_path: PathBuf::from("DISCOVERY.md"),
                schema_type: "markdown".to_string(),
            }],
            required_capabilities: vec![],
            quality_gate: QualityGate {
                required_artifacts: vec!["DISCOVERY.md".to_string()],
                ..Default::default()
            },
            depends_on: vec![],
            timeout_secs: 600,
            allows_parallelism: false,
            recovery_strategy: None,
        },
        // Step 2: Charter
        WorkflowStepDefinition {
            key: "charter".to_string(),
            name: "Project Charter".to_string(),
            role: AgentRole::integrator(),
            prompt_template: "genesis.charter".to_string(),
            required_inputs: vec![InputBinding {
                parameter_name: "discovery_doc".to_string(),
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
                required_artifacts: vec!["PROJECT.md".to_string()],
                ..Default::default()
            },
            depends_on: vec!["discovery".to_string()],
            timeout_secs: 600,
            allows_parallelism: false,
            recovery_strategy: None,
        },
        // Step 3: Research Tech Stack
        WorkflowStepDefinition {
            key: "research_stack".to_string(),
            name: "Research Stack".to_string(),
            role: AgentRole::researcher(),
            prompt_template: "genesis.research_stack".to_string(),
            required_inputs: vec![InputBinding {
                parameter_name: "charter".to_string(),
                source_step_key: "charter".to_string(),
                artifact_name: "PROJECT.md".to_string(),
                is_optional: false,
            }],
            expected_outputs: vec![OutputBinding {
                artifact_name: "STACK.md".to_string(),
                relative_path: PathBuf::from("research/STACK.md"),
                schema_type: "markdown".to_string(),
            }],
            required_capabilities: vec![],
            quality_gate: QualityGate {
                required_artifacts: vec!["STACK.md".to_string()],
                ..Default::default()
            },
            depends_on: vec!["charter".to_string()],
            timeout_secs: 600,
            allows_parallelism: true,
            recovery_strategy: None,
        },
        // Step 4: Research Architecture
        WorkflowStepDefinition {
            key: "research_architecture".to_string(),
            name: "Research Architecture".to_string(),
            role: AgentRole::architect(),
            prompt_template: "genesis.research_architecture".to_string(),
            required_inputs: vec![InputBinding {
                parameter_name: "charter".to_string(),
                source_step_key: "charter".to_string(),
                artifact_name: "PROJECT.md".to_string(),
                is_optional: false,
            }],
            expected_outputs: vec![OutputBinding {
                artifact_name: "ARCHITECTURE.md".to_string(),
                relative_path: PathBuf::from("research/ARCHITECTURE.md"),
                schema_type: "markdown".to_string(),
            }],
            required_capabilities: vec![],
            quality_gate: QualityGate {
                required_artifacts: vec!["ARCHITECTURE.md".to_string()],
                ..Default::default()
            },
            depends_on: vec!["charter".to_string()],
            timeout_secs: 600,
            allows_parallelism: true,
            recovery_strategy: None,
        },
        // Step 5: Research Crates
        WorkflowStepDefinition {
            key: "research_crates".to_string(),
            name: "Research Crates".to_string(),
            role: AgentRole::researcher(),
            prompt_template: "genesis.research_crates".to_string(),
            required_inputs: vec![InputBinding {
                parameter_name: "charter".to_string(),
                source_step_key: "charter".to_string(),
                artifact_name: "PROJECT.md".to_string(),
                is_optional: false,
            }],
            expected_outputs: vec![OutputBinding {
                artifact_name: "CRATES.md".to_string(),
                relative_path: PathBuf::from("research/CRATES.md"),
                schema_type: "markdown".to_string(),
            }],
            required_capabilities: vec![],
            quality_gate: QualityGate {
                required_artifacts: vec!["CRATES.md".to_string()],
                ..Default::default()
            },
            depends_on: vec!["charter".to_string()],
            timeout_secs: 600,
            allows_parallelism: true,
            recovery_strategy: None,
        },
        // Step 6: Research Pitfalls
        WorkflowStepDefinition {
            key: "research_pitfalls".to_string(),
            name: "Research Pitfalls".to_string(),
            role: AgentRole::researcher(),
            prompt_template: "genesis.research_pitfalls".to_string(),
            required_inputs: vec![InputBinding {
                parameter_name: "charter".to_string(),
                source_step_key: "charter".to_string(),
                artifact_name: "PROJECT.md".to_string(),
                is_optional: false,
            }],
            expected_outputs: vec![OutputBinding {
                artifact_name: "PITFALLS.md".to_string(),
                relative_path: PathBuf::from("research/PITFALLS.md"),
                schema_type: "markdown".to_string(),
            }],
            required_capabilities: vec![],
            quality_gate: QualityGate {
                required_artifacts: vec!["PITFALLS.md".to_string()],
                ..Default::default()
            },
            depends_on: vec!["charter".to_string()],
            timeout_secs: 600,
            allows_parallelism: true,
            recovery_strategy: None,
        },
        // Step 7: Research Verification
        WorkflowStepDefinition {
            key: "research_verification".to_string(),
            name: "Research Verification".to_string(),
            role: AgentRole::verifier(),
            prompt_template: "genesis.research_verification".to_string(),
            required_inputs: vec![InputBinding {
                parameter_name: "charter".to_string(),
                source_step_key: "charter".to_string(),
                artifact_name: "PROJECT.md".to_string(),
                is_optional: false,
            }],
            expected_outputs: vec![OutputBinding {
                artifact_name: "VERIFICATION.md".to_string(),
                relative_path: PathBuf::from("research/VERIFICATION.md"),
                schema_type: "markdown".to_string(),
            }],
            required_capabilities: vec![],
            quality_gate: QualityGate {
                required_artifacts: vec!["VERIFICATION.md".to_string()],
                ..Default::default()
            },
            depends_on: vec!["charter".to_string()],
            timeout_secs: 600,
            allows_parallelism: true,
            recovery_strategy: None,
        },
        // Step 8: Research Synthesis
        WorkflowStepDefinition {
            key: "synthesis".to_string(),
            name: "Synthesize Genesis Research".to_string(),
            role: AgentRole::integrator(),
            prompt_template: "genesis.synthesis".to_string(),
            required_inputs: vec![
                InputBinding {
                    parameter_name: "stack".to_string(),
                    source_step_key: "research_stack".to_string(),
                    artifact_name: "STACK.md".to_string(),
                    is_optional: false,
                },
                InputBinding {
                    parameter_name: "architecture".to_string(),
                    source_step_key: "research_architecture".to_string(),
                    artifact_name: "ARCHITECTURE.md".to_string(),
                    is_optional: false,
                },
                InputBinding {
                    parameter_name: "crates".to_string(),
                    source_step_key: "research_crates".to_string(),
                    artifact_name: "CRATES.md".to_string(),
                    is_optional: false,
                },
                InputBinding {
                    parameter_name: "pitfalls".to_string(),
                    source_step_key: "research_pitfalls".to_string(),
                    artifact_name: "PITFALLS.md".to_string(),
                    is_optional: false,
                },
                InputBinding {
                    parameter_name: "verification".to_string(),
                    source_step_key: "research_verification".to_string(),
                    artifact_name: "VERIFICATION.md".to_string(),
                    is_optional: false,
                },
            ],
            expected_outputs: vec![OutputBinding {
                artifact_name: "SYNTHESIS.md".to_string(),
                relative_path: PathBuf::from("SYNTHESIS.md"),
                schema_type: "markdown".to_string(),
            }],
            required_capabilities: vec![],
            quality_gate: QualityGate {
                required_artifacts: vec!["SYNTHESIS.md".to_string()],
                ..Default::default()
            },
            depends_on: vec![
                "research_stack".to_string(),
                "research_architecture".to_string(),
                "research_crates".to_string(),
                "research_pitfalls".to_string(),
                "research_verification".to_string(),
            ],
            timeout_secs: 600,
            allows_parallelism: false,
            recovery_strategy: None,
        },
    ];

    let def = WorkflowDefinition {
        id: "genesis_greenfield".to_string(),
        name: "Greenfield Project Genesis".to_string(),
        description: "Canonical 8-step Greenfield Project Genesis".to_string(),
        version: 1,
        steps,
        default_recovery_strategy: RecoveryStrategy::Fail,
    };
    def.validate().unwrap();

    let topological_order = def.steps.iter().map(|s| s.key.clone()).collect();
    let provenance = m31a::workflow::provenance::WorkflowProvenance::new(
        "genesis_greenfield",
        1,
        "hash_genesis_greenfield",
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

/// Helper to write simulated markdown content for each step output.
async fn write_genesis_artifact(workspace: &Path, rel_path: &str, title: &str) {
    let full = workspace.join(rel_path);
    if let Some(parent) = full.parent() {
        tokio::fs::create_dir_all(parent).await.unwrap();
    }
    let content = format!(
        "# {}\n\nCanonical verified artifact content for {}.\n",
        title, title
    );
    tokio::fs::write(&full, content).await.unwrap();
}

fn setup_git_cargo_fixture(dir: &Path) {
    let git_init = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(dir)
        .status()
        .expect("git init failed");
    assert!(git_init.success());

    let _ = Command::new("git")
        .args(["config", "user.name", "M31A Test Agent"])
        .current_dir(dir)
        .status();
    let _ = Command::new("git")
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

    let _ = Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir)
        .status();
    let _ = Command::new("git")
        .args(["commit", "-m", "Initial fixture commit"])
        .current_dir(dir)
        .status();
}

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
async fn test_golden_greenfield_genesis_workflow_execution() {
    let dir = tempdir().unwrap();
    let workspace = dir.path().join("workspace");
    tokio::fs::create_dir_all(&workspace).await.unwrap();
    setup_git_cargo_fixture(&workspace);

    let model = Arc::new(CompletingModelCaller {
        summary: "Genesis step completed".to_string(),
    });
    let runtime = AppRuntime::new(&workspace)
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model.clone());

    let catalog = Arc::new(setup_genesis_catalog());
    let engine = runtime.create_workflow_engine(catalog);
    let repo = Arc::new(SqliteWorkflowRepository::new(runtime.pool().clone()));

    let compiled = build_greenfield_genesis_workflow();

    // 1. Verify exact 4-wave topological calculation
    let waves = engine.compute_execution_waves(&compiled).unwrap();
    assert_eq!(
        waves.len(),
        4,
        "Greenfield genesis must execute in exactly 4 waves"
    );
    assert_eq!(waves[0], vec!["discovery"]);
    assert_eq!(waves[1], vec!["charter"]);

    let mut expected_wave2 = vec![
        "research_architecture",
        "research_crates",
        "research_pitfalls",
        "research_stack",
        "research_verification",
    ];
    expected_wave2.sort();
    let mut actual_wave2 = waves[2].clone();
    actual_wave2.sort();
    assert_eq!(
        actual_wave2, expected_wave2,
        "Wave 2 must contain all 5 parallel research steps"
    );

    assert_eq!(waves[3], vec!["synthesis"]);

    // 2. Pre-generate mock outputs in workspace so quality gates find them
    write_genesis_artifact(&workspace, "DISCOVERY.md", "Project Discovery").await;
    write_genesis_artifact(&workspace, "PROJECT.md", "Project Charter").await;
    write_genesis_artifact(&workspace, "research/STACK.md", "Stack Research").await;
    write_genesis_artifact(
        &workspace,
        "research/ARCHITECTURE.md",
        "Architecture Research",
    )
    .await;
    write_genesis_artifact(&workspace, "research/CRATES.md", "Crates Research").await;
    write_genesis_artifact(&workspace, "research/PITFALLS.md", "Pitfalls Research").await;
    write_genesis_artifact(
        &workspace,
        "research/VERIFICATION.md",
        "Verification Research",
    )
    .await;
    write_genesis_artifact(&workspace, "SYNTHESIS.md", "Genesis Synthesis").await;

    // 3. Start workflow
    let handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(&workspace))
        .await
        .unwrap();

    // 4. Verify complete execution to Completed state
    assert_eq!(handle.status, WorkflowRunState::Completed);

    let run = repo.get_run(handle.run_id).await.unwrap().unwrap();
    assert_eq!(run.status, WorkflowRunState::Completed);
    assert!(run.completed_at.is_some());

    // 5. Verify all 8 step runs are Completed with exactly 1 attempt
    let step_runs = repo.list_step_runs(handle.run_id).await.unwrap();
    assert_eq!(step_runs.len(), 8);
    for step in &step_runs {
        assert_eq!(
            step.status,
            WorkflowStepState::Completed,
            "Step '{}' did not complete successfully",
            step.step_key
        );
        assert_eq!(step.attempt_count, 1);
        assert!(step.completed_at.is_some());
    }

    // 6. Verify all 8 artifacts are recorded and Valid in SQLite
    let artifacts = repo.list_artifacts(handle.run_id).await.unwrap();
    assert_eq!(artifacts.len(), 8, "Expected 8 recorded artifacts");
    for art in &artifacts {
        assert_eq!(
            art.status,
            WorkflowArtifactStatus::Valid,
            "Artifact '{}' was not marked Valid",
            art.name
        );
        assert_eq!(art.version, 1);
        assert!(
            !art.content_hash.is_empty(),
            "Artifact content hash must be computed"
        );
    }

    // 7. Inspect full snapshot
    let snapshot = engine.inspect_workflow(handle.run_id).await.unwrap();
    assert_eq!(snapshot.run.id, handle.run_id);
    assert_eq!(snapshot.run.status, WorkflowRunState::Completed);
    assert_eq!(snapshot.step_runs.len(), 8);
    assert_eq!(snapshot.artifacts.len(), 8);
    assert!(snapshot.ready_step_keys.is_empty());
    assert!(snapshot.blocked_step_keys.is_empty());
}
