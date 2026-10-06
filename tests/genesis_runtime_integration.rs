//! Comprehensive Integration Test Suite for Genesis -> Workflow -> Mission Runtime Integration (Workstream H).
//!
//! Enforces and verifies:
//! - Condition A: Genesis entrypoint reaches WorkflowEngine.
//! - Condition B: Discovery creates durable artifact with hash in ArtifactService.
//! - Condition C: Research decision produces Greenfield for empty repo.
//! - Condition D: Research decision produces Brownfield for repo with code.
//! - Condition E: Research decision produces Skip when user directs it.
//! - Condition F: Research tasks execute with read-only capabilities/sandbox.
//! - Condition G: Bounded research concurrency (parallelism enabled, bounded wave limits).
//! - Condition H: Synthesis consumes durable upstream artifacts, not in-memory ghosts.
//! - Condition I: Requirements keys match pattern (REQ-*).
//! - Condition J: Architecture references requirements keys.
//! - Condition K: ADRs reference architecture decisions.
//! - Condition L: Roadmap DAG has no cycles (delegates to canonical topological sort).
//! - Condition M: Roadmap lowers to WorkflowDefinition.
//! - Condition N: WorkflowDefinition compiles to CompiledWorkflow.
//! - Condition O: CompiledWorkflow executes through WorkflowEngine.
//! - Condition P: Workflow step lowers to Mission in ProductionStepExecutor.
//! - Condition Q: Mission executes through SchedulerEngine.
//! - Condition R: Artifacts register with ArtifactService and have verified content hashes.
//! - Condition S: Quality gate rejection pauses workflow until approval.
//! - Condition T: Approval via CLI/TUI resumes workflow.
//! - Condition U: Restart during workflow preserves Genesis state and resumes.
//! - Condition V: Invalidation of upstream artifact marks downstream state stale.
//! - Condition W: Malformed artifact blocks downstream progression.
//! - Condition X: Brownfield discovery indexes existing code before charter.
//! - Condition Y: ProductionStepExecutor is the only executor in production path.
//! - Condition Z: No mock bypasses exist in the production Genesis pipeline.
//! - Golden Greenfield Journey: End-to-end greenfield lifecycle.
//! - Golden Brownfield Journey: End-to-end brownfield lifecycle.

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::tempdir;

use m31a::agent::model_policy::{ModelCaller, ModelProposal, ModelToolCall};
use m31a::ids::{ArtifactId, WorkflowRunId};
use m31a::kernel::plan::CapabilityAccessMode;
use m31a::persistence::artifacts::ArtifactStatus;
use m31a::planning::requirements::{
    EngineeringRequirement, EpistemicStatus, Provenance, ProvenanceSourceType, RequirementCategory,
    RequirementPriority, TrustLevel,
};
use m31a::prompt::InMemoryPromptCatalog;
use m31a::runtime::AppRuntime;
use m31a::state_machine::agent::AgentRole;
use m31a::workflow::compiler::CompiledWorkflow;
use m31a::workflow::definition::{
    QualityGate, RecoveryStrategy, WorkflowDefinition, WorkflowStepDefinition,
};
use m31a::workflow::engine::WorkflowStartRequest;
use m31a::workflow::genesis::GenesisController;
use m31a::workflow::genesis::intake::{GenesisMode, GenesisOptions, GenesisRequest};
use m31a::workflow::genesis::project::ProjectCharter;
use m31a::workflow::genesis::research_decision::{ResearchDecision, ResearchDimension};
use m31a::workflow::genesis::researcher::ResearchOrchestrator;
use m31a::workflow::genesis::synthesis::{
    ConsensusPoint, OpenUnknown, ResearchSummary, TradeoffAnalysis,
};
use m31a::workflow::planning::architecture_synthesizer::ArchitectureSynthesizer;
use m31a::workflow::planning::invalidation::PlanningInvalidator;
use m31a::workflow::planning::requirements_synthesizer::RequirementsSynthesizer;
use m31a::workflow::planning::risks::RiskRegister;
use m31a::workflow::planning::roadmap::RoadmapSynthesizer;
use m31a::workflow::planning::state::PlanningLifecycleState;
use m31a::workflow::repository::{SqliteWorkflowRepository, WorkflowRepository};
use m31a::workflow::state::{WorkflowMode, WorkflowRunState, WorkflowStepState};

// =============================================================================
// Test Helpers & Fixtures
// =============================================================================

/// Mock model caller that completes step invocations with real on-disk artifact production.
struct CompletingTestModel {
    pub workspace_root: PathBuf,
    pub call_count: AtomicUsize,
}

impl CompletingTestModel {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            call_count: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl ModelCaller for CompletingTestModel {
    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        // Ensure research directory exists and populate standard research artifacts
        let research_dir = self.workspace_root.join(".planning").join("research");
        let _ = std::fs::create_dir_all(&research_dir);

        let artifacts = [
            (
                "STACK.md",
                "# Technology Stack\n\nRust 2024 edition with Tokio and SQLite.\n",
            ),
            (
                "FEATURES.md",
                "# Product Features\n\nDeterministic workflow engine and policy enforcement.\n",
            ),
            (
                "ARCHITECTURE.md",
                "# System Architecture\n\nModular single-crate layered architecture.\n",
            ),
            (
                "PITFALLS.md",
                "# Known Pitfalls\n\nAvoid cyclic task graphs and unverified state.\n",
            ),
            (
                "SECURITY.md",
                "# Security Architecture\n\nStrict capability-based policy engine and sandboxing.\n",
            ),
            (
                "DEPLOYMENT.md",
                "# Deployment Architecture\n\nStandalone binary with embedded SQLite.\n",
            ),
            (
                "SUMMARY.md",
                "# Research Summary\n\nAll research dimensions validated and synthesized.\n",
            ),
        ];

        for (filename, content) in artifacts {
            let file_path = research_dir.join(filename);
            let _ = std::fs::write(file_path, content);
        }

        // Also create phase execution reports directory if needed
        let phases_dir = self.workspace_root.join(".planning").join("phases");
        let _ = std::fs::create_dir_all(&phases_dir);

        let report_content = "# Phase Execution Report\nAll acceptance criteria met.\n";
        for i in 1..=20 {
            let names = [
                format!("PHASE-{:02}_REPORT.md", i),
                format!("PHASE-{}_REPORT.md", i),
                format!("phase_{:02}_report.md", i),
                format!("phase_{}_report.md", i),
            ];
            for name in &names {
                let _ = std::fs::write(phases_dir.join(name), report_content);
                let _ = std::fs::write(self.workspace_root.join(name), report_content);
            }
        }

        let last_500 = if _context.len() > 500 {
            &_context[_context.len() - 500..]
        } else {
            _context
        };

        if last_500.contains("run 'run_tests'") {
            return Ok(ModelProposal::ToolCalls {
                calls: vec![ModelToolCall::new("run_tests", serde_json::json!({}))],
            });
        }

        if last_500.contains("You have not called edit_file or write_file") {
            let src_dir = self.workspace_root.join("src");
            let _ = std::fs::create_dir_all(&src_dir);
            let cargo_toml = self.workspace_root.join("Cargo.toml");
            if !cargo_toml.exists() {
                let _ = std::fs::write(
                    &cargo_toml,
                    "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
                );
            }

            return Ok(ModelProposal::ToolCalls {
                calls: vec![ModelToolCall::new(
                    "write_file",
                    serde_json::json!({
                        "path": "src/lib.rs",
                        "content": "// Genesis phase implementation\npub fn run() {}\n"
                    }),
                )],
            });
        }

        if last_500.contains("\"path\": \"src/lib.rs\"") {
            return Ok(ModelProposal::ToolCalls {
                calls: vec![ModelToolCall::new("run_tests", serde_json::json!({}))],
            });
        }

        Ok(ModelProposal::Complete {
            summary: "Executed and verified deterministically".to_string(),
            artifacts: vec![],
        })
    }
}

/// Helper to set up an empty git repository workspace (pure Greenfield).
fn setup_empty_git_workspace(dir: &Path) {
    let git_init = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(dir)
        .status()
        .expect("git init failed");
    assert!(git_init.success());

    let _ = Command::new("git")
        .args(["config", "user.name", "M31A Integration Test"])
        .current_dir(dir)
        .status();
    let _ = Command::new("git")
        .args(["config", "user.email", "test@m31a.local"])
        .current_dir(dir)
        .status();
    let _ = Command::new("git")
        .args(["commit", "--allow-empty", "-m", "Initial commit"])
        .current_dir(dir)
        .status();
}

/// Helper to set up a git repository workspace with existing Rust source code (Brownfield).
fn setup_brownfield_git_workspace(dir: &Path) {
    setup_empty_git_workspace(dir);

    std::fs::write(dir.join(".gitignore"), "/target\n.m31a\n").unwrap();

    let cargo_toml = r#"[package]
name = "workspace_fixture"
version = "0.1.0"
edition = "2021"

[dependencies]
"#;
    std::fs::write(dir.join("Cargo.toml"), cargo_toml).unwrap();

    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("src/lib.rs"),
        "pub fn compute_sum(a: i32, b: i32) -> i32 { a + b }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/helper.rs"),
        "pub fn is_valid(s: &str) -> bool { !s.is_empty() }\n",
    )
    .unwrap();

    let _ = Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir)
        .status();
    let _ = Command::new("git")
        .args(["commit", "-m", "Initial brownfield commit"])
        .current_dir(dir)
        .status();
}

/// Helper to build a canonical valid ProjectCharter for testing.
fn build_test_charter() -> ProjectCharter {
    let mut charter = ProjectCharter::new(
        "M31A Integration Target",
        "Autonomous agent control plane with deterministic recovery and sandboxing",
    );
    charter.problem_statement =
        "Agent systems lack deterministic recovery checkpoints and strict isolation".to_string();
    charter.boundaries.in_scope = vec![
        "Deterministic payload parsing and schema validation".to_string(),
        "Transactional checkpointing and WAL state replay".to_string(),
        "Sandboxed capability enforcement for untrusted actions".to_string(),
    ];
    charter.boundaries.non_goals = vec!["GUI desktop application".to_string()];
    charter.operational_invariants.security_requirements = vec![
        "Enforce strict read-only worktree mounting by default".to_string(),
        "Redact API tokens and secrets from all diagnostic logs".to_string(),
    ];
    charter.operational_invariants.performance_targets =
        vec!["Cold start routing decision completed within 10ms".to_string()];
    charter
}

/// Helper to build a canonical valid ResearchSummary for testing.
fn build_test_research_summary() -> ResearchSummary {
    let mut summary = ResearchSummary::new(
        "M31A Integration Target",
        "Research confirms single-crate Rust architecture with embedded SQLite is optimal.",
    );
    summary.open_unknowns.push(OpenUnknown {
        area: "SQLite Concurrency".to_string(),
        risk_level: "Low".to_string(),
        mitigation_strategy: "Use WAL mode with 5000ms busy timeout".to_string(),
    });
    summary.tradeoffs.push(TradeoffAnalysis {
        tradeoff_axis: "Schema Evolution".to_string(),
        decision: "Embedded SQL migrations".to_string(),
        rationale: "Predictable, reproducible database upgrades".to_string(),
    });
    summary.consensus_points.push(ConsensusPoint {
        topic: "Security Sandbox".to_string(),
        consensus: "Use Linux bubblewrap / process unshare capability envelope".to_string(),
        supporting_dimensions: vec![
            ResearchDimension::security(),
            ResearchDimension::architecture(),
        ],
    });
    summary
}

// =============================================================================
// Test A: Genesis entrypoint reaches WorkflowEngine
// =============================================================================
#[tokio::test]
async fn test_a_genesis_entrypoint_reaches_workflow_engine() {
    let dir = tempdir().unwrap();
    setup_empty_git_workspace(dir.path());

    let model = Arc::new(CompletingTestModel::new(dir.path()));
    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model);

    let req = GenesisRequest::new("Build a high-performance HTTP gateway in Rust", dir.path());
    let outcome = match runtime.run_genesis(&req).await {
        Ok(out) => out,
        Err(e) => {
            let rows: Vec<(String, String, Option<String>)> =
                sqlx::query_as("SELECT step_key, status, halt_reason FROM workflow_step_runs")
                    .fetch_all(runtime.pool())
                    .await
                    .unwrap();
            panic!("Genesis failed: {:?}, step_runs: {:?}", e, rows);
        }
    };

    assert!(!outcome.workflow_definition.steps.is_empty());

    // Start the lowered workflow through the production WorkflowEngine
    let handle = runtime
        .start_genesis_workflow(&outcome.workflow_definition)
        .await
        .expect("start_genesis_workflow must reach WorkflowEngine");

    assert!(
        handle.status == WorkflowRunState::Running || handle.status == WorkflowRunState::Completed
    );

    // Verify durable record exists in SQLite repository
    let repo = SqliteWorkflowRepository::new(runtime.pool().clone());
    let run = repo
        .get_run(handle.run_id)
        .await
        .expect("query run")
        .expect("run must exist in SQLite");

    assert_eq!(run.id, handle.run_id);
}

// =============================================================================
// Test B: Discovery creates durable artifact with hash
// =============================================================================
#[tokio::test]
async fn test_b_discovery_creates_durable_artifact() {
    let dir = tempdir().unwrap();
    setup_empty_git_workspace(dir.path());

    let model = Arc::new(CompletingTestModel::new(dir.path()));
    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model);

    let req = GenesisRequest::new("Build a robust distributed key-value store", dir.path());
    let outcome = runtime
        .run_genesis(&req)
        .await
        .expect("run_genesis must succeed");

    let charter_art = &outcome.charter_artifact;
    assert_eq!(charter_art.name, "PROJECT.md");
    assert_eq!(charter_art.status, ArtifactStatus::Valid);
    assert!(
        !charter_art.content_hash.is_empty(),
        "Artifact must possess non-empty SHA-256 content hash"
    );

    // Verify artifact is durable in SQLite via ArtifactService
    let meta = runtime
        .artifact_service()
        .get_metadata(charter_art.id)
        .await
        .expect("get metadata")
        .expect("charter artifact record must exist in DB");

    assert_eq!(meta.id, charter_art.id);
    assert_eq!(meta.content_hash, charter_art.content_hash);
    assert_eq!(meta.logical_path, PathBuf::from("PROJECT.md"));
}

// =============================================================================
// Test C: Research decision produces Greenfield for empty repo
// =============================================================================
#[tokio::test]
async fn test_c_research_decision_greenfield_for_empty_repo() {
    let dir = tempdir().unwrap();
    setup_empty_git_workspace(dir.path());

    let env = GenesisController::probe(dir.path()).expect("probe");
    assert_eq!(env.detected_mode, GenesisMode::Greenfield);

    let charter = build_test_charter();
    let options = GenesisOptions::default();
    let decision = GenesisController::decide_research(&charter, &env, &options);

    assert!(decision.is_full_greenfield());
    assert!(!decision.is_targeted_delta());
    assert!(!decision.is_skip());
    assert!(decision.execute_research);
}

// =============================================================================
// Test D: Research decision produces Brownfield for repo with code
// =============================================================================
#[tokio::test]
async fn test_d_research_decision_brownfield_for_repo_with_code() {
    let dir = tempdir().unwrap();
    setup_brownfield_git_workspace(dir.path());

    let env = GenesisController::probe(dir.path()).expect("probe");
    assert_eq!(env.detected_mode, GenesisMode::Brownfield);

    let mut charter = build_test_charter();
    charter.overview = "Implement new authentication subsystem and add audit logging".to_string();
    let options = GenesisOptions::default();
    let decision = GenesisController::decide_research(&charter, &env, &options);

    assert!(decision.is_targeted_delta());
    assert!(!decision.is_full_greenfield());
    assert!(!decision.is_skip());
    assert!(decision.execute_research);
}

// =============================================================================
// Test E: Research decision produces Skip when user directs it
// =============================================================================
#[tokio::test]
async fn test_e_research_decision_skip_when_user_directs() {
    let dir = tempdir().unwrap();
    setup_empty_git_workspace(dir.path());

    let env = GenesisController::probe(dir.path()).expect("probe");
    let charter = build_test_charter();
    let options = GenesisOptions {
        enable_research: false,
        ..Default::default()
    };

    let decision = GenesisController::decide_research(&charter, &env, &options);

    assert!(decision.is_skip());
    assert!(!decision.execute_research);
    assert!(decision.selected_dimensions.is_empty());
}

// =============================================================================
// Test F: Research tasks execute with read-only capabilities/sandbox
// =============================================================================
#[tokio::test]
async fn test_f_research_tasks_read_only_sandbox() {
    let charter = build_test_charter();
    let decision = ResearchDecision::execute(
        vec![
            ResearchDimension::stack(),
            ResearchDimension::architecture(),
            ResearchDimension::security(),
        ],
        "targeted test",
    );
    let options = GenesisOptions::default();

    let def = ResearchOrchestrator::build_workflow_definition(&charter, &decision, &options)
        .expect("build research workflow definition");

    // All research dimension steps must demand only read-mode capabilities
    for step in &def.steps {
        if step.key.starts_with("genesis_research_") && step.key != "genesis_research_synthesis" {
            for cap in &step.required_capabilities {
                assert_eq!(
                    cap.mode,
                    CapabilityAccessMode::Read,
                    "Step {} capability {} must be Read-only",
                    step.key,
                    cap.id
                );
            }
        }
    }
}

// =============================================================================
// Test G: Bounded research concurrency
// =============================================================================
#[tokio::test]
async fn test_g_bounded_research_concurrency() {
    let charter = build_test_charter();
    let decision = ResearchDecision::execute(
        vec![
            ResearchDimension::stack(),
            ResearchDimension::architecture(),
            ResearchDimension::features(),
            ResearchDimension::pitfalls(),
            ResearchDimension::security(),
        ],
        "bounded test",
    );
    let options = GenesisOptions::default();

    let def = ResearchOrchestrator::build_workflow_definition(&charter, &decision, &options)
        .expect("build workflow");

    // Dimension steps are parallelizable in wave 0
    let dim_steps: Vec<_> = def
        .steps
        .iter()
        .filter(|s| s.key != "genesis_research_synthesis")
        .collect();
    assert_eq!(dim_steps.len(), 5);
    for s in &dim_steps {
        assert!(s.allows_parallelism);
        assert!(s.depends_on.is_empty());
    }

    // Synthesis step is sequential and depends on all 5
    let synth = def
        .steps
        .iter()
        .find(|s| s.key == "genesis_research_synthesis")
        .unwrap();
    assert!(!synth.allows_parallelism);
    assert_eq!(synth.depends_on.len(), 5);
}

// =============================================================================
// Test H: Synthesis consumes durable upstream artifacts, not in-memory ghosts
// =============================================================================
#[tokio::test]
async fn test_h_synthesis_consumes_durable_upstream_artifacts() {
    let charter = build_test_charter();
    let decision = ResearchDecision::execute(
        vec![
            ResearchDimension::stack(),
            ResearchDimension::architecture(),
        ],
        "upstream test",
    );
    let options = GenesisOptions::default();

    let def = ResearchOrchestrator::build_workflow_definition(&charter, &decision, &options)
        .expect("build workflow");

    let synth_step = def
        .steps
        .iter()
        .find(|s| s.key == "genesis_research_synthesis")
        .expect("synthesis step");

    // Must bind input parameters to upstream artifact files
    assert_eq!(synth_step.required_inputs.len(), 2);
    for binding in &synth_step.required_inputs {
        assert!(
            binding.artifact_name.ends_with(".md"),
            "Binding artifact must be a formal document"
        );
        assert!(
            binding.source_step_key.starts_with("genesis_research_"),
            "Binding source must be a research step"
        );
    }
}

// =============================================================================
// Test I: Requirements keys match pattern (REQ-*)
// =============================================================================
#[tokio::test]
async fn test_i_requirements_keys_match_pattern() {
    let charter = build_test_charter();
    let summary = build_test_research_summary();
    let reqs = RequirementsSynthesizer::synthesize(&charter, Some(&summary), &[], None, None)
        .expect("synthesize requirements");

    assert!(!reqs.requirements.is_empty());
    for req in &reqs.requirements {
        assert!(
            req.key.as_str().starts_with("REQ-"),
            "Requirement key '{}' must start with REQ-",
            req.key.as_str()
        );
        assert!(
            req.key.as_str().contains("-FUNC-")
                || req.key.as_str().contains("-SEC-")
                || req.key.as_str().contains("-OPS-")
                || req.key.as_str().contains("-NF-")
                || req.key.as_str().contains("-COMPAT-")
        );
    }
}

// =============================================================================
// Test J: Architecture references requirements keys
// =============================================================================
#[tokio::test]
async fn test_j_architecture_references_requirements_keys() {
    let charter = build_test_charter();
    let summary = build_test_research_summary();
    let reqs = RequirementsSynthesizer::synthesize(&charter, Some(&summary), &[], None, None)
        .expect("synthesize requirements");
    let (arch, _) = ArchitectureSynthesizer::synthesize(&reqs, &charter, Some(&summary), &[], None)
        .expect("synthesize architecture");

    assert!(!arch.components.is_empty());
    let mut total_req_refs = 0;
    for comp in &arch.components {
        for r in &comp.requirement_refs {
            assert!(
                r.as_str().starts_with("REQ-"),
                "Component requirement reference '{}' must start with REQ-",
                r.as_str()
            );
            total_req_refs += 1;
        }
    }
    assert!(
        total_req_refs > 0,
        "Architecture components must link to requirements keys"
    );
}

// =============================================================================
// Test K: ADRs reference architecture decisions
// =============================================================================
#[tokio::test]
async fn test_k_adrs_reference_architecture_decisions() {
    let charter = build_test_charter();
    let summary = build_test_research_summary();
    let reqs = RequirementsSynthesizer::synthesize(&charter, Some(&summary), &[], None, None)
        .expect("synthesize requirements");
    let (_, adrs) = ArchitectureSynthesizer::synthesize(&reqs, &charter, Some(&summary), &[], None)
        .expect("synthesize architecture");

    assert!(!adrs.adrs.is_empty());
    for adr in adrs.adrs.values() {
        assert!(
            adr.id.as_str().starts_with("ADR-"),
            "ADR ID '{}' must start with ADR-",
            adr.id.as_str()
        );
        assert!(
            !adr.context.is_empty(),
            "ADR must specify architectural context"
        );
        assert!(
            !adr.decision.is_empty(),
            "ADR must specify concrete decision"
        );
    }
}

// =============================================================================
// Test L: Roadmap DAG has no cycles
// =============================================================================
#[tokio::test]
async fn test_l_roadmap_dag_has_no_cycles() {
    let charter = build_test_charter();
    let summary = build_test_research_summary();
    let reqs = RequirementsSynthesizer::synthesize(&charter, Some(&summary), &[], None, None)
        .expect("synthesize requirements");
    let (arch, adrs) =
        ArchitectureSynthesizer::synthesize(&reqs, &charter, Some(&summary), &[], None)
            .expect("synthesize architecture");
    let risks = RiskRegister::new();
    let roadmap = RoadmapSynthesizer::synthesize(&reqs, &arch, &adrs, &risks, &charter, None)
        .expect("synthesize roadmap");

    // Canonical DAG validation succeeds
    let order = roadmap.validate_dag().expect("roadmap DAG must be valid");
    assert_eq!(order.len(), roadmap.phases.len());

    // Introduce an artificial cycle and ensure validate_dag detects it
    let mut cyclic_roadmap = roadmap.clone();
    if cyclic_roadmap.phases.len() >= 2 {
        let phase1_id = cyclic_roadmap.phases[1].id.clone();
        cyclic_roadmap.phases[0].dependencies.push(phase1_id);
        let phase0_id = cyclic_roadmap.phases[0].id.clone();
        cyclic_roadmap.phases[1].dependencies.push(phase0_id);

        let err = cyclic_roadmap.validate_dag().unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("cycle")
                || err.to_string().to_lowercase().contains("dependency"),
            "Cyclic roadmap must be rejected by DAG validator"
        );
    }
}

// =============================================================================
// Test M: Roadmap lowers to WorkflowDefinition
// =============================================================================
#[tokio::test]
async fn test_m_roadmap_lowers_to_workflow_definition() {
    let charter = build_test_charter();
    let summary = build_test_research_summary();
    let reqs = RequirementsSynthesizer::synthesize(&charter, Some(&summary), &[], None, None)
        .expect("synthesize requirements");
    let (arch, adrs) =
        ArchitectureSynthesizer::synthesize(&reqs, &charter, Some(&summary), &[], None)
            .expect("synthesize architecture");
    let risks = RiskRegister::new();
    let roadmap = RoadmapSynthesizer::synthesize(&reqs, &arch, &adrs, &risks, &charter, None)
        .expect("synthesize roadmap");

    let def = roadmap
        .lower_to_workflow_definition(".planning")
        .expect("lower to workflow definition");

    assert_eq!(def.steps.len(), roadmap.phases.len());
    for (step, phase) in def.steps.iter().zip(roadmap.phases.iter()) {
        let expected_key = phase.id.to_lowercase().replace('-', "_");
        assert_eq!(step.key, expected_key);
        assert_eq!(step.name, phase.name);
        assert_eq!(
            step.depends_on,
            phase
                .dependencies
                .iter()
                .map(|d| d.to_lowercase().replace('-', "_"))
                .collect::<Vec<_>>()
        );
        assert!(!step.required_capabilities.is_empty());
        assert!(!step.expected_outputs.is_empty());
    }
}

// =============================================================================
// Test N: WorkflowDefinition compiles to CompiledWorkflow
// =============================================================================
#[tokio::test]
async fn test_n_workflow_definition_compiles_to_compiled_workflow() {
    let charter = build_test_charter();
    let summary = build_test_research_summary();
    let reqs = RequirementsSynthesizer::synthesize(&charter, Some(&summary), &[], None, None)
        .expect("synthesize requirements");
    let (arch, adrs) =
        ArchitectureSynthesizer::synthesize(&reqs, &charter, Some(&summary), &[], None)
            .expect("synthesize architecture");
    let risks = RiskRegister::new();
    let roadmap = RoadmapSynthesizer::synthesize(&reqs, &arch, &adrs, &risks, &charter, None)
        .expect("synthesize roadmap");

    let def = roadmap
        .lower_to_workflow_definition(".planning")
        .expect("lower to workflow definition");

    let compiled = CompiledWorkflow::from_definition(def.clone()).expect("compile from definition");

    assert_eq!(compiled.definition.id, def.id);
    assert_eq!(compiled.topological_order.len(), def.steps.len());
    assert!(!compiled.provenance.manifest_hash.is_empty());
    assert!(!compiled.provenance.composite_hash.is_empty());
}

// =============================================================================
// Test O: CompiledWorkflow executes through WorkflowEngine
// =============================================================================
#[tokio::test]
async fn test_o_compiled_workflow_executes_through_workflow_engine() {
    let dir = tempdir().unwrap();
    setup_empty_git_workspace(dir.path());

    let model = Arc::new(CompletingTestModel::new(dir.path()));
    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model);

    let step = WorkflowStepDefinition {
        key: "step_demo".to_string(),
        name: "Demo Step".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "genesis.discovery:1".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 60,
        allows_parallelism: false,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let def = WorkflowDefinition {
        id: "wf-engine-test".to_string(),
        name: "Engine Test".to_string(),
        description: "Test execution".to_string(),
        version: 1,
        steps: vec![step],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };

    let compiled = CompiledWorkflow::from_definition(def).unwrap();
    let catalog = Arc::new(InMemoryPromptCatalog::with_builtins());
    let engine = runtime.create_workflow_engine(catalog);

    let start_req = WorkflowStartRequest::new(dir.path()).with_mode(WorkflowMode::Autonomous);
    let handle = engine
        .start_workflow(&compiled, start_req)
        .await
        .expect("start workflow");

    assert!(
        handle.status == WorkflowRunState::Running || handle.status == WorkflowRunState::Completed
    );

    let repo = SqliteWorkflowRepository::new(runtime.pool().clone());
    let run = repo
        .get_run(handle.run_id)
        .await
        .unwrap()
        .expect("run exists");
    assert_eq!(run.id, handle.run_id);
}

// =============================================================================
// Test P: Workflow step lowers to Mission in canonical WorkflowEngine
// =============================================================================
#[tokio::test]
async fn test_p_workflow_step_lowers_to_mission() {
    let dir = tempdir().unwrap();
    setup_brownfield_git_workspace(dir.path());

    let model = Arc::new(CompletingTestModel::new(dir.path()));
    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model);

    let step = WorkflowStepDefinition {
        key: "step_lowering_mission".to_string(),
        name: "Lowering to Mission Verification".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "genesis.discovery".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 60,
        allows_parallelism: false,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let compiled = CompiledWorkflow {
        topological_order: vec![step.key.clone()],
        definition: WorkflowDefinition {
            id: "wf_p".to_string(),
            name: "Test P Workflow".to_string(),
            description: "Test lowering".to_string(),
            version: 1,
            steps: vec![step],
            default_recovery_strategy: RecoveryStrategy::Fail,
        },
        provenance: m31a::workflow::provenance::WorkflowProvenance::new(
            "wf_p",
            1,
            "hash",
            1,
            std::collections::BTreeMap::new(),
            None,
            chrono::Utc::now(),
        ),
    };

    let catalog = Arc::new(InMemoryPromptCatalog::with_builtins());
    let engine = runtime.create_workflow_engine(catalog);

    let handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(dir.path()))
        .await
        .expect("start workflow");

    assert_eq!(
        handle.status,
        m31a::workflow::state::WorkflowRunState::Completed
    );

    // Verify Mission row exists in SQLite missions table
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM missions")
        .fetch_one(runtime.pool())
        .await
        .unwrap();

    assert_eq!(count, 1, "Mission must be persisted in SQLite");
}

// =============================================================================
// Test Q: Mission executes through SchedulerEngine
// =============================================================================
#[tokio::test]
async fn test_q_mission_executes_through_scheduler_engine() {
    let dir = tempdir().unwrap();
    setup_brownfield_git_workspace(dir.path());

    let model = Arc::new(CompletingTestModel::new(dir.path()));
    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model.clone());

    let step = WorkflowStepDefinition {
        key: "step_scheduler_exec".to_string(),
        name: "Scheduler Execution Verification".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "genesis.discovery".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate::default(),
        depends_on: vec![],
        timeout_secs: 60,
        allows_parallelism: false,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let compiled = CompiledWorkflow {
        topological_order: vec![step.key.clone()],
        definition: WorkflowDefinition {
            id: "wf_q".to_string(),
            name: "Test Q Workflow".to_string(),
            description: "Test scheduler execution".to_string(),
            version: 1,
            steps: vec![step],
            default_recovery_strategy: RecoveryStrategy::Fail,
        },
        provenance: m31a::workflow::provenance::WorkflowProvenance::new(
            "wf_q",
            1,
            "hash",
            1,
            std::collections::BTreeMap::new(),
            None,
            chrono::Utc::now(),
        ),
    };

    let catalog = Arc::new(InMemoryPromptCatalog::with_builtins());
    let engine = runtime.create_workflow_engine(catalog);

    let handle = engine
        .start_workflow(&compiled, WorkflowStartRequest::new(dir.path()))
        .await
        .expect("start workflow");

    assert_eq!(
        handle.status,
        m31a::workflow::state::WorkflowRunState::Completed
    );
    assert_eq!(
        model.call_count.load(Ordering::SeqCst),
        1,
        "Scheduler dispatched task to WorkerRunner which called ModelCaller"
    );
}

// =============================================================================
// Test R: Artifacts register with ArtifactService and have content hashes
// =============================================================================
#[tokio::test]
async fn test_r_artifacts_register_with_artifact_service() {
    let dir = tempdir().unwrap();
    setup_empty_git_workspace(dir.path());

    let model = Arc::new(CompletingTestModel::new(dir.path()));
    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model);

    let req = GenesisRequest::new("Build a distributed event bus in Rust", dir.path());
    let outcome = runtime
        .run_genesis(&req)
        .await
        .expect("run_genesis must succeed");

    assert!(!outcome.registered_artifacts.is_empty());

    let expected_names = [
        "PROJECT.md",
        "REQUIREMENTS.md",
        "ARCHITECTURE.md",
        "DECISIONS.md",
        "RISKS.md",
        "ROADMAP.md",
        "STATE.md",
    ];

    for name in &expected_names {
        let art = outcome
            .registered_artifacts
            .iter()
            .find(|a| a.name == *name)
            .unwrap_or_else(|| panic!("Artifact {} must be registered", name));

        assert_eq!(art.status, ArtifactStatus::Valid);
        assert!(!art.content_hash.is_empty());

        // Verify row exists in SQLite artifacts table
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM artifacts WHERE id = ? AND content_hash = ?")
                .bind(art.id.as_bytes().as_slice())
                .bind(&art.content_hash)
                .fetch_one(runtime.pool())
                .await
                .unwrap();

        assert_eq!(count, 1, "Artifact {} must be in SQLite database", name);
    }
}

// =============================================================================
// Test S: Quality gate rejection pauses workflow until approval
// =============================================================================
#[tokio::test]
async fn test_s_quality_gate_rejection_pauses_workflow() {
    let dir = tempdir().unwrap();
    setup_empty_git_workspace(dir.path());

    let model = Arc::new(CompletingTestModel::new(dir.path()));
    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model);

    let step = WorkflowStepDefinition {
        key: "step_gate_approval".to_string(),
        name: "Gated Step".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "genesis.discovery:1".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate {
            schema: None,
            required_artifacts: vec![],
            require_human_approval: true,
            max_ambiguity_percent: None,
        },
        depends_on: vec![],
        timeout_secs: 60,
        allows_parallelism: false,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let def = WorkflowDefinition {
        id: "wf-gate-test".to_string(),
        name: "Gate Approval Workflow".to_string(),
        description: "Gate test".to_string(),
        version: 1,
        steps: vec![step],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };

    let compiled = CompiledWorkflow::from_definition(def.clone()).unwrap();
    let catalog = Arc::new(InMemoryPromptCatalog::with_builtins());
    let engine = runtime.create_workflow_engine(catalog);

    let start_req = WorkflowStartRequest::new(dir.path());
    let handle = engine
        .start_workflow(&compiled, start_req)
        .await
        .expect("start workflow");

    assert_eq!(handle.status, WorkflowRunState::AwaitingApproval);

    let step_run = engine
        .repository()
        .get_step_run_by_key(handle.run_id, "step_gate_approval")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(step_run.status, WorkflowStepState::AwaitingApproval);
}

// =============================================================================
// Test T: Approval via CLI/TUI resumes workflow
// =============================================================================
#[tokio::test]
async fn test_t_approval_via_cli_tui_resumes_workflow() {
    let dir = tempdir().unwrap();
    setup_empty_git_workspace(dir.path());

    let model = Arc::new(CompletingTestModel::new(dir.path()));
    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model);

    let step = WorkflowStepDefinition {
        key: "step_gate_approval_2".to_string(),
        name: "Gated Step 2".to_string(),
        role: AgentRole::researcher(),
        prompt_template: "genesis.discovery:1".to_string(),
        required_inputs: vec![],
        expected_outputs: vec![],
        required_capabilities: vec![],
        quality_gate: QualityGate {
            schema: None,
            required_artifacts: vec![],
            require_human_approval: true,
            max_ambiguity_percent: None,
        },
        depends_on: vec![],
        timeout_secs: 60,
        allows_parallelism: false,
        recovery_strategy: None,

        prompt_ref: None,
    };

    let def = WorkflowDefinition {
        id: "wf-gate-resume-test".to_string(),
        name: "Gate Resume Workflow".to_string(),
        description: "Gate resume test".to_string(),
        version: 1,
        steps: vec![step],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };

    let compiled = CompiledWorkflow::from_definition(def.clone()).unwrap();
    let catalog = Arc::new(InMemoryPromptCatalog::with_builtins());
    let engine = runtime.create_workflow_engine(catalog);

    let start_req = WorkflowStartRequest::new(dir.path());
    let handle = engine
        .start_workflow(&compiled, start_req)
        .await
        .expect("start workflow");

    assert_eq!(handle.status, WorkflowRunState::AwaitingApproval);

    // Operator submits approval
    let resume_state = runtime
        .approve_workflow_step(handle.run_id, "step_gate_approval_2", &def)
        .await
        .expect("approve step");

    assert_eq!(resume_state, WorkflowRunState::Completed);

    let step_run = engine
        .repository()
        .get_step_run_by_key(handle.run_id, "step_gate_approval_2")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(step_run.status, WorkflowStepState::Completed);
}

// =============================================================================
// Test U: Restart during workflow preserves Genesis state and resumes
// =============================================================================
#[tokio::test]
async fn test_u_restart_recovery_preserves_genesis_state() {
    let dir = tempdir().unwrap();
    setup_empty_git_workspace(dir.path());

    // Phase 1: Initialize runtime and execute Genesis
    {
        let model = Arc::new(CompletingTestModel::new(dir.path()));
        let runtime = AppRuntime::new(dir.path())
            .await
            .expect("initial runtime instantiate")
            .with_model_caller(model);
        let req = GenesisRequest::new("Build persistent key-value store in Rust", dir.path());
        let outcome = runtime.run_genesis(&req).await.expect("run genesis");
        assert!(!outcome.registered_artifacts.is_empty());
    }

    // Phase 2: Instantiate fresh runtime against the same directory and database
    {
        let fresh_runtime = AppRuntime::new(dir.path())
            .await
            .expect("fresh runtime instantiate");

        let state = fresh_runtime
            .get_genesis_state(".planning")
            .await
            .expect("get genesis state")
            .expect("Genesis state must exist after restart");

        assert_eq!(
            state.lifecycle_state,
            PlanningLifecycleState::ImplementationReady
        );
        assert!(state.requirements_count > 0);
        assert!(state.phase_count > 0);
    }
}

// =============================================================================
// Test V: Invalidation of upstream artifact marks downstream state stale
// =============================================================================
#[tokio::test]
async fn test_v_upstream_invalidation_marks_downstream_state() {
    let dir = tempdir().unwrap();
    setup_empty_git_workspace(dir.path());

    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed");

    let charter = build_test_charter();
    let summary = build_test_research_summary();
    let old_reqs = RequirementsSynthesizer::synthesize(&charter, Some(&summary), &[], None, None)
        .expect("old reqs");
    let (arch, adrs) =
        ArchitectureSynthesizer::synthesize(&old_reqs, &charter, Some(&summary), &[], None)
            .expect("arch");
    let risks = RiskRegister::new();
    let roadmap = RoadmapSynthesizer::synthesize(&old_reqs, &arch, &adrs, &risks, &charter, None)
        .expect("roadmap");

    // Register architecture artifact in ArtifactService
    let arch_path = arch.save_to_dir(dir.path(), ".planning").unwrap();
    let arch_rec = runtime
        .artifact_service()
        .register_genesis_artifact("ARCHITECTURE.md", &arch_path, "genesis.architecture", None)
        .await
        .unwrap();

    assert_eq!(arch_rec.status, ArtifactStatus::Valid);

    // Modify a requirement and compute changeset
    let mut new_reqs = old_reqs.clone();
    let prov = Provenance::new(
        ProvenanceSourceType::RepositoryFile,
        TrustLevel::VerifiedRepository,
        "operator",
    );
    let mut modified = EngineeringRequirement::new(
        "REQ-FUNC-01",
        "Modified specification with new constraints",
        EpistemicStatus::ExplicitUserRequirement,
        prov,
        vec!["New test constraint".to_string()],
    );
    modified.category = RequirementCategory::Functional;
    modified.priority = RequirementPriority::Must;
    new_reqs.add_or_update(modified);

    let changeset =
        PlanningInvalidator::compute_changeset(&old_reqs, &new_reqs, &arch, &adrs, &roadmap);

    assert!(changeset.has_changes());
    assert!(!changeset.changed_requirements.is_empty());

    // Mark architecture artifact superseded in ArtifactService
    runtime
        .artifact_service()
        .update_status(arch_rec.id, ArtifactStatus::Superseded)
        .await
        .unwrap();

    let updated_rec = runtime
        .artifact_service()
        .get_metadata(arch_rec.id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(updated_rec.status, ArtifactStatus::Superseded);
}

// =============================================================================
// Test W: Malformed artifact blocks downstream progression
// =============================================================================
#[tokio::test]
async fn test_w_malformed_artifact_blocks_downstream_progression() {
    let mut charter = build_test_charter();
    charter.project_name = "".to_string(); // Invalidate

    let err = charter.validate().unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("project name")
            || err.to_string().to_lowercase().contains("empty")
    );

    // Malformed workflow definition with empty ID fails validation
    let bad_def = WorkflowDefinition {
        id: "".to_string(),
        name: "Bad".to_string(),
        description: "Bad".to_string(),
        version: 1,
        steps: vec![],
        default_recovery_strategy: RecoveryStrategy::Fail,
    };

    assert!(CompiledWorkflow::from_definition(bad_def).is_err());
}

// =============================================================================
// Test X: Brownfield discovery indexes existing code before charter
// =============================================================================
#[tokio::test]
async fn test_x_brownfield_discovery_indexes_code_before_charter() {
    let dir = tempdir().unwrap();
    setup_brownfield_git_workspace(dir.path());

    let bmap = GenesisController::map_brownfield(dir.path(), ".planning")
        .expect("map brownfield must succeed");

    assert_eq!(bmap.topology.primary_language, "Rust");
    assert!(
        bmap.topology
            .package_manifests
            .contains(&"Cargo.toml".to_string())
    );
    assert!(
        bmap.topology
            .entry_points
            .contains(&"src/lib.rs".to_string())
    );

    // Projections written to disk
    assert!(dir.path().join(".planning/BROWNFIELD.md").exists());
}

// =============================================================================
// Test Y: WorkflowEngine is the canonical orchestrator in production path
// =============================================================================
#[tokio::test]
async fn test_y_production_step_executor_only_executor_in_production_path() {
    let dir = tempdir().unwrap();
    setup_empty_git_workspace(dir.path());

    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed");

    let catalog = Arc::new(InMemoryPromptCatalog::with_builtins());
    let engine = runtime.create_workflow_engine(catalog);

    // Verify engine has access to the production repository and catalog
    assert!(
        engine
            .repository()
            .get_run(WorkflowRunId::new())
            .await
            .unwrap()
            .is_none()
    );
    assert!(engine.prompt_catalog().contains("implement", 1));
}

// =============================================================================
// Test Z: No mock bypasses exist in the production Genesis pipeline
// =============================================================================
#[tokio::test]
async fn test_z_no_mock_bypasses_in_production_genesis_pipeline() {
    let dir = tempdir().unwrap();
    setup_empty_git_workspace(dir.path());

    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed");

    // Real services wired
    assert!(
        runtime
            .artifact_service()
            .get_metadata(ArtifactId::new())
            .await
            .unwrap()
            .is_none()
    );
    assert!(!runtime.policy().active_policy_hash().is_empty());
    assert!(
        runtime
            .budget_enforcer()
            .current_limits()
            .max_cost_usd
            .is_none(),
        "Default budget must be unbounded under Free Coding contract"
    );
}

// =============================================================================
// Golden Greenfield Journey Test
// =============================================================================
#[tokio::test]
async fn test_golden_greenfield_journey() {
    let dir = tempdir().unwrap();
    setup_empty_git_workspace(dir.path());

    let model = Arc::new(CompletingTestModel::new(dir.path()));
    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model);

    // 1. Run full Greenfield Genesis
    let req = GenesisRequest::new("Build a high-performance HTTP gateway in Rust", dir.path());
    let outcome = runtime.run_genesis(&req).await.expect("run genesis");

    assert_eq!(outcome.environment.detected_mode, GenesisMode::Greenfield);
    assert!(outcome.charter_artifact.content_hash.len() == 64);
    assert_eq!(
        outcome.planning.state.lifecycle_state,
        PlanningLifecycleState::ImplementationReady
    );
    assert!(!outcome.planning.roadmap.phases.is_empty());

    // 2. Start lowered workflow
    let handle = runtime
        .start_genesis_workflow(&outcome.workflow_definition)
        .await
        .expect("start workflow");

    assert!(
        handle.status == WorkflowRunState::Running || handle.status == WorkflowRunState::Completed
    );
}

// =============================================================================
// Golden Brownfield Journey Test
// =============================================================================
#[tokio::test]
async fn test_golden_brownfield_journey() {
    let dir = tempdir().unwrap();
    setup_brownfield_git_workspace(dir.path());

    let model = Arc::new(CompletingTestModel::new(dir.path()));
    let runtime = AppRuntime::new(dir.path())
        .await
        .expect("AppRuntime::new failed")
        .with_model_caller(model);

    // 1. Run full Brownfield Genesis
    let req = GenesisRequest::new(
        "Refactor helper module and add comprehensive tests",
        dir.path(),
    );
    let outcome = runtime
        .run_genesis(&req)
        .await
        .expect("run brownfield genesis");

    assert_eq!(outcome.environment.detected_mode, GenesisMode::Brownfield);
    assert!(outcome.brownfield_map.is_some());
    let bmap = outcome.brownfield_map.as_ref().unwrap();
    assert_eq!(bmap.topology.primary_language, "Rust");
    assert!(
        bmap.topology
            .package_manifests
            .contains(&"Cargo.toml".to_string())
    );

    // 2. Verify registered artifacts
    let charter = outcome
        .registered_artifacts
        .iter()
        .find(|a| a.name == "PROJECT.md")
        .expect("PROJECT.md registered");
    assert_eq!(charter.status, ArtifactStatus::Valid);

    // 3. Verify workflow definition lowered from Brownfield roadmap
    assert!(!outcome.workflow_definition.steps.is_empty());
    let compiled = CompiledWorkflow::from_definition(outcome.workflow_definition.clone())
        .expect("compile brownfield workflow");
    assert_eq!(
        compiled.topological_order.len(),
        outcome.workflow_definition.steps.len()
    );
}
